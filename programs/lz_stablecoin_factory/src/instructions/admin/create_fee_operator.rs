use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct CreateFeeOperatorCtx<'info> {
    #[account(
        init,
        payer = admin,
        seeds = [b"fee_operator", operator.key().as_ref()],
        bump,
        space = 8 + FeeOperator::INIT_SPACE
    )]
    pub claim_fee_operator: Account<'info, FeeOperator>,
    
    /// CHECK: operator to be appointed
    pub operator: UncheckedAccount<'info>,
    
    #[account(
        mut,
        constraint = is_admin(&admin.key()) @ StablecoinError::Unauthorized,
    )]
    pub admin: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

pub fn handle_create_fee_operator(ctx: Context<CreateFeeOperatorCtx>) -> Result<()> {
    let claim_fee_operator = &mut ctx.accounts.claim_fee_operator;
    claim_fee_operator.initialize(ctx.accounts.operator.key())?;
    claim_fee_operator.bump = ctx.bumps.claim_fee_operator;
    
    emit_cpi!(EvtCreateClaimFeeOperator {
        operator: ctx.accounts.operator.key(),
    });
    
    Ok(())
}