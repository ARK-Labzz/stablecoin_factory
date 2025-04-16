use super::*;

pub fn get_quote_price_feed(currency: &str) -> Result<Option<Pubkey>> {
    match currency {
        "MXN" => Ok(Some(Pubkey::new_from_array([
            190, 121, 80, 74, 55, 112, 120, 68, 56, 54, 83, 74, 113, 67, 99, 112,
            101, 119, 78, 55, 72, 100, 78, 107, 101, 80, 114, 83, 116, 67, 69, 68
        ]))),
        "EUR" => Ok(Some(Pubkey::new_from_array([
            69, 119, 98, 109, 52, 51, 111, 121, 120, 78, 117, 69, 106, 80, 67, 107,
            57, 55, 115, 67, 74, 80, 72, 65, 116, 85, 117, 119, 115, 112, 102, 71
        ]))),
        "BRL" => Ok(Some(Pubkey::new_from_array([
            55, 54, 112, 68, 81, 86, 56, 100, 51, 84, 98, 112, 114, 122, 49, 120,
            67, 67, 81, 119, 114, 86, 107, 70, 110, 100, 55, 55, 90, 86, 57, 57
        ]))),
        "GBP" => Ok(Some(Pubkey::new_from_array([
            66, 71, 69, 54, 115, 75, 89, 116, 99, 80, 77, 117, 113, 69, 90, 76,
            88, 110, 109, 115, 54, 87, 86, 110, 89, 99, 116, 51, 97, 110, 55, 105
        ]))),
        "USD" => Ok(None), // USD to USD doesn't need a quote price feed
        _ => Ok(None)      // Return None for unsupported currencies
    }
}

pub fn get_base_price_feed() -> Result<Pubkey> {
    Ok(Pubkey::new_from_array([
        71, 99, 107, 72, 109, 67, 119, 83, 121, 89, 118, 89, 68, 84, 74, 97,
        120, 52, 104, 104, 84, 122, 71, 77, 121, 75, 86, 53, 74, 109, 103, 75
    ]))
}