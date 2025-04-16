use super::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PriceFeedsArgs {
    pub base_price_feed: Pubkey,
    pub quote_price_feed: Option<Pubkey>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct UpdatePriceFeeds<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"factory"],
        bump = factory.bump,
        constraint = factory.authority == authority.key() @ StablecoinError::Unauthorized
    )]
    pub factory: Box<Account<'info, Factory>>,
}

impl UpdatePriceFeeds<'_> {
    pub fn handler(
        ctx: Context<UpdatePriceFeeds>,
        args: PriceFeedsArgs
    ) -> Result<()> {
        let factory = &mut ctx.accounts.factory;
        
        factory.payment_base_price_feed_account = args.base_price_feed;
        factory.payment_quote_price_feed_account = args.quote_price_feed;
        
        let clock = Clock::get()?;
        emit_cpi!(PriceFeedsUpdatedEvent {
            authority: ctx.accounts.authority.key(),
            factory: factory.key(),
            base_price_feed: args.base_price_feed,
            quote_price_feed: args.quote_price_feed,
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
}