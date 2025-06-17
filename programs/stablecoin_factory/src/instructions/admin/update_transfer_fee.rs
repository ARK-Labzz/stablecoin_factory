use super::*;

#[derive(Accounts)]
pub struct UpdateTransferFee<'info> {
    #[account(
        constraint = is_admin(&admin.key()) @ StablecoinError::Unauthorized,
    )]
    pub admin: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,
    
    #[account(mut)]
    pub mint_account: InterfaceAccount<'info, Mint>,
    
    pub token_program: Program<'info, Token2022>,
}

pub fn handle_update_transfer_fee(
    ctx: Context<UpdateTransferFee>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {

    token_extension::update_transfer_fee_config(
        &ctx.accounts.admin,
        &ctx.accounts.mint_account.to_account_info(),
        &ctx.accounts.token_program,
        transfer_fee_basis_points,
        maximum_fee,
    )?;
    
    let factory = &mut ctx.accounts.factory;
    factory.transfer_fee_bps = transfer_fee_basis_points;
    factory.maximum_transfer_fee = maximum_fee;
    
    Ok(())
}