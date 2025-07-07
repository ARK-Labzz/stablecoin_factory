use super::*;

#[event]
pub struct FactoryInitializedEvent {
    pub authority: Pubkey,
    pub factory: Pubkey,
    pub min_usdc_reserve: u16,           
    pub bond_reserve_numerator: u8,       
    pub bond_reserve_denominator: u8,     
    pub yield_share_protocol: u16,        
    pub yield_share_issuer: u16,          
    pub yield_share_holders: u16,          
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinInitializedEvent {
    pub creator: Pubkey,
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
pub struct GlobalUsdcAccountsCreatedEvent {
    pub usdc_reserve: Pubkey,
    pub usdc_reserve_authority: Pubkey,
    pub usdc_mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinBondHoldingSetupEvent {
    pub sovereign_coin: Pubkey,
    pub bond_holding: Pubkey,
    pub bond_ownership: Pubkey,
    pub bond_mint: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinCreatedEvent {
    pub creator: Pubkey,
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
    pub sovereign_coin_amount: u64,
    pub reserve_amount: u64,
    pub bond_amount: u64,
    pub protocol_fee: u64,
    pub timestamp: i64,
}

#[event]
pub struct SovereignCoinInterestBearingWithTransferFeeInitializedEvent {
    pub sovereign_coin: Pubkey,
    pub mint: Pubkey,
    pub interest_rate: i16,
    pub transfer_fee_bps: u16,
    pub maximum_fee: u64,
    pub timestamp: i64
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
    pub sovereign_amount: u64,
    pub usdc_amount: u64,
    pub from_usdc_reserve: u64,
    pub from_protocol_vault: u64,
    pub from_bond_redemption: u64,
    pub protocol_fee: u64,
    pub timestamp: i64,
    pub redemption_type: RedemptionTypeState,
}

/// Create claim fee operator
#[event]
pub struct EvtCreateClaimFeeOperator {
    pub operator: Pubkey,
}

/// Close claim fee operator
#[event]
pub struct EvtCloseClaimFeeOperator {
    pub claim_fee_operator: Pubkey,
    pub operator: Pubkey,
}

#[event]
pub struct SovereignCoinInterestRateUpdatedEvent {
    pub sovereign_coin: Pubkey,
    pub mint: Pubkey,
    pub old_rate: i16, 
    pub new_rate: i16, 
    pub bond_mint: Pubkey,
    pub timestamp: i64, 
}

// Event emitted when a sovereign coin with interest-bearing properties is initialized.
#[event]
pub struct SovereignCoinInterestBearingInitializedEvent {
    /// Public key of the sovereign coin account.
    pub sovereign_coin: Pubkey,
    /// Public key of the mint associated with the sovereign coin.
    pub mint: Pubkey,
    /// Initial interest rate set for the sovereign coin.
    pub interest_rate: i16,
    pub timestamp: i64,
}

#[event]
pub struct BondInfoEvent {
    pub sovereign_coin: Pubkey,
    pub bond_issuance_number: u64,
    pub payment_feed_type: u8,
    pub bond_account: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct OFTSent {
    pub guid: [u8; 32],
    pub dst_eid: u32,
    pub from: Pubkey,
    pub amount_sent_ld: u64,
    pub amount_received_ld: u64,
}

#[event]
pub struct OFTReceived {
    pub guid: [u8; 32],
    pub src_eid: u32,
    pub to: Pubkey,
    pub amount_received_ld: u64,
}

#[event]
pub struct LzOftInitializedEvent {
    pub sovereign_coin: Pubkey,
    pub oft_store: Pubkey,
    pub oft_type: OFTType,
    pub token_mint: Pubkey,
    pub token_escrow: Pubkey,
    pub shared_decimals: u8,
    pub admin: Pubkey,
    pub cross_chain_admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LzConfigUpdatedEvent {
    pub sovereign_coin: Pubkey,
    pub oft_store: Pubkey,
    pub config_type: String,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LzPeerConfigUpdatedEvent {
    pub sovereign_coin: Pubkey,
    pub remote_eid: u32,
    pub config_type: String,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LzPauseStateChangedEvent {
    pub sovereign_coin: Pubkey,
    pub oft_store: Pubkey,
    pub paused: bool,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LzFeeWithdrawnEvent {
    pub sovereign_coin: Pubkey,
    pub oft_store: Pubkey,
    pub amount: u64,
    pub recipient: Pubkey,
    pub admin: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct LzEmergencyStopEvent {
    pub sovereign_coin: Pubkey,
    pub oft_store: Pubkey,
    pub emergency_admin: Pubkey,
    pub timestamp: i64,
}