pub const MAX_BOND_MAPPINGS: usize = 6;

pub const PRICE_PRECISION: u64 = 1_000_000_000; // 1e9 precision for prices
pub const BASIS_POINT_MAX: u64 = 10_000;        // 100% in basis points        
pub const SCALE_OFFSET: u32 = 64;
pub const ONE_Q64: u128 = 1u128 << SCALE_OFFSET; // 1.0 in Q64.64 fixed-point
