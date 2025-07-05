use super::*;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PreviewExchangeArgs {
    pub usdc_amount: Option<u64>,      // User input in USDC field
    pub sovereign_amount: Option<u64>,  // User input in sovereign field
    pub bundle: Option<Vec<u8>>,       // Optional bundle for on-demand verification
    pub base_feed_hash: Option<[u8; 32]>, // Feed hash for bundle approach
    pub quote_feed_hash: Option<[u8; 32]>, // Quote feed hash for bundle approach
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PreviewExchangeResult {
    pub usdc_amount: u64,
    pub sovereign_amount: u64,
}

#[derive(Accounts)]
pub struct PreviewExchange<'info> {
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
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    /// CHECK: Oracle account for base price (for pull feed approach)
    pub payment_base_price_feed_account: Option<UncheckedAccount<'info>>,
    
    /// CHECK: Oracle account for quote price (optional, for pull feed approach)
    pub payment_quote_price_feed_account: Option<UncheckedAccount<'info>>,
    
    /// Sysvar for instructions (for bundle approach)
    pub instructions: Option<Sysvar<'info, Instructions>>,
}

impl PreviewExchange<'_> {
    pub fn handler(
        ctx: Context<PreviewExchange>,
        args: PreviewExchangeArgs,
    ) -> Result<PreviewExchangeResult> {
        let sovereign_coin = &ctx.accounts.sovereign_coin;
        let target_currency = std::str::from_utf8(&sovereign_coin.target_fiat_currency)?;
        
        // Determine which approach to use based on provided accounts
        let use_bundle_approach = ctx.accounts.queue.is_some() && 
                                 ctx.accounts.slothashes.is_some() && 
                                 ctx.accounts.instructions.is_some() &&
                                 args.bundle.is_some();
        
        // Automatically detect which field has input
        match (args.usdc_amount, args.sovereign_amount) {
            (Some(usdc_amount), None) => {
                // User input USDC, calculate sovereign
                let sovereign_amount = if use_bundle_approach {
                    // Use bundle verification approach (recommended)
                    conversion::calculate_sovereign_coin_amount_from_bundle(
                        usdc_amount,
                        &ctx.accounts.queue.as_ref().unwrap().to_account_info(),
                        &ctx.accounts.slothashes.as_ref().unwrap().to_account_info(),
                        &ctx.accounts.instructions.as_ref().unwrap().to_account_info(),
                        args.bundle.as_ref().unwrap(),
                        args.base_feed_hash.as_ref().map(|h| h.as_ref()),
                        args.quote_feed_hash.as_ref().map(|h| h.as_ref()),
                        target_currency,
                        sovereign_coin.decimals,
                    )?
                } else {
                    // Use pull feed approach (fallback)
                    conversion::calculate_sovereign_coin_amount(
                        usdc_amount,
                        &ctx.accounts.payment_base_price_feed_account
                            .as_ref()
                            .ok_or(StablecoinError::InvalidPriceFeed)?
                            .to_account_info(),
                        ctx.accounts.payment_quote_price_feed_account.as_ref()
                            .map(|acc| &acc.to_account_info()),
                        target_currency,
                        sovereign_coin.decimals,
                    )?
                };
                
                Ok(PreviewExchangeResult {
                    usdc_amount,
                    sovereign_amount,
                })
            },
            (None, Some(sovereign_amount)) => {
                // User input sovereign, calculate USDC
                let usdc_amount = if use_bundle_approach {
                    // Use bundle verification approach (recommended)
                    conversion::calculate_usdc_for_sovereign_amount_from_bundle(
                        sovereign_amount,
                        &ctx.accounts.queue.as_ref().unwrap().to_account_info(),
                        &ctx.accounts.slothashes.as_ref().unwrap().to_account_info(),
                        &ctx.accounts.instructions.as_ref().unwrap().to_account_info(),
                        args.bundle.as_ref().unwrap(),
                        args.base_feed_hash.as_ref().map(|h| h.as_ref()),
                        args.quote_feed_hash.as_ref().map(|h| h.as_ref()),
                        target_currency,
                        sovereign_coin.decimals,
                    )?
                } else {
                    // Use pull feed approach (fallback)
                    conversion::calculate_usdc_for_sovereign_amount(
                        sovereign_amount,
                        &ctx.accounts.payment_base_price_feed_account
                            .as_ref()
                            .ok_or(StablecoinError::InvalidPriceFeed)?
                            .to_account_info(),
                        ctx.accounts.payment_quote_price_feed_account.as_ref()
                            .map(|acc| &acc.to_account_info()),
                        target_currency,
                        sovereign_coin.decimals,
                    )?
                };
                
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