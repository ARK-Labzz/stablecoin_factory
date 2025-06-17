use super::*;

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface, Token2022};
use crate::{
    errors::StablecoinError,
    state::{Factory, SovereignCoin, RedeemSovereignState, RedemptionTypeState},
    events::SovereignCoinRedeemedEvent,
    math::{safe_math::SafeMath, token_extension, oracle},
};

#[event_cpi]
#[derive(Accounts)]
pub struct ExecuteRedeemFromFiatAndProtocol<'info> {
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

    // Load our state from the previous instruction
    #[account(
        mut,
        seeds = [b"redeem_state", payer.key().as_ref(), sovereign_coin.key().as_ref()],
        bump = redeem_state.bump,
        constraint = redeem_state.authority == authority.key() && redeem_state.payer == payer.key(),
        constraint = redeem_state.redemption_type == RedemptionTypeState::FiatAndProtocol,
        close = payer // Close the account after use to reclaim rent
    )]
    pub redeem_state: Box<Account<'info, RedeemSovereignState>>,
    
    // The user's sovereign coin account (will be debited)
    #[account(
        mut,
        token::mint = sovereign_coin_mint,
        token::authority = payer,
    )]
    pub user_sovereign_coin_account: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // The user's USDC account (will be credited)
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = payer,
    )]
    pub user_fiat_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub sovereign_coin_mint: Box<InterfaceAccount<'info, Mint>>,

    // Protocol vault
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = factory,
        constraint = protocol_vault.key() == factory.protocol_vault @ StablecoinError::InvalidProtocolVault
    )]
    pub protocol_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    // Fiat reserve 
    #[account(
        mut,
        token::mint = fiat_token_mint,
        token::authority = authority,
    )]
    pub fiat_reserve: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // Bond holding account
    #[account(
        mut,
        token::mint = bond_token_mint,
        token::authority = authority,
    )]
    pub bond_holding: Box<InterfaceAccount<'info, TokenAccount>>,
    
    // User's bond account
    #[account(
        init_if_needed,
        payer = payer,
        token::mint = bond_token_mint,
        token::authority = payer,
    )]
    pub bond_ownership: Box<InterfaceAccount<'info, TokenAccount>>,
    
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

impl ExecuteRedeemFromFiatAndProtocol<'_> {
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
    
        if redeem_state.from_fiat_reserve > 0 {
            token_extension::transfer_with_fee(
                &ctx.accounts.token_2022_program,
                &ctx.accounts.fiat_reserve.to_account_info(),
                &ctx.accounts.fiat_token_mint.to_account_info(),
                &ctx.accounts.user_fiat_token_account.to_account_info(),
                &ctx.accounts.authority,
                redeem_state.from_fiat_reserve,
                ctx.accounts.fiat_token_mint.decimals,
            )?;
        }
    
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
                        from: ctx.accounts.protocol_vault.to_account_info(),
                        mint: ctx.accounts.fiat_token_mint.to_account_info(),
                        to: ctx.accounts.user_fiat_token_account.to_account_info(),
                        authority: ctx.accounts.factory.to_account_info(),
                    },
                    factory_signer,
                ),
                redeem_state.from_protocol_vault,
                ctx.accounts.fiat_token_mint.decimals,
            )?;

            let bond_amount = conversion::calculate_bond_amount(
                redeem_state.from_protocol_vault, 
                &ctx.accounts.payment_base_price_feed_account.to_account_info(),
                ctx.accounts.payment_quote_price_feed_account.as_ref()
                    .map(|acc| &acc.to_account_info()),
                &fiat_currency,
                ctx.accounts.bond_token_mint.decimals,
            )?;

            token_interface::transfer_checked(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.bond_holding.to_account_info(),
                        mint: ctx.accounts.bond_token_mint.to_account_info(),
                        to: ctx.accounts.bond_ownership.to_account_info(),
                        authority: ctx.accounts.authority.to_account_info(),
                    },
                ),
                bond_amount, 
                ctx.accounts.bond_token_mint.decimals,
            )?;
        }
    
        sovereign_coin.total_supply = sovereign_coin.total_supply
            .safe_sub(redeem_state.sovereign_amount)?;
        sovereign_coin.fiat_amount = sovereign_coin.fiat_amount
            .safe_sub(redeem_state.from_fiat_reserve)?;
        // We don't update bond_amount since we're just transferring ownership
    
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinRedeemedEvent {
            payer: ctx.accounts.payer.key(),
            sovereign_coin: ctx.accounts.sovereign_coin.key(),
            sovereign_amount: redeem_state.sovereign_amount,
            usdc_amount: redeem_state.usdc_amount,
            from_fiat_reserve: redeem_state.from_fiat_reserve,
            from_protocol_vault: redeem_state.from_protocol_vault,
            from_bond_redemption: 0,
            protocol_fee: redeem_state.protocol_fee,
            timestamp: clock.unix_timestamp,
            redemption_type: RedemptionTypeState::FiatAndProtocol,
        });
    
        Ok(())
    }
}