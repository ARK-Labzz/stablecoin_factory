use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct InitializeFactory<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        init,
        payer = authority,
        space = 8 + Factory::INIT_SPACE,
        seeds = [b"factory"],
        bump
    )]
    pub factory: Box<Account<'info, Factory>>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

impl InitializeFactory<'_> {
    pub fn handler(
        ctx: Context<InitializeFactory>,
        bump: u8,
        min_usdc_reserve: u16,           // In bps (e.g., 2000 = 20%)
        bond_reserve_numerator: u8,       // 30 in 30/9
        bond_reserve_denominator: u8,     // 9 in 30/9
        yield_share_protocol: u16,        // In bps
        yield_share_issuer: u16,         // In bps
        yield_share_holders: u16,        // In bps
    ) -> Result<()> {
        let factory = &mut ctx.accounts.factory;
        
        require!(
            yield_share_protocol
                .checked_add(yield_share_issuer)
                .and_then(|sum| sum.checked_add(yield_share_holders))
                .ok_or(StablecoinError::MathOverflow)? == 10_000,
            StablecoinError::InvalidYieldDistribution
        );

        require!(
            min_usdc_reserve <= 10_000,
            StablecoinError::InvalidReservePercentage
        );

        require!(
            bond_reserve_denominator > 0,
            StablecoinError::InvalidBondReserveRatio
        );
        
       
        factory.bump = bump;
        factory.authority = ctx.accounts.authority.key();
        factory.treasury = ctx.accounts.authority.key();
        
        factory.total_sovereign_coins = 0;
        factory.total_supply_all_coins = 0;
        
        factory.bond_rating_ordinals = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        
        
        factory.min_usdc_reserve_percentage = min_usdc_reserve;
        factory.bond_reserve_numerator = bond_reserve_numerator;
        factory.bond_reserve_denominator = bond_reserve_denominator;
        factory.global_usdc_reserve = Pubkey::default();
        factory.global_usdc_account = Pubkey::default();
        
        
        factory.yield_share_protocol = yield_share_protocol;
        factory.yield_share_issuer = yield_share_issuer;
        factory.yield_share_holders = yield_share_holders;

        factory.payment_base_price_feed_account = Pubkey::default();
        factory.protocol_vault = Pubkey::default();
        factory.global_usdc_reserve = Pubkey::default();
        factory.global_usdc_account = Pubkey::default();
        factory.payment_quote_price_feed_account = None;
        
        factory.transfer_fee_bps = 0;
        factory.maximum_transfer_fee = 0;

       
        let clock = Clock::get()?;
        emit_cpi!(FactoryInitializedEvent {
            authority: ctx.accounts.authority.key(),
            factory: ctx.accounts.factory.key(),
            min_usdc_reserve,
            bond_reserve_numerator,
            bond_reserve_denominator,
            yield_share_protocol,
            yield_share_issuer,
            yield_share_holders,
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
}