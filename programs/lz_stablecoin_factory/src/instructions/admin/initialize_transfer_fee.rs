use super::*;

#[derive(Accounts)]
pub struct InitializeTransferFee<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,

    /// The Sovereign Coin IBT mint
    #[account(mut)]
    pub mint: Signer<'info>,  
    
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = factory,
    )]
    pub sovereign_coin_protocol_vault: InterfaceAccount<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_initialize_transfer_fee(
    ctx: Context<InitializeTransferFee>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {

    // Initialize transfer fee with factory as authority (no factory signer needed here)
    token_extension::initialize_transfer_fee_config(
        &ctx.accounts.payer, // User pays for account creation
        &ctx.accounts.mint, // The IBT mint
        &ctx.accounts.token_program,
        &ctx.accounts.system_program,
        &ctx.accounts.factory.to_account_info(), // Factory PDA as authority
        transfer_fee_basis_points,
        maximum_fee,
        ctx.accounts.sovereign_coin.decimals,
    )?;
    
    // Update factory state
    let factory = &mut ctx.accounts.factory;
    factory.transfer_fee_bps = transfer_fee_basis_points;
    factory.maximum_transfer_fee = maximum_fee;
    
    Ok(())
}