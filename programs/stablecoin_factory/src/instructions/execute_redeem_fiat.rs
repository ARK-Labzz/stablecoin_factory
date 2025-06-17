use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteRedeemFromFiat<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

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

    // Load our state from the previous instruction
    #[account(
        mut,
        seeds = [b"redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = redeem_state.bump,
        constraint = redeem_state.authority == authority.key() && redeem_state.payer == payer.key(),
        constraint = redeem_state.redemption_type == RedemptionTypeState::FiatReserveOnly,
        close = payer 
    )]
    pub redeem_state: Account<'info, RedeemSovereignState>,
    
    // The user's sovereign coin account (will be debited)
    #[account(
        mut,
        token::mint = sovereign_coin_mint,
        token::authority = payer,
    )]
    pub user_sovereign_coin_account: InterfaceAccount<'info, TokenAccount>,
    
    // The user's USDC account (will be credited)
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = payer,
    )]
    pub user_fiat_token_account: InterfaceAccount<'info, TokenAccount>,

    pub sovereign_coin_mint: InterfaceAccount<'info, Mint>,

    // Our fiat reserve for USDC stablecoins (to send funds from)
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = authority,
    )]
    pub fiat_reserve: InterfaceAccount<'info, TokenAccount>,
    
    pub fiat_token_mint: InterfaceAccount<'info, Mint>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl ExecuteRedeemFromFiat<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        let redeem_state = &ctx.accounts.redeem_state;
        
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.sovereign_coin_mint.to_account_info(),
                    from: ctx.accounts.user_sovereign_coin_account.to_account_info(),
                    authority: ctx.accounts.payer.to_account_info(),
                },
            ),
            redeem_state.sovereign_amount,
        )?;
    
        token_extension::transfer_with_fee(
            &ctx.accounts.token_2022_program,
            &ctx.accounts.fiat_reserve.to_account_info(),
            &ctx.accounts.fiat_token_mint.to_account_info(),
            &ctx.accounts.user_fiat_token_account.to_account_info(),
            &ctx.accounts.authority,
            redeem_state.from_fiat_reserve,
            ctx.accounts.fiat_token_mint.decimals,
        )?;
        
        sovereign_coin.total_supply = sovereign_coin.total_supply
            .safe_sub(redeem_state.sovereign_amount)?; 
        sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
            .safe_sub(redeem_state.from_fiat_reserve)?;
    
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinRedeemedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            sovereign_amount: redeem_state.sovereign_amount,
            usdc_amount: redeem_state.usdc_amount,
            from_fiat_reserve: redeem_state.from_fiat_reserve,
            from_protocol_vault: 0,
            from_bond_redemption: 0,
            protocol_fee: redeem_state.protocol_fee,
            timestamp: clock.unix_timestamp,
            redemption_type: RedemptionTypeState::FiatReserveOnly,
        });
    
        Ok(())
    }
}