use super::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct MintSovereignArgs {
    pub usdc_amount: u64,
}

#[event_cpi]
#[derive(Accounts)]
pub struct MintSovereignCoin<'info> {
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

    #[account(
        init_if_needed,
        payer = authority,
        token::mint = sovereign_coin_mint,
        token::authority = payer,
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub sovereign_coin_mint: Box<InterfaceAccount<'info, Mint>>,

    // Token Accounts:
    // Where we store our protocol fees (ie. tx fees being carried out in the protocol)
    #[account(
        init_if_needed,
        payer = authority,
        token::mint = fiat_token_mint,
        token::authority = factory,
    )]
    pub protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    // This token accoun is our fiat reserve for storing USDC stablecoins that 
    // wouldn't be used to purchase bonds
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
    
    pub mint: InterfaceAccount<'info, Mint>,
    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: We are only reading from this account
    #[account(
        constraint = payment_base_price_feed_account.key() == factory.payment_base_price_feed_account @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_base_price_feed_account: UncheckedAccount<'info>,

    /// CHECK: We are only reading from this account
    #[account(
        constraint = payment_quote_price_feed_account.key() == factory.payment_quote_price_feed_account.expect("No price feed configured") @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_quote_price_feed_account: Option<UncheckedAccount<'info>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub rent: Sysvar<'info, Rent>,
}

impl MintSovereignCoin<'_> {
    pub fn handler(ctx: Context<Self>, args: MintSovereignArgs) -> Result<()> {
        let factory = &ctx.accounts.factory;
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let protocol_vault = &ctx.accounts.protocol_vault;
        let fiat_reserve = &ctx.accounts.fiat_reserve;
        let bond_holding = &ctx.accounts.bond_holding;
        let previous_balance = ctx.accounts.user_sovereign_coin_account.amount;

        
        require!(args.usdc_amount > 0, StablecoinError::InvalidAmount);
        
        
        let (net_amount, protocol_fee) = if factory.mint_fee_bps > 0 {
            let fee_amount = PreciseNumber::new(args.usdc_amount as u128)
                .ok_or(StablecoinError::MathError)?
                .checked_mul(&PreciseNumber::new(factory.mint_fee_bps as u128)
                    .ok_or(StablecoinError::MathError)?)
                .ok_or(StablecoinError::MathError)?
                .checked_div(&PreciseNumber::new(10_000u128)
                    .ok_or(StablecoinError::MathError)?)
                .ok_or(StablecoinError::MathError)?
                .to_imprecise()
                .ok_or(StablecoinError::MathError)? as u64;
                    
            let net = args.usdc_amount
                .checked_sub(fee_amount)
                .ok_or(StablecoinError::MathError)?;
                    
            (net, fee_amount)
        } else {
            (args.usdc_amount, 0)
        };

       
        let required_reserve_percentage = calculate_required_reserve(
            factory.min_fiat_reserve_percentage,
            sovereign_coin.bond_rating,
            factory.bond_reserve_numerator,
            factory.bond_reserve_denominator,
        )?;

       
        let net_usdc = PreciseNumber::new(net_amount as u128)
            .ok_or(StablecoinError::MathError)?;
        let reserve_percentage = PreciseNumber::new(required_reserve_percentage as u128)
            .ok_or(StablecoinError::MathError)?;
        let basis_points = PreciseNumber::new(10_000u128)
            .ok_or(StablecoinError::MathError)?;

        let reserve_amount = net_usdc
            .checked_mul(&reserve_percentage)
            .ok_or(StablecoinError::MathError)?
            .checked_div(&basis_points)
            .ok_or(StablecoinError::MathError)?
            .to_imprecise()
            .ok_or(StablecoinError::MathError)? as u64;

        let bond_amount = net_amount
            .checked_sub(reserve_amount)
            .ok_or(StablecoinError::MathError)?;

        
        require!(
            reserve_amount > 0 && bond_amount > 0,
            StablecoinError::InvalidCalculatedAmount
        );
        
        
        if protocol_fee > 0 {
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.payer.to_account_info(),
                        mint: ctx.accounts.fiat_token_mint.to_account_info(),
                        to: ctx.accounts.protocol_vault.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                ),
                protocol_fee,
                ctx.accounts.fiat_token_mint.decimals,
            )?;
        }

        
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.payer.to_account_info(),
                    mint: ctx.accounts.fiat_token_mint.to_account_info(),
                    to: ctx.accounts.fiat_reserve.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            reserve_amount,
            ctx.accounts.fiat_token_mint.decimals,
        )?;

       
        require!(
            reserve_amount > 0 && bond_amount > 0,
            StablecoinError::InvalidCalculatedAmount
        );
        
        
        require!(
            (reserve_amount as u64).checked_add(bond_amount)
                .ok_or(StablecoinError::MathError)? == args.usdc_amount,
            StablecoinError::InvalidCalculatedAmount
        );

       
        let purchase_bond_ix = PurchaseBondV2 {
            user_wallet: ctx.accounts.payer.key(),
            user_token_account: fiat_reserve.to_account_info().key(),
            user_payment_token_account: ctx.accounts.payer.to_account_info().key(),
            bond_account: bond_holding.to_account_info().key(),
            issuance_account: find_issuance_pda(bond_holding.key(), 1).0,
            payment_account: find_payment_pda(bond_holding.key()).0,
            payment_token_account: protocol_vault.to_account_info().key(),
            kyc_account: find_kyc_pda(ctx.accounts.payer.key()).0,
            mint_account: ctx.accounts.bond_token_mint.key(),
            payment_mint_account: ctx.accounts.fiat_token_mint.key(),
            payment_feed_account: find_payment_feed_pda(PaymentFeedType::Stub).0,
            // Price feed accounts have not yet been implemented so an error occurs here
            payment_base_price_feed_account: factory.payment_base_price_feed_account,
            payment_quote_price_feed_account: factory.payment_quote_price_feed_account,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            token2022_program: spl_token_2022::id(),
            system_program: solana_program::system_program::id(),
        }
        .instruction(PurchaseBondV2InstructionArgs {
            amount: bond_amount,
        });

        solana_program::program::invoke(
            &purchase_bond_ix,
            &[
                ctx.accounts.payer.to_account_info(),
                fiat_reserve.to_account_info(),
                bond_holding.to_account_info(),
                protocol_vault.to_account_info(),
                ctx.accounts.bond_token_mint.to_account_info(),
                ctx.accounts.fiat_token_mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // Mint the stablecoin to the user's wallet
        let mint_to_user_ix = spl_token::instruction::mint_to(
            &spl_token::id(),
            &ctx.accounts.fiat_token_mint.key(),
            &ctx.accounts.user_sovereign_coin_account.key(),
            &ctx.accounts.authority.key(),
            &[],
            args.usdc_amount,
        )?;
        solana_program::program::invoke(
            &mint_to_user_ix,
            &[
                ctx.accounts.fiat_token_mint.to_account_info(),
                ctx.accounts.user_sovereign_coin_account.to_account_info(),
                ctx.accounts.authority.to_account_info(),
            ],
        )?;

        
        sovereign_coin.total_supply = sovereign_coin.total_supply
            .checked_add(args.usdc_amount)
            .ok_or(StablecoinError::MathError)?;
        sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
            .checked_add(reserve_amount)
            .ok_or(StablecoinError::MathError)?;
        sovereign_coin.bond_amount = sovereign_coin.bond_amount
            .checked_add(bond_amount)
            .ok_or(StablecoinError::MathError)?;

        // After minting, verify the balance increased by the amount of sovereign coins minted
        ctx.accounts.user_sovereign_coin_account.reload()?;
        
        require!(
            ctx.accounts.user_sovereign_coin_account.amount == previous_balance.checked_add(args.usdc_amount).unwrap(),
            StablecoinError::MintVerificationFailed
        );

       
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinMintedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            usdc_amount: args.usdc_amount,
            reserve_amount,
            bond_amount,
            protocol_fee,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}