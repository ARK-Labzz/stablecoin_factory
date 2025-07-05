use super::*;

/// Convert Switchboard Decimal to our manual mantissa/scale format
fn decimal_to_mantissa_scale(decimal_str: &str) -> Result<(i128, u32)> {
    // Switchboard Decimal comes as string like "1234.567890"
    let parts: Vec<&str> = decimal_str.split('.').collect();
    
    let integer_part = parts[0].parse::<i128>()
        .map_err(|_| StablecoinError::MathError)?;
    
    let (fractional_digits, scale) = if parts.len() > 1 {
        let fractional_str = parts[1];
        let scale = fractional_str.len() as u32;
        let fractional_part = fractional_str.parse::<u128>()
            .map_err(|_| StablecoinError::MathError)?;
        (fractional_part, scale)
    } else {
        (0, 0)
    };
    
    // Combine: mantissa = integer_part * 10^scale + fractional_part
    let scale_multiplier = 10_i128.checked_pow(scale)
        .ok_or(StablecoinError::MathError)?;
    
    let mantissa = integer_part
        .checked_mul(scale_multiplier)
        .ok_or(StablecoinError::MathError)?
        .checked_add(fractional_digits as i128)
        .ok_or(StablecoinError::MathError)?;
    
    Ok((mantissa, scale))
}

/// Get price data from Switchboard On-Demand oracle in our mantissa/scale format
pub fn get_oracle_price_on_demand(
    feed_account: &AccountInfo,
) -> Result<(i128, u32)> {
    
    let data = feed_account.try_borrow_data()?;
    let feed = PullFeedAccountData::parse(data)
        .map_err(|_| StablecoinError::InvalidPriceFeed)?;
    
    // Use anchor's Clock but convert it to the right format
    let anchor_clock = anchor_lang::prelude::Clock::get()
        .map_err(|_| StablecoinError::InvalidPriceFeed)?;
    
    // Create a solana_program Clock from anchor Clock
    let solana_clock = anchor_lang::solana_program::sysvar::clock::Clock {
        slot: anchor_clock.slot,
        epoch_start_timestamp: anchor_clock.epoch_start_timestamp,
        epoch: anchor_clock.epoch,
        leader_schedule_epoch: anchor_clock.leader_schedule_epoch,
        unix_timestamp: anchor_clock.unix_timestamp,
    };
    
    let decimal_value = feed.value(&solana_clock)
        .map_err(|_| StablecoinError::InvalidPriceFeed)?;
    
    let decimal_str = decimal_value.to_string();
    decimal_to_mantissa_scale(&decimal_str)
}


/// Calculate cross price using On-Demand feeds (mantissa/scale)
pub fn calculate_cross_price_on_demand(
    base_feed: &AccountInfo,   // USDC/USD
    quote_feed: &AccountInfo,  // USD/EUR (or other currency)
) -> Result<(i128, u32)> {
    let (base_mantissa, base_scale) = get_oracle_price_on_demand(base_feed)?;
    let (quote_mantissa, quote_scale) = get_oracle_price_on_demand(quote_feed)?;
    
    // Calculate combined scale
    let result_scale = base_scale.checked_add(quote_scale)
        .ok_or(StablecoinError::MathError)?;
        
    // Calculate combined mantissa (base * quote)
    let result_mantissa = base_mantissa.checked_mul(quote_mantissa)
        .ok_or(StablecoinError::MathError)?;
    
    Ok((result_mantissa, result_scale))
}


/// Updated calculate_sovereign_coin_amount using On-Demand (mantissa/scale)
pub fn calculate_sovereign_coin_amount_on_demand(
    usdc_amount: u64,
    base_price_feed: Option<&AccountInfo>,
    quote_price_feed: Option<&AccountInfo>,
    target_currency: &str,
    decimals: u8,
) -> Result<u64> {
    let (price_mantissa, price_scale) = if target_currency == "USD" {
        // For USD sovereign coins, use 1:1 with USDC
        return Ok(usdc_amount);
    } else if let (Some(base_feed), Some(quote_feed)) = (base_price_feed, quote_price_feed) {
        // Calculate cross price
        calculate_cross_price_on_demand(base_feed, quote_feed)?
    } else if let Some(base_feed) = base_price_feed {
        // Single feed conversion
        get_oracle_price_on_demand(base_feed)?
    } else {
        return Err(StablecoinError::InvalidPriceFeed.into());
    };

    // Use your existing calculation logic
    let usdc_scaled = (usdc_amount as u128)
        .checked_mul(10u128.pow(decimals as u32))
        .ok_or(StablecoinError::MathError)?;
    
    let sovereign_scaled = usdc_scaled
        .checked_mul(price_mantissa.unsigned_abs() as u128)
        .ok_or(StablecoinError::MathError)?;
    
    let total_scale = price_scale.checked_add(decimals as u32)
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

pub fn calculate_bond_equivalent(
    usdc_amount: u64,
    base_price_feed: &AccountInfo,
    quote_price_feed: Option<&AccountInfo>,
    target_currency: &str,
    decimals: u8,
) -> Result<u64> {
    let (price_mantissa, price_scale) = if target_currency == "USD" {
        return Ok(usdc_amount);
    } else if let Some(quote_feed) = quote_price_feed {
        calculate_cross_price_on_demand(base_price_feed, quote_feed)?
    } else {
        get_oracle_price_on_demand(base_price_feed)?
    };

    let usdc_scaled = (usdc_amount as u128)
        .checked_mul(10u128.pow(decimals as u32))
        .ok_or(StablecoinError::MathError)?;
    
    let bond_scaled = usdc_scaled
        .checked_mul(price_mantissa.unsigned_abs() as u128)
        .ok_or(StablecoinError::MathError)?;
    
    let total_scale = price_scale.checked_add(decimals as u32)
        .ok_or(StablecoinError::MathError)?;
    
    let divisor = 10u128.checked_pow(total_scale as u32)
        .ok_or(StablecoinError::MathError)?;
    
    let bond_amount = bond_scaled
        .checked_div(divisor)
        .ok_or(StablecoinError::MathError)?;
    
    if bond_amount > u64::MAX as u128 {
        return Err(StablecoinError::MathError.into());
    }
    
    Ok(bond_amount as u64)
}