use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteRedeemFromBonds<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

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
            authority.key().as_ref(), 
            &sovereign_coin.symbol[..sovereign_coin.symbol.iter().position(|&x| x == 0).unwrap_or(8)]
        ],
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.authority == authority.key()
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    // Load our state from the previous instruction
    #[account(
        mut,
        seeds = [b"redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = redeem_state.bump,
        constraint = redeem_state.authority == authority.key() && redeem_state.payer == payer.key(),
        constraint = redeem_state.redemption_type == RedemptionTypeState::InstantBondRedemption || 
                   redeem_state.redemption_type == RedemptionTypeState::NFTBondRedemption,
        close = payer // Close the account after use to reclaim rent
    )]
    pub redeem_state: Box<Account<'info, RedeemSovereignState>>,
    
    // The user's sovereign coin account (will be debited)
    #[account(
        mut,
        token::mint = sovereign_coin_mint,
        token::authority = payer,
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // The user's USDC account (will be credited)
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = payer,
    )]
    pub user_fiat_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub sovereign_coin_mint: Box<InterfaceAccount<'info, Mint>>,

    // Where we store protocol fees
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = factory,
    )]
    pub protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    // Our fiat reserve for USDC stablecoins
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = authority,
    )]
    pub fiat_reserve: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // Where we store the collaterized bonds
    #[account(
        mut,
        token::mint = bond_token_mint,
        token::authority = authority,
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // Required for bond redemption
    #[account(
        mut,
        token::mint = bond_token_mint,
        token::authority = payer,
    )]
    pub bond_ownership: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // Optional: Only needed for NFT redemption
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = nft_token_mint,
        associated_token::authority = payer,
    )]
    pub nft_token_account: Option<InterfaceAccount<'info, TokenAccount>>,
    
    /// CHECK: We are only reading from this account
    pub nft_token_mint: UncheckedAccount<'info>,

    /// CHECK: We are only reading from this account
    pub nft_collection_mint: UncheckedAccount<'info>,

    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Oracle account
    #[account(
        constraint = payment_base_price_feed_account.key() == factory.payment_base_price_feed_account @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_base_price_feed_account: UncheckedAccount<'info>,

    /// CHECK: Quote oracle account
    #[account(
        constraint = payment_quote_price_feed_account.is_none() || 
                   payment_quote_price_feed_account.as_ref().unwrap().key() == 
                   factory.payment_quote_price_feed_account.expect("No price feed configured") 
                   @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_quote_price_feed_account: Option<UncheckedAccount<'info>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub metadata_program: Program<'info, Metadata>,
    pub rent: Sysvar<'info, Rent>,
}

impl ExecuteRedeemFromBonds<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let redeem_state = &ctx.accounts.redeem_state;

        token_interface::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.sovereign_coin_mint.to_account_info(),
                    from: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            redeem_state.sovereign_amount,
        )?;

        if redeem_state.from_fiat_reserve > 0 {
            token_extension::transfer_with_fee(
                &ctx.accounts.token_2022_program,
                &ctx.accounts.fiat_reserve.to_account_info(),
                &ctx.accounts.fiat_token_mint.to_account_info(),
                &ctx.accounts.user_fiat_token_account.to_account_info(),
                &ctx.accounts.authority,
                redeem_state.from_fiat_reserve,
                ctx.accounts.fiat_token_mint.decimals,
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
                        from: ctx.accounts.protocol_vault.to_account_info(),
                        mint: ctx.accounts.fiat_token_mint.to_account_info(),
                        to: ctx.accounts.user_fiat_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                redeem_state.from_protocol_vault,
                ctx.accounts.fiat_token_mint.decimals,
            )?;
        }

        let mut redemption_type = RedemptionTypeState::InstantBondRedemption;
        
        let instant_redemption_ix = InstantBondRedemption {
            user_wallet: ctx.accounts.payer.key(),
            user_bond_token_account: ctx.accounts.bond_ownership.key(),
            user_payment_token_account: ctx.accounts.user_fiat_token_account.key(),
            bond_account: ctx.accounts.bond_holding.key(),
            mint_account: ctx.accounts.bond_token_mint.key(),
            issuance_account: find_issuance_pda(ctx.accounts.bond_holding.key(), 1).0,
            payment_mint_account: ctx.accounts.fiat_token_mint.key(),
            payment_feed_account: find_payment_feed_pda(PaymentFeedType::Stub).0,
            // The two sell liquidity accounts will be provided by etherfuse
            sell_liquidity_account: ctx.accounts.protocol_vault.key(),
            sell_liquidity_token_account: ctx.accounts.protocol_vault.key(),
            fee_collector_wallet_token_account: ctx.accounts.protocol_vault.key(),
            payment_base_price_feed_account: ctx.accounts.payment_base_price_feed_account.key(),
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            token2022_program: spl_token_2022::id(),
            system_program: solana_program::system_program::id(),
            payment_quote_price_feed_account: ctx.accounts.payment_quote_price_feed_account
                .as_ref()
                .map(|acc| acc.key()),
        }
        .instruction(InstantBondRedemptionInstructionArgs {
            amount: redeem_state.from_bond_redemption,
        });
    
        let instant_result = solana_program::program::invoke(
            &instant_redemption_ix,
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.bond_ownership.to_account_info(),
                ctx.accounts.user_fiat_token_account.to_account_info(),
                ctx.accounts.bond_holding.to_account_info(),
                ctx.accounts.bond_token_mint.to_account_info(),
                ctx.accounts.protocol_vault.to_account_info(),
                ctx.accounts.fiat_token_mint.to_account_info(),
                ctx.accounts.payment_base_price_feed_account.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        );

        if instant_result.is_err() {
            require!(
                ctx.accounts.nft_token_account.is_some(),
                StablecoinError::NFTTokenAccountRequired
            );

            redemption_type = RedemptionTypeState::NFTBondRedemption;
            
            let redeem_bond_ix = RedeemBond {
                user_wallet: ctx.accounts.payer.key(),
                bond_account: ctx.accounts.bond_holding.key(),
                mint_account: ctx.accounts.bond_token_mint.key(),
                issuance_account: find_issuance_pda(ctx.accounts.bond_holding.key(), 1).0,
                user_nft_token_account: ctx.accounts.nft_token_account.as_ref().unwrap().key(),
                user_payment_token_account: ctx.accounts.user_fiat_token_account.key(),
                payment_mint_account: ctx.accounts.fiat_token_mint.key(),
                payment_feed_account: find_payment_feed_pda(PaymentFeedType::Stub).0,
                nft_mint_account: ctx.accounts.nft_token_mint.key(),
                // It would be better to just pass in the public keys provided by etherfuse
                nft_metadata_account: MetadataMpl::find_pda(&ctx.accounts.nft_token_mint.key()).0,
                nft_master_edition_account: MasterEditionMpl::find_pda(&ctx.accounts.nft_token_mint.key()).0,
                nft_collection_metadata_account: MetadataMpl::find_pda(&ctx.accounts.nft_collection_mint.key()).0,
                nft_issuance_vault_account: find_nft_issuance_vault_pda(ctx.accounts.nft_token_mint.key()).0,
                nft_issuance_vault_token_account: Pubkey::default(),
                payout_account: ctx.accounts.authority.key(),
                payout_token_account: ctx.accounts.protocol_vault.key(),
                token2022_program: spl_token_2022::id(),
                associated_token_program: ctx.accounts.associated_token_program.key(),
                token_program: ctx.accounts.token_program.key(),
                metadata_program: ctx.accounts.metadata_program.key(),
                system_program: ctx.accounts.system_program.key(),
            }
            .instruction();

            let nft_result = solana_program::program::invoke(
                &redeem_bond_ix,
                &[
                    ctx.accounts.payer.to_account_info(),
                    ctx.accounts.bond_holding.to_account_info(),
                    ctx.accounts.bond_token_mint.to_account_info(),
                    ctx.accounts.nft_token_account.as_ref().unwrap().to_account_info(),
                    ctx.accounts.user_fiat_token_account.to_account_info(),
                    ctx.accounts.fiat_token_mint.to_account_info(),
                    ctx.accounts.nft_token_mint.to_account_info(),
                    ctx.accounts.metadata.to_account_info(),
                    ctx.accounts.master_edition_account.to_account_info(),
                    ctx.accounts.protocol_vault.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                    ctx.accounts.associated_token_program.to_account_info(),
                    ctx.accounts.metadata_program.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            );

            if nft_result.is_err() {
                return Err(error!(StablecoinError::NFTRedemptionFailed));
            }
        }
    
        sovereign_coin.total_supply = sovereign_coin.total_supply
            .safe_sub(redeem_state.sovereign_amount)?;
        sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
            .safe_sub(redeem_state.from_fiat_reserve)?;
        
        let bond_amount_redeemed = redeem_state.from_bond_redemption;
        sovereign_coin.bond_amount = sovereign_coin.bond_amount
            .safe_sub(bond_amount_redeemed)?;
    
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinRedeemedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            sovereign_amount: redeem_state.sovereign_amount,
            usdc_amount: redeem_state.usdc_amount,
            from_fiat_reserve: redeem_state.from_fiat_reserve,
            from_protocol_vault: redeem_state.from_protocol_vault,
            from_bond_redemption: redeem_state.from_bond_redemption,
            protocol_fee: redeem_state.protocol_fee,
            timestamp: clock.unix_timestamp,
            redemption_type,
        });
    
        Ok(())
    }  
}