use super::*;

#[cfg(not(feature = "devnet"))]
pub mod mainnet_admin {
    use anchor_lang::{prelude::Pubkey, solana_program::pubkey};

    pub const ADMINS: [Pubkey; 2] = [
        pubkey!("DfspRVBfdMLVikV9irYFoaL97EpBNYAYPCeqSHBSy6Ku"),
        pubkey!("AopUFgSHXJmcQARjTJex43NYcaQSMcVWmKtcYybo43Xm"),
    ];
}

#[cfg(feature = "devnet")]
pub mod devnet_admin {
    use anchor_lang::{prelude::Pubkey, solana_program::pubkey};

    pub const ADMINS: [Pubkey; 3] = [
        pubkey!("DfspRVBfdMLVikV9irYFoaL97EpBNYAYPCeqSHBSy6Ku"),
        pubkey!("AopUFgSHXJmcQARjTJex43NYcaQSMcVWmKtcYybo43Xm"),
        pubkey!("98JVBdaCP7QPdstpYYuUfDAYpw6ZZ5qeetbNmxuJdLbN"), // minh
    ];
}

// Verify if a Pubkey is an admin
#[cfg(feature = "local")]
pub fn is_admin(admin: &Pubkey) -> bool {
    // All keys allowed in local development
    true
}

#[cfg(not(feature = "local"))]
pub fn is_admin(admin: &Pubkey) -> bool {
    // Use the appropriate admins array based on build config
    #[cfg(feature = "devnet")]
    let admins = &devnet_admin::ADMINS;
    
    #[cfg(not(feature = "devnet"))]
    let admins = &mainnet_admin::ADMINS;
    
    // Check if the provided pubkey is in the admins list
    admins.iter().any(|predefined_admin| predefined_admin == admin)
}

// Verify if a Pubkey is a fee operator
pub fn is_fee_operator(
    operator: &Pubkey,
    claim_fee_operator_loader: &AccountLoader<FeeOperator>,
) -> Result<bool> {
    let claim_fee_operator = claim_fee_operator_loader.load()?;
    Ok(claim_fee_operator.operator == *operator)
}