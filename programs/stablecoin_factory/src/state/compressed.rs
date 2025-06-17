use super::*;

#[account]
#[derive(InitSpace)]
pub struct CompressedMintState {
    pub authority: Pubkey,
    pub payer: Pubkey,
    pub sovereign_coin: Pubkey,
    pub usdc_amount: u64,
    pub sovereign_amount: u64,
    pub reserve_amount: u64,
    pub bond_amount: u64,
    pub protocol_fee: u64,
    pub merkle_tree_index: u8,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct CompressedRedeemState {
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
    pub merkle_tree_index: u8,
    pub bump: u8,
}