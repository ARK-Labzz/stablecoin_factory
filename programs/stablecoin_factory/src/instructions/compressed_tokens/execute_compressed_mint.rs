use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteCompressedMint<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    
    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Box<Account<'info, Factory>>,

    #[account(
        mut,
        seeds = [b"sovereign_coin", authority.key().as_ref(), &sovereign_coin.symbol],
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.is_compressed @ StablecoinError::NotCompressedToken,
        constraint = sovereign_coin.merkle_tree.is_some() @ StablecoinError::MerkleTreeNotSet,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    #[account(
        mut,
        seeds = [b"compressed_mint_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = compressed_mint_state.bump,
        constraint = compressed_mint_state.authority == authority.key() && compressed_mint_state.payer == payer.key(),
        close = payer 
    )]
    pub compressed_mint_state: Box<Account<'info, CompressedMintState>>,

    // User's source USDC account (to pay from)
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = payer,
    )]
    pub user_fiat_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    // Protocol's fiat reserve
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = authority,
    )]
    pub fiat_reserve: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // Protocol's bond holding
    #[account(
        mut,
        token::mint = bond_token_mint,
        token::authority = authority,
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Light Protocol accounts
    pub light_compressed_token_program: Program<'info, LightCompressedToken>,
    
    /// CHECK: Validated by Light Protocol
    #[account(mut)]
    pub merkle_tree: AccountInfo<'info>,
    
    /// CHECK: Validated by Light Protocol
    pub registered_program_pda: AccountInfo<'info>,
    /// CHECK: Validated by Light Protocol
    pub noop_program: AccountInfo<'info>,
    /// CHECK: Validated by Light Protocol
    pub account_compression_authority: AccountInfo<'info>,
    /// CHECK: Validated by Light Protocol
    pub account_compression_program: AccountInfo<'info>,
    /// CHECK: Validated by Light Protocol
    pub light_system_program: AccountInfo<'info>,
    
    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ExecuteCompressedMint>,
    output_accounts_merkle_tree_indices: Vec<u8>,
) -> Result<()> {
    let sovereign_coin = &mut ctx.accounts.sovereign_coin;
    let mint_state = &ctx.accounts.compressed_mint_state;
    
    // 1. Transfer USDC to fiat reserve with fee
    if mint_state.reserve_amount > 0 {
        token_extension::transfer_with_fee(
            &ctx.accounts.token_2022_program,
            &ctx.accounts.user_fiat_token_account.to_account_info(),
            &ctx.accounts.fiat_token_mint.to_account_info(),
            &ctx.accounts.fiat_reserve.to_account_info(),
            &ctx.accounts.payer,
            mint_state.reserve_amount,
            ctx.accounts.fiat_token_mint.decimals,
        )?;
    }

    // 2. Purchase bonds (similar to your ExecuteMintSovereignCoin)
    // ... Bond purchase code ...
    
    // 3. Mint compressed sovereign coins to user
    
    // First create the output account data for user
    let output_user_account = light_compressed_token::process_transfer::PackedTokenTransferOutputData {
        amount: mint_state.sovereign_amount,
        owner: ctx.accounts.payer.key(),
        lamports: None, // No lamports needed
        merkle_tree_index: output_accounts_merkle_tree_indices[0],
        tlv: None,
    };
    
    // Create CPI accounts
    let cpi_accounts = light_compressed_token::cpi::accounts::MintToInstruction {
        fee_payer: ctx.accounts.payer.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
        mint: sovereign_coin.mint.to_account_info(),
        registered_program_pda: ctx.accounts.registered_program_pda.to_account_info(),
        noop_program: ctx.accounts.noop_program.to_account_info(),
        account_compression_authority: ctx.accounts.account_compression_authority.to_account_info(),
        account_compression_program: ctx.accounts.account_compression_program.to_account_info(),
        self_program: ctx.accounts.light_compressed_token_program.to_account_info(),
        light_system_program: ctx.accounts.light_system_program.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
    };
    
    let mut cpi_ctx = CpiContext::new(
        ctx.accounts.light_compressed_token_program.to_account_info(),
        cpi_accounts,
    );
    
    // Add merkle tree account
    cpi_ctx.remaining_accounts.push(ctx.accounts.merkle_tree.to_account_info());
    
    // Prepare inputs for mint_to
    // This is what Light Protocol expects
    let mint_to_data = light_compressed_token::process_transfer::MintToInstruction {
        mint: sovereign_coin.mint,
        // Set the amount and output account details
        amount_and_pubkey_pairs: vec![(
            mint_state.sovereign_amount,
            ctx.accounts.payer.key(),
        )],
        merkle_tree_indexes: output_accounts_merkle_tree_indices,
        lamports: None, // No lamports needed
    };
    
    // Serialize the instruction data
    let mut mint_to_data_serialized = Vec::new();
    mint_to_data.serialize(&mut mint_to_data_serialized)?;
    
    // Call Light Protocol's mint_to function
    light_compressed_token::cpi::mint_to(cpi_ctx, mint_to_data_serialized)?;
    
    // 4. Update sovereign coin state
    sovereign_coin.total_supply = sovereign_coin.total_supply
        .safe_add(mint_state.sovereign_amount)?;
    sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
        .safe_add(mint_state.reserve_amount)?;
    sovereign_coin.bond_amount = sovereign_coin.bond_amount
        .safe_add(mint_state.bond_amount)?;
    
    // 5. Emit event
    let clock = Clock::get()?;
    emit_cpi!(CompressedSovereignCoinMintedEvent {
        payer: ctx.accounts.payer.key(),
        sovereign_coin: ctx.accounts.sovereign_coin.key(),
        usdc_amount: mint_state.usdc_amount,
        sovereign_amount: mint_state.sovereign_amount,
        reserve_amount: mint_state.reserve_amount,
        bond_amount: mint_state.bond_amount, 
        protocol_fee: mint_state.protocol_fee,
        merkle_tree: sovereign_coin.merkle_tree.unwrap(),
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}