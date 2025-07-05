use super::*;

/// Calculate Etherfuse fee based on bond yield using integer math
pub fn calculate_etherfuse_fee(yield_basis_points: u16) -> Result<u16> {
    // Fee calculation is based on yield percentage
    // Fixed point calculation with 4 decimal places of precision
    // This means 1.0000 = 10000
    
    // Convert all percentages to our fixed point representation
    // 4.5% = 450 basis points = 45000 in our fixed point
    const YIELD_THRESHOLD_1: u32 = 450 * 100; // 4.5% threshold
    const YIELD_THRESHOLD_2: u32 = 1000 * 100; // 10% threshold
    
    // Convert fee percentages to fixed point
    const FEE_FIXED_1: u32 = 25 * 100;      // 0.25% = 2500 in fixed point
    const FEE_FIXED_2: u32 = 150 * 100;     // 1.5% = 15000 in fixed point
    
    // Formula coefficients in fixed point
    // 0.2273 = 2273 in fixed point
    const SLOPE: u32 = 2273;
    // 0.7727 = 7727 in fixed point
    const INTERCEPT: u32 = 7727;
    
    // Convert basis points to our fixed point representation
    let yield_fixed = (yield_basis_points as u32) * 100;
    
    // Calculate fee using Etherfuse's formula with integer math
    let fee_fixed = if yield_fixed < YIELD_THRESHOLD_1 {
        FEE_FIXED_1
    } else if yield_fixed >= YIELD_THRESHOLD_2 {
        FEE_FIXED_2
    } else {
        // Formula: fee = 0.2273 * yield - 0.7727
        // In fixed point: fee_fixed = (SLOPE * yield_fixed / 10000) - INTERCEPT
        // To avoid precision loss, we do multiplication first
        let fee_pre = yield_fixed.checked_mul(SLOPE).ok_or(error!(StablecoinError::MathOverflow))?;
        let fee_scaled = fee_pre.checked_div(10000).ok_or(error!(StablecoinError::MathOverflow))?;
        fee_scaled.checked_sub(INTERCEPT).ok_or(error!(StablecoinError::MathOverflow))?
    };
    
    // Convert from fixed point back to basis points
    // Division by 100 to go from fixed point to basis points
    let fee_bps = fee_fixed.checked_div(100).ok_or(error!(StablecoinError::MathOverflow))? as u16;
    
    Ok(fee_bps)
}

/// Calculate interest rate for sovereign coin from bond interest rate
pub fn calculate_sovereign_interest_rate(
    bond_rate: i16,      // Bond interest rate in basis points
    bond_amount: u64,    // Amount of bonds held
) -> Result<i16> {
    // Constant tax withholding in basis points (0.5% = 50 basis points)
    const TAX_WITHHOLDING_BPS: u16 = 50;
    
    // Bond rate must be positive
    if bond_rate <= 0 {
        return Ok(0);
    }
    
    // Convert to u16 for calculation
    let bond_rate_bps = bond_rate as u16;
    
    // Calculate Etherfuse fee in basis points
    let fee_bps = calculate_etherfuse_fee(bond_rate_bps)?;
    
    // Calculate net rate: bond_rate - fee - tax
    // Using safe math to prevent underflow
    let net_rate_bps = bond_rate_bps
        .checked_sub(fee_bps)
        .ok_or(error!(StablecoinError::MathOverflow))?
        .checked_sub(TAX_WITHHOLDING_BPS)
        .ok_or(error!(StablecoinError::MathOverflow))?;
    
    // Convert back to i16
    Ok(net_rate_bps as i16)
}

/// Calculate overall interest rate from multiple bonds
/// This is optimized to work without vectors
pub fn calculate_weighted_interest_rate(
    bond_rates: [i16; MAX_BOND_MAPPINGS],  // Interest rates in basis points
    bond_amounts: [u64; MAX_BOND_MAPPINGS], // Bond amounts
    bond_count: usize,                     // Number of active bonds
) -> Result<i16> {
    let mut weighted_sum: u128 = 0;
    let mut total_amount: u128 = 0;
    
    // Calculate weighted sum
    for i in 0..bond_count {
        // Skip negative rates (invalid) and zero amounts
        if bond_rates[i] <= 0 || bond_amounts[i] == 0 {
            continue;
        }
        
        // Calculate net rate after fees and tax
        let net_rate = calculate_sovereign_interest_rate(bond_rates[i], bond_amounts[i])?;
        
        // Skip if net rate is zero or negative
        if net_rate <= 0 {
            continue;
        }
        
        // Convert to u128 for safe multiplication
        let rate_u128 = net_rate as u128;
        let amount_u128 = bond_amounts[i] as u128;
        
        // Add to weighted sum: rate * amount
        let product = rate_u128
            .checked_mul(amount_u128)
            .ok_or(error!(StablecoinError::MathOverflow))?;
        
        weighted_sum = weighted_sum
            .checked_add(product)
            .ok_or(error!(StablecoinError::MathOverflow))?;
        
        // Add to total amount
        total_amount = total_amount
            .checked_add(amount_u128)
            .ok_or(error!(StablecoinError::MathOverflow))?;
    }
    
    // Calculate weighted average
    if total_amount == 0 {
        return Ok(0);
    }
    
    // Safe division for weighted average
    let result = weighted_sum
        .checked_div(total_amount)
        .ok_or(error!(StablecoinError::MathOverflow))?;
    
    // Ensure the result fits in i16
    if result > i16::MAX as u128 {
        return Ok(i16::MAX);
    }
    
    Ok(result as i16)
}