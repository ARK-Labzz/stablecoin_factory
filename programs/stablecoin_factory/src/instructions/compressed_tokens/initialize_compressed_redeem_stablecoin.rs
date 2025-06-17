use super::*;

#[derive(Accounts)]
#[instruction(args: InitializeCompressedRedeemArgs)]
pub struct InitializeCompressedRedeem<'info> {
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
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.is_compressed @ StablecoinError::NotCompressedToken,
        constraint = sovereign_coin.merkle_tree.is_some() @ StablecoinError::MerkleTreeNotSet,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    // Fiat reserve (to check balance)
    #[account(
        token::mint = fiat_token_mint,
        token::authority = authority,
    )]
    pub fiat_reserve: Box<InterfaceAccount<'info, TokenAccount>>,

    // Protocol vault (to check balance)
    #[account(
        constraint = protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Light Protocol accounts
    pub light_compressed_token_program: Program<'info, LightCompressedToken>,
    
    /// CHECK: Validated by Light Protocol
    #[account(mut)]
    pub merkle_tree: AccountInfo<'info>,
    
    // Compressed redeem state storage
    #[account(
        init,
        payer = payer,
        space = 8 + CompressedRedeemState::INIT_SPACE,
        seeds = [b"compressed_redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump
    )]
    pub compressed_redeem_state: Box<Account<'info, CompressedRedeemState>>,
    
    // Oracle price feed accounts
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

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeCompressedRedeemArgs {
    pub sovereign_amount: u64,
    pub merkle_tree_index: u8,
}

pub fn handler(
    ctx: Context<InitializeCompressedRedeem>,
    args: InitializeCompressedRedeemArgs
) -> Result<()> {
    let factory = &ctx.accounts.factory;
    let sovereign_coin = &ctx.accounts.sovereign_coin;
    let redeem_state = &mut ctx.accounts.compressed_redeem_state;
    
    require!(args.sovereign_amount > 0, StablecoinError::InvalidAmount);
    
    // Calculate USDC amount based on exchange rate
    let usdc_amount = conversion::calculate_usdc_for_sovereign_amount(
        args.sovereign_amount,
        &ctx.accounts.payment_base_price_feed_account,
        ctx.accounts.payment_quote_price_feed_account.as_ref()
            .map(|acc| &acc.to_account_info()),
        std::str::from_utf8(&sovereign_coin.target_fiat_currency)?,
        sovereign_coin.decimals,
    )?;
    
    // Calculate protocol fee
    let (net_amount, protocol_fee) = fee::calculate_protocol_fee(
        usdc_amount,
        factory.transfer_fee_bps,  
    )?;
    
    // Calculate user's share of the fiat reserve
    let user_share_of_fiat_reserve = utils::mul_div(
        args.sovereign_amount,
        sovereign_coin.fiat_amount,
        sovereign_coin.total_supply,
        Rounding::Down,
    )?;
    
    // Determine how much to take from each source
    let from_fiat_reserve = utils::min(net_amount, user_share_of_fiat_reserve);
    let remaining_after_fiat = net_amount.checked_sub(from_fiat_reserve)
        .ok_or(StablecoinError::MathError)?;
    
    let protocol_vault_balance = ctx.accounts.protocol_vault.amount;
    let from_protocol_vault = utils::min(remaining_after_fiat, protocol_vault_balance);
    let from_bond_redemption = remaining_after_fiat.checked_sub(from_protocol_vault)
        .ok_or(StablecoinError::MathError)?;
    
    // Determine the redemption type
    let redemption_type = if from_bond_redemption == 0 {
        if from_protocol_vault == 0 {
            RedemptionTypeState::FiatReserveOnly
        } else {
            RedemptionTypeState::FiatAndProtocol
        }
    } else {
        RedemptionTypeState::InstantBondRedemption
    };
    
    
    redeem_state.authority = ctx.accounts.authority.key();
    redeem_state.payer = ctx.accounts.payer.key();
    redeem_state.sovereign_coin = ctx.accounts.sovereign_coin.key();
    redeem_state.sovereign_amount = args.sovereign_amount;
    redeem_state.usdc_amount = usdc_amount;
    redeem_state.net_amount = net_amount;
    redeem_state.from_fiat_reserve = from_fiat_reserve;
    redeem_state.from_protocol_vault = from_protocol_vault;
    redeem_state.from_bond_redemption = from_bond_redemption;
    redeem_state.protocol_fee = protocol_fee;
    redeem_state.redemption_type = redemption_type;
    redeem_state.merkle_tree_index = args.merkle_tree_index;
    redeem_state.bump = ctx.bumps.compressed_redeem_state;
    
    Ok(())
}