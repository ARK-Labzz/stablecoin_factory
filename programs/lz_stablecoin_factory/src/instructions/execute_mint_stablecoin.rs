use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteMintSovereignCoin<'info> {
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
        seeds = [b"mint_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = mint_state.bump,
        constraint = mint_state.payer == payer.key(),
        close = payer 
    )]
    pub mint_state: Box<Account<'info, MintSovereignState>>,

    /// User's source USDC account (to pay from)
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = payer,
    )]
    pub user_usdc_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User's sovereign coin account (to receive)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = payer,
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = mint.key() == sovereign_coin.mint @ StablecoinError::InvalidSovereignCoinMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Protocol's fiat reserve
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
        constraint = global_usdc_account.key() == factory.global_usdc_account @ StablecoinError::InvalidGlobalUsdcAccount,
    )]
    pub global_usdc_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    /// Protocol's bond holding
    #[account(
        mut,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory,
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Protocol's USDC fee vault
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory,
        constraint = usdc_protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub usdc_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        constraint = usdc_mint.key() == USDC_MINT @ StablecoinError::InvalidUSDCMint
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        constraint = bond_token_mint.key() == sovereign_coin.bond_mint @ StablecoinError::InvalidBondMint
    )]
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Oracle account
    #[account(
        constraint = payment_base_price_feed_account.key() == factory.payment_base_price_feed_account @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_base_price_feed_account: UncheckedAccount<'info>,

    /// CHECK: Quote oracle account
    pub payment_quote_price_feed_account: Option<UncheckedAccount<'info>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl ExecuteMintSovereignCoin<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let mint_state = &ctx.accounts.mint_state;
        let previous_balance = ctx.accounts.user_sovereign_coin_account.amount;

        // Validate mint state hasn't expired
        let clock = Clock::get()?;
        require!(
            mint_state.created_at + 300 > clock.unix_timestamp,
            StablecoinError::MintStateExpired
        );

        // Factory signer seeds
        let factory_seeds = &[
            b"factory".as_ref(),
            &[ctx.accounts.factory.bump],
        ];
        let factory_signer = &[&factory_seeds[..]];

        // Transfer protocol fee to protocol vault
        if mint_state.protocol_fee > 0 {
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.user_usdc_token_account.to_account_info(),
                        mint: ctx.accounts.usdc_mint.to_account_info(),
                        to: ctx.accounts.usdc_protocol_vault.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                ),
                mint_state.protocol_fee,
                ctx.accounts.usdc_mint.decimals,
            )?;
        }

        // Transfer reserve amount to global USDC reserve
        if mint_state.reserve_amount > 0 {
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.user_usdc_token_account.to_account_info(),
                        mint: ctx.accounts.usdc_mint.to_account_info(),
                        to: ctx.accounts.global_usdc_reserve.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                ),
                mint_state.reserve_amount,
                ctx.accounts.usdc_mint.decimals,
            )?;
        }

        // Transfer bond amount to global USDC account
        if mint_state.bond_amount > 0 {
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.user_usdc_token_account.to_account_info(),
                        mint: ctx.accounts.usdc_mint.to_account_info(),
                        to: ctx.accounts.global_usdc_account.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                ),
                mint_state.bond_amount,
                ctx.accounts.usdc_mint.decimals,
            )?;
        }

        // (These would be set immediately after the sovereign coin is initialized)
        let bond_issuance_number = sovereign_coin.bond_issuance_number;
        let payment_feed_type = sovereign_coin.get_payment_feed_type()?;
        let (bond_pda, _bond_bump) = find_bond_pda(ctx.accounts.bond_token_mint.key());
        let (issuance_pda, _issuance_bump) = find_issuance_pda(bond_pda, bond_issuance_number);
        let (payment_pda, _payment_bump) = find_payment_pda(issuance_pda);
        let (payment_feed_pda, _payment_feed_bump) = find_payment_feed_pda(payment_feed_type);
        let (kyc_pda, _kyc_bump) = find_kyc_pda(ctx.accounts.factory.key()); 
        
        // Get associated token addresses - use the get ata with program id instead
        let payment_token_account = get_associated_token_address(&payment_pda, &ctx.accounts.usdc_mint.key());

        // Create purchase bond instruction
        let purchase_bond_ix = PurchaseBondV2 {
            user_wallet: ctx.accounts.factory.key(), 
            user_token_account: ctx.accounts.bond_holding.key(), 
            user_payment_token_account: ctx.accounts.global_usdc_account.key(),
            bond_account: bond_pda,
            issuance_account: issuance_pda,
            payment_account: payment_pda,
            payment_token_account,
            kyc_account: kyc_pda,
            mint_account: ctx.accounts.bond_token_mint.key(),
            payment_mint_account: ctx.accounts.usdc_mint.key(),
            payment_feed_account: payment_feed_pda,
            payment_base_price_feed_account: ctx.accounts.factory.payment_base_price_feed_account,
            payment_quote_price_feed_account: ctx.accounts.factory.payment_quote_price_feed_account,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            token2022_program: spl_token_2022::id(),
            system_program: solana_program::system_program::id(),
        }
        .instruction(PurchaseBondV2InstructionArgs {
            amount: mint_state.bond_amount, 
        });

        solana_program::program::invoke_signed(
            &purchase_bond_ix,
            &[
                ctx.accounts.factory.to_account_info(),
                ctx.accounts.bond_holding.to_account_info(),
                ctx.accounts.global_usdc_account.to_account_info(),
                ctx.accounts.bond_token_mint.to_account_info(),
                ctx.accounts.usdc_mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.token_2022_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
            ],
            factory_signer,
        )?;

        token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                    authority: ctx.accounts.factory.to_account_info(),
                },
                factory_signer, 
            ),
            mint_state.sovereign_amount,
        )?;

        sovereign_coin.total_supply = sovereign_coin.total_supply
            .safe_add(mint_state.sovereign_amount)?;
        sovereign_coin.usdc_amount = sovereign_coin.usdc_amount
            .safe_add(mint_state.reserve_amount)?;
        sovereign_coin.bond_amount = sovereign_coin.bond_amount
            .safe_add(mint_state.bond_amount)?; 

        ctx.accounts.user_sovereign_coin_account.reload()?;
        require!(
            ctx.accounts.user_sovereign_coin_account.amount == previous_balance.safe_add(mint_state.sovereign_amount)?,
            StablecoinError::MintVerificationFailed
        );

        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinMintedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            usdc_amount: mint_state.usdc_amount,
            sovereign_coin_amount: mint_state.sovereign_amount,
            reserve_amount: mint_state.reserve_amount,
            bond_amount: mint_state.bond_amount,
            protocol_fee: mint_state.protocol_fee,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}