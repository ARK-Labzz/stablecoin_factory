use super::*;

#[derive(Accounts)]
pub struct SetupInterestBearingMintWithTransferFee<'info> {
    #[account(mut)]
    pub payer: Signer<'info>, // Only signer needed

    #[account(
        mut,
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

    // The IBT mint with transfer fee
    #[account(mut)]
    pub mint: Signer<'info>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = factory,
    )]
    pub sovereign_coin_protocol_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl SetupInterestBearingMintWithTransferFee<'_> {
    pub fn handler(
        ctx: Context<Self>,
        initial_rate: i16,
        transfer_fee_basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        // Initialize mint with both extensions and factory as all authorities
        token_extension::initialize_ibt_mint_with_transfer_fee(
            &ctx.accounts.payer, // Payer pays for account creation
            &ctx.accounts.mint, // The mint being created
            &ctx.accounts.token_program,
            &ctx.accounts.system_program,
            &ctx.accounts.factory.to_account_info(), // Factory as authority
            initial_rate,
            transfer_fee_basis_points,
            maximum_fee,
            ctx.accounts.sovereign_coin.decimals,
        )?;

        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        sovereign_coin.mint = ctx.accounts.mint.key();
        sovereign_coin.interest_rate = initial_rate;
        sovereign_coin.is_interest_bearing = true;

        
        let factory = &mut ctx.accounts.factory;
        factory.protocol_vault = ctx.accounts.sovereign_coin_protocol_vault.key();
        factory.transfer_fee_bps = transfer_fee_basis_points;
        factory.maximum_transfer_fee = maximum_fee;

        
        let clock = Clock::get()?;
        emit!(SovereignCoinInterestBearingWithTransferFeeInitializedEvent {
            sovereign_coin: sovereign_coin.key(),
            mint: sovereign_coin.mint,
            interest_rate: initial_rate,
            transfer_fee_bps: transfer_fee_basis_points,
            maximum_fee,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}