use super::*;

/// Calculate protocol fee for minting or redeeming
pub fn calculate_protocol_fee(
    amount: u64,
    fee_bps: u16,
) -> Result<(u64, u64)> {
    require!(fee_bps <= BASIS_POINT_MAX, StablecoinError::InvalidFeeBasisPoints);
    
    if fee_bps == 0 {
        return Ok((amount, 0));
    }
    
    let fee_amount = (amount as u128)
        .safe_mul(fee_bps as u128)?
        .safe_div(BASIS_POINT_MAX as u128)?;
        
    if fee_amount > u64::MAX as u128 {
        return Err(StablecoinError::MathError.into());
    }
    
    let fee = fee_amount as u64;
    let net_amount = amount.safe_sub(fee)?;
    
    Ok((net_amount, fee))
}

/// Calculate transfer fee based on transfer fee config
/// This is used when you already have the TransferFeeConfig
pub fn calculate_transfer_fee_from_config(
    amount: u64,
    fee_config: &TransferFeeConfig,
) -> Result<u64> {
    let epoch = Clock::get()?.epoch;
    
    // Use the TokenExtension's built-in fee calculation function
    let fee = fee_config.calculate_epoch_fee(epoch, amount)
        .ok_or(StablecoinError::MathError)?;
        
    Ok(fee)
}

/// Calculate maximum amount to transfer, including fee
pub fn calculate_gross_amount_with_fee(
    net_amount: u64,
    fee_bps: u16,
) -> Result<u64> {
    require!(fee_bps < BASIS_POINT_MAX, StablecoinError::InvalidFeeBasisPoints);
    
    if fee_bps == 0 {
        return Ok(net_amount);
    }
    
    // Formula: gross = net * BASIS_POINT_MAX / (BASIS_POINT_MAX - fee_bps)
    let numerator = (net_amount as u128).safe_mul(BASIS_POINT_MAX as u128)?;
    let denominator = (BASIS_POINT_MAX as u128).safe_sub(fee_bps as u128)?;
    let gross_amount = numerator.safe_div(denominator)?;
    
    if gross_amount > u64::MAX as u128 {
        return Err(StablecoinError::MathError.into());
    }
    
    Ok(gross_amount as u64)
}