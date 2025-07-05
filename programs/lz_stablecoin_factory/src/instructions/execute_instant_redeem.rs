use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteInstantRedemption<'info> {
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

    #[account(
        mut,
        seeds = [b"redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = redeem_state.bump,
        constraint = redeem_state.payer == payer.key(),
        constraint = redeem_state.redemption_type == RedemptionTypeState::InstantBondRedemption,
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
        associated_token::mint = usdc_token_mint,
        associated_token::authority = payer,
    )]
    pub user_usdc_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = mint.key() == sovereign_coin.mint @ StablecoinError::InvalidSovereignCoinMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = usdc_token_mint,
        associated_token::authority = factory,
        constraint = global_usdc_reserve.key() == factory.global_usdc_reserve @ StablecoinError::InvalidGlobalUsdcReserve
    )]
    pub global_usdc_reserve: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = usdc_token_mint,
        associated_token::authority = factory,
        constraint = usdc_protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub usdc_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = usdc_token_mint,
        associated_token::authority = factory,
        constraint = global_usdc_account.key() == factory.global_usdc_account @ StablecoinError::InvalidGlobalUsdcAccount,
    )]
    pub global_usdc_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory,
        constraint = bond_holding.key() == sovereign_coin.bond_holding @ StablecoinError::InvalidBondHolding,
        constraint = bond_holding.amount >= redeem_state.from_bond_redemption @ StablecoinError::InsufficientBondBalance
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = usdc_token_mint.key() == USDC_MINT @ StablecoinError::InvalidUSDCMint
    )]
    pub usdc_token_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(
        constraint = bond_token_mint.key() == sovereign_coin.bond_mint @ StablecoinError::InvalidBondMint
    )]
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl ExecuteInstantRedemption<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let redeem_state = &ctx.accounts.redeem_state;

        let initial_user_sovereign_balance = ctx.accounts.user_sovereign_coin_account.amount;
        let initial_user_usdc_balance = ctx.accounts.user_usdc_token_account.amount;
        let global_usdc_balance_before = ctx.accounts.global_usdc_account.amount;

        let clock = Clock::get()?;
        require!(
            redeem_state.created_at + 300 > clock.unix_timestamp, 
            StablecoinError::RedeemStateExpired
        );

        // Burn sovereign coins
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

        // Transfer from USDC reserve if applicable
        if redeem_state.from_usdc_reserve > 0 {
            let factory_seeds = &[
                b"factory".as_ref(),
                &[ctx.accounts.factory.bump],
            ];
            let factory_signer = &[&factory_seeds[..]];
            
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.global_usdc_reserve.to_account_info(),
                        mint: ctx.accounts.usdc_token_mint.to_account_info(),
                        to: ctx.accounts.user_usdc_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                redeem_state.from_usdc_reserve,
                ctx.accounts.usdc_token_mint.decimals,
            )?;
        }

        // Transfer from protocol vault if applicable
        if redeem_state.from_protocol_vault > 0 {
            let factory_seeds = &[
                b"factory".as_ref(),
                &[ctx.accounts.factory.bump],
            ];
            let factory_signer = &[&factory_seeds[..]];

            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.usdc_protocol_vault.to_account_info(),
                        mint: ctx.accounts.usdc_token_mint.to_account_info(),
                        to: ctx.accounts.user_usdc_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                redeem_state.from_protocol_vault,
                ctx.accounts.usdc_token_mint.decimals,
            )?;
        }

        // Attempt instant bond redemption
        let bond_issuance_number = sovereign_coin.bond_issuance_number;
        let payment_feed_type = sovereign_coin.get_payment_feed_type()?;
        let (bond_pda, _) = find_bond_pda(ctx.accounts.bond_token_mint.key());
        let (issuance_pda, _) = find_issuance_pda(bond_pda, bond_issuance_number);
        // let (payment_pda, _) = find_payment_pda(issuance_pda);
        let (payment_feed_pda, _) = find_payment_feed_pda(payment_feed_type);
        let (sell_liquidity_pda, _) = find_sell_liquidity_pda(bond_pda);
        let sell_liquidity_token_account = get_associated_token_address(&sell_liquidity_pda, &ctx.accounts.usdc_token_mint.key());
        let fee_collector_wallet_token_account = get_associated_token_address(&ETHERFUSE_FEE_COLLECTOR, &ctx.accounts.usdc_token_mint.key());

        let factory_seeds = &[
            b"factory".as_ref(),
            &[ctx.accounts.factory.bump],
        ];
        let factory_signer = &[&factory_seeds[..]];

        let instant_redemption_ix = InstantBondRedemption {
            user_wallet: ctx.accounts.factory.key(),
            user_bond_token_account: ctx.accounts.bond_holding.key(),
            user_payment_token_account: ctx.accounts.global_usdc_account.key(),
            bond_account: bond_pda,
            mint_account: ctx.accounts.bond_token_mint.key(),
            issuance_account: issuance_pda,
            payment_mint_account: ctx.accounts.usdc_token_mint.key(),
            payment_feed_account: payment_feed_pda,
            sell_liquidity_account: sell_liquidity_pda,
            sell_liquidity_token_account,
            fee_collector_wallet_token_account,
            payment_base_price_feed_account: ctx.accounts.factory.payment_base_price_feed_account,
            payment_quote_price_feed_account: ctx.accounts.factory.payment_quote_price_feed_account,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            token2022_program: spl_token_2022::id(),
            system_program: solana_program::system_program::id(),
        }
        .instruction(InstantBondRedemptionInstructionArgs {
            amount: redeem_state.from_bond_redemption,
        });

        let instant_result = solana_program::program::invoke_signed(
            &instant_redemption_ix,
            &[
                ctx.accounts.factory.to_account_info(),
                ctx.accounts.bond_holding.to_account_info(),
                ctx.accounts.global_usdc_account.to_account_info(),
                ctx.accounts.bond_token_mint.to_account_info(),
                ctx.accounts.usdc_token_mint.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.token_2022_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            factory_signer,
        );

        // If instant redemption fails, return error to let client try NFT redemption
        require!(instant_result.is_ok(), StablecoinError::InstantRedemptionFailed);

        // Transfer bond redemption proceeds to user
        ctx.accounts.global_usdc_account.reload()?;
        let global_usdc_balance_after = ctx.accounts.global_usdc_account.amount;
        let actual_usdc_received = global_usdc_balance_after.safe_sub(global_usdc_balance_before)?;

        if actual_usdc_received > 0 {
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.global_usdc_account.to_account_info(),
                        mint: ctx.accounts.usdc_token_mint.to_account_info(),
                        to: ctx.accounts.user_usdc_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                actual_usdc_received,
                ctx.accounts.usdc_token_mint.decimals,
            )?;
        }

        // Update sovereign coin state
        sovereign_coin.total_supply = sovereign_coin.total_supply.safe_sub(redeem_state.sovereign_amount)?;
        sovereign_coin.usdc_amount = sovereign_coin.usdc_amount.safe_sub(redeem_state.from_usdc_reserve)?;
        sovereign_coin.bond_amount = sovereign_coin.bond_amount.safe_sub(redeem_state.from_bond_redemption)?;

        // Verification
        ctx.accounts.user_sovereign_coin_account.reload()?;
        ctx.accounts.user_usdc_token_account.reload()?;

        require!(
            ctx.accounts.user_sovereign_coin_account.amount == 
            initial_user_sovereign_balance.safe_sub(redeem_state.sovereign_amount)?,
            StablecoinError::RedemptionVerificationFailed
        );

        require!(
            ctx.accounts.user_usdc_token_account.amount >= initial_user_usdc_balance,
            StablecoinError::InsufficientRedemptionPayout
        );

        emit_cpi!(SovereignCoinRedeemedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            sovereign_amount: redeem_state.sovereign_amount,
            usdc_amount: redeem_state.usdc_amount,
            from_usdc_reserve: redeem_state.from_usdc_reserve,
            from_protocol_vault: redeem_state.from_protocol_vault,
            from_bond_redemption: redeem_state.from_bond_redemption,
            protocol_fee: redeem_state.protocol_fee,
            timestamp: clock.unix_timestamp,
            redemption_type: RedemptionTypeState::InstantBondRedemption,
        });

        Ok(())
    }
}