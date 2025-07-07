use super::*;


#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct LzSendParams {
    pub dst_eid: u32,
    pub to: [u8; 32],
    pub amount_ld: u64,
    pub min_amount_ld: u64,
    pub options: Vec<u8>,
    pub compose_msg: Option<Vec<u8>>,
    pub native_fee: u64,
    pub lz_token_fee: u64,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(params: LzSendParams)]
pub struct LzSend<'info> {
    pub signer: Signer<'info>,

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
        seeds = [
            PEER_SEED,
            oft_store.key().as_ref(),
            &params.dst_eid.to_be_bytes()
        ],
        bump = peer.bump
    )]
    pub peer: Account<'info, PeerConfig>,

    #[account(
        mut,
        seeds = [OFT_SEED, oft_store.token_escrow.as_ref()],
        bump = oft_store.bump,
        address = sovereign_coin.oft_store.unwrap() @ StablecoinError::LzInvalidOftStore,
    )]
    pub oft_store: Account<'info, OFTStore>,

    #[account(
        mut,
        token::authority = signer,
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub token_source: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        address = oft_store.token_escrow,
        token::authority = oft_store.key(),
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub token_escrow: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        address = oft_store.token_mint,
        mint::token_program = token_program
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    // Protocol fee collection account
    #[account(
        mut,
        address = factory.protocol_vault,
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub protocol_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl LzSend<'_> {
    pub fn apply(
        ctx: &mut Context<LzSend>,
        params: &LzSendParams,
    ) -> Result<(MessagingReceipt, StablecoinCrossChainReceipt)> {
        require!(!ctx.accounts.oft_store.paused, StablecoinError::LzPaused);

        // Calculate comprehensive fee breakdown
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

        // Check and update rate limits
        if let Some(rate_limiter) = ctx.accounts.peer.outbound_rate_limiter.as_mut() {
            rate_limiter.try_consume(amount_received_ld)?;
        }
        if let Some(rate_limiter) = ctx.accounts.peer.inbound_rate_limiter.as_mut() {
            rate_limiter.refill(amount_received_ld)?;
        }

        // Handle token operations based on OFT type
        if ctx.accounts.oft_store.oft_type == OFTType::Adapter {
            Self::handle_adapter_send(ctx, amount_sent_ld, protocol_fee_ld, lz_fee_ld)?;
        } else {
            Self::handle_native_send(ctx, amount_sent_ld, protocol_fee_ld, lz_fee_ld)?;
        }

        // Update sovereign coin state
        ctx.accounts.sovereign_coin.total_supply = ctx.accounts.sovereign_coin.total_supply
            .checked_sub(amount_received_ld)
            .ok_or(StablecoinError::MathOverflow)?;

        // Send cross-chain message
        require!(
            ctx.accounts.oft_store.key() == ctx.remaining_accounts[1].key(),
            StablecoinError::LzInvalidSender
        );

        let amount_sd = ctx.accounts.oft_store.ld2sd(amount_received_ld);
        let msg_receipt = oapp::endpoint_cpi::send(
            ctx.accounts.oft_store.endpoint_program,
            ctx.accounts.oft_store.key(),
            ctx.remaining_accounts,
            &[OFT_SEED, ctx.accounts.token_escrow.key().as_ref(), &[ctx.accounts.oft_store.bump]],
            EndpointSendParams {
                dst_eid: params.dst_eid,
                receiver: ctx.accounts.peer.peer_address,
                message: msg_codec::encode(
                    params.to,
                    amount_sd,
                    ctx.accounts.signer.key(),
                    &params.compose_msg,
                ),
                options: ctx
                    .accounts
                    .peer
                    .enforced_options
                    .combine_options(&params.compose_msg, &params.options)?,
                native_fee: params.native_fee,
                lz_token_fee: params.lz_token_fee,
            },
        )?;

        // Emit events
        emit_cpi!(StablecoinSent {
            guid: msg_receipt.guid,
            dst_eid: params.dst_eid,
            from: ctx.accounts.token_source.key(),
            amount_sent_ld,
            amount_received_ld,
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
        });

        let stablecoin_receipt = StablecoinCrossChainReceipt {
            amount_sent_ld,
            amount_received_ld,
            protocol_fee_ld,
            transfer_fee_ld,
            lz_fee_ld,
        };

        Ok((msg_receipt, stablecoin_receipt))
    }

    fn handle_adapter_send(
        ctx: &mut Context<LzSend>,
        amount_sent_ld: u64,
        protocol_fee_ld: u64,
        lz_fee_ld: u64,
    ) -> Result<()> {
        let total_amount = amount_sent_ld
            .checked_add(protocol_fee_ld)
            .and_then(|sum| sum.checked_add(lz_fee_ld))
            .ok_or(StablecoinError::MathOverflow)?;

        // Transfer all tokens from user to escrow (includes transfer fees)
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.token_source.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.token_escrow.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
            ),
            total_amount,
            ctx.accounts.token_mint.decimals,
        )?;

        // Transfer protocol fee to protocol vault
        if protocol_fee_ld > 0 {
            let seeds: &[&[u8]] = &[
                OFT_SEED,
                ctx.accounts.token_escrow.key().as_ref(),
                &[ctx.accounts.oft_store.bump]
            ];

            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.token_escrow.to_account_info(),
                        mint: ctx.accounts.token_mint.to_account_info(),
                        to: ctx.accounts.protocol_vault.to_account_info(),
                        authority: ctx.accounts.oft_store.to_account_info(),
                    },
                ).with_signer(&[&seeds]),
                protocol_fee_ld,
                ctx.accounts.token_mint.decimals,
            )?;
        }

        // Keep lz_fee_ld in escrow for fee collection
        // The actual tokens being sent cross-chain stay locked in escrow
        ctx.accounts.oft_store.tvl_ld = ctx.accounts.oft_store.tvl_ld
            .checked_add(amount_sent_ld)
            .ok_or(StablecoinError::MathOverflow)?;

        Ok(())
    }

    fn handle_native_send(
        ctx: &mut Context<LzSend>,
        amount_sent_ld: u64,
        protocol_fee_ld: u64,
        lz_fee_ld: u64,
    ) -> Result<()> {
        // For native tokens, burn the tokens being sent cross-chain
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    from: ctx.accounts.token_source.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
            ),
            amount_sent_ld,
        )?;

        // Transfer protocol fee to protocol vault
        if protocol_fee_ld > 0 {
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.token_source.to_account_info(),
                        mint: ctx.accounts.token_mint.to_account_info(),
                        to: ctx.accounts.protocol_vault.to_account_info(),
                        authority: ctx.accounts.signer.to_account_info(),
                    },
                ),
                protocol_fee_ld,
                ctx.accounts.token_mint.decimals,
            )?;
        }

        // Transfer LayerZero fee to escrow for fee collection
        if lz_fee_ld > 0 {
            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.token_source.to_account_info(),
                        mint: ctx.accounts.token_mint.to_account_info(),
                        to: ctx.accounts.token_escrow.to_account_info(),
                        authority: ctx.accounts.signer.to_account_info(),
                    },
                ),
                lz_fee_ld,
                ctx.accounts.token_mint.decimals,
            )?;
        }

        Ok(())
    }
}