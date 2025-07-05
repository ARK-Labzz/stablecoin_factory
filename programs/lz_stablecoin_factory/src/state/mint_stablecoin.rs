use super::*;


#[account]
#[derive(InitSpace)]
pub struct MintSovereignState {
    pub payer: Pubkey,
    pub sovereign_coin: Pubkey,
    pub user_sovereign_coin_account: Pubkey,
    pub usdc_amount: u64,
    pub sovereign_amount: u64,
    pub reserve_amount: u64,
    pub bond_amount: u64,
    pub protocol_fee: u64,
    pub created_at: i64,
    pub bump: u8,
}