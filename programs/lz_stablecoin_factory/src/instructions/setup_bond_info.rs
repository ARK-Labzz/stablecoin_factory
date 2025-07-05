use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct SetupBondInfo<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

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
        constraint = sovereign_coin.creator == creator.key()
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,

    pub bond_token_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: Bond account - validated by reading its data
    #[account(
        constraint = bond_account.key() == find_bond_pda(bond_token_mint.key()).0 @ StablecoinError::InvalidBondAccount
    )]
    pub bond_account: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub token_2022_program: Program<'info, Token2022>,
}

impl SetupBondInfo<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        // Reading the bond account data using the SDK's built-in parsing
        let bond_data = ctx.accounts.bond_account.try_borrow_data()?;
        let bond = Bond::from_bytes(&bond_data)
            .map_err(|_| StablecoinError::InvalidBondAccountData)?;

        require!(
            bond.mint == ctx.accounts.bond_token_mint.key(),
            StablecoinError::InvalidBondMint
        );

        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        sovereign_coin.bond_issuance_number = bond.issuance_number;
        
        sovereign_coin.set_payment_feed_type(bond.payment_feed_type)?;

        let clock = Clock::get()?;
        emit_cpi!(BondInfoEvent {
            sovereign_coin: sovereign_coin.key(),
            bond_issuance_number: sovereign_coin.bond_issuance_number,
            payment_feed_type: sovereign_coin.payment_feed_type, 
            bond_account: ctx.accounts.bond_account.key(),
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}