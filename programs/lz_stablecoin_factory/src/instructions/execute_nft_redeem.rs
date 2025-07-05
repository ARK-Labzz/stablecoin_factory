use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteNFTRedemption<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Box<Account<'info, Factory>>,

    #[account(
        mut,
        seeds = [
            b"sovereign_coin", 
            factory.key().as_ref(), 
            &sovereign_coin.symbol[..sovereign_coin.symbol.iter().position(|&x| x == 0).unwrap_or(8)]
        ],
        bump = sovereign_coin.bump,
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    #[account(
        mut,
        seeds = [b"redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = redeem_state.bump,
        constraint = redeem_state.payer == payer.key(),
        constraint = redeem_state.redemption_type == RedemptionTypeState::NFTBondRedemption,
        close = payer
    )]
    pub redeem_state: Box<Account<'info, RedeemSovereignState>>,
    
    #[account(
        mut,
        token::mint = mint,
        token::authority = payer,
        constraint = user_sovereign_coin_account.amount >= redeem_state.sovereign_amount @ StablecoinError::InsufficientBalance
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    #[account(
        mut,
        associated_token::mint = usdc_token_mint,
        associated_token::authority = payer,
    )]
    pub user_usdc_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        constraint = mint.key() == sovereign_coin.mint @ StablecoinError::InvalidSovereignCoinMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = usdc_token_mint,
        associated_token::authority = factory,
        constraint = global_usdc_reserve.key() == factory.global_usdc_reserve @ StablecoinError::InvalidGlobalUsdcReserve
    )]
    pub global_usdc_reserve: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = usdc_token_mint,
        associated_token::authority = factory,
        constraint = usdc_protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub usdc_protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = bond_token_mint,
        associated_token::authority = factory,
        constraint = bond_holding.key() == sovereign_coin.bond_holding @ StablecoinError::InvalidBondHolding,
        constraint = bond_holding.amount >= redeem_state.from_bond_redemption @ StablecoinError::InsufficientBondBalance
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,

    // NFT accounts - required for this instruction
    #[account(
        init,
        payer = payer,
        associated_token::mint = nft_token_mint,
        associated_token::authority = factory,
    )]
    pub factory_nft_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = nft_token_mint,
        associated_token::authority = payer,
    )]
    pub user_nft_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: NFT mint - validated in NFT redemption logic
    pub nft_token_mint: UncheckedAccount<'info>,

    /// CHECK: NFT collection mint - validated in NFT redemption logic
    pub nft_collection_mint: UncheckedAccount<'info>,

    #[account(
        constraint = usdc_token_mint.key() == USDC_MINT @ StablecoinError::InvalidUSDCMint
    )]
    pub usdc_token_mint: Box<InterfaceAccount<'info, Mint>>,
    
    #[account(
        constraint = bond_token_mint.key() == sovereign_coin.bond_mint @ StablecoinError::InvalidBondMint
    )]
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub metadata_program: Program<'info, Metadata>,
    pub rent: Sysvar<'info, Rent>,
}

impl ExecuteNFTRedemption<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let redeem_state = &ctx.accounts.redeem_state;

        let initial_user_sovereign_balance = ctx.accounts.user_sovereign_coin_account.amount;
        let initial_user_usdc_balance = ctx.accounts.user_usdc_token_account.amount;

        let clock = Clock::get()?;
        require!(
            redeem_state.created_at + 300 > clock.unix_timestamp, 
            StablecoinError::RedeemStateExpired
        );

        // Burn sovereign coins
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            redeem_state.sovereign_amount,
        )?;

        // Transfer from USDC reserve if applicable
        if redeem_state.from_usdc_reserve > 0 {
            let factory_seeds = &[
                b"factory".as_ref(),
                &[ctx.accounts.factory.bump],
            ];
            let factory_signer = &[&factory_seeds[..]];
            
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.global_usdc_reserve.to_account_info(),
                        mint: ctx.accounts.usdc_token_mint.to_account_info(),
                        to: ctx.accounts.user_usdc_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                redeem_state.from_usdc_reserve,
                ctx.accounts.usdc_token_mint.decimals,
            )?;
        }

        // Transfer from protocol vault if applicable
        if redeem_state.from_protocol_vault > 0 {
            let factory_seeds = &[
                b"factory".as_ref(),
                &[ctx.accounts.factory.bump],
            ];
            let factory_signer = &[&factory_seeds[..]];

            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.usdc_protocol_vault.to_account_info(),
                        mint: ctx.accounts.usdc_token_mint.to_account_info(),
                        to: ctx.accounts.user_usdc_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                redeem_state.from_protocol_vault,
                ctx.accounts.usdc_token_mint.decimals,
            )?;
        }

        // Execute NFT bond redemption
        let bond_issuance_number = sovereign_coin.bond_issuance_number;
        let payment_feed_type = sovereign_coin.get_payment_feed_type()?;
        let (bond_pda, _) = find_bond_pda(ctx.accounts.bond_token_mint.key());
        let (issuance_pda, _) = find_issuance_pda(bond_pda, bond_issuance_number);
        // let (payment_pda, _) = find_payment_pda(issuance_pda);
        let (payment_feed_pda, _) = find_payment_feed_pda(payment_feed_type);
        
        let nft_mint_key = ctx.accounts.nft_token_mint.key();
        let nft_collection_mint_key = ctx.accounts.nft_collection_mint.key();
        
        let (nft_issuance_vault_pda, _) = find_nft_issuance_vault_pda(nft_mint_key);
        let nft_issuance_vault_token_account = get_associated_token_address(&nft_issuance_vault_pda, &nft_mint_key);
        let (payout_pda, _) = find_payout_pda(issuance_pda);
        let payout_token_account = get_associated_token_address(&payout_pda, &ctx.accounts.usdc_token_mint.key());
        let (nft_metadata_account, _) = MetadataMpl::find_pda(&nft_mint_key);
        let (nft_master_edition_account, _) = MasterEditionMpl::find_pda(&nft_mint_key);
        let (nft_collection_metadata_account, _) = MetadataMpl::find_pda(&nft_collection_mint_key);

        let factory_seeds = &[
            b"factory".as_ref(),
            &[ctx.accounts.factory.bump],
        ];
        let factory_signer = &[&factory_seeds[..]];

        let redeem_bond_ix = RedeemBond {
            user_wallet: ctx.accounts.factory.key(),
            bond_account: bond_pda,
            mint_account: ctx.accounts.bond_token_mint.key(),
            issuance_account: issuance_pda,
            user_nft_token_account: ctx.accounts.factory_nft_account.key(),
            user_payment_token_account: ctx.accounts.user_usdc_token_account.key(), // Direct to user for NFT redemption
            payment_mint_account: ctx.accounts.usdc_token_mint.key(),
            payment_feed_account: payment_feed_pda,
            nft_mint_account: nft_mint_key,
            nft_metadata_account,
            nft_master_edition_account,
            nft_collection_metadata_account,
            nft_issuance_vault_account: nft_issuance_vault_pda,
            nft_issuance_vault_token_account,
            payout_account: payout_pda,
            payout_token_account,
            token2022_program: spl_token_2022::id(),
            associated_token_program: ctx.accounts.associated_token_program.key(),
            token_program: ctx.accounts.token_program.key(),
            metadata_program: ctx.accounts.metadata_program.key(),
            system_program: ctx.accounts.system_program.key(),
        }
        .instruction();

        let nft_result = solana_program::program::invoke_signed(
            &redeem_bond_ix,
            &[
                ctx.accounts.factory.to_account_info(),
                ctx.accounts.bond_holding.to_account_info(),
                ctx.accounts.bond_token_mint.to_account_info(),
                ctx.accounts.factory_nft_account.to_account_info(),
                ctx.accounts.user_usdc_token_account.to_account_info(),
                ctx.accounts.usdc_token_mint.to_account_info(),
                ctx.accounts.nft_token_mint.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.token_2022_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
                ctx.accounts.metadata_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            factory_signer,
        );

        require!(nft_result.is_ok(), StablecoinError::NFTRedemptionFailed);

        // Transfer NFT from factory to user
        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.factory_nft_account.to_account_info(),
                    mint: ctx.accounts.nft_token_mint.to_account_info(),
                    to: ctx.accounts.user_nft_account.to_account_info(),
                    authority: ctx.accounts.factory.to_account_info(),
                },
                factory_signer,
            ),
            1,
            0
        )?;

        // Update sovereign coin state
        sovereign_coin.total_supply = sovereign_coin.total_supply.safe_sub(redeem_state.sovereign_amount)?;
        sovereign_coin.usdc_amount = sovereign_coin.usdc_amount.safe_sub(redeem_state.from_usdc_reserve)?;
        sovereign_coin.bond_amount = sovereign_coin.bond_amount.safe_sub(redeem_state.from_bond_redemption)?;

        // Verification
        ctx.accounts.user_sovereign_coin_account.reload()?;
        ctx.accounts.user_usdc_token_account.reload()?;

        require!(
            ctx.accounts.user_sovereign_coin_account.amount == 
            initial_user_sovereign_balance.safe_sub(redeem_state.sovereign_amount)?,
            StablecoinError::RedemptionVerificationFailed
        );

        require!(
            ctx.accounts.user_usdc_token_account.amount >= initial_user_usdc_balance,
            StablecoinError::InsufficientRedemptionPayout
        );

        emit_cpi!(SovereignCoinRedeemedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            sovereign_amount: redeem_state.sovereign_amount,
            usdc_amount: redeem_state.usdc_amount,
            from_usdc_reserve: redeem_state.from_usdc_reserve,
            from_protocol_vault: redeem_state.from_protocol_vault,
            from_bond_redemption: redeem_state.from_bond_redemption,
            protocol_fee: redeem_state.protocol_fee,
            timestamp: clock.unix_timestamp,
            redemption_type: RedemptionTypeState::NFTBondRedemption,
        });

        Ok(())
    }
}