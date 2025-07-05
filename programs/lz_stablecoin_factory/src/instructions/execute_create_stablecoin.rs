use super::*;

#[event_cpi]
#[derive(Accounts)]
pub struct FinalizeSetup<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        mut, 
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
        constraint = sovereign_coin.creator == creator.key()
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
    #[account(
        constraint = mint.key() == sovereign_coin.mint @ StablecoinError::InvalidSovereignCoinMint
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,
    
    /// CHECK: Will be created via CPI to token metadata program
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,
    
    pub token_metadata_program: Program<'info, Metadata>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

impl FinalizeSetup<'_> {
    pub fn handler(ctx: Context<Self>) -> Result<()> {
        let sovereign_coin = &ctx.accounts.sovereign_coin;
        
        require!(
            ctx.accounts.mint.mint_authority == Some(ctx.accounts.factory.key()).into(),
            StablecoinError::InvalidMintAuthority
        );
        
        let name = std::str::from_utf8(
            &sovereign_coin.name
                .iter()
                .take_while(|&&b| b != 0)
                .cloned()
                .collect::<Vec<u8>>()
        ).unwrap_or("").to_string();
        
        let symbol = std::str::from_utf8(
            &sovereign_coin.symbol
                .iter()
                .take_while(|&&b| b != 0)
                .cloned()
                .collect::<Vec<u8>>()
        ).unwrap_or("").to_string();
        
        let uri = std::str::from_utf8(
            &sovereign_coin.uri
                .iter()
                .take_while(|&&b| b != 0)
                .cloned()
                .collect::<Vec<u8>>()
        ).unwrap_or("").to_string();

        require!(
            !name.is_empty() && name.len() <= 32,
            StablecoinError::InvalidNameLength
        );
        require!(
            !symbol.is_empty() && symbol.len() <= 8,
            StablecoinError::InvalidSymbolLength
        );
        require!(
            uri.len() <= 200,
            StablecoinError::InvalidUriLength
        );
        
        let initial_count = ctx.accounts.factory.total_sovereign_coins;

        let factory_seeds = &[
            b"factory".as_ref(),
            &[ctx.accounts.factory.bump],
        ];
        let factory_signer = &[&factory_seeds[..]];
        
        let cpi_program = ctx.accounts.token_metadata_program.to_account_info();
        let cpi_accounts = CreateMetadataAccountsV3 {
            metadata: ctx.accounts.metadata.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            mint_authority: ctx.accounts.factory.to_account_info(),
            payer: ctx.accounts.creator.to_account_info(),
            update_authority: ctx.accounts.factory.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        };
        
        create_metadata_accounts_v3(
            CpiContext::new_with_signer(cpi_program, cpi_accounts, factory_signer),
            DataV2 {
                name: name.clone(),
                symbol: symbol.clone(),
                uri: uri.clone(),
                seller_fee_basis_points: 0,
                creators: None,
                collection: None,
                uses: None,
            },
            false,
            true,
            None,
        )?;
        
        let factory = &mut ctx.accounts.factory;
        factory.total_sovereign_coins = factory.total_sovereign_coins
            .checked_add(1)
            .ok_or(StablecoinError::ArithmeticOverflow)?;
        
        require!(
            factory.total_sovereign_coins == initial_count + 1,
            StablecoinError::StateUpdateFailed
        );
       
        let fiat_currency = std::str::from_utf8(
            &sovereign_coin.target_fiat_currency
                .iter()
                .take_while(|&&b| b != 0)
                .cloned()
                .collect::<Vec<u8>>()
        ).unwrap_or("").to_string();
        
        
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinCreatedEvent {
            creator: ctx.accounts.creator.key(),
            sovereign_coin: sovereign_coin.key(),
            mint: ctx.accounts.mint.key(),
            name,
            symbol,
            fiat_currency,
            bond_mint: sovereign_coin.bond_mint,
            bond_account: sovereign_coin.bond_account,
            bond_rating: sovereign_coin.bond_rating,
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
}
