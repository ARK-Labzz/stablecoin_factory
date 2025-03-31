use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct SetupMint<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub authority: Signer<'info>,
    
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
    
   
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub rent: Sysvar<'info, Rent>,
}

impl SetupMint<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
       
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        sovereign_coin.mint = ctx.accounts.mint.key();

      
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinSetupMintEvent {
            mint: sovereign_coin.mint,
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
}
