use super::*;

#[derive(Accounts)]
pub struct SetupInterestBearingMint<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    
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
    
    // Make mint a signer since we need to create it
    #[account(mut)]
    pub mint: Signer<'info>,
    
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn handle_setup_interest_bearing_mint(
    ctx: Context<SetupInterestBearingMint>,
    initial_rate: i16,
) -> Result<()> {
    token_extension::initialize_interest_bearing_mint(
        &ctx.accounts.payer,
        &ctx.accounts.mint,
        &ctx.accounts.token_program, 
        &ctx.accounts.system_program,
        &ctx.accounts.authority.key(),
        initial_rate,
        ctx.accounts.sovereign_coin.decimals,
    )?;
    
    token_extension::check_interest_bearing_mint_data(
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.authority.key(),
    )?;
    
    let sovereign_coin = &mut ctx.accounts.sovereign_coin;
    sovereign_coin.mint = ctx.accounts.mint.key();
    sovereign_coin.interest_rate = initial_rate;
    sovereign_coin.is_interest_bearing = true;
    
    let clock = Clock::get()?;
    emit!(SovereignCoinInterestBearingInitializedEvent {
        sovereign_coin: sovereign_coin.key(),
        mint: sovereign_coin.mint,
        interest_rate: initial_rate,
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}