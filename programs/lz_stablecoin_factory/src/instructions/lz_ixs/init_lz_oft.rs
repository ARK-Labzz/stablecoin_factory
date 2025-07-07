use super::*;


#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitLzOftParams {
    pub oft_type: OFTType,
    pub shared_decimals: u8,
    pub endpoint_program: Option<Pubkey>,
    pub cross_chain_admin: Option<Pubkey>,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(params: InitLzOftParams)]
pub struct InitLzOft<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        constraint = is_admin(&admin.key()) @ StablecoinError::LzUnauthorized
    )]
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
        constraint = sovereign_coin.creator == admin.key() || is_admin(&admin.key()) @ StablecoinError::LzUnauthorized,
        constraint = !sovereign_coin.is_cross_chain_enabled @ StablecoinError::LzAlreadyEnabled,
    )]
    pub sovereign_coin: Account<'info, SovereignCoin>,

    #[account(
        init,
        payer = payer,
        space = 8 + OFTStore::INIT_SPACE,
        seeds = [OFT_SEED, token_escrow.key().as_ref()],
        bump
    )]
    pub oft_store: Account<'info, OFTStore>,

    #[account(
        init,
        payer = payer,
        space = 8 + LzReceiveTypesAccounts::INIT_SPACE,
        seeds = [LZ_RECEIVE_TYPES_SEED, oft_store.key().as_ref()],
        bump
    )]
    pub lz_receive_types_accounts: Account<'info, LzReceiveTypesAccounts>,

    #[account(
        address = sovereign_coin.mint,
        mint::token_program = token_program
    )]
    pub token_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = payer,
        token::authority = oft_store,
        token::mint = token_mint,
        token::token_program = token_program,
    )]
    pub token_escrow: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl InitLzOft<'_> {
    pub fn validate(accounts: &InitLzOft, _params: &InitLzOftParams) -> Result<()> {
        // Ensure the sovereign coin exists and is properly configured
        require!(
            accounts.sovereign_coin.mint != Pubkey::default(),
            StablecoinError::InvalidMint
        );

        // Ensure the sovereign coin has proper reserves and setup
        require!(
            accounts.sovereign_coin.bond_holding != Pubkey::default(),
            StablecoinError::InvalidBondHolding
        );

        // Ensure not already cross-chain enabled
        require!(
            !accounts.sovereign_coin.is_cross_chain_enabled,
            StablecoinError::LzAlreadyEnabled
        );

        Ok(())
    }

    pub fn apply(ctx: &mut Context<InitLzOft>, params: &InitLzOftParams) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let oft_store = &mut ctx.accounts.oft_store;

        require!(
            ctx.accounts.token_mint.decimals >= params.shared_decimals,
            StablecoinError::LzInvalidDecimals
        );

        require!(
            params.shared_decimals == DEFAULT_SHARED_DECIMALS,
            StablecoinError::LzInvalidDecimals
        );

        oft_store.oft_type = params.oft_type.clone();
        oft_store.ld2sd_rate = 10u64.pow((ctx.accounts.token_mint.decimals - params.shared_decimals) as u32);
        oft_store.token_mint = ctx.accounts.token_mint.key();
        oft_store.token_escrow = ctx.accounts.token_escrow.key();
        oft_store.endpoint_program = params.endpoint_program.unwrap_or(ENDPOINT_ID);
        oft_store.bump = ctx.bumps.oft_store;
        oft_store.tvl_ld = 0;
        oft_store.admin = ctx.accounts.admin.key();
        oft_store.default_fee_bps = 0; 
        oft_store.paused = false;
        oft_store.pauser = Some(ctx.accounts.admin.key());
        oft_store.unpauser = Some(ctx.accounts.admin.key());

        ctx.accounts.lz_receive_types_accounts.oft_store = oft_store.key();
        ctx.accounts.lz_receive_types_accounts.token_mint = ctx.accounts.token_mint.key();

        sovereign_coin.oft_store = Some(oft_store.key());
        sovereign_coin.is_cross_chain_enabled = true;
        sovereign_coin.cross_chain_admin = params.cross_chain_admin.or(Some(ctx.accounts.admin.key()));

        oapp::endpoint_cpi::register_oapp(
            oft_store.endpoint_program,
            oft_store.key(),
            ctx.remaining_accounts,
            &[OFT_SEED, ctx.accounts.token_escrow.key().as_ref(), &[ctx.bumps.oft_store]],
            RegisterOAppParams { 
                delegate: sovereign_coin.cross_chain_admin.unwrap() 
            },
        )?;

        let clock = Clock::get()?;
        emit_cpi!(LzOftInitializedEvent {
            sovereign_coin: sovereign_coin.key(),
            oft_store: oft_store.key(),
            oft_type: params.oft_type.clone(),
            token_mint: ctx.accounts.token_mint.key(),
            token_escrow: ctx.accounts.token_escrow.key(),
            shared_decimals: params.shared_decimals,
            admin: ctx.accounts.admin.key(),
            cross_chain_admin: sovereign_coin.cross_chain_admin.unwrap(),
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}