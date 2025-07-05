use super::*;

/// Fixed-point math with precise decimal operations
#[derive(Clone, Debug)]
pub struct FixedPoint {
    value: u128,
    scale: u8,
}

impl FixedPoint {
    /// Create a new fixed-point value
    pub fn new(value: u128) -> Option<Self> {
        Some(Self { value, scale: 0 })
    }
    
    /// Create with specific scale (10^scale)
    pub fn new_with_scale(value: u128, scale: u8) -> Option<Self> {
        Some(Self { value, scale })
    }
    
    /// Create fixed-point from integer and decimal parts
    pub fn from_decimal(integer: u64, decimal: u64, decimal_places: u8) -> Result<Self> {
        // Create from integer part
        let integer_part = Self {
            value: integer as u128,
            scale: 0,
        };
        
        // Calculate decimal scaling factor
        let scaling_factor = 10u128.checked_pow(decimal_places as u32)
            .ok_or(StablecoinError::MathError)?;
            
        // Create decimal part
        let decimal_part = Self {
            value: decimal as u128,
            scale: decimal_places,
        };
        
        // Add parts together
        integer_part.checked_add(&decimal_part)
    }
    
    /// Convert to u64, with error if too large
    pub fn to_u64(&self) -> Result<u64> {
        if self.scale == 0 {
            // No decimal part, just check if it fits in u64
            if self.value > u64::MAX as u128 {
                return Err(StablecoinError::MathError.into());
            }
            return Ok(self.value as u64);
        }
        
        // With decimal part, need to divide by 10^scale
        let divisor = 10u128.checked_pow(self.scale as u32)
            .ok_or(StablecoinError::MathError)?;
            
        let result = self.value
            .checked_div(divisor)
            .ok_or(StablecoinError::MathError)?;
            
        if result > u64::MAX as u128 {
            return Err(StablecoinError::MathError.into());
        }
        
        Ok(result as u64)
    }
    
    /// Add two fixed-point values
    pub fn checked_add(&self, other: &Self) -> Result<Self> {
        // Adjust scales to match
        let (left, right) = self.align_scales(other)?;
        
        let result_value = left.value.safe_add(right.value)?;
        
        Ok(Self {
            value: result_value,
            scale: left.scale,
        })
    }
    
    /// Subtract one fixed-point value from another
    pub fn checked_sub(&self, other: &Self) -> Result<Self> {
        // Adjust scales to match
        let (left, right) = self.align_scales(other)?;
        
        let result_value = left.value.safe_sub(right.value)?;
        
        Ok(Self {
            value: result_value,
            scale: left.scale,
        })
    }
    
    /// Multiply two fixed-point values
    pub fn checked_mul(&self, other: &Self) -> Result<Self> {
        let result_value = self.value.safe_mul(other.value)?;
        let result_scale = self.scale.checked_add(other.scale)
            .ok_or(StablecoinError::MathError)?;
            
        Ok(Self {
            value: result_value,
            scale: result_scale,
        })
    }
    
    /// Divide one fixed-point value by another
    pub fn checked_div(&self, other: &Self) -> Result<Self> {
        require!(other.value != 0, StablecoinError::DivisionByZero);
        
        // For better precision, multiply numerator by 10^6 before division
        const EXTRA_PRECISION: u32 = 6;
        
        let scaled_value = self.value
            .checked_mul(10u128.pow(EXTRA_PRECISION))
            .ok_or(StablecoinError::MathError)?;
            
        let result_value = scaled_value
            .checked_div(other.value)
            .ok_or(StablecoinError::MathError)?;
            
        // Calculate new scale: self.scale - other.scale + EXTRA_PRECISION
        let result_scale = self.scale
            .checked_add(EXTRA_PRECISION as u8)
            .ok_or(StablecoinError::MathError)?
            .checked_sub(other.scale)
            .unwrap_or_else(|| EXTRA_PRECISION as u8); // If other.scale > self.scale + EXTRA_PRECISION
            
        Ok(Self {
            value: result_value,
            scale: result_scale,
        })
    }
    
    /// Align two fixed-point values to have the same scale
    fn align_scales(&self, other: &Self) -> Result<(Self, Self)> {
        if self.scale == other.scale {
            return Ok((self.clone(), other.clone()));
        }
        
        if self.scale < other.scale {
            // Scale up self to match other
            let scale_diff = other.scale - self.scale;
            let scaling_factor = 10u128.checked_pow(scale_diff as u32)
                .ok_or(StablecoinError::MathError)?;
                
            let scaled_value = self.value
                .checked_mul(scaling_factor)
                .ok_or(StablecoinError::MathError)?;
                
            Ok((
                Self { value: scaled_value, scale: other.scale },
                other.clone()
            ))
        } else {
            // Scale up other to match self
            let scale_diff = self.scale - other.scale;
            let scaling_factor = 10u128.checked_pow(scale_diff as u32)
                .ok_or(StablecoinError::MathError)?;
                
            let scaled_value = other.value
                .checked_mul(scaling_factor)
                .ok_or(StablecoinError::MathError)?;
                
            Ok((
                self.clone(),
                Self { value: scaled_value, scale: self.scale }
            ))
        }
    }
}