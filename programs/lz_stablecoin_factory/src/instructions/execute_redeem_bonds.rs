use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteRedeemFromBonds<'info> {
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
        constraint = redeem_state.redemption_type == RedemptionTypeState::InstantBondRedemption || 
                   redeem_state.redemption_type == RedemptionTypeState::NFTBondRedemption,
        close = payer
    )]
    pub redeem_state: Box<Account<'info, RedeemSovereignState>>,
    
    // User's sovereign coin account (will be debited) - need to initialize first
    #[account(
        mut,
        token::mint = mint,
        token::authority = payer,
        constraint = user_sovereign_coin_account.amount >= redeem_state.sovereign_amount @ StablecoinError::InsufficientBalance
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // User's USDC account (will be credited)
    #[account(
        mut,
        token::mint = usdc_token_mint,
        token::authority = payer,
    )]
    pub user_usdc_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = mint.key() == sovereign_coin.mint @ StablecoinError::InvalidSovereignCoinMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    // Protocol vault for fees
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = factory,
    )]
    pub sovereign_coin_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    // USDC reserve for USDC stablecoins
    #[account(
        mut,
        token::mint = usdc_token_mint,
        token::authority = factory,
        constraint = global_usdc_reserve.key() == factory.global_usdc_reserve @ StablecoinError::InvalidGlobalUsdcReserve
    )]
    pub global_usdc_reserve: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Protocol's USDC fee vault
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory,
        constraint = usdc_protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub usdc_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory,
        constraint = global_usdc_reserve.key() == factory.global_usdc_account @ StablecoinError::InvalidGlobalUsdcAccount,
    )]
    pub global_usdc_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // Protocol's bond holding account 
    #[account(
        mut,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory,
        constraint = bond_holding.key() == sovereign_coin.bond_holding @ StablecoinError::InvalidBondHolding,
        constraint = bond_holding.amount >= redeem_state.from_bond_redemption @ StablecoinError::InsufficientBondBalance
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,
    
    
    // The token account for bonds owned by the protocol
    #[account(
        mut,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory, 
        constraint = bond_ownership.key() == bond_holding.key() @ StablecoinError::InvalidBondOwnership
    )]
    pub bond_ownership: Box<InterfaceAccount<'info, TokenAccount>>,
    
    
    // If NFT redemption is needed, the account should already exist but we will init because mint is always unique and not guaranteed
    // to be initialized and we need to ensure the account is created before invoking the NFT redemption logic.
    // If NFT redemption is not needed, this account will be None.
    // If it is None, we will not attempt to invoke the NFT redemption logic.
    #[account(
        init,
        payer = payer,
        associated_token::mint = nft_token_mint,
        associated_token::authority = factory,
    )]
    pub nft_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = nft_token_mint,
        associated_token::authority = payer,
    )]
    pub user_nft_account: Option<InterfaceAccount<'info, TokenAccount>>,
    
    /// CHECK: NFT mint - validated in NFT redemption logic
    pub nft_token_mint: Option<UncheckedAccount<'info>>,

    /// CHECK: NFT collection mint - validated in NFT redemption logic  
    pub nft_collection_mint: Option<UncheckedAccount<'info>>,

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
    pub metadata_program: Program<'info, Metadata>,
    pub rent: Sysvar<'info, Rent>,
}

impl ExecuteRedeemFromBonds<'_> {
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

        let mut redemption_type = RedemptionTypeState::InstantBondRedemption;
        let bond_issuance_number = sovereign_coin.bond_issuance_number;
        let payment_feed_type = sovereign_coin.payment_feed_type.clone();
        let (bond_pda, _bond_bump) = find_bond_pda(ctx.accounts.bond_token_mint.key());
        let (issuance_pda, _issuance_bump) = find_issuance_pda(bond_pda, bond_issuance_number);
        let (payment_pda, _payment_bump) = find_payment_pda(issuance_pda);
        let (payment_feed_pda, _payment_feed_bump) = find_payment_feed_pda(payment_feed_type);
        let (sell_liquidity_pda, _sell_liquidity_bump) = find_sell_liquidity_pda(bond_pda);
        let sell_liquidity_token_account = get_associated_token_address(&sell_liquidity_pda, &ctx.accounts.usdc_token_mint.key());
        let fee_collector_wallet_token_account =   get_associated_token_address(ETHERFUSE_FEE_COLLECTOR, &ctx.accounts.usdc_token_mint.key());
        
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
    
            // Using factory signer for bond redemption
            let factory_seeds = &[
                b"factory".as_ref(),
                &[ctx.accounts.factory.bump],
            ];
            let factory_signer = &[&factory_seeds[..]];
        
            let instant_result = solana_program::program::invoke_signed(
                &instant_redemption_ix,
                &[
                    ctx.accounts.factory.to_account_info(),
                    ctx.accounts.bond_holding.to_account_info(),
                    ctx.accounts.global_usdc_account.to_account_info(),
                    ctx.accounts.bond_token_mint.to_account_info(),
                    ctx.accounts.sovereign_coin_protocol_vault.to_account_info(),
                    ctx.accounts.usdc_token_mint.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                    ctx.accounts.token_2022_program.to_account_info(),
                    ctx.accounts.associated_token_program.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    
                ],
                factory_signer, 
            );

            if instant_result.is_ok() {
            // Reload to get updated balance
            ctx.accounts.global_usdc_account.reload()?;
            let global_usdc_balance_after = ctx.accounts.global_usdc_account.amount;
            
            // Calculate actual USDC received from bond redemption
            let actual_usdc_received = global_usdc_balance_after
                .safe_sub(global_usdc_balance_before)?;
            
            if actual_usdc_received > 0 {
                let factory_seeds = &[
                    b"factory".as_ref(),
                    &[ctx.accounts.factory.bump],
                ];
                let factory_signer = &[&factory_seeds[..]];

                // Transfer the actual amount received to user
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
        }

        if instant_result.is_err() {
            require!(
                ctx.accounts.nft_token_account.is_some(),
                StablecoinError::NFTTokenAccountRequired
            );

            redemption_type = RedemptionTypeState::NFTBondRedemption;
            let (nft_issuance_vault_pda, _nft_issuance_vault_bump) = find_nft_issuance_vault_pda(ctx.accounts.nft_token_mint.key());
            let nft_issuance_vault_token_account = get_associated_token_address(&nft_issuance_vault_pda, &ctx.accounts.nft_token_mint.key());
            let (payout_pda, _payout_bump) = find_payout_pda(issuance_pda);
            let payout_token_account = get_associated_token_address(&payout_pda, &ctx.accounts.usdc_token_mint.key());
            let (nft_metadata_account, _nft_metadata_account_bump) = MetadataMpl::find_pda(&ctx.accounts.nft_token_mint.key());
            let (nft_master_edition_account, _nft_master_edition_account_bump) = MasterEditionMpl::find_pda(&ctx.accounts.nft_token_mint.key());
            let (nft_collection_metadata_account, _nft_collection_metadata_account_bump) = MetadataMpl::find_pda(&ctx.accounts.nft_collection_mint.key());


            let redeem_bond_ix = RedeemBond {
                user_wallet: ctx.accounts.factory.key(), 
                bond_account: bond_pda,
                mint_account: ctx.accounts.bond_token_mint.key(),
                issuance_account: issuance_pda,
                user_nft_token_account: ctx.accounts.nft_token_account.as_ref().unwrap().key(),
                user_payment_token_account: ctx.accounts.global_usdc_account.key(), 
                payment_mint_account: ctx.accounts.usdc_token_mint.key(),
                payment_feed_account: payment_feed_pda,
                nft_mint_account: ctx.accounts.nft_token_mint.key(),
                nft_metadata_account,
                nft_master_edition_account,
                nft_collection_metadata_account,
                nft_issuance_vault_account: nft_issuance_vault_pda,
                nft_issuance_vault_token_account,
                payout_account: payout_pda,
                payout_token_account,
                token2022_program: spl_token_2022::id(),
                associated_token_program: ctx.accounts.associated_token_program.key(),
                token_program: ctx.accounts.token_program.key(),
                metadata_program: ctx.accounts.metadata_program.key(),
                system_program: ctx.accounts.system_program.key(),
            }
            .instruction();

                let nft_result = solana_program::program::invoke_signed(
                    &redeem_bond_ix,
                    &[
                        ctx.accounts.factory.to_account_info(), 
                        ctx.accounts.bond_holding.to_account_info(),
                        ctx.accounts.bond_token_mint.to_account_info(),
                        ctx.accounts.nft_token_account.as_ref().unwrap().to_account_info(),
                        ctx.accounts.global_usdc_account.to_account_info(),
                        ctx.accounts.usdc_token_mint.to_account_info(),
                        ctx.accounts.nft_token_mint.to_account_info(),
                        ctx.accounts.sovereign_coin_protocol_vault.to_account_info(),
                        ctx.accounts.token_program.to_account_info(),
                        ctx.accounts.token_2022_program.to_account_info(),
                        ctx.accounts.associated_token_program.to_account_info(),
                        ctx.accounts.metadata_program.to_account_info(),
                        ctx.accounts.system_program.to_account_info(),
                        // Add metadata and master edition accounts
                    ],
                    factory_signer, 
                );

                if nft_result.is_ok() {
                    token_interface::transfer(
                        CpiContext::new_with_signer(
                            ctx.accounts.token_program.to_account_info(),
                            Transfer {
                                from: ctx.accounts.nft_token_account.as_ref().unwrap().to_account_info(),
                                to: ctx.accounts.user_nft_account.as_ref().unwrap().to_account_info(),
                                authority: ctx.accounts.factory.to_account_info(),
                            },
                            factory_signer,
                        ),
                        1, 
                    )?;
                }
                if nft_result.is_err() {
                    return Err(error!(StablecoinError::NFTRedemptionFailed));
                }
        }
    
        sovereign_coin.total_supply = sovereign_coin.total_supply
            .safe_sub(redeem_state.sovereign_amount)?;
        sovereign_coin.usdc_amount = sovereign_coin.usdc_amount
            .safe_sub(redeem_state.from_usdc_reserve)?;
        
        let bond_amount_redeemed = redeem_state.from_bond_redemption;
        sovereign_coin.bond_amount = sovereign_coin.bond_amount
            .safe_sub(bond_amount_redeemed)?;

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
            redemption_type,
        });
    
        Ok(())
    }  
}