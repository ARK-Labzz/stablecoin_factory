use super::*;


// Set Pause State
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct SetLzPauseParams {
    pub paused: bool,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(params: SetLzPauseParams)]
pub struct SetLzPause<'info> {
    /// pauser or unpauser
    pub signer: Signer<'info>,

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
        mut,
        seeds = [OFT_SEED, oft_store.token_escrow.as_ref()],
        bump = oft_store.bump,
        address = sovereign_coin.oft_store.unwrap() @ StablecoinError::LzInvalidOftStore,
        constraint = is_valid_pause_signer(signer.key(), &oft_store, params.paused) @ StablecoinError::LzUnauthorized
    )]
    pub oft_store: Account<'info, OFTStore>,
}

impl SetLzPause<'_> {
    pub fn apply(ctx: &mut Context<SetLzPause>, params: &SetLzPauseParams) -> Result<()> {
        ctx.accounts.oft_store.paused = params.paused;
        
        emit_cpi!(LzPauseStateChangedEvent {
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            oft_store: ctx.accounts.oft_store.key(),
            paused: params.paused,
            admin: ctx.accounts.signer.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        Ok(())
    }
}

// Withdraw LayerZero Fees
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawLzFeeParams {
    pub fee_ld: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(params: WithdrawLzFeeParams)]
pub struct WithdrawLzFee<'info> {
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
        seeds = [OFT_SEED, oft_store.token_escrow.as_ref()],
        bump = oft_store.bump,
        address = sovereign_coin.oft_store.unwrap() @ StablecoinError::LzInvalidOftStore,
        constraint = is_authorized_lz_admin(&admin.key(), &sovereign_coin, &oft_store) @ StablecoinError::LzUnauthorized
    )]
    pub oft_store: Account<'info, OFTStore>,

    #[account(
        address = oft_store.token_mint,
        mint::token_program = token_program
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        address = oft_store.token_escrow,
        token::authority = oft_store,
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub token_escrow: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub fee_dest: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl WithdrawLzFee<'_> {
    pub fn apply(ctx: &mut Context<WithdrawLzFee>, params: &WithdrawLzFeeParams) -> Result<()> {
        // Calculate available fees (total escrow balance minus locked TVL)
        let available_fees = ctx.accounts.token_escrow.amount
            .checked_sub(ctx.accounts.oft_store.tvl_ld)
            .ok_or(StablecoinError::InsufficientLiquidity)?;
        
        require!(
            available_fees >= params.fee_ld,
            StablecoinError::LzInvalidFee
        );

        let seeds: &[&[u8]] = &[
            OFT_SEED,
            &ctx.accounts.token_escrow.key().to_bytes(),
            &[ctx.accounts.oft_store.bump],
        ];

        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.token_escrow.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.fee_dest.to_account_info(),
                    authority: ctx.accounts.oft_store.to_account_info(),
                },
            ).with_signer(&[&seeds]),
            params.fee_ld,
            ctx.accounts.token_mint.decimals,
        )?;

        emit_cpi!(LzFeeWithdrawnEvent {
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            oft_store: ctx.accounts.oft_store.key(),
            amount: params.fee_ld,
            recipient: ctx.accounts.fee_dest.key(),
            admin: ctx.accounts.admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}

// Emergency Stop Cross-Chain Operations
#[event_cpi]
#[derive(Accounts)]
pub struct EmergencyStopLz<'info> {
    pub emergency_admin: Signer<'info>,

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
        mut,
        seeds = [OFT_SEED, oft_store.token_escrow.as_ref()],
        bump = oft_store.bump,
        address = sovereign_coin.oft_store.unwrap() @ StablecoinError::LzInvalidOftStore,
        constraint = is_emergency_admin(&emergency_admin.key(), &sovereign_coin, &oft_store) @ StablecoinError::LzUnauthorized
    )]
    pub oft_store: Account<'info, OFTStore>,
}

impl EmergencyStopLz<'_> {
    pub fn apply(ctx: &mut Context<EmergencyStopLz>) -> Result<()> {
        // Emergency pause all LayerZero operations
        ctx.accounts.oft_store.paused = true;
        
        emit_cpi!(LzEmergencyStopEvent {
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            oft_store: ctx.accounts.oft_store.key(),
            emergency_admin: ctx.accounts.emergency_admin.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });
        
        Ok(())
    }
}

// Get LayerZero Status
#[derive(Accounts)]
pub struct GetLzStatus<'info> {
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
    )]
    pub sovereign_coin: Account<'info, SovereignCoin>,

    #[account(
        seeds = [OFT_SEED, oft_store.token_escrow.as_ref()],
        bump = oft_store.bump,
        address = sovereign_coin.oft_store.unwrap() @ StablecoinError::LzInvalidOftStore,
    )]
    pub oft_store: Account<'info, OFTStore>,

    #[account(address = oft_store.token_mint)]
    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(address = oft_store.token_escrow)]
    pub token_escrow: InterfaceAccount<'info, TokenAccount>,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct LzStatusResult {
    pub is_enabled: bool,
    pub is_paused: bool,
    pub oft_type: OFTType,
    pub tvl_ld: u64,
    pub available_fees_ld: u64,
    pub total_supply: u64,
    pub admin: Pubkey,
    pub cross_chain_admin: Option<Pubkey>,
    pub default_fee_bps: u16,
}

impl GetLzStatus<'_> {
    pub fn apply(ctx: &Context<GetLzStatus>) -> Result<LzStatusResult> {
        let available_fees = ctx.accounts.token_escrow.amount
            .checked_sub(ctx.accounts.oft_store.tvl_ld)
            .unwrap_or(0);

        Ok(LzStatusResult {
            is_enabled: ctx.accounts.sovereign_coin.is_cross_chain_enabled,
            is_paused: ctx.accounts.oft_store.paused,
            oft_type: ctx.accounts.oft_store.oft_type.clone(),
            tvl_ld: ctx.accounts.oft_store.tvl_ld,
            available_fees_ld: available_fees,
            total_supply: ctx.accounts.sovereign_coin.total_supply,
            admin: ctx.accounts.oft_store.admin,
            cross_chain_admin: ctx.accounts.sovereign_coin.cross_chain_admin,
            default_fee_bps: ctx.accounts.oft_store.default_fee_bps,
        })
    }
}

// Helper functions
fn is_valid_pause_signer(signer: Pubkey, oft_store: &OFTStore, paused: bool) -> bool {
    if paused {
        // For pausing, check if signer is pauser
        oft_store.pauser == Some(signer) || is_admin(&signer)
    } else {
        // For unpausing, check if signer is unpauser
        oft_store.unpauser == Some(signer) || is_admin(&signer)
    }
}

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

fn is_emergency_admin(
    user: &Pubkey,
    sovereign_coin: &SovereignCoin,
    oft_store: &OFTStore,
) -> bool {
    // Emergency admins have broader access for safety
    is_admin(user) || 
    *user == oft_store.admin || 
    *user == sovereign_coin.creator ||
    sovereign_coin.cross_chain_admin == Some(*user)
}