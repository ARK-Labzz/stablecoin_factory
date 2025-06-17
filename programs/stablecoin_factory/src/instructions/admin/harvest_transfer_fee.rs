use super::*;

#[derive(Accounts)]
pub struct HarvestFees<'info> {
    #[account(mut)]
    pub mint_account: InterfaceAccount<'info, Mint>,
    
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

pub fn handle_harvest_fees<'info>(
    ctx: Context<'_, '_, 'info, 'info, HarvestFees<'info>>
) -> Result<()> {
    let token_accounts = ctx.remaining_accounts
        .iter()
        .filter_map(|account| {
            InterfaceAccount::<TokenAccount>::try_from(account)
                .ok()
                .filter(|token_account| token_account.mint == ctx.accounts.mint_account.key())
                .map(|_| account.to_account_info())
        })
        .collect::<Vec<_>>();
    
    require!(!token_accounts.is_empty(), StablecoinError::NoTokenAccountsToHarvest);
    
    token_extension::harvest_fees(
        &ctx.accounts.token_program,
        &ctx.accounts.mint_account.to_account_info(),
        token_accounts,
    )?;
    
    Ok(())
}