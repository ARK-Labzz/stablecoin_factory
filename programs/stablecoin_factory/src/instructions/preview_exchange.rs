use super::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PreviewExchangeArgs {
    pub usdc_amount: Option<u64>,      // User input in USDC field
    pub sovereign_amount: Option<u64>,  // User input in sovereign field
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PreviewExchangeResult {
    pub usdc_amount: u64,
    pub sovereign_amount: u64,
}

#[derive(Accounts)]
pub struct PreviewExchange<'info> {
    pub sovereign_coin: Account<'info, SovereignCoin>,
    /// CHECK: Oracle account for base price
    pub payment_base_price_feed_account: UncheckedAccount<'info>,
    /// CHECK: Oracle account for quote price (optional)
    pub payment_quote_price_feed_account: Option<UncheckedAccount<'info>>,
}

impl PreviewExchange<'_> {
    pub fn handler(
        ctx: Context<PreviewExchange>,
        args: PreviewExchangeArgs,
    ) -> Result<PreviewExchangeResult> {
    let sovereign_coin = &ctx.accounts.sovereign_coin;
    let target_currency = std::str::from_utf8(&sovereign_coin.target_fiat_currency)?;
    
    // Automatically detect which field has input
    match (args.usdc_amount, args.sovereign_amount) {
        (Some(usdc_amount), None) => {
            // User input USDC, calculate sovereign
            let sovereign_amount = conversion::calculate_sovereign_coin_amount(
                usdc_amount,
                &ctx.accounts.payment_base_price_feed_account,
                ctx.accounts.payment_quote_price_feed_account.as_ref()
                    .map(|acc| &acc.to_account_info()),
                target_currency,
                sovereign_coin.decimals,
            )?;
            
            Ok(PreviewExchangeResult {
                usdc_amount,
                sovereign_amount,
            })
        },
        (None, Some(sovereign_amount)) => {
            // User input sovereign, calculate USDC
            let usdc_amount = conversion::calculate_usdc_for_sovereign_amount(
                sovereign_amount,
                &ctx.accounts.payment_base_price_feed_account,
                ctx.accounts.payment_quote_price_feed_account.as_ref()
                    .map(|acc| &acc.to_account_info()),
                target_currency,
                sovereign_coin.decimals,
            )?;
            
            Ok(PreviewExchangeResult {
                usdc_amount,
                sovereign_amount,
            })
        },
        (Some(_), Some(_)) => {
            // Both fields have values - error
            Err(StablecoinError::InvalidPreviewInput.into())
        },
        (None, None) => {
            // No input provided - return zeros
            Ok(PreviewExchangeResult {
                usdc_amount: 0,
                sovereign_amount: 0,
            })
        },
    }
    }
}