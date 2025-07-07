use super::*;


#[account]
#[derive(InitSpace)]
pub struct SovereignCoin {
    pub bump: u8,
    pub creator: Pubkey,     // The issuer of this sovereign coin
    pub factory: Pubkey,       // Reference to the factory that created this coin
    pub name: [u8; 32],        // Name of the sovereign coin
    pub symbol: [u8; 8],       // Symbol (e.g., "USDe", "MXNe")
    pub decimals: u8,          // Typically 6 or 9
    #[max_len(200)]
    pub uri: Vec<u8>,        // URI to off-chain metadata (JSON)
    pub target_fiat_currency: [u8; 8],
    pub bond_mint: Pubkey,       // The Stablebond token mint
    pub bond_account: Pubkey,    // The Stablebond account PDA
    pub mint: Pubkey,          // The actual SPL token mint
    pub bond_holding: Pubkey,  // Token account holding the bond tokens (bonds in this account are owned by users and we are just storing it for them) 
    pub bond_ownership: Pubkey,  // Token account for bonds actually owned by the protocol
    pub total_supply: u64,     // Current total supply of this sovereign coin
    pub bond_rating: u8,       // Current bond rating ordinal (1-10)
    pub required_reserve_percentage: u16, // Calculated reserve requirement
    pub usdc_amount: u64,      // Current amount of fiat reserves
    pub bond_amount: u64,      // Current amount of bond holdings
    pub interest_rate: i16,  
    pub is_interest_bearing: bool,
    pub bond_issuance_number: u64,
    pub payment_feed_type: u8,  // 0=UsdcUsd, 1=UsdcMxn, 2=SwitchboardUsdcUsd, etc.
    pub oft_store: Option<Pubkey>,
    pub is_cross_chain_enabled: bool,
    pub cross_chain_admin: Option<Pubkey>,
}

impl SovereignCoin {
    /// Convert stored u8 back to PaymentFeedType
    pub fn get_payment_feed_type(&self) -> Result<PaymentFeedType> {
        match self.payment_feed_type {
            0 => Ok(PaymentFeedType::UsdcUsd),
            1 => Ok(PaymentFeedType::UsdcMxn),
            2 => Ok(PaymentFeedType::SwitchboardUsdcUsd),
            3 => Ok(PaymentFeedType::SwitchboardUsdcMxn),
            4 => Ok(PaymentFeedType::SwitchboardUsdcBrl),
            5 => Ok(PaymentFeedType::SwitchboardUsdcGbp),
            6 => Ok(PaymentFeedType::SwitchboardUsdcEur),
            7 => Ok(PaymentFeedType::SwitchboardOnDemandUsdcUsd),
            8 => Ok(PaymentFeedType::SwitchboardOnDemandUsdcMxn),
            9 => Ok(PaymentFeedType::SwitchboardOnDemandUsdcBrl),
            10 => Ok(PaymentFeedType::SwitchboardOnDemandUsdcGbp),
            11 => Ok(PaymentFeedType::SwitchboardOnDemandUsdcEur),
            12 => Ok(PaymentFeedType::Stub),
            _ => Err(StablecoinError::InvalidPriceFeed.into()),
        }
    }
    
    /// Set PaymentFeedType from the enum - handles all known variants
    pub fn set_payment_feed_type(&mut self, feed_type: PaymentFeedType) -> Result<()> {
        self.payment_feed_type = match feed_type {
            PaymentFeedType::UsdcUsd => 0,
            PaymentFeedType::UsdcMxn => 1,
            PaymentFeedType::SwitchboardUsdcUsd => 2,
            PaymentFeedType::SwitchboardUsdcMxn => 3,
            PaymentFeedType::SwitchboardUsdcBrl => 4,
            PaymentFeedType::SwitchboardUsdcGbp => 5,
            PaymentFeedType::SwitchboardUsdcEur => 6,
            PaymentFeedType::Stub => 7,
            PaymentFeedType::SwitchboardOnDemandUsdcUsd => 8,
            PaymentFeedType::SwitchboardOnDemandUsdcMxn => 9,
            PaymentFeedType::SwitchboardOnDemandUsdcBrl => 10,
            PaymentFeedType::SwitchboardOnDemandUsdcGbp => 11,
            PaymentFeedType::SwitchboardOnDemandUsdcEur => 12,

            // If there are other variants, add them here
            // You can check the stablebond-sdk docs or source code for complete list
        };
        Ok(())
    }

    pub fn u8_to_payment_feed_type(value: u8) -> Result<PaymentFeedType> {
    match value {
        0 => Ok(PaymentFeedType::UsdcUsd),
        1 => Ok(PaymentFeedType::UsdcMxn),
        2 => Ok(PaymentFeedType::SwitchboardUsdcUsd),
        3 => Ok(PaymentFeedType::SwitchboardUsdcMxn),
        4 => Ok(PaymentFeedType::Stub),
        _ => Err(StablecoinError::InvalidPriceFeed.into()),
    }
}

}
