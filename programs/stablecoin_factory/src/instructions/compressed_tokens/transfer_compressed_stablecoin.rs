use super::*;

// In instructions/transfer_compressed_sovereign_coin.rs
#[derive(Accounts)]
pub struct TransferCompressedSovereignCoin<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    
    #[account(
        seeds = [b"sovereign_coin", authority.key().as_ref(), &sovereign_coin.symbol],
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.is_compressed @ StablecoinError::NotCompressedToken,
        constraint = sovereign_coin.merkle_tree.is_some() @ StablecoinError::MerkleTreeNotSet,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
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
    
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<TransferCompressedSovereignCoin>,
    amount: u64,
    recipient: Pubkey,
    proof: CompressedProof,
    input_token_data_with_context: Vec<InputTokenDataWithContext>,
    output_merkle_tree_indices: Vec<u8>,
) -> Result<()> {
    // Create output accounts for transfer
    let output_recipient = light_compressed_token::process_transfer::PackedTokenTransferOutputData {
        amount,
        owner: recipient,
        lamports: None,
        merkle_tree_index: output_merkle_tree_indices[0],
        tlv: None,
    };
    
    // Create change output if needed
    let input_sum: u64 = input_token_data_with_context.iter().map(|data| data.amount).sum();
    let change_amount = input_sum - amount;
    
    let mut output_accounts = vec![output_recipient];
    
    // Add change output if needed
    if change_amount > 0 {
        let output_change = light_compressed_token::process_transfer::PackedTokenTransferOutputData {
            amount: change_amount,
            owner: ctx.accounts.payer.key(),
            lamports: None,
            merkle_tree_index: output_merkle_tree_indices[1],
            tlv: None,
        };
        
        output_accounts.push(output_change);
    }
    
    // Create CPI accounts
    let cpi_accounts = light_compressed_token::cpi::accounts::TransferInstruction {
        fee_payer: ctx.accounts.payer.to_account_info(),
        authority: ctx.accounts.payer.to_account_info(),
        registered_program_pda: ctx.accounts.registered_program_pda.to_account_info(),
        noop_program: ctx.accounts.noop_program.to_account_info(),
        account_compression_authority: ctx.accounts.account_compression_authority.to_account_info(),
        account_compression_program: ctx.accounts.account_compression_program.to_account_info(),
        self_program: ctx.accounts.light_compressed_token_program.to_account_info(),
        light_system_program: ctx.accounts.light_system_program.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
        token_pool_pda: None,
        compress_or_decompress_token_account: None,
        token_program: None,
        cpi_authority_pda: None,
    };
    
    let mut cpi_ctx = CpiContext::new(
        ctx.accounts.light_compressed_token_program.to_account_info(),
        cpi_accounts,
    );
    
    // Add merkle tree account
    cpi_ctx.remaining_accounts.push(ctx.accounts.merkle_tree.to_account_info());
    
    // Create transfer instruction data
    let transfer_data = light_compressed_token::process_transfer::CompressedTokenInstructionDataTransfer {
        proof: Some(proof),
        mint: ctx.accounts.sovereign_coin.mint,
        delegated_transfer: None,
        input_token_data_with_context,
        output_compressed_accounts: output_accounts,
        is_compress: false,
        compress_or_decompress_amount: None,
        cpi_context: None,
        lamports_change_account_merkle_tree_index: None,
        with_transaction_hash: false,
    };
    
    // Serialize the instruction data
    let mut transfer_data_serialized = Vec::new();
    transfer_data.serialize(&mut transfer_data_serialized)?;
    
    // Call Light Protocol's transfer function
    light_compressed_token::cpi::transfer(cpi_ctx, transfer_data_serialized)?;
    
    // Emit event
    let clock = Clock::get()?;
    emit!(CompressedSovereignCoinTransferredEvent {
        from: ctx.accounts.payer.key(),
        to: recipient,
        sovereign_coin: ctx.accounts.sovereign_coin.key(),
        amount,
        timestamp: clock.unix_timestamp,
    });
    
    Ok(())
}