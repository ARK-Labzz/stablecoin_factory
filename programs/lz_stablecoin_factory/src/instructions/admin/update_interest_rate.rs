use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct UpdateInterestRate<'info> {

        #[account(
        mut,
        constraint = is_admin(&admin.key()) @ StablecoinError::Unauthorized,
    )]
    pub admin: Signer<'info>,
    
    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Box<Account<'info, Factory>>,
    
    #[account(
        mut,
        seeds = [
            b"sovereign_coin",
            factory.key().as_ref(), 
            &sovereign_coin.symbol[..sovereign_coin.symbol.iter().position(|&x| x == 0).unwrap_or(8)]
        ],
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.is_interest_bearing @ StablecoinError::NotInterestBearing,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
    #[account(
        mut,
        constraint = mint.key() == sovereign_coin.mint @ StablecoinError::InvalidSovereignCoinMint,
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    
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

    pub fn handle_update_interest_rate(ctx: Context<UpdateInterestRate>, manual_rate: Option<i16>) -> Result<()> {
        let new_rate = if let Some(rate) = manual_rate {
            rate
        } else {
            let bond_rate = token_extension::read_current_interest_rate(
                &ctx.accounts.bond_token_mint.to_account_info()
            )?;
            
            interest::calculate_sovereign_interest_rate(
                bond_rate,
                ctx.accounts.bond_holding.amount,
            )?
        };
        let factory_seeds = &[
            b"factory".as_ref(),
            &[ctx.accounts.factory.bump],
        ];
        let factory_signer = &[&factory_seeds[..]];
        token_extension::update_interest_rate_signed(
            &ctx.accounts.factory.to_account_info(),
            factory_signer,
            &ctx.accounts.mint.to_account_info(),
            &ctx.accounts.token_2022_program,
            new_rate,
        )?;
        
        let old_rate = ctx.accounts.sovereign_coin.interest_rate;
        
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        sovereign_coin.interest_rate = new_rate;
        
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinInterestRateUpdatedEvent {
            sovereign_coin: sovereign_coin.key(),
            mint: sovereign_coin.mint,
            old_rate,
            new_rate,
            bond_mint: sovereign_coin.bond_mint,
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
