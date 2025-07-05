use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct SetupBondHolding<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

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
        constraint = sovereign_coin.bond_holding == Pubkey::default() @ StablecoinError::BondHoldingAlreadyExists,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
    // The global fiat reserve token account 
    #[account(
        constraint = global_usdc_reserve.mint == USDC_MINT @ StablecoinError::InvalidUSDCMint,
        constraint = global_usdc_reserve.key() == factory.global_usdc_reserve @ StablecoinError::InvalidUSDCReserve,
    )]
    pub global_usdc_reserve: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // Creates a bond holding token account for this specific sovereign coin
    #[account(
        init,
        payer = creator,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory,
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,

    // Creates a bond ownership token account to store the bonds the protocol actually owns
    #[account(
        init,
        payer = creator,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory,
    )]
    pub bond_ownership: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        constraint = bond_token_mint.key() == sovereign_coin.bond_mint @ StablecoinError::InvalidBondMint
    )]
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

impl SetupBondHolding<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        
        // Set the bond holding token account for this sovereign coin
        sovereign_coin.bond_holding = ctx.accounts.bond_holding.key();
        sovereign_coin.bond_ownership = ctx.accounts.bond_ownership.key();

        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinBondHoldingSetupEvent {
            sovereign_coin: sovereign_coin.key(),
            bond_holding: sovereign_coin.bond_holding,
            bond_ownership: sovereign_coin.bond_ownership,
            bond_mint: ctx.accounts.bond_token_mint.key(),
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
}
