use super::*;

#[derive(Accounts)]
pub struct InitializeTransferFee<'info> {
    #[account(
        mut,
        constraint = is_admin(&admin.key()) @ StablecoinError::Unauthorized,
    )]
    pub admin: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,
    
    #[account(
        mut,
        seeds = [b"sovereign_coin", authority.key().as_ref(), &sovereign_coin.symbol],
        bump = sovereign_coin.bump,
    )]
    pub sovereign_coin: Account<'info, SovereignCoin>,
    
    /// CHECK: The authority of the sovereign coin
    pub authority: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub mint: Signer<'info>,
    
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint,
        associated_token::authority = factory,
    )]
    pub protocol_vault: InterfaceAccount<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_initialize_transfer_fee(
    ctx: Context<InitializeTransferFee>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    token_extension::initialize_transfer_fee_config(
        &ctx.accounts.admin,
        &ctx.accounts.mint,
        &ctx.accounts.token_program,
        &ctx.accounts.system_program,
        transfer_fee_basis_points,
        maximum_fee,
        ctx.accounts.sovereign_coin.decimals,
    )?;
    
    let factory = &mut ctx.accounts.factory;
    factory.protocol_vault = ctx.accounts.protocol_vault.key();
    factory.transfer_fee_bps = transfer_fee_basis_points;
    factory.maximum_transfer_fee = maximum_fee;
    
    Ok(())
}