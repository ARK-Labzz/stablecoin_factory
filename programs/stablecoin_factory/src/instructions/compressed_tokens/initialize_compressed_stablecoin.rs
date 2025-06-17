use super::*;

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: SovereignCoinArgs)]
pub struct CreateCompressedSovereignCoin<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub authority: Signer<'info>,
    
    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Box<Account<'info, Factory>>,
    
    #[account(
        init,
        payer = payer,
        space = 8 + SovereignCoin::INIT_SPACE,
        seeds = [b"sovereign_coin", authority.key().as_ref(), args.symbol.as_bytes()],
        bump
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(ctx: Context<CreateCompressedSovereignCoin>, args: SovereignCoinArgs) -> Result<()> {
    // Similar to your existing InitSovereignCoin handler
    // But also set is_compressed to true
    
    let sovereign_coin = &mut ctx.accounts.sovereign_coin;
    sovereign_coin.bump = ctx.bumps.sovereign_coin;
    sovereign_coin.authority = ctx.accounts.authority.key();
    sovereign_coin.factory = ctx.accounts.factory.key();
    
    // Copy name and symbol
    let name_bytes = args.name.as_bytes();
    let symbol_bytes = args.symbol.as_bytes();
    
    sovereign_coin.name = [0u8; 32];
    sovereign_coin.symbol = [0u8; 8];
    
    sovereign_coin.name[..name_bytes.len()].copy_from_slice(name_bytes);
    sovereign_coin.symbol[..symbol_bytes.len()].copy_from_slice(symbol_bytes);
    
    // Set URI
    let uri_bytes = args.uri.as_bytes();
    sovereign_coin.uri = [0u8; 200];
    sovereign_coin.uri[..uri_bytes.len()].copy_from_slice(uri_bytes);

    // Set currency
    let fiat_bytes = args.fiat_currency.as_bytes();
    sovereign_coin.target_fiat_currency = [0u8; 8];
    sovereign_coin.target_fiat_currency[..fiat_bytes.len()].copy_from_slice(fiat_bytes);
    
    // Find and set the bond mapping
    // (Similar to existing code)
    
    // Mark as compressed
    sovereign_coin.is_compressed = true;
    sovereign_coin.merkle_tree = None; // Will be set in SetupCompressedMerkleTree
    
    // Set defaults
    sovereign_coin.decimals = 6;
    sovereign_coin.total_supply = 0;
    sovereign_coin.fiat_amount = 0;
    sovereign_coin.bond_amount = 0;
    sovereign_coin.is_interest_bearing = false;
    sovereign_coin.interest_rate = 0;

    let clock = Clock::get()?;
    emit_cpi!(CompressedSovereignCoinInitializedEvent {
        authority: ctx.accounts.authority.key(),
        sovereign_coin: sovereign_coin.key(),
        name: args.name,
        symbol: args.symbol,
        fiat_currency: args.fiat_currency,
        bond_mint: sovereign_coin.bond_mint,
        bond_account: sovereign_coin.bond_account,
        bond_rating: sovereign_coin.bond_rating,
        decimals: sovereign_coin.decimals,
        total_supply: sovereign_coin.total_supply,
        required_reserve_percentage: sovereign_coin.required_reserve_percentage,
        fiat_amount: sovereign_coin.fiat_amount,
        bond_amount: sovereign_coin.bond_amount,
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}