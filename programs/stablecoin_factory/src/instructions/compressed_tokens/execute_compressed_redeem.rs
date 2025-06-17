use super::*;


#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteCompressedRedeem<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sovereign_coin", authority.key().as_ref(), &sovereign_coin.symbol],
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.is_compressed @ StablecoinError::NotCompressedToken,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    #[account(
        mut,
        seeds = [b"compressed_redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = compressed_redeem_state.bump,
        constraint = compressed_redeem_state.authority == authority.key() && compressed_redeem_state.payer == payer.key(),
        close = payer 
    )]
    pub compressed_redeem_state: Box<Account<'info, CompressedRedeemState>>,

    // User's USDC account (will receive funds)
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = payer,
    )]
    pub user_fiat_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    // Fiat reserve
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = authority,
    )]
    pub fiat_reserve: Box<InterfaceAccount<'info, TokenAccount>>,

    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Light Protocol accounts
    pub light_compressed_token_program: Program<'info, LightCompressedToken>,
    
    /// CHECK: Validated by Light Protocol
    #[account(mut)]
    pub merkle_tree: AccountInfo<'info>,
    
    /// CHECK: Light Protocol validations
    pub registered_program_pda: AccountInfo<'info>,
    /// CHECK: Light Protocol validations
    pub noop_program: AccountInfo<'info>,
    /// CHECK: Light Protocol validations
    pub account_compression_authority: AccountInfo<'info>,
    /// CHECK: Light Protocol validations
    pub account_compression_program: AccountInfo<'info>,
    /// CHECK: Light Protocol validations
    pub light_system_program: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ExecuteCompressedRedeem>,
    proof: CompressedProof,
    input_token_data_with_context: Vec<InputTokenDataWithContext>,
) -> Result<()> {
    let sovereign_coin = &mut ctx.accounts.sovereign_coin;
    let redeem_state = &ctx.accounts.compressed_redeem_state;
    
    // 1. Burn compressed sovereign coins
    // Create CPI accounts
    let cpi_accounts = light_compressed_token::cpi::accounts::BurnInstruction {
        fee_payer: ctx.accounts.payer.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(), // User is burning their tokens
        registered_program_pda: ctx.accounts.registered_program_pda.to_account_info(),
        noop_program: ctx.accounts.noop_program.to_account_info(),
        account_compression_authority: ctx.accounts.account_compression_authority.to_account_info(),
        account_compression_program: ctx.accounts.account_compression_program.to_account_info(),
        self_program: ctx.accounts.light_compressed_token_program.to_account_info(),
        mint: sovereign_coin.mint.to_account_info(),
        light_system_program: ctx.accounts.light_system_program.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
    };
    
    let mut cpi_ctx = CpiContext::new(
        ctx.accounts.light_compressed_token_program.to_account_info(),
        cpi_accounts,
    );
    
    // Add merkle tree account
    cpi_ctx.remaining_accounts.push(ctx.accounts.merkle_tree.to_account_info());
    
    // Create burn instruction data
    let burn_data = light_compressed_token::process_burn::BurnInstruction {
        proof: Some(proof),
        mint: sovereign_coin.mint,
        amount: redeem_state.sovereign_amount,
        input_token_data_with_context,
        from_token_pool_account: None, // Not burning from token pool
    };
    
    // Serialize the instruction data
    let mut burn_data_serialized = Vec::new();
    burn_data.serialize(&mut burn_data_serialized)?;
    
    // Call Light Protocol's burn function
    light_compressed_token::cpi::burn(cpi_ctx, burn_data_serialized)?;
    
    // 2. Transfer USDC from fiat reserve to user if needed
    if redeem_state.from_fiat_reserve > 0 {
        token_extension::transfer_with_fee(
            &ctx.accounts.token_2022_program,
            &ctx.accounts.fiat_reserve.to_account_info(),
            &ctx.accounts.fiat_token_mint.to_account_info(),
            &ctx.accounts.user_fiat_token_account.to_account_info(),
            &ctx.accounts.authority,
            redeem_state.from_fiat_reserve,
            ctx.accounts.fiat_token_mint.decimals,
        )?;
    }
    
    // 3. Transfer from protocol vault if needed (would need protocol vault account)
    // (This would be similar to ExecuteRedeemFromFiatAndProtocol)
    
    // 4. Update sovereign coin state
    sovereign_coin.total_supply = sovereign_coin.total_supply
        .safe_sub(redeem_state.sovereign_amount)?;
    sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
        .safe_sub(redeem_state.from_fiat_reserve)?;
    // Adjust bond amount if needed
    
    // 5. Emit event
    let clock = Clock::get()?;
    emit_cpi!(CompressedSovereignCoinRedeemedEvent {
        payer: ctx.accounts.payer.key(),
        sovereign_coin: ctx.accounts.sovereign_coin.key(),
        sovereign_amount: redeem_state.sovereign_amount,
        usdc_amount: redeem_state.usdc_amount,
        from_fiat_reserve: redeem_state.from_fiat_reserve,
        from_protocol_vault: redeem_state.from_protocol_vault,
        from_bond_redemption: redeem_state.from_bond_redemption,
        protocol_fee: redeem_state.protocol_fee,
        timestamp: clock.unix_timestamp,
        redemption_type: redeem_state.redemption_type,
    });
    
    Ok(())
}