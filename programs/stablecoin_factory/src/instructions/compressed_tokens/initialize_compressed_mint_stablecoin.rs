use super::*;

#[derive(Accounts)]
#[instruction(args: InitializeCompressedMintArgs)]
pub struct InitializeCompressedMint<'info> {
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
        constraint = sovereign_coin.authority == authority.key(),
        constraint = sovereign_coin.is_compressed @ StablecoinError::NotCompressedToken,
        constraint = sovereign_coin.merkle_tree.is_some() @ StablecoinError::MerkleTreeNotSet,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
    // Compressed mint state storage
    #[account(
        init,
        payer = payer,
        space = 8 + CompressedMintState::INIT_SPACE,
        seeds = [b"compressed_mint_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump
    )]
    pub compressed_mint_state: Box<Account<'info, CompressedMintState>>,
    
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
pub struct InitializeCompressedMintArgs {
    pub usdc_amount: u64,
    pub merkle_tree_index: u8,
}

pub fn handler(
    ctx: Context<InitializeCompressedMint>, 
    args: InitializeCompressedMintArgs
) -> Result<()> {
    
    let factory = &ctx.accounts.factory;
    let sovereign_coin = &ctx.accounts.sovereign_coin;
    let mint_state = &mut ctx.accounts.compressed_mint_state;
    
    require!(args.usdc_amount > 0, StablecoinError::InvalidAmount);
    
    let (net_amount, protocol_fee) = fee::calculate_protocol_fee(
        args.usdc_amount,
        factory.transfer_fee_bps,
    )?;

    let required_reserve_percentage = reserve::calculate_required_reserve(
        factory.min_fiat_reserve_percentage,
        sovereign_coin.bond_rating,
        factory.bond_reserve_numerator,
        factory.bond_reserve_denominator,
    )?;

    let (reserve_amount, bond_amount) = reserve::calculate_reserve_and_bond_amounts(
        net_amount,
        required_reserve_percentage,
    )?;
    
    let sovereign_amount = conversion::calculate_sovereign_coin_amount(
        args.usdc_amount,
        &ctx.accounts.payment_base_price_feed_account,
        ctx.accounts.payment_quote_price_feed_account.as_ref()
            .map(|acc| &acc.to_account_info()),
        std::str::from_utf8(&sovereign_coin.target_fiat_currency)?,
        sovereign_coin.decimals,
    )?;

    mint_state.authority = ctx.accounts.authority.key();
    mint_state.payer = ctx.accounts.payer.key();
    mint_state.sovereign_coin = ctx.accounts.sovereign_coin.key();
    mint_state.usdc_amount = args.usdc_amount;
    mint_state.sovereign_amount = sovereign_amount;
    mint_state.reserve_amount = reserve_amount;
    mint_state.bond_amount = bond_amount;
    mint_state.protocol_fee = protocol_fee;
    mint_state.merkle_tree_index = args.merkle_tree_index;
    mint_state.bump = ctx.bumps.compressed_mint_state;
    
    Ok(())
}