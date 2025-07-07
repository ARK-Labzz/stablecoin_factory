use super::*;

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct SetPeerConfigParams {
    pub remote_eid: u32,
    pub config: PeerConfigParam,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub enum PeerConfigParam {
    PeerAddress([u8; 32]),
    FeeBps(Option<u16>),
    EnforcedOptions { send: Vec<u8>, send_and_call: Vec<u8> },
    OutboundRateLimit(Option<RateLimitParams>),
    InboundRateLimit(Option<RateLimitParams>),
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct RateLimitParams {
    pub refill_per_second: Option<u64>,
    pub capacity: Option<u64>,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(params: SetPeerConfigParams)]
pub struct SetPeerConfig<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,

    #[account(
        seeds = [
            b"sovereign_coin", 
            factory.key().as_ref(), 
            &sovereign_coin.symbol[..sovereign_coin.symbol.iter().position(|&x| x == 0).unwrap_or(8)]
        ],
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.is_cross_chain_enabled @ StablecoinError::LzNotEnabled,
    )]
    pub sovereign_coin: Account<'info, SovereignCoin>,

    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + PeerConfig::INIT_SPACE,
        seeds = [PEER_SEED, oft_store.key().as_ref(), &params.remote_eid.to_be_bytes()],
        bump
    )]
    pub peer: Account<'info, PeerConfig>,

    #[account(
        seeds = [OFT_SEED, oft_store.token_escrow.as_ref()],
        bump = oft_store.bump,
        address = sovereign_coin.oft_store.unwrap() @ StablecoinError::LzInvalidOftStore,
        constraint = is_authorized_lz_admin(&admin.key(), &sovereign_coin, &oft_store) @ StablecoinError::LzUnauthorized
    )]
    pub oft_store: Account<'info, OFTStore>,

    pub system_program: Program<'info, System>,
}

impl SetPeerConfig<'_> {
    pub fn apply(ctx: &mut Context<SetPeerConfig>, params: &SetPeerConfigParams) -> Result<()> {
        let peer = &mut ctx.accounts.peer;
        let sovereign_coin = &ctx.accounts.sovereign_coin;

        // Validate remote EID (Endpoint ID)
        require!(params.remote_eid > 0, StablecoinError::LzInvalidPeer);

        match params.config.clone() {
            PeerConfigParam::PeerAddress(peer_address) => {
                // Validate peer address is not empty
                require!(
                    peer_address != [0u8; 32],
                    StablecoinError::LzInvalidPeer
                );
                peer.peer_address = peer_address;
                
                emit_cpi!(LzPeerConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    remote_eid: params.remote_eid,
                    config_type: "PeerAddress".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            PeerConfigParam::FeeBps(fee_bps) => {
                if let Some(fee_bps) = fee_bps {
                    require!(fee_bps < MAX_FEE_BASIS_POINTS, StablecoinError::LzInvalidFee);
                }
                peer.fee_bps = fee_bps;
                
                emit_cpi!(LzPeerConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    remote_eid: params.remote_eid,
                    config_type: "FeeBps".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            PeerConfigParam::EnforcedOptions { send, send_and_call } => {
                // Validate options format (Type 3 options)
                oapp::options::assert_type_3(&send)?;
                peer.enforced_options.send = send;
                
                oapp::options::assert_type_3(&send_and_call)?;
                peer.enforced_options.send_and_call = send_and_call;
                
                emit_cpi!(LzPeerConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    remote_eid: params.remote_eid,
                    config_type: "EnforcedOptions".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            PeerConfigParam::OutboundRateLimit(rate_limit_params) => {
                Self::update_rate_limiter(
                    &mut peer.outbound_rate_limiter,
                    &rate_limit_params,
                )?;
                
                emit_cpi!(LzPeerConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    remote_eid: params.remote_eid,
                    config_type: "OutboundRateLimit".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            PeerConfigParam::InboundRateLimit(rate_limit_params) => {
                Self::update_rate_limiter(
                    &mut peer.inbound_rate_limiter,
                    &rate_limit_params,
                )?;
                
                emit_cpi!(LzPeerConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    remote_eid: params.remote_eid,
                    config_type: "InboundRateLimit".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
        }
        
        peer.bump = ctx.bumps.peer;
        Ok(())
    }

    fn update_rate_limiter(
        rate_limiter: &mut Option<RateLimiter>,
        params: &Option<RateLimitParams>,
    ) -> Result<()> {
        if let Some(param) = params {
            let mut limiter = rate_limiter.clone().unwrap_or_default();
            
            if let Some(capacity) = param.capacity {
                require!(capacity > 0, StablecoinError::LzInvalidRateLimit);
                limiter.set_capacity(capacity)?;
            }
            
            if let Some(refill_rate) = param.refill_per_second {
                limiter.set_rate(refill_rate)?;
            }
            
            *rate_limiter = Some(limiter);
        } else {
            // Remove rate limiting
            *rate_limiter = None;
        }
        Ok(())
    }
}

// Helper function to check if user is authorized for LayerZero operations
fn is_authorized_lz_admin(
    user: &Pubkey,
    sovereign_coin: &SovereignCoin,
    oft_store: &OFTStore,
) -> bool {
    // Protocol admins always have access
    if is_admin(user) {
        return true;
    }
    
    // OFT store admin
    if *user == oft_store.admin {
        return true;
    }
    
    // Cross-chain admin
    if let Some(cross_chain_admin) = sovereign_coin.cross_chain_admin {
        if *user == cross_chain_admin {
            return true;
        }
    }
    
    // Sovereign coin creator
    if *user == sovereign_coin.creator {
        return true;
    }
    
    false
}