use super::*;

/// Get price data from a Switchboard oracle
pub fn get_oracle_price(oracle_feed: &AccountInfo) -> Result<(i128, u32)> {
    let feed_data = AggregatorAccountData::new(oracle_feed)?;
    let price = feed_data.get_result()?;
    
    // Return mantissa and scale
    Ok((price.mantissa, price.scale))
}

pub fn calculate_cross_price(
    base_feed: &AccountInfo,   // USDC/USD
    quote_feed: &AccountInfo,  // USD/EUR (or other currency)
) -> Result<(i128, u32)> {
    let (base_mantissa, base_scale) = get_oracle_price(base_feed)?;
    let (quote_mantissa, quote_scale) = get_oracle_price(quote_feed)?;
    
    // For your feeds, we need to compute: base_price * quote_price
    // Calculate combined scale
    let result_scale = base_scale.checked_add(quote_scale)
        .ok_or(StablecoinError::MathError)?;
        
    // Calculate combined mantissa (base * quote)
    let result_mantissa = base_mantissa.checked_mul(quote_mantissa)
        .ok_or(StablecoinError::MathError)?;
    
    Ok((result_mantissa, result_scale))
}

/// Calculate bond equivalent amount from USDC using oracle price
pub fn calculate_bond_equivalent(
    usdc_amount: u64,
    base_feed: &AccountInfo,
    quote_feed: Option<&AccountInfo>,
    fiat_currency: &str,
    bond_decimals: u8,
) -> Result<u64> {
    let (price_mantissa, price_scale) = if fiat_currency == "USD" {
        get_oracle_price(base_feed)?
    } else if let Some(quote_feed) = quote_feed {
        calculate_cross_price(base_feed, quote_feed)?
    } else {
        return Err(StablecoinError::InvalidPriceFeed.into());
    };
    
    // Convert price to a scale factor (price_mantissa * 10^-price_scale)
    // For bonds, we need to scale USDC to the target currency
    // Scale up USDC amount to maintain precision
    let scaled_usdc = (usdc_amount as u128)
        .safe_mul(10u128.pow(price_scale + 6))?;
        
    let bond_amount = scaled_usdc
        .safe_div(price_mantissa.unsigned_abs() as u128)?;
        
    let scale_diff = (price_scale as i32) + 6 - (bond_decimals as i32);
    let result = if scale_diff > 0 {
        bond_amount.safe_div(10u128.pow(scale_diff as u32))?
    } else if scale_diff < 0 {
        bond_amount.safe_mul(10u128.pow((-scale_diff) as u32))?
    } else {
        bond_amount
    };
    
    if result > u64::MAX as u128 {
        return Err(StablecoinError::MathError.into());
    }
    
    Ok(result as u64)
}

