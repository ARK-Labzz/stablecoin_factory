use super::*;

// In instructions/setup_compressed_merkle_tree.rs
#[derive(Accounts)]
pub struct SetupCompressedMerkleTree<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"sovereign_coin", authority.key().as_ref(), &sovereign_coin.symbol],
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.authority == authority.key(),
        constraint = sovereign_coin.is_compressed @ StablecoinError::NotCompressedToken,
        constraint = sovereign_coin.merkle_tree.is_none() @ StablecoinError::MerkleTreeAlreadySet,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
    // Light Protocol specific accounts
    pub light_compressed_token_program: Program<'info, LightCompressedToken>,
    
    /// CHECK: Light Protocol manages this account
    #[account(mut)]
    pub merkle_tree: AccountInfo<'info>,
    
    // Other Light Protocol required accounts
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
    
    #[account(mut)]
    pub mint: Signer<'info>,
    
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<SetupCompressedMerkleTree>,
    initial_rate: Option<i16>
) -> Result<()> {
    // 1. Initialize token mint with interest-bearing extension if rate is provided
    if let Some(rate) = initial_rate {
        token_extension::initialize_interest_bearing_mint(
            &ctx.accounts.payer,
            &ctx.accounts.mint,
            &ctx.accounts.token_2022_program,
            &ctx.accounts.system_program,
            &ctx.accounts.authority.key(),
            rate,
            ctx.accounts.sovereign_coin.decimals,
        )?;
        
        // Update sovereign coin state for interest
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        sovereign_coin.is_interest_bearing = true;
        sovereign_coin.interest_rate = rate;
    } else {
        // Initialize regular mint
        // Similar to your existing SetupMint logic
    }
    
    // 2. Create the token pool in Light Protocol
    let cpi_accounts = light_compressed_token::cpi::accounts::CreateTokenPool {
        fee_payer: ctx.accounts.payer.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        registered_program_pda: ctx.accounts.registered_program_pda.to_account_info(),
        noop_program: ctx.accounts.noop_program.to_account_info(),
        account_compression_authority: ctx.accounts.account_compression_authority.to_account_info(),
        account_compression_program: ctx.accounts.account_compression_program.to_account_info(),
        self_program: ctx.accounts.light_compressed_token_program.to_account_info(),
        light_system_program: ctx.accounts.light_system_program.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
    };
    
    let cpi_ctx = CpiContext::new(
        ctx.accounts.light_compressed_token_program.to_account_info(),
        cpi_accounts,
    );
    
    // Call the CPI to create the token pool
    light_compressed_token::cpi::create_token_pool(cpi_ctx)?;
    
    let sovereign_coin = &mut ctx.accounts.sovereign_coin;
    sovereign_coin.mint = ctx.accounts.mint.key();
    sovereign_coin.merkle_tree = Some(ctx.accounts.merkle_tree.key());
    
    let clock = Clock::get()?;
    emit!(CompressedSovereignCoinMerkleTreeSetupEvent {
        sovereign_coin: sovereign_coin.key(),
        mint: sovereign_coin.mint,
        merkle_tree: sovereign_coin.merkle_tree.unwrap(),
        is_interest_bearing: sovereign_coin.is_interest_bearing,
        interest_rate: sovereign_coin.interest_rate,
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}