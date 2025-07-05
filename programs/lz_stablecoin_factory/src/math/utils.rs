use super::*;

/// Rounding direction
#[derive(PartialEq, Clone, Copy)]
pub enum Rounding {
    /// Rounding up
    Up,
    /// Rounding down
    Down,
}

/// Calculate percentage of a value
pub fn calculate_percentage(value: u64, percentage_bps: u16) -> Result<u64> {
    let result = (value as u128)
        .safe_mul(percentage_bps as u128)?
        .safe_div(10_000)?;
        
    if result > u64::MAX as u128 {
        return Err(StablecoinError::MathError.into());
    }
    
    Ok(result as u64)
}

/// Multiply and then divide, with optional rounding
pub fn mul_div(x: u64, y: u64, denominator: u64, rounding: Rounding) -> Result<u64> {
    require!(denominator > 0, StablecoinError::DivisionByZero);
    
    let prod = (x as u128).safe_mul(y as u128)?;
    
    let result = match rounding {
        Rounding::Up => {
            let adjusted = prod.safe_add(denominator as u128 - 1)?;
            adjusted.safe_div(denominator as u128)?
        },
        Rounding::Down => {
            prod.safe_div(denominator as u128)?
        }
    };
    
    if result > u64::MAX as u128 {
        return Err(StablecoinError::MathError.into());
    }
    
    Ok(result as u64)
}

/// Calculate minimum of two values
pub fn min(a: u64, b: u64) -> u64 {
    if a < b { a } else { b }
}

/// Calculate maximum of two values
pub fn max(a: u64, b: u64) -> u64 {
    if a > b { a } else { b }
}