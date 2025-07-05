use super::*;


#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Copy, InitSpace)]
pub struct BondCurrencyMapping {
    pub active: bool,
    pub fiat_currency: [u8; 8],      // Currency code (e.g., "USD", "MXN")
    pub bond_mint: Pubkey,           // The Stablebond token mint
    pub bond_rating: u8,             // Bond rating (1-10)
}


#[account]
#[derive(InitSpace)]
pub struct Factory {
    pub bump: u8,
    pub authority: Pubkey,  // Admin who can update certain parameters
    pub treasury: Pubkey,   // Treasury account to collect fees
    pub total_sovereign_coins: u64,  // Count of all sovereign coins created
    pub total_supply_all_coins: u128, // Combined market cap of all coins
    pub bond_rating_ordinals: [u8; 10],  // AAA=1, AA=2, etc.
    pub global_usdc_reserve: Pubkey,  // Global USDC token account 
    pub global_usdc_account: Pubkey, // Global USDC token account that buys the bond
    pub min_usdc_reserve_percentage: u16,  // Base 20% 
    pub bond_reserve_numerator: u8,        // 30 in the 30/9 ratio
    pub bond_reserve_denominator: u8,      // 9 in the 30/9 ratio
    pub yield_share_protocol: u16,        // Protocol's share in bps (e.g. 1000 = 10%)
    pub yield_share_issuer: u16,          // Issuer's share in bps (e.g. 2000 = 20%)
    pub yield_share_holders: u16,         // Holders' share in bps (e.g. 7000 = 70%) 
    pub transfer_fee_bps: u16,              // Fee in basis points for minting, if any
    pub maximum_transfer_fee: u64,              // Fee in basis points for burning, if any
    pub protocol_vault: Pubkey,
    pub bond_mappings_count: u8,
    pub bond_mappings: [BondCurrencyMapping; MAX_BOND_MAPPINGS],
    pub payment_base_price_feed_account: Pubkey,      // USDC/USD price feed
    pub payment_quote_price_feed_account: Option<Pubkey>,  // Optional quote price feed
}
