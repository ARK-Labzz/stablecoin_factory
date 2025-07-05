use super::*;

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,
    
    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,
    
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = factory,
    )]
    pub sovereign_coin_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
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
    pub claim_fee_operator: Account<'info, FeeOperator>,
    
    pub token_program: Program<'info, Token2022>,
}

pub fn handle_withdraw_fees(
    ctx: Context<WithdrawFees>,
) -> Result<()> {
    let factory_seeds = &[
        b"factory".as_ref(),
        &[ctx.accounts.factory.bump],
    ];
    let factory_signer = &[&factory_seeds[..]];
    token_extension::withdraw_fees_signed(
        &ctx.accounts.factory.to_account_info(),
        factory_signer,
        &ctx.accounts.token_program,
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.sovereign_coin_protocol_vault.to_account_info(),
    )?;
    
    Ok(())
}