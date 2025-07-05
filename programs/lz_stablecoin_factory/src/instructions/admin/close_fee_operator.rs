use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct CloseFeeOperatorCtx<'info> {
    #[account(
        mut,
        close = rent_receiver,
    )]
    pub claim_fee_operator: Account<'info, FeeOperator>,
    
    /// CHECK: rent receiver
    #[account(mut)]
    pub rent_receiver: UncheckedAccount<'info>,
    
    #[account(
        constraint = is_admin(&admin.key()) @ StablecoinError::Unauthorized,
    )]
    pub admin: Signer<'info>,
}

pub fn handle_close_fee_operator(ctx: Context<CloseFeeOperatorCtx>) -> Result<()> {
    msg!("Fee operator closed");
    
    emit_cpi!(EvtCloseClaimFeeOperator {
        claim_fee_operator: ctx.accounts.claim_fee_operator.key(),
        operator: ctx.accounts.claim_fee_operator.operator,
    });
    
    Ok(())
}