use super::*;

#[event_cpi]
#[derive(Accounts)]
#[instruction(params: LzReceiveParams)]
pub struct LzReceive<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

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
            &params.src_eid.to_be_bytes()
        ],
        bump = peer.bump,
        constraint = peer.peer_address == params.sender @ StablecoinError::LzInvalidSender
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
        address = oft_store.token_escrow,
        token::authority = oft_store,
        token::mint = token_mint,
        token::token_program = token_program
    )]
    pub token_escrow: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: the wallet address to receive the token
    #[account(
        address = Pubkey::from(msg_codec::send_to(&params.message)) @ StablecoinError::LzInvalidTokenDest
    )]
    pub to_address: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = token_mint,
        associated_token::authority = to_address,
        associated_token::token_program = token_program
    )]
    pub token_dest: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        address = oft_store.token_mint,
        mint::token_program = token_program
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    // Only used for native mint, the mint authority can be:
    //      1. a spl-token multisig account with oft_store as one of the signers, and the quorum **MUST** be 1-of-n. (recommended)
    //      2. or the mint_authority is oft_store itself.
    #[account(
        constraint = token_mint.mint_authority == COption::Some(mint_authority.key()) @ StablecoinError::LzInvalidMintAuthority
    )]
    pub mint_authority: Option<AccountInfo<'info>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl LzReceive<'_> {
    pub fn apply(ctx: &mut Context<LzReceive>, params: &LzReceiveParams) -> Result<()> {
        require!(!ctx.accounts.oft_store.paused, StablecoinError::LzPaused);

        let oft_store_seed = ctx.accounts.token_escrow.key();
        let seeds: &[&[u8]] = &[OFT_SEED, oft_store_seed.as_ref(), &[ctx.accounts.oft_store.bump]];

        // Validate and clear the payload from LayerZero
        let accounts_for_clear = &ctx.remaining_accounts[0..Clear::MIN_ACCOUNTS_LEN];
        oapp::endpoint_cpi::clear(
            ctx.accounts.oft_store.endpoint_program,
            ctx.accounts.oft_store.key(),
            accounts_for_clear,
            seeds,
            ClearParams {
                receiver: ctx.accounts.oft_store.key(),
                src_eid: params.src_eid,
                sender: params.sender,
                nonce: params.nonce,
                guid: params.guid,
                message: params.message.clone(),
            },
        )?;

        // Convert the amount from shared decimals to local decimals
        let amount_sd = msg_codec::amount_sd(&params.message);
        let mut amount_received_ld = ctx.accounts.oft_store.sd2ld(amount_sd);

        // Apply rate limiting
        if let Some(rate_limiter) = ctx.accounts.peer.inbound_rate_limiter.as_mut() {
            rate_limiter.try_consume(amount_received_ld)?;
        }
        // Refill the outbound rate limiter
        if let Some(rate_limiter) = ctx.accounts.peer.outbound_rate_limiter.as_mut() {
            rate_limiter.refill(amount_received_ld)?;
        }

        // Handle token distribution based on OFT type
        if ctx.accounts.oft_store.oft_type == OFTType::Adapter {
            Self::handle_adapter_receive(ctx, seeds, amount_received_ld)?;
        } else if let Some(mint_authority) = &ctx.accounts.mint_authority {
            Self::handle_native_receive(ctx, seeds, mint_authority, amount_received_ld)?;
        } else {
            return Err(StablecoinError::LzInvalidMintAuthority.into());
        }

        // Update the amount_received_ld with post transfer fee amount for accurate accounting
        amount_received_ld = get_post_fee_amount_ld(&ctx.accounts.token_mint, amount_received_ld)?;

        // Update sovereign coin total supply
        ctx.accounts.sovereign_coin.total_supply = ctx.accounts.sovereign_coin.total_supply
            .checked_add(amount_received_ld)
            .ok_or(StablecoinError::MathOverflow)?;

        // Handle compose message if present
        if let Some(message) = msg_codec::compose_msg(&params.message) {
            oapp::endpoint_cpi::send_compose(
                ctx.accounts.oft_store.endpoint_program,
                ctx.accounts.oft_store.key(),
                &ctx.remaining_accounts[Clear::MIN_ACCOUNTS_LEN..],
                seeds,
                SendComposeParams {
                    to: ctx.accounts.to_address.key(),
                    guid: params.guid,
                    index: 0, // only 1 compose msg per lzReceive
                    message: compose_msg_codec::encode(
                        params.nonce,
                        params.src_eid,
                        amount_received_ld,
                        &message,
                    ),
                },
            )?;
        }

        // Emit cross-chain receive event
        emit_cpi!(StablecoinReceived {
            guid: params.guid,
            src_eid: params.src_eid,
            to: ctx.accounts.to_address.key(),
            amount_received_ld,
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
        });

        Ok(())
    }

    fn handle_adapter_receive(
        ctx: &mut Context<LzReceive>,
        seeds: &[&[u8]],
        amount_received_ld: u64,
    ) -> Result<()> {
        // For adapter type, unlock tokens from escrow
        ctx.accounts.oft_store.tvl_ld = ctx.accounts.oft_store.tvl_ld
            .checked_sub(amount_received_ld)
            .ok_or(StablecoinError::InsufficientLiquidity)?;

        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.token_escrow.to_account_info(),
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.token_dest.to_account_info(),
                    authority: ctx.accounts.oft_store.to_account_info(),
                },
            ).with_signer(&[&seeds]),
            amount_received_ld,
            ctx.accounts.token_mint.decimals,
        )?;

        Ok(())
    }

    fn handle_native_receive(
        ctx: &mut Context<LzReceive>,
        seeds: &[&[u8]],
        mint_authority: &AccountInfo,
        amount_received_ld: u64,
    ) -> Result<()> {
        // For native type, mint new tokens
        let ix = spl_token_2022::instruction::mint_to(
            ctx.accounts.token_program.key,
            &ctx.accounts.token_mint.key(),
            &ctx.accounts.token_dest.key(),
            mint_authority.key,
            &[&ctx.accounts.oft_store.key()],
            amount_received_ld,
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.token_dest.to_account_info(),
                ctx.accounts.token_mint.to_account_info(),
                mint_authority.to_account_info(),
                ctx.accounts.oft_store.to_account_info(),
            ],
            &[&seeds],
        )?;

        Ok(())
    }
}

// LzReceiveTypes implementation for account prediction
#[derive(Accounts)]
pub struct LzReceiveTypes<'info> {
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

    #[account(address = oft_store.token_mint)]
    pub token_mint: InterfaceAccount<'info, Mint>,
}

impl LzReceiveTypes<'_> {
    pub fn apply(
        ctx: &Context<LzReceiveTypes>,
        params: &LzReceiveParams,
    ) -> Result<Vec<oapp::endpoint_cpi::LzAccount>> {
        use anchor_spl::{
            associated_token::{get_associated_token_address_with_program_id, ID as ASSOCIATED_TOKEN_ID},
            token_2022::spl_token_2022::solana_program::program_option::COption,
        };

        let (peer, _) = Pubkey::find_program_address(
            &[PEER_SEED, ctx.accounts.oft_store.key().as_ref(), &params.src_eid.to_be_bytes()],
            ctx.program_id,
        );

        // Build account list for LzReceive instruction
        let mut accounts = vec![
            oapp::endpoint_cpi::LzAccount { 
                pubkey: Pubkey::default(), 
                is_signer: true, 
                is_writable: true 
            }, // 0: payer
            oapp::endpoint_cpi::LzAccount { 
                pubkey: ctx.accounts.factory.key(), 
                is_signer: false, 
                is_writable: false 
            }, // 1: factory
            oapp::endpoint_cpi::LzAccount { 
                pubkey: ctx.accounts.sovereign_coin.key(), 
                is_signer: false, 
                is_writable: true 
            }, // 2: sovereign_coin
            oapp::endpoint_cpi::LzAccount { 
                pubkey: peer, 
                is_signer: false, 
                is_writable: true 
            }, // 3: peer
            oapp::endpoint_cpi::LzAccount { 
                pubkey: ctx.accounts.oft_store.key(), 
                is_signer: false, 
                is_writable: true 
            }, // 4: oft_store
            oapp::endpoint_cpi::LzAccount {
                pubkey: ctx.accounts.oft_store.token_escrow.key(),
                is_signer: false,
                is_writable: true,
            }, // 5: token_escrow
        ];

        // Add recipient and token accounts
        let to_address = Pubkey::from(msg_codec::send_to(&params.message));
        let token_program = ctx.accounts.token_mint.to_account_info().owner;
        let token_dest = get_associated_token_address_with_program_id(
            &to_address,
            &ctx.accounts.oft_store.token_mint,
            token_program,
        );
        let mint_authority = if let COption::Some(mint_authority) = ctx.accounts.token_mint.mint_authority {
            mint_authority
        } else {
            ctx.program_id.key()
        };

        accounts.extend_from_slice(&[
            oapp::endpoint_cpi::LzAccount { 
                pubkey: to_address, 
                is_signer: false, 
                is_writable: false 
            }, // 6: to_address
            oapp::endpoint_cpi::LzAccount { 
                pubkey: token_dest, 
                is_signer: false, 
                is_writable: true 
            }, // 7: token_dest
            oapp::endpoint_cpi::LzAccount {
                pubkey: ctx.accounts.token_mint.key(),
                is_signer: false,
                is_writable: true,
            }, // 8: token_mint
            oapp::endpoint_cpi::LzAccount { 
                pubkey: mint_authority, 
                is_signer: false, 
                is_writable: false 
            }, // 9: mint_authority
            oapp::endpoint_cpi::LzAccount { 
                pubkey: *token_program, 
                is_signer: false, 
                is_writable: false 
            }, // 10: token_program
            oapp::endpoint_cpi::LzAccount { 
                pubkey: ASSOCIATED_TOKEN_ID, 
                is_signer: false, 
                is_writable: false 
            }, // 11: associated_token_program
        ]);

        // Add system program and event authority
        let (event_authority_account, _) =
            Pubkey::find_program_address(&[oapp::endpoint_cpi::EVENT_SEED], &ctx.program_id);
        accounts.extend_from_slice(&[
            oapp::endpoint_cpi::LzAccount {
                pubkey: solana_program::system_program::ID,
                is_signer: false,
                is_writable: false,
            }, // 12: system_program
        ]);

        let endpoint_program = ctx.accounts.oft_store.endpoint_program;
        
        // Add accounts for LayerZero clear operation
        let accounts_for_clear = oapp::endpoint_cpi::get_accounts_for_clear(
            endpoint_program,
            &ctx.accounts.oft_store.key(),
            params.src_eid,
            &params.sender,
            params.nonce,
        );
        accounts.extend(accounts_for_clear);

        // Add accounts for compose message if present
        if let Some(message) = msg_codec::compose_msg(&params.message) {
            let amount_sd = msg_codec::amount_sd(&params.message);
            let amount_ld = ctx.accounts.oft_store.sd2ld(amount_sd);
            let amount_received_ld = if ctx.accounts.oft_store.oft_type == OFTType::Native {
                amount_ld
            } else {
                get_post_fee_amount_ld(&ctx.accounts.token_mint, amount_ld)?
            };

            let accounts_for_composing = oapp::endpoint_cpi::get_accounts_for_send_compose(
                endpoint_program,
                &ctx.accounts.oft_store.key(),
                &to_address,
                &params.guid,
                0,
                &compose_msg_codec::encode(
                    params.nonce,
                    params.src_eid,
                    amount_received_ld,
                    &message,
                ),
            );
            accounts.extend(accounts_for_composing);
        }

        Ok(accounts)
    }
}