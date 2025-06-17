use super::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeRedeemStablecoinArgs {
    pub sovereign_amount: u64,  
}

#[derive(Accounts)]
pub struct InitializeRedeemStablecoin<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,

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

    // The user's sovereign coin account (will be checked for sufficient balance)
    #[account(
        mut,
        token::mint = sovereign_coin_mint,
        token::authority = payer,
    )]
    pub user_sovereign_coin_account: InterfaceAccount<'info, TokenAccount>,
    
    pub sovereign_coin_mint: InterfaceAccount<'info, Mint>,

    // Our fiat reserve for USDC stablecoins (to check balance)
    #[account(
        token::mint = fiat_token_mint,
        token::authority = authority,
    )]
    pub fiat_reserve: InterfaceAccount<'info, TokenAccount>,
    
    // Protocol vault (to check balance)  
    #[account(
        constraint = protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub protocol_vault: InterfaceAccount<'info, TokenAccount>,
    
    pub fiat_token_mint: InterfaceAccount<'info, Mint>,
    
    // This is our state account to store calculations
    #[account(
        init,
        payer = payer,
        space = 8 + RedeemSovereignState::INIT_SPACE,
        seeds = [b"redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump
    )]
    pub redeem_state: Account<'info, RedeemSovereignState>,

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
}

impl InitializeRedeemStablecoin<'_> {
    pub fn handler(ctx: Context<Self>, args: InitializeRedeemStablecoinArgs) -> Result<()> {
        let factory = &ctx.accounts.factory;
        let sovereign_coin = &ctx.accounts.sovereign_coin;
        let redeem_state = &mut ctx.accounts.redeem_state;
        let sovereign_amount = args.sovereign_amount;

        require!(sovereign_amount > 0, StablecoinError::InvalidAmount);
        require!(
            ctx.accounts.user_sovereign_coin_account.amount >= sovereign_amount,
            StablecoinError::InsufficientBalance
        );

        let usdc_amount = calculate_usdc_for_sovereign_amount(
            sovereign_amount,
            &ctx.accounts.payment_base_price_feed_account,
            ctx.accounts.payment_quote_price_feed_account.as_ref()
                .map(|acc| &acc.to_account_info()),
            std::str::from_utf8(&sovereign_coin.target_fiat_currency)?,
            sovereign_coin.decimals,
        )?;
    
        let (net_amount, protocol_fee) = fee::calculate_protocol_fee(
            usdc_amount,
            factory.transfer_fee_bps,  
        )?;
    
        let user_share_of_fiat_reserve = utils::mul_div(
            ctx.accounts.user_sovereign_coin_account.amount,
            sovereign_coin.fiat_amount,
            sovereign_coin.total_supply,
            Rounding::Down,
        )?;
    
        let from_fiat_reserve = utils::min(net_amount, user_share_of_fiat_reserve);
        let remaining_after_fiat = net_amount.checked_sub(from_fiat_reserve)
            .ok_or(StablecoinError::MathError)?;
    
        let protocol_vault_balance = ctx.accounts.protocol_vault.amount;
        let from_protocol_vault = utils::min(remaining_after_fiat, protocol_vault_balance);
        let from_bond_redemption = remaining_after_fiat.checked_sub(from_protocol_vault)
            .ok_or(StablecoinError::MathError)?;
    
        let redemption_type = if from_bond_redemption == 0 {
            if from_protocol_vault == 0 {
                RedemptionTypeState::FiatReserveOnly
            } else {
                RedemptionTypeState::FiatAndProtocol
            }
        } else {
            // We need bond redemption - default to instant, NFT will be determined in ExecuteRedeemFromBonds
            RedemptionTypeState::InstantBondRedemption
        };

        redeem_state.authority = ctx.accounts.authority.key();
        redeem_state.payer = ctx.accounts.payer.key();
        redeem_state.sovereign_coin = ctx.accounts.sovereign_coin.key();
        redeem_state.sovereign_amount = sovereign_amount;  
        redeem_state.usdc_amount = usdc_amount;     
        redeem_state.net_amount = net_amount;
        redeem_state.from_fiat_reserve = from_fiat_reserve;
        redeem_state.from_protocol_vault = from_protocol_vault;
        redeem_state.from_bond_redemption = from_bond_redemption;
        redeem_state.protocol_fee = protocol_fee;
        redeem_state.redemption_type = redemption_type;
        redeem_state.bump = ctx.bumps.redeem_state;
        
        Ok(())
    }
}

