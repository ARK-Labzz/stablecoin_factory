use super::*;

#[derive(Clone, AnchorSerialize, AnchorDeserialize, InitSpace, PartialEq)]
pub enum OFTType {
    Native,   // Mint/burn tokens for cross-chain transfers
    Adapter,  // Lock/unlock tokens for cross-chain transfers
}

#[account]
#[derive(InitSpace)]
pub struct OFTStore {
    pub oft_type: OFTType,
    pub ld2sd_rate: u64,            // Local decimals to shared decimals conversion rate
    pub token_mint: Pubkey,         // The stablecoin mint
    pub token_escrow: Pubkey,       // Escrow account for locked tokens
    pub endpoint_program: Pubkey,   // LayerZero endpoint program
    pub bump: u8,
    pub tvl_ld: u64,               // Total value locked in local decimals
    pub admin: Pubkey,             // LayerZero admin
    pub default_fee_bps: u16,      // Default cross-chain fee in basis points
    pub paused: bool,              // Emergency pause state
    pub pauser: Option<Pubkey>,    // Account that can pause operations
    pub unpauser: Option<Pubkey>,  // Account that can unpause operations
}

impl OFTStore {
    // Convert local decimals to shared decimals
    pub fn ld2sd(&self, amount_ld: u64) -> u64 {
        amount_ld / self.ld2sd_rate
    }

    // Convert shared decimals to local decimals
    pub fn sd2ld(&self, amount_sd: u64) -> u64 {
        amount_sd * self.ld2sd_rate
    }

    // Remove dust for precise calculations
    pub fn remove_dust(&self, amount_ld: u64) -> u64 {
        (amount_ld / self.ld2sd_rate) * self.ld2sd_rate
    }
}

#[account]
#[derive(InitSpace)]
pub struct PeerConfig {
    pub peer_address: [u8; 32],                    // Remote chain OFT contract address
    pub bump: u8,
    pub fee_bps: Option<u16>,                      // Per-chain fee override
    pub enforced_options: EnforcedOptions,         // Enforced execution options
    pub outbound_rate_limiter: Option<RateLimiter>, // Rate limiting for outbound transfers
    pub inbound_rate_limiter: Option<RateLimiter>,  // Rate limiting for inbound transfers
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, InitSpace, Default)]
pub struct EnforcedOptions {
    pub send: Vec<u8>,          // Options for simple send
    pub send_and_call: Vec<u8>, // Options for send with compose message
}

impl EnforcedOptions {
    pub fn combine_options(
        &self,
        compose_msg: &Option<Vec<u8>>,
        user_options: &[u8],
    ) -> Result<Vec<u8>> {
        let enforced = if compose_msg.is_some() {
            &self.send_and_call
        } else {
            &self.send
        };

        // Combine enforced options with user options
        // This is a simplified implementation - in production, you'd properly merge options
        if enforced.is_empty() {
            Ok(user_options.to_vec())
        } else if user_options.is_empty() {
            Ok(enforced.clone())
        } else {
            // Combine both options (implementation depends on LayerZero options format)
            let mut combined = enforced.clone();
            combined.extend_from_slice(user_options);
            Ok(combined)
        }
    }
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize, InitSpace, Default)]
pub struct RateLimiter {
    pub capacity: u64,        // Maximum tokens in bucket
    pub tokens: u64,          // Current tokens available
    pub refill_rate: u64,     // Tokens per second refill rate
    pub last_refill: i64,     // Last refill timestamp
}

impl RateLimiter {
    pub fn try_consume(&mut self, amount: u64) -> Result<()> {
        self.refill_tokens()?;
        
        if self.tokens >= amount {
            self.tokens -= amount;
            Ok(())
        } else {
            Err(StablecoinError::LzRateLimitExceeded.into())
        }
    }

    pub fn refill(&mut self, amount: u64) -> Result<()> {
        self.refill_tokens()?;
        self.tokens = std::cmp::min(self.capacity, self.tokens + amount);
        Ok(())
    }

    pub fn set_capacity(&mut self, capacity: u64) -> Result<()> {
        self.capacity = capacity;
        self.tokens = std::cmp::min(self.tokens, capacity);
        Ok(())
    }

    pub fn set_rate(&mut self, rate: u64) -> Result<()> {
        self.refill_rate = rate;
        Ok(())
    }

    fn refill_tokens(&mut self) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        if self.last_refill == 0 {
            self.last_refill = now;
            return Ok(());
        }

        let time_passed = now.saturating_sub(self.last_refill) as u64;
        let tokens_to_add = time_passed.saturating_mul(self.refill_rate);
        
        self.tokens = std::cmp::min(self.capacity, self.tokens.saturating_add(tokens_to_add));
        self.last_refill = now;
        
        Ok(())
    }
}

#[account]
#[derive(InitSpace)]
pub struct LzReceiveTypesAccounts {
    pub oft_store: Pubkey,
    pub token_mint: Pubkey,
}