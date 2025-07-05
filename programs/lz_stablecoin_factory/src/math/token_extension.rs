use super::*;


pub fn initialize_transfer_fee_config<'info>(
    payer: &Signer<'info>,
    mint_account: &Signer<'info>,
    token_program: &Program<'info, Token2022>,
    system_program: &Program<'info, System>,
    factory_pda: &AccountInfo<'info>, // Factory PDA will be the authority
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
    decimals: u8,
) -> Result<()> {
    // Calculate space required for mint and extension data
    let mint_size = ExtensionType::try_calculate_account_len::<PodMint>(
        &[ExtensionType::TransferFeeConfig]
    )?;

    // Calculate minimum lamports required for size of mint account with extensions
    let lamports = (Rent::get()?).minimum_balance(mint_size);

    // Create new account with space for mint and extension data
    create_account(
        CpiContext::new(
            system_program.to_account_info(),
            CreateAccount {
                from: payer.to_account_info(),
                to: mint_account.to_account_info(),
            },
        ),
        lamports,
        mint_size as u64,
        &token_program.key(),
    )?;

    // Initialize the transfer fee extension data
    transfer_fee_initialize(
        CpiContext::new(
            token_program.to_account_info(),
            TransferFeeInitialize {
                token_program_id: token_program.to_account_info(),
                mint: mint_account.to_account_info(),
            },
        ),
        Some(&factory_pda.key()), // Factory PDA as transfer fee config authority
        Some(&factory_pda.key()), // Factory PDA as withdraw authority
        transfer_fee_basis_points,
        maximum_fee,
    )?;

    // Initialize the standard mint account data
    // Use factory PDA as mint authority
    initialize_mint2(
        CpiContext::new(
            token_program.to_account_info(),
            InitializeMint2 {
                mint: mint_account.to_account_info(),
            },
        ),
        decimals,
        &factory_pda.key(), // Factory PDA as mint authority
        Some(&factory_pda.key()), // Factory PDA as freeze authority
    )?;

    Ok(())
}

pub fn initialize_ibt_mint_with_transfer_fee<'info>(
    payer: &Signer<'info>,
    mint_account: &Signer<'info>,
    token_program: &Program<'info, Token2022>,
    system_program: &Program<'info, System>,
    factory_authority: &AccountInfo<'info>, // Factory PDA as ALL authorities
    initial_rate: i16,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
    decimals: u8,
) -> Result<()> {
    // Calculate space required for mint with BOTH extensions
    let mint_size = ExtensionType::try_calculate_account_len::<PodMint>(
        &[
            ExtensionType::InterestBearingConfig,
            ExtensionType::TransferFeeConfig,
        ]
    )?;

    // Calculate minimum lamports required for mint account with extensions
    let lamports = (Rent::get()?).minimum_balance(mint_size);

    // Create new account with space for mint and BOTH extensions
    create_account(
        CpiContext::new(
            system_program.to_account_info(),
            CreateAccount {
                from: payer.to_account_info(),
                to: mint_account.to_account_info(),
            },
        ),
        lamports,
        mint_size as u64,
        &token_program.key(),
    )?;

    // Initialize the interest-bearing extension first
    // Factory as rate authority
    interest_bearing_mint_initialize(
        CpiContext::new(
            token_program.to_account_info(),
            InterestBearingMintInitialize {
                token_program_id: token_program.to_account_info(),
                mint: mint_account.to_account_info(),
            },
        ),
        Some(factory_authority.key()), // Factory as rate authority
        initial_rate,
    )?;

    // Initialize the transfer fee extension second
    // Factory as transfer fee authority
    transfer_fee_initialize(
        CpiContext::new(
            token_program.to_account_info(),
            TransferFeeInitialize {
                token_program_id: token_program.to_account_info(),
                mint: mint_account.to_account_info(),
            },
        ),
        Some(&factory_authority.key()), // Factory as config authority
        Some(&factory_authority.key()), // Factory as withdraw authority
        transfer_fee_basis_points,
        maximum_fee,
    )?;

    // Initialize the standard mint account data last (only once!)
    // Factory as mint and freeze authority
    initialize_mint2(
        CpiContext::new(
            token_program.to_account_info(),
            InitializeMint2 {
                mint: mint_account.to_account_info(),
            },
        ),
        decimals,
        &factory_authority.key(), // Factory as mint authority
        Some(&factory_authority.key()), // Factory as freeze authority
    )?;

    Ok(())
}

/// Read transfer fee config from a mint account
pub fn read_transfer_fee_config(mint: &AccountInfo) -> Result<TransferFeeConfig> {
    let mint_data = mint.data.borrow();
    let mint_with_extension = StateWithExtensions::<MintState>::unpack(&mint_data)?;
    let transfer_fee_config = mint_with_extension.get_extension::<TransferFeeConfig>()?;
    
    Ok(*transfer_fee_config)
}

/// Calculate fee for current epoch from mint account
/// This is used when you need to read the config from the mint
pub fn calculate_transfer_fee_from_mint(mint: &AccountInfo, amount: u64) -> Result<u64> {
    let fee_config = read_transfer_fee_config(mint)?;
    let epoch = Clock::get()?.epoch;
    
    fee_config.calculate_epoch_fee(epoch, amount)
        .ok_or(error!(StablecoinError::MathError))
}

/// Transfer tokens with fee calculation
pub fn transfer_with_fee<'info>(
    token_program: &Program<'info, Token2022>,
    source: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &Signer<'info>,
    amount: u64,
    decimals: u8,
) -> Result<u64> {
    // Calculate the expected fee
    let fee = calculate_transfer_fee_from_mint(mint, amount)?;
    
    // Execute transfer with fee
    transfer_checked_with_fee(
        CpiContext::new(
            token_program.to_account_info(),
            TransferCheckedWithFee {
                token_program_id: token_program.to_account_info(),
                source: source.to_account_info(),
                mint: mint.to_account_info(),
                destination: destination.to_account_info(),
                authority: authority.to_account_info(),
            },
        ),
        amount,
        decimals,
        fee,
    )?;
    
    Ok(fee)
}

pub fn transfer_with_fee_signed<'info>(
    token_program: &Program<'info, Token2022>,
    source: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    signer_seeds: &[&[&[u8]]],
    amount: u64,
    decimals: u8,
) -> Result<u64> {
    let fee = calculate_transfer_fee_from_mint(mint, amount)?;
    
    transfer_checked_with_fee(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            TransferCheckedWithFee {
                token_program_id: token_program.to_account_info(),
                source: source.to_account_info(),
                mint: mint.to_account_info(),
                destination: destination.to_account_info(),
                authority: authority.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        decimals,
        fee,
    )?;
    
    Ok(fee)
}

/// Harvest withheld tokens from accounts to mint
pub fn harvest_fees<'info>(
    token_program: &Program<'info, Token2022>,
    mint: &AccountInfo<'info>,
    token_accounts: Vec<AccountInfo<'info>>,
) -> Result<()> {
    harvest_withheld_tokens_to_mint(
        CpiContext::new(
            token_program.to_account_info(),
            HarvestWithheldTokensToMint {
                token_program_id: token_program.to_account_info(),
                mint: mint.to_account_info(),
            },
        ),
        token_accounts,
    )?;
    
    Ok(())
}

/// Withdraw withheld tokens from mint to destination
pub fn withdraw_fees<'info>(
    token_program: &Program<'info, Token2022>,
    mint: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &Signer<'info>,
) -> Result<()> {
    withdraw_withheld_tokens_from_mint(
        CpiContext::new(
            token_program.to_account_info(),
            WithdrawWithheldTokensFromMint {
                token_program_id: token_program.to_account_info(),
                mint: mint.to_account_info(),
                destination: destination.to_account_info(),
                authority: authority.to_account_info(),
            },
        )
    )?;
    Ok(())
}

/// Update transfer fee configuration
pub fn update_transfer_fee_config<'info>(
    authority: &Signer<'info>,
    mint_account: &AccountInfo<'info>,
    token_program: &Program<'info, Token2022>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    anchor_spl::token_interface::transfer_fee_set(
        CpiContext::new(
            token_program.to_account_info(),
            anchor_spl::token_interface::TransferFeeSetTransferFee {
                token_program_id: token_program.to_account_info(),
                mint: mint_account.to_account_info(),
                authority: authority.to_account_info(),
            },
        ),
        transfer_fee_basis_points,
        maximum_fee,
    )?;
    
    Ok(())
}

pub fn update_interest_rate_signed<'info>(
    factory_authority: &AccountInfo<'info>,
    factory_signer: &[&[&[u8]]], // Factory PDA seeds
    mint_account: &AccountInfo<'info>,
    token_program: &Program<'info, Token2022>,
    new_rate: i16,
) -> Result<()> {
    interest_bearing_mint_update_rate(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            InterestBearingMintUpdateRate {
                token_program_id: token_program.to_account_info(),
                mint: mint_account.to_account_info(),
                rate_authority: factory_authority.to_account_info(),
            },
            factory_signer,
        ),
        new_rate,
    )?;

    Ok(())
}

/// Update transfer fee configuration (factory signs)
pub fn update_transfer_fee_config_signed<'info>(
    factory_authority: &AccountInfo<'info>,
    factory_signer: &[&[&[u8]]], // Factory PDA seeds
    mint_account: &AccountInfo<'info>,
    token_program: &Program<'info, Token2022>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    anchor_spl::token_interface::transfer_fee_set(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            anchor_spl::token_interface::TransferFeeSetTransferFee {
                token_program_id: token_program.to_account_info(),
                mint: mint_account.to_account_info(),
                authority: factory_authority.to_account_info(),
            },
            factory_signer,
        ),
        transfer_fee_basis_points,
        maximum_fee,
    )?;

    Ok(())
}

/// Withdraw withheld tokens from mint to destination (factory signs)
pub fn withdraw_fees_signed<'info>(
    factory_authority: &AccountInfo<'info>,
    factory_signer: &[&[&[u8]]], // Factory PDA seeds
    token_program: &Program<'info, Token2022>,
    mint: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
) -> Result<()> {
    withdraw_withheld_tokens_from_mint(
        CpiContext::new_with_signer(
            token_program.to_account_info(),
            WithdrawWithheldTokensFromMint {
                token_program_id: token_program.to_account_info(),
                mint: mint.to_account_info(),
                destination: destination.to_account_info(),
                authority: factory_authority.to_account_info(),
            },
            factory_signer,
        )
    )?;
    Ok(())
}

/// Initialize a mint with interest-bearing extension
pub fn initialize_interest_bearing_mint<'info>(
    payer: &Signer<'info>,
    mint_account: &Signer<'info>,
    token_program: &Program<'info, Token2022>,
    system_program: &Program<'info, System>,
    rate_authority: &Pubkey,
    initial_rate: i16,
    decimals: u8,
) -> Result<()> {
    // Calculate space required for mint and extension data
    let mint_size = ExtensionType::try_calculate_account_len::<PodMint>(
        &[ExtensionType::InterestBearingConfig]
    )?;

    // Calculate minimum lamports required for mint account with extensions
    let lamports = (Rent::get()?).minimum_balance(mint_size);

    // Create new account with space for mint and extension data
    create_account(
        CpiContext::new(
            system_program.to_account_info(),
            CreateAccount {
                from: payer.to_account_info(),
                to: mint_account.to_account_info(),
            },
        ),
        lamports,
        mint_size as u64,
        &token_program.key(),
    )?;

    // Initialize the interest-bearing extension
    interest_bearing_mint_initialize(
        CpiContext::new(
            token_program.to_account_info(),
            InterestBearingMintInitialize {
                token_program_id: token_program.to_account_info(),
                mint: mint_account.to_account_info(),
            },
        ),
        Some(*rate_authority),
        initial_rate,
    )?;

    // Initialize the standard mint account data
    initialize_mint2(
        CpiContext::new(
            token_program.to_account_info(),
            InitializeMint2 {
                mint: mint_account.to_account_info(),
            },
        ),
        decimals,
        rate_authority,
        Some(rate_authority),
    )?;

    Ok(())
}

/// Update interest rate for a mint
pub fn update_interest_rate<'info>(
    authority: &Signer<'info>,
    mint_account: &AccountInfo<'info>,
    token_program: &Program<'info, Token2022>,
    new_rate: i16,
) -> Result<()> {
    interest_bearing_mint_update_rate(
        CpiContext::new(
            token_program.to_account_info(),
            InterestBearingMintUpdateRate {
                token_program_id: token_program.to_account_info(),
                mint: mint_account.to_account_info(),
                rate_authority: authority.to_account_info(),
            },
        ),
        new_rate,
    )?;
    
    Ok(())
}

/// Read interest-bearing config from a mint account
pub fn read_interest_bearing_config(mint: &AccountInfo) -> Result<InterestBearingConfig> {
    let mint_data = mint.data.borrow();
    let mint_with_extension = StateWithExtensions::<MintState>::unpack(&mint_data)?;
    let interest_config = mint_with_extension.get_extension::<InterestBearingConfig>()?;
    
    Ok(*interest_config)
}

/// Check interest-bearing mint data
pub fn check_interest_bearing_mint_data(mint_account_info: &AccountInfo, authority_key: &Pubkey) -> Result<()> {
    let mint_data = mint_account_info.data.borrow();
    let mint_with_extension = StateWithExtensions::<MintState>::unpack(&mint_data)?;
    let extension_data = mint_with_extension.get_extension::<InterestBearingConfig>()?;

    require!(
        extension_data.rate_authority == OptionalNonZeroPubkey::try_from(Some(*authority_key))?,
        StablecoinError::InvalidRateAuthority
    );

    Ok(())
}

/// Read the current interest rate from a mint
pub fn read_current_interest_rate(mint_account_info: &AccountInfo) -> Result<i16> {
    let config = read_interest_bearing_config(mint_account_info)?;
    Ok(config.current_rate.into())
}