use super::*;

#[derive(Accounts)]
pub struct WithdrawFromProtocolAccount<'info> {
    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,
    
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = factory,
        constraint = usdc_protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub usdc_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    
    /// CHECK: The destination account - verified by fee operator
    #[account(mut)]
    pub destination: UncheckedAccount<'info>,
    
    #[account(
        constraint = is_fee_operator(
            &operator.key(), 
            &claim_fee_operator
        ).map_err(|_| error!(StablecoinError::Unauthorized))? @ StablecoinError::Unauthorized,
    )]
    pub operator: Signer<'info>,
    
    #[account(
        seeds = [b"fee_operator", operator.key().as_ref()],
        bump,
    )]
    pub claim_fee_operator: Account<'info, FeeOperator>,
    
    #[account(
        constraint = usdc_mint.key() == USDC_MINT @ StablecoinError::InvalidUSDCMint
    )]
    pub usdc_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub token_program: Interface<'info, TokenInterface>,
}

pub fn handle_withdraw_from_protocol_account(
    ctx: Context<WithdrawFromProtocolAccount>,
    amount: u64,
) -> Result<()> {
    let factory_seeds = &[
        b"factory".as_ref(),
        &[ctx.accounts.factory.bump],
    ];
    let factory_signer = &[&factory_seeds[..]];
    
    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.usdc_protocol_vault.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
                authority: ctx.accounts.factory.to_account_info(),
            },
            factory_signer,
        ),
        amount,
        ctx.accounts.usdc_mint.decimals,
    )?;
    
    Ok(())
}