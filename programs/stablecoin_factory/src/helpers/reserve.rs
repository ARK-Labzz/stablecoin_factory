use super::*;

pub fn calculate_required_reserve(
    base_bps: u16,      
    ordinal: u8,        
    bond_reserve_numerator: u8,    
    bond_reserve_denominator: u8,  
) -> Result<u16> {      
    
    require!(base_bps <= 10_000, StablecoinError::InvalidReservePercentage);
    require!(ordinal >= 1 && ordinal <= 10, StablecoinError::InvalidBondRating);
    require!(bond_reserve_denominator > 0, StablecoinError::InvalidBondReserveRatio);
    
    
    let base = PreciseNumber::new(base_bps as u128)
        .ok_or(StablecoinError::MathError)?;
    
    let ordinal_factor = PreciseNumber::new(ordinal as u128)
        .ok_or(StablecoinError::MathError)?
        .checked_sub(&PreciseNumber::new(1).ok_or(StablecoinError::MathError)?)
        .ok_or(StablecoinError::MathError)?;
        
    let numerator = PreciseNumber::new(bond_reserve_numerator as u128)
        .ok_or(StablecoinError::MathError)?;
    let denominator = PreciseNumber::new(bond_reserve_denominator as u128)
        .ok_or(StablecoinError::MathError)?;
    
    
    let adjustment = ordinal_factor
        .checked_mul(&numerator)
        .ok_or(StablecoinError::MathError)?
        .checked_div(&denominator)
        .ok_or(StablecoinError::MathError)?;
        
   
    let total = base
        .checked_add(&adjustment)
        .ok_or(StablecoinError::MathError)?;
    
    
    let bps = total
        .to_imprecise()
        .ok_or(StablecoinError::MathError)?
        .try_into()
        .map_err(|_| StablecoinError::MathError)?;
        
    require!(bps <= 10_000, StablecoinError::ReserveExceeds100Percent);
    
    Ok(bps)
}