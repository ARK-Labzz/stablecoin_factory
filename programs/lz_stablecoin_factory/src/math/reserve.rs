use super::*;

/// Calculate required reserve percentage based on bond rating
pub fn calculate_required_reserve(
    base_bps: u16,
    bond_rating: u8,
    bond_reserve_numerator: u8,
    bond_reserve_denominator: u8,
) -> Result<u16> {
    require!(base_bps <= BASIS_POINT_MAX, StablecoinError::InvalidReservePercentage);
    require!(bond_rating >= 1 && bond_rating <= 10, StablecoinError::InvalidBondRating);
    require!(bond_reserve_denominator > 0, StablecoinError::InvalidBondReserveRatio);
    
    // Convert to fixed point for precise math
    let base = FixedPoint::new(base_bps as u128)
        .ok_or(StablecoinError::MathError)?;
    
    // Calculate ordinal factor (rating - 1)
    let ordinal_factor = FixedPoint::new((bond_rating - 1) as u128)
        .ok_or(StablecoinError::MathError)?;
    
    // Create numerator and denominator
    let numerator = FixedPoint::new(bond_reserve_numerator as u128)
        .ok_or(StablecoinError::MathError)?;
    let denominator = FixedPoint::new(bond_reserve_denominator as u128)
        .ok_or(StablecoinError::MathError)?;
    
    // Calculate adjustment: (rating - 1) * (numerator / denominator)
    let ratio = numerator.checked_div(&denominator)?;
    let adjustment = ordinal_factor.checked_mul(&ratio)?;
    
    // Calculate total percentage: base + adjustment
    let total = base.checked_add(&adjustment)?;
    
    // Convert back to basis points
    let result_bps = total.to_u64()?;
    require!(result_bps <= BASIS_POINT_MAX as u64, StablecoinError::ReserveExceeds100Percent);
    
    Ok(result_bps as u16)
}

/// Calculate reserve and bond amounts for mint
pub fn calculate_reserve_and_bond_amounts(
    net_amount: u64,
    required_reserve_percentage: u16,
) -> Result<(u64, u64)> {
    require!(required_reserve_percentage <= BASIS_POINT_MAX, StablecoinError::InvalidReservePercentage);
    
    // Calculate reserve amount: net_amount * required_reserve_percentage / BASIS_POINT_MAX
    let reserve_amount = (net_amount as u128)
        .safe_mul(required_reserve_percentage as u128)?
        .safe_div(BASIS_POINT_MAX as u128)? as u64;
    
    // Calculate bond amount: net_amount - reserve_amount
    let bond_amount = net_amount.safe_sub(reserve_amount)?;
    
    require!(reserve_amount > 0 && bond_amount > 0, StablecoinError::InvalidCalculatedAmount);
    
    Ok((reserve_amount, bond_amount))
}

