use super::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RedeemSovereignArgs {
    pub usdc_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct RedeemSovereignCoin<'info> {
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
        seeds = [b"sovereign_coin", authority.key().as_ref(), &sovereign_coin.symbol],
        bump = sovereign_coin.bump
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

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
    
    // Optional: Only needed for large redemptions
    #[account(
        init_if_needed,
        payer = payer,
        token::mint = bond_token_mint,
        token::authority = payer,
    )]
    pub bond_ownership: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    
    // Optional: Only needed for NFT redemption
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = nft_token_mint,
        associated_token::authority = payer,
    )]
    pub nft_token_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    
    /// CHECK: We are only reading from this account
    pub nft_token_mint: UncheckedAccount<'info>,

    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: We are only reading from this account
    #[account(
        constraint = payment_base_price_feed_account.key() == factory.payment_base_price_feed_account @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_base_price_feed_account: UncheckedAccount<'info>,

    /// CHECK: We are only reading from this account
    #[account(
        constraint = payment_quote_price_feed_account.key() == factory.payment_quote_price_feed_account.unwrap_or_default() @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_quote_price_feed_account: Option<UncheckedAccount<'info>>,

    /// CHECK: Will be created via CPI to token metadata program
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,

    /// CHECK: For NFT redemption
    pub master_edition_account: UncheckedAccount<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub metadata_program: Program<'info, Metadata>,
    pub rent: Sysvar<'info, Rent>,
}

impl RedeemSovereignCoin<'_> {
    pub fn handler(ctx: Context<Self>, args: RedeemSovereignArgs) -> Result<()> {
        let factory = &ctx.accounts.factory;
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let usdc_amount = args.usdc_amount;
    
        // Basic validation
        require!(usdc_amount > 0, StablecoinError::InvalidAmount);
        require!(
            ctx.accounts.user_sovereign_coin_account.amount >= usdc_amount,
            StablecoinError::InsufficientBalance
        );
    
        // Calculate burn fee if applicable
        let (net_amount, protocol_fee) = if factory.burn_fee_bps > 0 {
            let fee_amount = PreciseNumber::new(usdc_amount as u128)
                .ok_or(StablecoinError::MathError)?
                .checked_mul(&PreciseNumber::new(factory.burn_fee_bps as u128)
                    .ok_or(StablecoinError::MathError)?)
                .ok_or(StablecoinError::MathError)?
                .checked_div(&PreciseNumber::new(10_000u128)
                    .ok_or(StablecoinError::MathError)?)
                .ok_or(StablecoinError::MathError)?
                .to_imprecise()
                .ok_or(StablecoinError::MathError)? as u64;
                    
            let net = usdc_amount
                .checked_sub(fee_amount)
                .ok_or(StablecoinError::MathError)?;
                    
            (net, fee_amount)
        } else {
            (usdc_amount, 0)
        };
    
        // Calculate user's share of the fiat reserve
        // User owns (user_sovereign_coin_amount / total_supply) of the reserves
        let user_share_of_fiat_reserve = PreciseNumber::new(ctx.accounts.user_sovereign_coin_account.amount as u128)
            .ok_or(StablecoinError::MathError)?
            .checked_mul(&PreciseNumber::new(sovereign_coin.fiat_amount as u128)
                .ok_or(StablecoinError::MathError)?)
            .ok_or(StablecoinError::MathError)?
            .checked_div(&PreciseNumber::new(sovereign_coin.total_supply as u128)
                .ok_or(StablecoinError::MathError)?)
            .ok_or(StablecoinError::MathError)?
            .to_imprecise()
            .ok_or(StablecoinError::MathError)? as u64;
    
        // Determine how much to take from each source
        let from_fiat_reserve = std::cmp::min(net_amount, user_share_of_fiat_reserve);
        let mut remaining_after_fiat = net_amount.checked_sub(from_fiat_reserve)
            .ok_or(StablecoinError::MathError)?;
    
        // First, collect protocol fee if applicable
        if protocol_fee > 0 {
            // Burn fee amount of sovereign coins from user
            token_interface::burn(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Burn {
                        mint: ctx.accounts.sovereign_coin_mint.to_account_info(),
                        from: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                ),
                protocol_fee,
            )?;
    
            // Update sovereign coin account state for protocol fee
            sovereign_coin.total_supply = sovereign_coin.total_supply
                .checked_sub(protocol_fee)
                .ok_or(StablecoinError::MathError)?;
        }
    
        // SCENARIO 1: Use only the fiat reserve
        if remaining_after_fiat == 0 {
            // Transfer from fiat reserve to user
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.fiat_reserve.to_account_info(),
                        mint: ctx.accounts.fiat_token_mint.to_account_info(),
                        to: ctx.accounts.user_fiat_token_account.to_account_info(),
                        authority: ctx.accounts.authority.to_account_info(),
                    },
                ),
                from_fiat_reserve,
                ctx.accounts.fiat_token_mint.decimals,
            )?;
    
            // Burn the sovereign coins from user
            token_interface::burn(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Burn {
                        mint: ctx.accounts.sovereign_coin_mint.to_account_info(),
                        from: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                ),
                net_amount,
            )?;
    
            // Update sovereign coin state
            sovereign_coin.total_supply = sovereign_coin.total_supply
                .checked_sub(net_amount)
                .ok_or(StablecoinError::MathError)?;
            sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
                .checked_sub(from_fiat_reserve)
                .ok_or(StablecoinError::MathError)?;
    
            // Emit event for small redemption
            let clock = Clock::get()?;
            emit_cpi!(SovereignCoinRedeemedEvent {
                payer: ctx.accounts.payer.key(),
                sovereign_coin: ctx.accounts.sovereign_coin.key(),
                usdc_amount,
                from_fiat_reserve,
                from_protocol_vault: 0,
                from_bond_redemption: 0,
                protocol_fee,
                timestamp: clock.unix_timestamp,
                redemption_type: RedemptionType::FiatReserveOnly,
            });
    
            return Ok(());
        }
    
        // SCENARIO 2: Use fiat reserve + protocol vault
        let protocol_vault_balance = ctx.accounts.protocol_vault.amount;
        let from_protocol_vault = std::cmp::min(remaining_after_fiat, protocol_vault_balance);
        remaining_after_fiat = remaining_after_fiat.checked_sub(from_protocol_vault)
            .ok_or(StablecoinError::MathError)?;
    
        // Handle fiat reserve transfer if needed
        if from_fiat_reserve > 0 {
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.fiat_reserve.to_account_info(),
                        mint: ctx.accounts.fiat_token_mint.to_account_info(),
                        to: ctx.accounts.user_fiat_token_account.to_account_info(),
                        authority: ctx.accounts.authority.to_account_info(),
                    },
                ),
                from_fiat_reserve,
                ctx.accounts.fiat_token_mint.decimals,
            )?;
        }
    
        // Handle protocol vault transfer if needed
        if from_protocol_vault > 0 {
            // Get signer seeds for factory PDA to sign the transfer
            let factory_seeds = &[
                b"factory".as_ref(),
                &[factory.bump],
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
                from_protocol_vault,
                ctx.accounts.fiat_token_mint.decimals,
            )?;
    
            // Transfer equivalent amount of bonds from bond_holding to bond_ownership
            require!(
                ctx.accounts.bond_ownership.is_some(),
                StablecoinError::BondOwnershipAccountRequired
            );
    
            // Calculate bond amount equivalent to USDC amount
            // This would typically involve price oracles, but we're using a 1:1 ratio for simplicity
            let bond_equivalent = from_protocol_vault; // Adjust this calculation as needed
    
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.bond_holding.to_account_info(),
                        mint: ctx.accounts.bond_token_mint.to_account_info(),
                        to: ctx.accounts.bond_ownership.as_ref().unwrap().to_account_info(),
                        authority: ctx.accounts.authority.to_account_info(),
                    },
                ),
                bond_equivalent,
                ctx.accounts.bond_token_mint.decimals,
            )?;
        }
    
        // If we've covered everything with fiat reserve and protocol vault
        if remaining_after_fiat == 0 {
            // Burn the sovereign coins from user
            token_interface::burn(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Burn {
                        mint: ctx.accounts.sovereign_coin_mint.to_account_info(),
                        from: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                ),
                net_amount,
            )?;
    
            // Update sovereign coin state
            sovereign_coin.total_supply = sovereign_coin.total_supply
                .checked_sub(net_amount)
                .ok_or(StablecoinError::MathError)?;
            sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
                .checked_sub(from_fiat_reserve)
                .ok_or(StablecoinError::MathError)?;
            // Note: We don't update bond_amount since we're just transferring ownership
    
            // Emit event for medium redemption
            let clock = Clock::get()?;
            emit_cpi!(SovereignCoinRedeemedEvent {
                payer: ctx.accounts.payer.key(),
                sovereign_coin: ctx.accounts.sovereign_coin.key(),
                usdc_amount,
                from_fiat_reserve,
                from_protocol_vault,
                from_bond_redemption: 0,
                protocol_fee,
                timestamp: clock.unix_timestamp,
                redemption_type: RedemptionType::FiatAndProtocol,
            });
    
            return Ok(());
        }
    
        // SCENARIO 3: Use fiat reserve + protocol vault + bond redemption
        // We've already handled fiat reserve and protocol vault transfers above
    
        // Now we need to handle the remaining amount through bond redemption
        let from_bond_redemption = remaining_after_fiat;
        
        // First try InstantBondRedemption
        let mut redemption_type = RedemptionType::InstantBondRedemption;
        
        // Make sure bond ownership account exists
        require!(
            ctx.accounts.bond_ownership.is_some(),
            StablecoinError::BondOwnershipAccountRequired
        );
        
        // Construct the InstantBondRedemption instruction
        let instant_redemption_ix = InstantBondRedemption {
            user_wallet: ctx.accounts.payer.key(),
            user_bond_token_account: ctx.accounts.bond_ownership.as_ref().unwrap().key(),
            user_payment_token_account: ctx.accounts.user_fiat_token_account.key(),
            bond_account: ctx.accounts.bond_holding.key(),
            mint_account: ctx.accounts.bond_token_mint.key(),
            issuance_account: find_issuance_pda(ctx.accounts.bond_holding.key(), 1).0,
            payment_mint_account: ctx.accounts.fiat_token_mint.key(),
            payment_feed_account: find_payment_feed_pda(PaymentFeedType::Stub).0,
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
            amount: from_bond_redemption,
        });
    
        // Try to invoke the instruction
        let instant_result = solana_program::program::invoke(
            &instant_redemption_ix,
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.bond_ownership.as_ref().unwrap().to_account_info(),
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
    
        // If instant redemption fails, try NFT redemption
        if instant_result.is_err() {
            // Check if NFT token account exists
            require!(
                ctx.accounts.nft_token_account.is_some(),
                StablecoinError::NFTTokenAccountRequired
            );

            redemption_type = RedemptionType::NFTBondRedemption;
            
            // Construct the RedeemBond instruction
            let redeem_bond_ix = RedeemBond {
                user_wallet: ctx.accounts.payer.key(),
                bond_account: ctx.accounts.bond_holding.key(),
                mint_account: ctx.accounts.bond_token_mint.key(),
                issuance_account: find_issuance_pda(ctx.accounts.bond_holding.key(), 1).0,
                user_nft_token_account: ctx.accounts.nft_token_account.as_ref().unwrap().key(),
                user_payment_token_account: ctx.accounts.user_fiat_token_account.key(),
                payment_mint_account: ctx.accounts.fiat_token_mint.key(),
                payment_feed_account: find_payment_feed_pda(PaymentFeedType::Stub).0,
                // These fields need proper initialization
                nft_mint_account: ctx.accounts.nft_token_mint.key(),
                nft_metadata_account: ctx.accounts.metadata.key(),
                nft_master_edition_account: ctx.accounts.master_edition_account.key(),
                nft_collection_metadata_account: Pubkey::default(),
                nft_issuance_vault_account: find_nft_issuance_vault_pda(ctx.accounts.nft_token_mint.key()).0,
                nft_issuance_vault_token_account: Pubkey::default(),
                payout_account: ctx.accounts.authority.key(),
                payout_token_account: ctx.accounts.protocol_vault.key(),
                token2022_program: spl_token_2022::id(),
                associated_token_program: ctx.accounts.associated_token_program.key(),
                token_program: ctx.accounts.token_program.key(),
                metadata_program: ctx.accounts.metadata.key(),
                system_program: ctx.accounts.system_program.key(),
            }
            .instruction();

            // Actually invoke the instruction
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
                    ctx.accounts.protocol_vault.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                    ctx.accounts.associated_token_program.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            );
            
            // Handle any errors from NFT redemption
            if nft_result.is_err() {
                return Err(error!(StablecoinError::NFTRedemptionFailed));
            }
        }
    
        // If we get here, instant redemption succeeded
        // Burn the sovereign coins from user
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.sovereign_coin_mint.to_account_info(),
                    from: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            net_amount,
        )?;
    
        // Update sovereign coin state
        sovereign_coin.total_supply = sovereign_coin.total_supply
            .checked_sub(net_amount)
            .ok_or(StablecoinError::MathError)?;
        sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
            .checked_sub(from_fiat_reserve)
            .ok_or(StablecoinError::MathError)?;
        
        // We need to update bond_amount by the amount that was redeemed
        // This assumes that 1 bond = 1 USDC for simplicity
        let bond_amount_redeemed = from_bond_redemption; // Adjust calculation as needed
        sovereign_coin.bond_amount = sovereign_coin.bond_amount
            .checked_sub(bond_amount_redeemed)
            .ok_or(StablecoinError::MathError)?;
    
        // Emit event for large redemption
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinRedeemedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            usdc_amount,
            from_fiat_reserve,
            from_protocol_vault,
            from_bond_redemption,
            protocol_fee,
            timestamp: clock.unix_timestamp,
            redemption_type,
        });
    
        Ok(())
    }    
}


#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum RedemptionType {
    FiatReserveOnly,
    FiatAndProtocol,
    InstantBondRedemption,
    NFTBondRedemption,
}