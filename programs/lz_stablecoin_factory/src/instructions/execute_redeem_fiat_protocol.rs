use super::*;


#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteRedeemFromFiatAndProtocol<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Box<Account<'info, Factory>>,

    #[account(
        mut,
        seeds = [
            b"sovereign_coin", 
            factory.key().as_ref(), 
            &sovereign_coin.symbol[..sovereign_coin.symbol.iter().position(|&x| x == 0).unwrap_or(8)]
        ],
        bump = sovereign_coin.bump,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    // Load our state from the previous instruction
    #[account(
        mut,
        seeds = [b"redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = redeem_state.bump,
        constraint = redeem_state.payer == payer.key(),
        constraint = redeem_state.redemption_type == RedemptionTypeState::UsdcReserveAndProtocol,
        close = payer 
    )]
    pub redeem_state: Box<Account<'info, RedeemSovereignState>>,
    
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = payer,
        constraint = user_sovereign_coin_account.amount >= redeem_state.sovereign_amount @ StablecoinError::InsufficientBalance
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = payer,
    )]
    pub user_usdc_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = mint.key() == sovereign_coin.mint @ StablecoinError::InvalidSovereignCoinMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory,
        constraint = global_usdc_reserve.key() == factory.global_usdc_reserve @ StablecoinError::InvalidGlobalUsdcReserve
    )]
    pub global_usdc_reserve: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory,
        constraint = usdc_protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub usdc_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory,
        constraint = bond_holding.key() == sovereign_coin.bond_holding @ StablecoinError::InvalidBondHolding,
        constraint = bond_holding.amount >= redeem_state.from_bond_redemption @ StablecoinError::InsufficientBondBalance
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        constraint = usdc_mint.key() == USDC_MINT @ StablecoinError::InvalidUSDCMint
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(
        constraint = bond_token_mint.key() == sovereign_coin.bond_mint @ StablecoinError::InvalidBondMint
    )]
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory,
        constraint = bond_ownership.key() == sovereign_coin.bond_ownership @ StablecoinError::InvalidBondHolding,
    )]
    pub bond_ownership: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Oracle account for base price (for pull feed approach)
    pub payment_base_price_feed_account: Option<UncheckedAccount<'info>>,

    /// CHECK: Quote oracle account (for pull feed approach)
    pub payment_quote_price_feed_account: Option<UncheckedAccount<'info>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl ExecuteRedeemFromFiatAndProtocol<'_> {
    pub fn handler(
        ctx: Context<Self>,
    ) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let redeem_state = &ctx.accounts.redeem_state;
        
        // Burn the sovereign coins
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            redeem_state.sovereign_amount,
        )?;
    
        let factory_seeds = &[
            b"factory".as_ref(),
            &[ctx.accounts.factory.bump],
        ];
        let factory_signer = &[&factory_seeds[..]];

        // Transfer USDC from reserve if needed
        if redeem_state.from_usdc_reserve > 0 {
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.global_usdc_reserve.to_account_info(),
                        mint: ctx.accounts.usdc_mint.to_account_info(),
                        to: ctx.accounts.user_usdc_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                redeem_state.from_usdc_reserve,
                ctx.accounts.usdc_mint.decimals,
            )?;
        }
    
        // Transfer USDC from protocol vault and handle bond redemption
        if redeem_state.from_protocol_vault > 0 {
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.usdc_protocol_vault.to_account_info(),
                        mint: ctx.accounts.usdc_mint.to_account_info(),
                        to: ctx.accounts.user_usdc_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                redeem_state.from_protocol_vault,
                ctx.accounts.usdc_mint.decimals,
            )?;

            // Calculate bond equivalent using updated oracle functions
            let target_currency = std::str::from_utf8(&sovereign_coin.target_fiat_currency)
            .map_err(|_| StablecoinError::InvalidFiatCurrency)?;
            


           let base_account_info = ctx.accounts.payment_base_price_feed_account
                .as_ref()
                .ok_or(StablecoinError::InvalidPriceFeed)?
                .to_account_info();
            let quote_account_info = ctx.accounts.payment_quote_price_feed_account.as_ref()
                .map(|acc| acc.to_account_info());

            let bond_amount = switchboard::calculate_bond_equivalent(
                redeem_state.from_protocol_vault, 
                &base_account_info, 
                quote_account_info.as_ref(), 
                target_currency,
                ctx.accounts.bond_token_mint.decimals,
            )?;

            // Transfer bonds from holding to ownership
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.bond_holding.to_account_info(),
                        mint: ctx.accounts.bond_token_mint.to_account_info(),
                        to: ctx.accounts.bond_ownership.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                bond_amount, 
                ctx.accounts.bond_token_mint.decimals,
            )?;
        }
    
        // Update sovereign coin state
        sovereign_coin.total_supply = sovereign_coin.total_supply
            .safe_sub(redeem_state.sovereign_amount)?;
        sovereign_coin.usdc_amount = sovereign_coin.usdc_amount
            .safe_sub(redeem_state.from_usdc_reserve)?;
    
        // Emit redemption event
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinRedeemedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            sovereign_amount: redeem_state.sovereign_amount,
            usdc_amount: redeem_state.usdc_amount,
            from_usdc_reserve: redeem_state.from_usdc_reserve,
            from_protocol_vault: redeem_state.from_protocol_vault,
            from_bond_redemption: 0,
            protocol_fee: redeem_state.protocol_fee,
            timestamp: clock.unix_timestamp,
            redemption_type: RedemptionTypeState::UsdcReserveAndProtocol,
        });
    
        Ok(())
    }
}