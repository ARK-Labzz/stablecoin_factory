use super::*;


#[account]
#[derive(InitSpace)]
pub struct MintSovereignState {
    pub authority: Pubkey,
    pub payer: Pubkey,
    pub sovereign_coin: Pubkey,
    pub usdc_amount: u64,
    pub sovereign_amount: u64,
    pub reserve_amount: u64,
    pub bond_amount: u64,
    pub protocol_fee: u64,
    pub bump: u8,
}