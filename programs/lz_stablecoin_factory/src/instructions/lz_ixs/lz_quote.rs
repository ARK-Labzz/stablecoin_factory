use super::*;


// Quote LayerZero send operation
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct QuoteLzSendParams {
    pub dst_eid: u32,
    pub to: [u8; 32],
    pub amount_ld: u64,
    pub min_amount_ld: u64,
    pub options: Vec<u8>,
    pub compose_msg: Option<Vec<u8>>,
    pub pay_in_lz_token: bool,
}

#[derive(Accounts)]
#[instruction(params: QuoteLzSendParams)]
pub struct QuoteLzSend<'info> {
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
    )]
    pub oft_store: Account<'info, OFTStore>,

    #[account(
        seeds = [
            PEER_SEED,
            oft_store.key().as_ref(),
            &params.dst_eid.to_be_bytes()
        ],
        bump = peer.bump
    )]
    pub peer: Account<'info, PeerConfig>,

    #[account(
        address = oft_store.token_mint,
        mint::token_program = token_program
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl QuoteLzSend<'_> {
    pub fn apply(ctx: &Context<QuoteLzSend>, params: &QuoteLzSendParams) -> Result<MessagingFee> {
        require!(!ctx.accounts.oft_store.paused, StablecoinError::LzPaused);

        let (_, amount_received_ld, _) = compute_stablecoin_fee_and_adjust_amount(
            params.amount_ld,
            &ctx.accounts.factory,
            &ctx.accounts.oft_store,
            &ctx.accounts.token_mint,
            ctx.accounts.peer.fee_bps,
        )?;
        
        require!(
            amount_received_ld >= params.min_amount_ld, 
            StablecoinError::LzSlippageExceeded
        );

        // Convert to shared decimals for cross-chain transfer
        let amount_sd = ctx.accounts.oft_store.ld2sd(amount_received_ld);

        // Call LayerZero endpoint to get quote
        oapp::endpoint_cpi::quote(
            ctx.accounts.oft_store.endpoint_program,
            ctx.remaining_accounts,
            QuoteParams {
                sender: ctx.accounts.oft_store.key(),
                dst_eid: params.dst_eid,
                receiver: ctx.accounts.peer.peer_address,
                message: msg_codec::encode(
                    params.to,
                    amount_sd,
                    Pubkey::default(), // Will be set during actual send
                    &params.compose_msg,
                ),
                pay_in_lz_token: params.pay_in_lz_token,
                options: ctx
                    .accounts
                    .peer
                    .enforced_options
                    .combine_options(&params.compose_msg, &params.options)?,
            },
        )
    }
}

// Quote comprehensive stablecoin cross-chain operation
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct QuoteStablecoinCrossChainParams {
    pub dst_eid: u32,
    pub to: [u8; 32],
    pub amount_ld: u64,
    pub min_amount_ld: u64,
    pub options: Vec<u8>,
    pub compose_msg: Option<Vec<u8>>,
    pub pay_in_lz_token: bool,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct QuoteStablecoinCrossChainResult {
    pub lz_fee: MessagingFee,
    pub stablecoin_limits: StablecoinCrossChainLimits,
    pub fee_breakdown: Vec<StablecoinFeeDetail>,
    pub cross_chain_receipt: StablecoinCrossChainReceipt,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct StablecoinFeeDetail {
    pub fee_amount_ld: u64,
    pub description: String,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct StablecoinCrossChainReceipt {
    pub amount_sent_ld: u64,
    pub amount_received_ld: u64,
    pub protocol_fee_ld: u64,
    pub transfer_fee_ld: u64,
    pub lz_fee_ld: u64,
}

#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct StablecoinCrossChainLimits {
    pub min_amount_ld: u64,
    pub max_amount_ld: u64,
    pub daily_limit_ld: u64,
    pub remaining_daily_limit_ld: u64,
}

#[derive(Accounts)]
#[instruction(params: QuoteStablecoinCrossChainParams)]
pub struct QuoteStablecoinCrossChain<'info> {
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
    )]
    pub oft_store: Account<'info, OFTStore>,

    #[account(
        seeds = [
            PEER_SEED,
            oft_store.key().as_ref(),
            &params.dst_eid.to_be_bytes()
        ],
        bump = peer.bump
    )]
    pub peer: Account<'info, PeerConfig>,

    #[account(
        address = oft_store.token_mint,
        mint::token_program = token_program
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl QuoteStablecoinCrossChain<'_> {
    pub fn apply(
        ctx: &Context<QuoteStablecoinCrossChain>, 
        params: &QuoteStablecoinCrossChainParams
    ) -> Result<QuoteStablecoinCrossChainResult> {
        require!(!ctx.accounts.oft_store.paused, StablecoinError::LzPaused);

        // Calculate all fees and final amounts
        let (amount_sent_ld, amount_received_ld, protocol_fee_ld, transfer_fee_ld, lz_fee_ld) = 
            compute_comprehensive_stablecoin_fees(
                params.amount_ld,
                &ctx.accounts.factory,
                &ctx.accounts.oft_store,
                &ctx.accounts.token_mint,
                ctx.accounts.peer.fee_bps,
            )?;

        require!(
            amount_received_ld >= params.min_amount_ld, 
            StablecoinError::LzSlippageExceeded
        );

        // Get LayerZero messaging fee
        let amount_sd = ctx.accounts.oft_store.ld2sd(amount_received_ld);
        let lz_messaging_fee = oapp::endpoint_cpi::quote(
            ctx.accounts.oft_store.endpoint_program,
            ctx.remaining_accounts,
            QuoteParams {
                sender: ctx.accounts.oft_store.key(),
                dst_eid: params.dst_eid,
                receiver: ctx.accounts.peer.peer_address,
                message: msg_codec::encode(
                    params.to,
                    amount_sd,
                    Pubkey::default(),
                    &params.compose_msg,
                ),
                pay_in_lz_token: params.pay_in_lz_token,
                options: ctx
                    .accounts
                    .peer
                    .enforced_options
                    .combine_options(&params.compose_msg, &params.options)?,
            },
        )?;

        // Calculate rate limits
        let (daily_limit, remaining_limit) = calculate_daily_limits(
            &ctx.accounts.peer,
            &ctx.accounts.sovereign_coin,
        )?;

        // Build fee breakdown
        let mut fee_breakdown = Vec::new();
        
        if protocol_fee_ld > 0 {
            fee_breakdown.push(StablecoinFeeDetail {
                fee_amount_ld: protocol_fee_ld,
                description: "Protocol Fee".to_string(),
            });
        }
        
        if transfer_fee_ld > 0 {
            fee_breakdown.push(StablecoinFeeDetail {
                fee_amount_ld: transfer_fee_ld,
                description: "Token Transfer Fee".to_string(),
            });
        }
        
        if lz_fee_ld > 0 {
            fee_breakdown.push(StablecoinFeeDetail {
                fee_amount_ld: lz_fee_ld,
                description: "Cross-Chain Fee".to_string(),
            });
        }

        let stablecoin_limits = StablecoinCrossChainLimits {
            min_amount_ld: 1, // Minimum 1 token unit
            max_amount_ld: u64::MAX,
            daily_limit_ld: daily_limit,
            remaining_daily_limit_ld: remaining_limit,
        };

        let cross_chain_receipt = StablecoinCrossChainReceipt {
            amount_sent_ld,
            amount_received_ld,
            protocol_fee_ld,
            transfer_fee_ld,
            lz_fee_ld,
        };

        Ok(QuoteStablecoinCrossChainResult {
            lz_fee: lz_messaging_fee,
            stablecoin_limits,
            fee_breakdown,
            cross_chain_receipt,
        })
    }
}

// Helper function to compute comprehensive fee breakdown for stablecoins
pub fn compute_stablecoin_fee_and_adjust_amount(
    amount_ld: u64,
    factory: &Factory,
    oft_store: &OFTStore,
    token_mint: &InterfaceAccount<Mint>,
    peer_fee_bps: Option<u16>,
) -> Result<(u64, u64, u64)> {
    
    // Calculate protocol fee first
    let (net_after_protocol_fee, protocol_fee) = fee::calculate_protocol_fee(
        amount_ld,
        factory.transfer_fee_bps,
    )?;

    // Handle stablecoin-specific transfer fees (Token2022 fees)
    let (amount_sent_ld, amount_after_transfer_fee, transfer_fee_ld) = if oft_store.oft_type == OFTType::Adapter {
        let amount_after_fee = get_post_fee_amount_ld(token_mint, net_after_protocol_fee)?;
        let pre_fee_amount = get_pre_fee_amount_ld(token_mint, amount_after_fee)?;
        let transfer_fee = pre_fee_amount.saturating_sub(amount_after_fee);
        (pre_fee_amount, amount_after_fee, transfer_fee)
    } else {
        // Native type doesn't have transfer fees during mint/burn
        (net_after_protocol_fee, net_after_protocol_fee, 0)
    };

    // Calculate LayerZero cross-chain fee
    let lz_fee_ld = oft_store.remove_dust(calculate_lz_fee(
        amount_after_transfer_fee,
        oft_store.default_fee_bps,
        peer_fee_bps,
    ));

    let final_amount_received = amount_after_transfer_fee.saturating_sub(lz_fee_ld);

    Ok((amount_sent_ld, final_amount_received, lz_fee_ld))
}

// Comprehensive fee calculation including all stablecoin fees
pub fn compute_comprehensive_stablecoin_fees(
    amount_ld: u64,
    factory: &Factory,
    oft_store: &OFTStore,
    token_mint: &InterfaceAccount<Mint>,
    peer_fee_bps: Option<u16>,
) -> Result<(u64, u64, u64, u64, u64)> {
    
    // Calculate protocol fee
    let (net_after_protocol_fee, protocol_fee) = fee::calculate_protocol_fee(
        amount_ld,
        factory.transfer_fee_bps,
    )?;

    // Handle transfer fees
    let (amount_sent_ld, amount_after_transfer_fee, transfer_fee_ld) = if oft_store.oft_type == OFTType::Adapter {
        let amount_after_fee = get_post_fee_amount_ld(token_mint, net_after_protocol_fee)?;
        let pre_fee_amount = get_pre_fee_amount_ld(token_mint, amount_after_fee)?;
        let transfer_fee = pre_fee_amount.saturating_sub(amount_after_fee);
        (pre_fee_amount, amount_after_fee, transfer_fee)
    } else {
        (net_after_protocol_fee, net_after_protocol_fee, 0)
    };

    // Calculate LayerZero fee
    let lz_fee_ld = oft_store.remove_dust(calculate_lz_fee(
        amount_after_transfer_fee,
        oft_store.default_fee_bps,
        peer_fee_bps,
    ));

    let final_amount_received = amount_after_transfer_fee.saturating_sub(lz_fee_ld);

    Ok((amount_sent_ld, final_amount_received, protocol_fee, transfer_fee_ld, lz_fee_ld))
}

fn calculate_lz_fee(pre_fee_amount: u64, default_fee_bps: u16, peer_fee_bps: Option<u16>) -> u64 {
    let final_fee_bps = if let Some(bps) = peer_fee_bps { 
        bps as u128 
    } else { 
        default_fee_bps as u128 
    };
    
    if final_fee_bps == 0 || pre_fee_amount == 0 {
        0
    } else {
        let fee = (pre_fee_amount as u128) * final_fee_bps;
        (fee / MAX_FEE_BASIS_POINTS as u128) as u64
    }
}

fn calculate_daily_limits(
    peer: &PeerConfig,
    _sovereign_coin: &SovereignCoin,
) -> Result<(u64, u64)> {
    // Get rate limiter capacity as daily limit
    let daily_limit = if let Some(rate_limiter) = &peer.outbound_rate_limiter {
        rate_limiter.capacity
    } else {
        u64::MAX // No limit set
    };

    // Calculate remaining limit (simplified - in production, track actual usage)
    let remaining_limit = if let Some(rate_limiter) = &peer.outbound_rate_limiter {
        rate_limiter.tokens
    } else {
        u64::MAX
    };

    Ok((daily_limit, remaining_limit))
}