use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct SetupGlobalUsdcAccounts<'info> {
    
    #[account(
        mut,
        constraint = factory.authority == admin.key() @ StablecoinError::Unauthorized,
        constraint = is_admin(&admin.key()) @ StablecoinError::Unauthorized
    )]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Box<Account<'info, Factory>>,
    
    #[account(
        init,
        payer = admin,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory, 
    )]
    pub global_usdc_reserve: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = admin,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory, 
    )]
    pub global_usdc_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = admin,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory, 
    )]
    pub usdc_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        constraint = usdc_mint.key() == USDC_MINT @ StablecoinError::InvalidUSDCMint
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

impl SetupGlobalUsdcAccounts<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let factory = &mut ctx.accounts.factory;
        factory.global_usdc_reserve = ctx.accounts.global_usdc_reserve.key();
        factory.global_usdc_account = ctx.accounts.global_usdc_account.key();
        factory.protocol_vault = ctx.accounts.usdc_protocol_vault.key();
        
        let clock = Clock::get()?;
        emit_cpi!(GlobalUsdcAccountsCreatedEvent {
            usdc_reserve: ctx.accounts.global_usdc_reserve.key(),
            usdc_reserve_authority: ctx.accounts.factory.key(),
            usdc_mint: ctx.accounts.usdc_mint.key(),
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
}