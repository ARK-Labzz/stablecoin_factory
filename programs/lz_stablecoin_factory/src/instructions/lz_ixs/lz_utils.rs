use super::*;


/// Calculate post-fee amount for Token2022 transfers
pub fn get_post_fee_amount_ld(token_mint: &InterfaceAccount<Mint>, amount_ld: u64) -> Result<u64> {
    let token_mint_info = token_mint.to_account_info();
    let token_mint_data = token_mint_info.try_borrow_data()?;
    let token_mint_ext = StateWithExtensions::<MintState>::unpack(&token_mint_data)?;
    
    let post_amount_ld = if let Ok(transfer_fee_config) = token_mint_ext.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .get_epoch_fee(Clock::get()?.epoch)
            .calculate_post_fee_amount(amount_ld)
            .ok_or(ProgramError::InvalidArgument)?
    } else {
        amount_ld
    };
    
    Ok(post_amount_ld)
}

/// Calculate pre-fee amount necessary to receive a specific post-fee amount
pub fn get_pre_fee_amount_ld(token_mint: &InterfaceAccount<Mint>, amount_ld: u64) -> Result<u64> {
    let token_mint_info = token_mint.to_account_info();
    let token_mint_data = token_mint_info.try_borrow_data()?;
    let token_mint_ext = StateWithExtensions::<MintState>::unpack(&token_mint_data)?;
    
    let pre_amount_ld = if let Ok(transfer_fee) = token_mint_ext.get_extension::<TransferFeeConfig>() {
        calculate_pre_fee_amount(transfer_fee.get_epoch_fee(Clock::get()?.epoch), amount_ld)
            .ok_or(ProgramError::InvalidArgument)?
    } else {
        amount_ld
    };
    
    Ok(pre_amount_ld)
}


fn calculate_pre_fee_amount(fee: &TransferFee, post_fee_amount: u64) -> Option<u64> {
    let maximum_fee = u64::from(fee.maximum_fee);
    let transfer_fee_basis_points = u16::from(fee.transfer_fee_basis_points) as u128;
    
    match (transfer_fee_basis_points, post_fee_amount) {
        // no fee, same amount
        (0, _) => Some(post_fee_amount),
        // 0 zero out, 0 in
        (_, 0) => Some(0),
        // 100%, cap at max fee
        (ONE_IN_BASIS_POINTS, _) => maximum_fee.checked_add(post_fee_amount),
        _ => {
            let numerator = (post_fee_amount as u128).checked_mul(ONE_IN_BASIS_POINTS)?;
            let denominator = ONE_IN_BASIS_POINTS.checked_sub(transfer_fee_basis_points)?;
            let raw_pre_fee_amount = ceil_div(numerator, denominator)?;

            if raw_pre_fee_amount.checked_sub(post_fee_amount as u128)? >= maximum_fee as u128 {
                post_fee_amount.checked_add(maximum_fee)
            } else {
                u64::try_from(raw_pre_fee_amount).ok()
            }
        },
    }
}

fn ceil_div(numerator: u128, denominator: u128) -> Option<u128> {
    numerator.checked_add(denominator)?.checked_sub(1)?.checked_div(denominator)
}

/// Validate LayerZero endpoint ID
pub fn validate_endpoint_id(eid: u32) -> Result<()> {
    require!(eid > 0, StablecoinError::LzInvalidPeer);
    // Add specific validation for known endpoint IDs if needed
    Ok(())
}

/// Calculate daily transfer limits based on sovereign coin configuration
pub fn calculate_sovereign_coin_limits(
    sovereign_coin: &SovereignCoin,
    factory: &Factory,
) -> Result<(u64, u64)> {
    // Calculate limits based on total supply and reserve ratios
    let max_single_transfer = sovereign_coin.total_supply
        .checked_div(100) // Max 1% of total supply per transfer
        .unwrap_or(u64::MAX);
    
    let daily_limit = sovereign_coin.total_supply
        .checked_div(10) // Max 10% of total supply per day
        .unwrap_or(u64::MAX);
    
    Ok((max_single_transfer, daily_limit))
}

/// Validate cross-chain transfer amount against limits
pub fn validate_transfer_limits(
    amount: u64,
    sovereign_coin: &SovereignCoin,
    factory: &Factory,
    peer: &PeerConfig,
) -> Result<()> {
    let (max_single, _) = calculate_sovereign_coin_limits(sovereign_coin, factory)?;
    
    require!(
        amount <= max_single,
        StablecoinError::LzSlippageExceeded
    );
    
    // Check rate limiter if configured
    if let Some(rate_limiter) = &peer.outbound_rate_limiter {
        require!(
            amount <= rate_limiter.tokens,
            StablecoinError::LzRateLimitExceeded
        );
    }
    
    Ok(())
}

/// Format cross-chain address for different networks
pub fn format_cross_chain_address(address: &[u8; 32], chain_id: u32) -> String {
    match chain_id {
        // Ethereum-like chains (20-byte addresses)
        1 | 137 | 56 => {
            let eth_addr = &address[12..32]; // Take last 20 bytes
            format!("0x{}", hex::encode(eth_addr))
        },
        // Solana (32-byte addresses)
        102 => {
            bs58::encode(address).into_string()
        },
        // Default: hex encoding
        _ => format!("0x{}", hex::encode(address)),
    }
}

/// Parse cross-chain address from string
pub fn parse_cross_chain_address(address_str: &str, chain_id: u32) -> Result<[u8; 32]> {
    let mut result = [0u8; 32];
    
    match chain_id {
        // Ethereum-like chains
        1 | 137 | 56 => {
            let clean_addr = address_str.strip_prefix("0x").unwrap_or(address_str);
            let decoded = hex::decode(clean_addr)
                .map_err(|_| StablecoinError::LzInvalidPeer)?;
            
            require!(decoded.len() == 20, StablecoinError::LzInvalidPeer);
            result[12..32].copy_from_slice(&decoded);
        },
        // Solana
        102 => {
            let decoded = bs58::decode(address_str)
                .into_vec()
                .map_err(|_| StablecoinError::LzInvalidPeer)?;
            
            require!(decoded.len() == 32, StablecoinError::LzInvalidPeer);
            result.copy_from_slice(&decoded);
        },
        // Default: hex
        _ => {
            let clean_addr = address_str.strip_prefix("0x").unwrap_or(address_str);
            let decoded = hex::decode(clean_addr)
                .map_err(|_| StablecoinError::LzInvalidPeer)?;
            
            require!(decoded.len() <= 32, StablecoinError::LzInvalidPeer);
            let start = 32 - decoded.len();
            result[start..].copy_from_slice(&decoded);
        },
    }
    
    Ok(result)
}

/// Calculate comprehensive fee breakdown for UI display
pub fn calculate_fee_breakdown(
    amount_ld: u64,
    factory: &Factory,
    oft_store: &OFTStore,
    token_mint: &InterfaceAccount<Mint>,
    peer_fee_bps: Option<u16>,
) -> Result<Vec<(String, u64)>> {
    let mut breakdown = Vec::new();
    
    // Protocol fee
    let (_, protocol_fee) = fee::calculate_protocol_fee(amount_ld, factory.transfer_fee_bps)?;
    if protocol_fee > 0 {
        breakdown.push(("Protocol Fee".to_string(), protocol_fee));
    }
    
    // Transfer fee (Token2022)
    if oft_store.oft_type == OFTType::Adapter {
        let post_fee = get_post_fee_amount_ld(token_mint, amount_ld)?;
        let transfer_fee = amount_ld.saturating_sub(post_fee);
        if transfer_fee > 0 {
            breakdown.push(("Transfer Fee".to_string(), transfer_fee));
        }
    }
    
    // LayerZero fee
    let final_fee_bps = peer_fee_bps.unwrap_or(oft_store.default_fee_bps);
    if final_fee_bps > 0 {
        let lz_fee = (amount_ld as u128 * final_fee_bps as u128 / MAX_FEE_BASIS_POINTS as u128) as u64;
        breakdown.push(("Cross-Chain Fee".to_string(), lz_fee));
    }
    
    Ok(breakdown)
}