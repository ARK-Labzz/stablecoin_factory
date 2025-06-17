
use super::*;

#[account]
#[derive(InitSpace)]
pub struct RedeemSovereignState {
    pub authority: Pubkey,
    pub payer: Pubkey,
    pub sovereign_coin: Pubkey,
    pub sovereign_amount: u64,
    pub usdc_amount: u64,
    pub net_amount: u64,
    pub from_fiat_reserve: u64,
    pub from_protocol_vault: u64,
    pub from_bond_redemption: u64,
    pub protocol_fee: u64,
    pub redemption_type: RedemptionTypeState,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, InitSpace)]
pub enum RedemptionTypeState {
    FiatReserveOnly,
    FiatAndProtocol,
    InstantBondRedemption,
    NFTBondRedemption,
}