use super::*;


#[account]
#[derive(InitSpace)]
pub struct SovereignCoin {
    pub bump: u8,
    pub authority: Pubkey,     // The issuer of this sovereign coin
    pub factory: Pubkey,       // Reference to the factory that created this coin
    pub name: [u8; 32],        // Name of the sovereign coin
    pub symbol: [u8; 8],       // Symbol (e.g., "USDe", "MXNe")
    pub decimals: u8,          // Typically 6 or 9
    pub uri: [u8; 200],        // URI to off-chain metadata (JSON)
    pub target_fiat_currency: [u8; 8],
    pub bond_mint: Pubkey,       // The Stablebond token mint
    pub bond_account: Pubkey,    // The Stablebond account PDA
    pub mint: Pubkey,          // The actual SPL token mint
    pub fiat_reserve: Pubkey,  // Token account holding the fiat token (i.e USDC)
    pub bond_holding: Pubkey,  // Token account holding the bond tokens
    pub total_supply: u64,     // Current total supply of this sovereign coin
    pub bond_rating: u8,       // Current bond rating ordinal (1-10)
    pub required_reserve_percentage: u16, // Calculated reserve requirement
    pub fiat_amount: u64,      // Current amount of fiat reserves
    pub bond_amount: u64,      // Current amount of bond holdings
    pub interest_rate: i16,  // Add this field
    pub is_interest_bearing: bool,
    pub is_compressed: bool,
    pub merkle_tree: Option<Pubkey>,
}