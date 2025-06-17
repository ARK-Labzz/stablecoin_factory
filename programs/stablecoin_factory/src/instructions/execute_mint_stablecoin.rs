use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteMintSovereignCoin<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

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
            authority.key().as_ref(), 
            &sovereign_coin.symbol[..sovereign_coin.symbol.iter().position(|&x| x == 0).unwrap_or(8)]
        ],
        bump = sovereign_coin.bump,
        constraint = sovereign_coin.authority == authority.key()
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    #[account(
        mut,
        seeds = [b"mint_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = mint_state.bump,
        constraint = mint_state.authority == authority.key() && mint_state.payer == payer.key(),
        close = payer 
    )]
    pub mint_state: Box<Account<'info, MintSovereignState>>,

    // User's source USDC account (to pay from)
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = payer,
    )]
    pub user_fiat_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    // User's sovereign coin account (to receive)
    #[account(
        init_if_needed,
        payer = authority,
        token::mint = sovereign_coin_mint,
        token::authority = payer,
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub sovereign_coin_mint: Box<InterfaceAccount<'info, Mint>>,

    // Protocol's fiat reserve
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = authority,
    )]
    pub fiat_reserve: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // Protocol's bond holding
    #[account(
        mut,
        token::mint = bond_token_mint,
        token::authority = authority,
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,
    
    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Oracle account
    #[account(
        constraint = payment_base_price_feed_account.key() == factory.payment_base_price_feed_account @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_base_price_feed_account: UncheckedAccount<'info>,

    /// CHECK: Quote oracle account
    #[account(
        constraint = payment_quote_price_feed_account.is_none() || 
                   payment_quote_price_feed_account.as_ref().unwrap().key() == 
                   factory.payment_quote_price_feed_account.expect("No price feed configured") 
                   @ StablecoinError::InvalidPriceFeed
    )]
    pub payment_quote_price_feed_account: Option<UncheckedAccount<'info>>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl ExecuteMintSovereignCoin<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let mint_state = &ctx.accounts.mint_state;
        let previous_balance = ctx.accounts.user_sovereign_coin_account.amount;

        if mint_state.reserve_amount > 0 {
            token_extension::transfer_with_fee(
                &ctx.accounts.token_2022_program,
                &ctx.accounts.user_fiat_token_account.to_account_info(),
                &ctx.accounts.fiat_token_mint.to_account_info(),
                &ctx.accounts.fiat_reserve.to_account_info(),
                &ctx.accounts.payer,
                mint_state.reserve_amount,
                ctx.accounts.fiat_token_mint.decimals,
            )?;
        }

        let fiat_currency = std::str::from_utf8(
            &sovereign_coin.target_fiat_currency
                .iter()
                .take_while(|&&b| b != 0)
                .cloned()
                .collect::<Vec<u8>>()
        ).unwrap_or("USD").to_string();

        let bond_amount_in_tokens = oracle::calculate_bond_equivalent(
            mint_state.bond_amount,
            &ctx.accounts.payment_base_price_feed_account.to_account_info(),
            ctx.accounts.payment_quote_price_feed_account.as_ref()
                .map(|acc| &acc.to_account_info()),
            &fiat_currency,
            ctx.accounts.bond_token_mint.decimals,
        )?;

        let issuance = find_issuance_pda(ctx.accounts.bond_holding.key(), 1).0;
        let payment_account = find_payment_pda(issuance).0;
        let payment_token_account = get_associated_token_address(&payment_account, &ctx.accounts.fiat_token_mint.key());
        
        let user_bond_token_account = get_associated_token_address_with_program_id(
            &ctx.accounts.payer.key(),
            &ctx.accounts.bond_token_mint.key(),
            &spl_token_2022::id(),
        );

        let factory_seeds = &[
            b"factory".as_ref(),
            &[ctx.accounts.factory.bump],
        ];
        let factory_signer = &[&factory_seeds[..]];

        let purchase_bond_ix = PurchaseBondV2 {
            user_wallet: ctx.accounts.payer.key(),
            user_token_account: user_bond_token_account,
            user_payment_token_account: ctx.accounts.user_fiat_token_account.to_account_info().key(),
            bond_account: ctx.accounts.bond_holding.to_account_info().key(),
            issuance_account: issuance,
            payment_account,
            payment_token_account,
            kyc_account: find_kyc_pda(ctx.accounts.factory.key()).0,
            mint_account: ctx.accounts.bond_token_mint.key(),
            payment_mint_account: ctx.accounts.fiat_token_mint.key(),
            payment_feed_account: find_payment_feed_pda(PaymentFeedType::Stub).0,
            payment_base_price_feed_account: ctx.accounts.factory.payment_base_price_feed_account,
            payment_quote_price_feed_account: ctx.accounts.factory.payment_quote_price_feed_account,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            token2022_program: spl_token_2022::id(),
            system_program: solana_program::system_program::id(),
        }
        .instruction(PurchaseBondV2InstructionArgs {
            amount: mint_state.bond_amount, 
        });

        solana_program::program::invoke_signed(
            &purchase_bond_ix,
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.user_fiat_token_account.to_account_info(),
                ctx.accounts.bond_holding.to_account_info(),
                ctx.accounts.factory.to_account_info(),
                ctx.accounts.bond_token_mint.to_account_info(),
                ctx.accounts.fiat_token_mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            factory_signer,
        )?;

        token_interface::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.sovereign_coin_mint.to_account_info(),
                    to: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            mint_state.sovereign_amount,
        )?;

        sovereign_coin.total_supply = sovereign_coin.total_supply
            .safe_add(mint_state.sovereign_amount)?;
        sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
            .safe_add(mint_state.reserve_amount)?;
        sovereign_coin.bond_amount = sovereign_coin.bond_amount
            .safe_add(mint_state.bond_amount)?; 

        ctx.accounts.user_sovereign_coin_account.reload()?;
        require!(
            ctx.accounts.user_sovereign_coin_account.amount == previous_balance.safe_add(mint_state.sovereign_amount)?,
            StablecoinError::MintVerificationFailed
        );

        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinMintedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            usdc_amount: mint_state.usdc_amount,
            sovereign_coin_amount: mint_state.sovereign_amount,
            reserve_amount: mint_state.reserve_amount,
            bond_amount: mint_state.bond_amount,
            protocol_fee: mint_state.protocol_fee,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}

