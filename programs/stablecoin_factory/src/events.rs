use super::*;

#[event]
pub struct FactoryInitializedEvent {
    pub authority: Pubkey,
    pub factory: Pubkey,
    pub min_fiat_reserve: u16,           
    pub bond_reserve_numerator: u8,       
    pub bond_reserve_denominator: u8,     
    pub yield_share_protocol: u16,        
    pub yield_share_issuer: u16,          
    pub yield_share_holders: u16,          
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinInitializedEvent {
    pub authority: Pubkey,
    pub sovereign_coin: Pubkey,
    pub name: String,
    pub symbol: String,
    pub fiat_currency: String,
    pub bond_mint: Pubkey,
    pub bond_account: Pubkey,
    pub bond_rating: u8,
    pub decimals: u8,
    pub total_supply: u64,  
    pub required_reserve_percentage: u16, 
    pub fiat_amount: u64,
    pub bond_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinSetupMintEvent {
    pub mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinTokenAccountsEvent {
    pub fiat_reserve: Pubkey,
    pub bond_holding: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinCreatedEvent {
    pub authority: Pubkey,
    pub sovereign_coin: Pubkey,
    pub mint: Pubkey,
    pub name: String,
    pub symbol: String,
    pub fiat_currency: String,
    pub bond_mint: Pubkey,
    pub bond_account: Pubkey,
    pub bond_rating: u8,
    pub timestamp: i64,
}

#[event]
pub struct BondMappingRegisteredEvent {
    pub authority: Pubkey,
    pub factory: Pubkey,
    pub fiat_currency: String,
    pub bond_mint: Pubkey,
    pub bond_rating: u8,
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinMintedEvent {
    pub payer: Pubkey,
    pub sovereign_coin: Pubkey,
    pub usdc_amount: u64,
    pub reserve_amount: u64,
    pub bond_amount: u64,
    pub protocol_fee: u64,
    pub timestamp: i64,
}

#[event]
pub struct PriceFeedsUpdatedEvent {
    pub authority: Pubkey,
    pub factory: Pubkey,
    pub base_price_feed: Pubkey,
    pub quote_price_feed: Option<Pubkey>,
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinRedeemedEvent {
    pub payer: Pubkey,
    pub sovereign_coin: Pubkey,
    pub usdc_amount: u64,
    pub from_fiat_reserve: u64,
    pub from_protocol_vault: u64,
    pub from_bond_redemption: u64,
    pub protocol_fee: u64,
    pub timestamp: i64,
    pub redemption_type: RedemptionType,
}

