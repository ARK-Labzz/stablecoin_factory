use super::*;

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub mint_account: InterfaceAccount<'info, Mint>,
    
    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,
    
    #[account(
        mut,
        token::mint = mint_account,
        token::authority = factory,
    )]
    pub protocol_vault: InterfaceAccount<'info, TokenAccount>,
    
    #[account(
        constraint = is_fee_operator(
            &operator.key(), 
            &claim_fee_operator
        ).map_err(|_| StablecoinError::Unauthorized)? @ StablecoinError::Unauthorized,
    )]
    pub operator: Signer<'info>,
    
    #[account(
        seeds = [b"fee_operator", operator.key().as_ref()],
        bump,
    )]
    pub claim_fee_operator: AccountLoader<'info, FeeOperator>,
    
    pub token_program: Program<'info, Token2022>,
}

pub fn handle_withdraw_fees(
    ctx: Context<WithdrawFees>,
) -> Result<()> {
    
    token_extension::withdraw_fees(
        &ctx.accounts.token_program,
        &ctx.accounts.mint_account.to_account_info(),
        &ctx.accounts.protocol_vault.to_account_info(),
        &ctx.accounts.factory,
    )?;
    
    Ok(())
}