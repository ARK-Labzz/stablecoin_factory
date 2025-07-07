use super::*;

pub const MAX_BOND_MAPPINGS: usize = 6;

pub const PRICE_PRECISION: u64 = 1_000_000_000; // 1e9 precision for prices
pub const BASIS_POINT_MAX: u16 = 10000;       // 100% in basis points        
pub const SCALE_OFFSET: u32 = 64;
pub const ONE_Q64: u128 = 1u128 << SCALE_OFFSET; // 1.0 in Q64.64 fixed-point
pub const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const ETHERFUSE_FEE_COLLECTOR: Pubkey = pubkey!("9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM");

// LayerZero seeds
pub const OFT_SEED: &[u8] = b"OFT";
pub const PEER_SEED: &[u8] = b"Peer";
pub const ENFORCED_OPTIONS_SEED: &[u8] = b"EnforcedOptions";
pub const LZ_RECEIVE_TYPES_SEED: &[u8] = oapp::LZ_RECEIVE_TYPES_SEED;

// LayerZero configuration
pub const DEFAULT_SHARED_DECIMALS: u8 = 6; // Standard for stablecoins

pub const MAX_FEE_BASIS_POINTS: u16 = 10_000;
pub const ONE_IN_BASIS_POINTS: u128 = MAX_FEE_BASIS_POINTS as u128;

pub const NONCE_OFFSET: usize = 0;
pub const SRC_EID_OFFSET: usize = 8;
pub const AMOUNT_LD_OFFSET: usize = 12;
pub const COMPOSE_FROM_OFFSET: usize = 20;
pub const COMPOSE_MSG_OFFSET: usize = 52;

pub const SEND_TO_OFFSET: usize = 0;
pub const SEND_AMOUNT_SD_OFFSET: usize = 32;
pub const COMPOSE_MSG_OFFSET: usize = 40;