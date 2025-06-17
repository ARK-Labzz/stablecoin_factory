use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct UpdateCompressedInterestRate<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
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
        constraint = sovereign_coin.is_interest_bearing @ StablecoinError::NotInterestBearing,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
    /// CHECK: Verified in constraint
    #[account(
        mut,
        constraint = mint.key() == sovereign_coin.mint @ StablecoinError::InvalidMint,
    )]
    pub mint: AccountInfo<'info>,
    
    #[account(
        constraint = bond_token_mint.key() == sovereign_coin.bond_mint @ StablecoinError::InvalidBondMint,
    )]
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(
        constraint = bond_holding.key() == sovereign_coin.bond_holding @ StablecoinError::InvalidBondHolding,
        constraint = bond_holding.mint == sovereign_coin.bond_mint @ StablecoinError::InvalidBondMint,
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub token_2022_program: Program<'info, Token2022>,
}

// Continuing from where we left off in instructions/update_compressed_interest_rate.rs
pub fn handler(ctx: Context<UpdateCompressedInterestRate>, manual_rate: Option<i16>) -> Result<()> {
    let new_rate = if let Some(rate) = manual_rate {
        // Use manually specified rate
        rate
    } else {
        // Read bond interest rate
        let bond_rate = token_extension::read_current_interest_rate(
            &ctx.accounts.bond_token_mint.to_account_info()
        )?;
        
        // Calculate sovereign coin rate
        interest::calculate_sovereign_interest_rate(
            bond_rate,
            ctx.accounts.bond_holding.amount,
        )?
    };
    
    // Update the interest rate
    token_extension::update_interest_rate(
        &ctx.accounts.authority,
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.token_2022_program,
        new_rate,
    )?;
    
    // Store previous rate for event
    let old_rate = ctx.accounts.sovereign_coin.interest_rate;
    
    // Update sovereign coin state
    let sovereign_coin = &mut ctx.accounts.sovereign_coin;
    sovereign_coin.interest_rate = new_rate;
    
    // Emit event
    let clock = Clock::get()?;
    emit_cpi!(CompressedSovereignCoinInterestRateUpdatedEvent {
        sovereign_coin: sovereign_coin.key(),
        mint: sovereign_coin.mint,
        old_rate,
        new_rate,
        bond_mint: sovereign_coin.bond_mint,
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}