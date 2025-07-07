use super::*;

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub enum SetLzConfigParams {
    Admin(Pubkey),
    CrossChainAdmin(Pubkey),
    Delegate(Pubkey), // OApp delegate for the endpoint
    DefaultFee(u16),
    Paused(bool),
    Pauser(Option<Pubkey>),
    Unpauser(Option<Pubkey>),
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(params: SetLzConfigParams)]
pub struct SetLzConfig<'info> {
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Account<'info, Factory>,

    #[account(
        mut,
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
        mut,
        seeds = [OFT_SEED, oft_store.token_escrow.as_ref()],
        bump = oft_store.bump,
        address = sovereign_coin.oft_store.unwrap() @ StablecoinError::LzInvalidOftStore,
        constraint = is_authorized_lz_admin(&admin.key(), &sovereign_coin, &oft_store) @ StablecoinError::LzUnauthorized
    )]
    pub oft_store: Account<'info, OFTStore>,
}

impl SetLzConfig<'_> {
    pub fn apply(ctx: &mut Context<SetLzConfig>, params: &SetLzConfigParams) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let oft_store = &mut ctx.accounts.oft_store;

        match params.clone() {
            SetLzConfigParams::Admin(admin) => {
                // Only current admin or protocol admin can change this
                require!(
                    ctx.accounts.admin.key() == oft_store.admin || is_admin(&ctx.accounts.admin.key()),
                    StablecoinError::LzUnauthorized
                );
                oft_store.admin = admin;
                
                emit_cpi!(LzConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    oft_store: oft_store.key(),
                    config_type: "Admin".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            SetLzConfigParams::CrossChainAdmin(cross_chain_admin) => {
                // Only sovereign coin creator or protocol admin can change this
                require!(
                    ctx.accounts.admin.key() == sovereign_coin.creator || is_admin(&ctx.accounts.admin.key()),
                    StablecoinError::LzUnauthorized
                );
                sovereign_coin.cross_chain_admin = Some(cross_chain_admin);
                
                emit_cpi!(LzConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    oft_store: oft_store.key(),
                    config_type: "CrossChainAdmin".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            SetLzConfigParams::Delegate(delegate) => {
                let oft_store_seed = oft_store.token_escrow.key();
                let seeds: &[&[u8]] = &[OFT_SEED, &oft_store_seed.to_bytes(), &[oft_store.bump]];
                
                oapp::endpoint_cpi::set_delegate(
                    oft_store.endpoint_program,
                    oft_store.key(),
                    &ctx.remaining_accounts,
                    seeds,
                    SetDelegateParams { delegate },
                )?;
                
                emit_cpi!(LzConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    oft_store: oft_store.key(),
                    config_type: "Delegate".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            SetLzConfigParams::DefaultFee(fee_bps) => {
                require!(fee_bps < MAX_FEE_BASIS_POINTS, StablecoinError::LzInvalidFee);
                oft_store.default_fee_bps = fee_bps;
                
                emit_cpi!(LzConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    oft_store: oft_store.key(),
                    config_type: "DefaultFee".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            SetLzConfigParams::Paused(paused) => {
                oft_store.paused = paused;
                
                emit_cpi!(LzConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    oft_store: oft_store.key(),
                    config_type: if paused { "Paused" } else { "Unpaused" }.to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            SetLzConfigParams::Pauser(pauser) => {
                oft_store.pauser = pauser;
                
                emit_cpi!(LzConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    oft_store: oft_store.key(),
                    config_type: "Pauser".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
            SetLzConfigParams::Unpauser(unpauser) => {
                oft_store.unpauser = unpauser;
                
                emit_cpi!(LzConfigUpdatedEvent {
                    sovereign_coin: sovereign_coin.key(),
                    oft_store: oft_store.key(),
                    config_type: "Unpauser".to_string(),
                    admin: ctx.accounts.admin.key(),
                    timestamp: Clock::get()?.unix_timestamp,
                });
            },
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