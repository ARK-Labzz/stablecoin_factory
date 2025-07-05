use super::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeRedeemStablecoinArgs {
    pub sovereign_amount: u64,  
}


#[derive(Accounts)]
#[instruction(args: InitializeRedeemStablecoinArgs)]
pub struct InitializeRedeemStablecoin<'info> {
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
            factory.key().as_ref(), 
            &sovereign_coin.symbol[..sovereign_coin.symbol.iter().position(|&x| x == 0).unwrap_or(8)]
        ],
        bump = sovereign_coin.bump,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    // The user's sovereign coin account (will be checked for sufficient balance)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = payer,
        constraint = user_sovereign_coin_account.amount >= args.sovereign_amount @ StablecoinError::InsufficientBalance
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
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
        constraint = usdc_mint.key() == USDC_MINT @ StablecoinError::InvalidUSDCMint
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,
    
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

        let target_currency = std::str::from_utf8(&sovereign_coin.target_fiat_currency)
            .map_err(|_| StablecoinError::InvalidFiatCurrency)?;

        let base_account_info = ctx.accounts.payment_base_price_feed_account.to_account_info();
        let quote_account_info = ctx.accounts.payment_quote_price_feed_account.as_ref()
            .map(|acc| acc.to_account_info());

        let usdc_amount = calculate_usdc_for_sovereign_amount(
            sovereign_amount,
            &base_account_info, 
            quote_account_info.as_ref(), 
            target_currency,
            sovereign_coin.decimals,
        )?;

        let (net_amount, protocol_fee) = fee::calculate_protocol_fee(
            usdc_amount,
            factory.transfer_fee_bps,  
        )?;
    
        let user_share_of_fiat_reserve = utils::mul_div(
            ctx.accounts.user_sovereign_coin_account.amount,
            sovereign_coin.usdc_amount,
            sovereign_coin.total_supply,
            Rounding::Down,
        )?;
    
        let from_fiat_reserve = utils::min(net_amount, user_share_of_fiat_reserve);
        let remaining_after_fiat = net_amount.checked_sub(from_fiat_reserve)
            .ok_or(StablecoinError::MathError)?;
    
        let protocol_vault_balance = ctx.accounts.usdc_protocol_vault.amount;
        let from_protocol_vault = utils::min(remaining_after_fiat, protocol_vault_balance);
        let from_bond_redemption = remaining_after_fiat.checked_sub(from_protocol_vault)
            .ok_or(StablecoinError::MathError)?;
    
        let redemption_type = if from_bond_redemption == 0 {
            if from_protocol_vault == 0 {
                RedemptionTypeState::UsdcReserveOnly
            } else {
                RedemptionTypeState::UsdcReserveAndProtocol
            }
        } else {
            // We need bond redemption - default to instant, NFT will be determined in ExecuteNFTRedemption
            RedemptionTypeState::InstantBondRedemption
        };
        let clock = Clock::get()?;
        redeem_state.payer = ctx.accounts.payer.key();
        redeem_state.sovereign_coin = ctx.accounts.sovereign_coin.key();
        redeem_state.sovereign_amount = sovereign_amount;  
        redeem_state.usdc_amount = usdc_amount;     
        redeem_state.net_amount = net_amount;
        redeem_state.from_usdc_reserve = from_fiat_reserve;
        redeem_state.from_protocol_vault = from_protocol_vault;
        redeem_state.from_bond_redemption = from_bond_redemption;
        redeem_state.protocol_fee = protocol_fee;
        redeem_state.redemption_type = redemption_type;
        redeem_state.created_at = clock.unix_timestamp;
        redeem_state.bump = ctx.bumps.redeem_state;
        
        Ok(())
    }
}

