use super::*;

/// Calculate the amount of sovereign coins to mint based on USDC input and exchange rate
pub fn calculate_sovereign_coin_amount(
    usdc_amount: u64,
    base_price_feed: &AccountInfo,  // USD/Target feed (e.g., USD/MXN)
    quote_price_feed: Option<&AccountInfo>,  // Optional secondary feed
    target_currency: &str,
    decimals: u8,  // Decimals of both USDC and sovereign coin
) -> Result<u64> {
    // Get the oracle price
    let (price_mantissa, price_scale) = if target_currency == "USD" {
        // For USD sovereign coins, use 1:1 with USDC
        return Ok(usdc_amount);
    } else if let Some(quote_feed) = quote_price_feed {
        // For other currencies, calculate cross price
        calculate_cross_price(base_price_feed, quote_feed)?
    } else {
        // For single feed (USD to target currency)
        get_oracle_price(base_price_feed)?
    };

    // Convert to FixedPoint using proper scaling
    // Since USDC has 6 decimals, we need to scale the amount
    let usdc_scaled = (usdc_amount as u128)
        .checked_mul(10u128.pow(decimals as u32))
        .ok_or(StablecoinError::MathError)?;
    
    // Calculate: amount * price
    let sovereign_scaled = usdc_scaled
        .checked_mul(price_mantissa.unsigned_abs() as u128)
        .ok_or(StablecoinError::MathError)?;
    
    // Adjust for oracle scale and coin decimals
    // Result = (usdc * 10^decimals * price_mantissa) / (10^price_scale * 10^decimals)
    let total_scale = price_scale.checked_add(decimals)
        .ok_or(StablecoinError::MathError)?;
    
    let divisor = 10u128.checked_pow(total_scale as u32)
        .ok_or(StablecoinError::MathError)?;
    
    let sovereign_amount = sovereign_scaled
        .checked_div(divisor)
        .ok_or(StablecoinError::MathError)?;
    
    if sovereign_amount > u64::MAX as u128 {
        return Err(StablecoinError::MathError.into());
    }
    
    Ok(sovereign_amount as u64)
}

/// Calculate the USDC amount needed to mint a specific amount of sovereign coins
/// This is the inverse operation of calculate_sovereign_coin_amount
pub fn calculate_usdc_for_sovereign_amount(
    sovereign_amount: u64,
    base_price_feed: &AccountInfo,
    quote_price_feed: Option<&AccountInfo>,
    target_currency: &str,
    decimals: u8,
) -> Result<u64> {
    // Get the oracle price
    let (price_mantissa, price_scale) = if target_currency == "USD" {
        // For USD sovereign coins, use 1:1 with USDC
        return Ok(sovereign_amount);
    } else if let Some(quote_feed) = quote_price_feed {
        // For other currencies, calculate cross price
        calculate_cross_price(base_price_feed, quote_feed)?
    } else {
        // For single feed (USD to target currency)
        get_oracle_price(base_price_feed)?
    };

    // Scale sovereign amount to match decimals
    let sovereign_scaled = (sovereign_amount as u128)
        .checked_mul(10u128.pow(price_scale as u32))
        .ok_or(StablecoinError::MathError)?;
    
    // Calculate: sovereign_amount / price
    let usdc_scaled = sovereign_scaled
        .checked_div(price_mantissa.unsigned_abs() as u128)
        .ok_or(StablecoinError::MathError)?;
    
    // Adjust for decimals
    let usdc_amount = usdc_scaled
        .checked_div(10u128.pow(decimals as u32))
        .ok_or(StablecoinError::MathError)?;
    
    if usdc_amount > u64::MAX as u128 {
        return Err(StablecoinError::MathError.into());
    }
    
    Ok(usdc_amount as u64)
}