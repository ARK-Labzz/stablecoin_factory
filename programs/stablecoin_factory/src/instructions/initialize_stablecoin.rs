use super::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SovereignCoinArgs {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub fiat_currency: String,
}

#[event_cpi]
#[derive(Accounts)]
#[instruction(args: SovereignCoinArgs)]
pub struct InitSovereignCoin<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub authority: Signer<'info>,
    
    #[account(
        seeds = [b"factory"],
        bump = factory.bump,
    )]
    pub factory: Box<Account<'info, Factory>>,
    
    #[account(
        init,
        payer = payer,
        space = 8 + SovereignCoin::INIT_SPACE,
        seeds = [b"sovereign_coin", authority.key().as_ref(), args.symbol.as_bytes()],
        bump
    )]
    pub sovereign_coin: Box<Account<'info, SovereignCoin>>,
    
   
    pub fiat_token_mint: Box<InterfaceAccount<'info, Mint>>,
    pub bond_token_mint: Box<InterfaceAccount<'info, Mint>>,
    
    
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

impl InitSovereignCoin<'_> {
    pub fn validate(&self, args: &SovereignCoinArgs) -> Result<()> {
        
        require!(args.name.len() <= 32, StablecoinError::NameTooLong);
        require!(args.symbol.len() <= 8, StablecoinError::SymbolTooLong);
        require!(args.uri.len() <= 200, StablecoinError::UriTooLong);
        require!(args.fiat_currency.len() <= 8, StablecoinError::FiatCurrencyTooLong);
        require!(args.fiat_currency.len() > 0, StablecoinError::InvalidFiatCurrency);
        
        
        let factory = &self.factory;
        let fiat_bytes = args.fiat_currency.as_bytes();
        let mut mapping_found = false;
        
        for i in 0..factory.bond_mappings_count as usize {
            let mapping = &factory.bond_mappings[i];
            if mapping.active {
                let mapping_currency = &mapping.fiat_currency;
                
                let mapping_len = mapping_currency.iter().take_while(|&&b| b != 0).count();
                if mapping_len == fiat_bytes.len() {
                    let mapping_prefix = &mapping_currency[..mapping_len];
                    if mapping_prefix == fiat_bytes {
                        mapping_found = true;
                        break;
                    }
                }
            }
        }
        
        require!(mapping_found, StablecoinError::NoBondMappingForCurrency);
        
        Ok(())
    }
    
    pub fn handler(ctx: Context<Self>, args: SovereignCoinArgs) -> Result<()> {
        
        Self::validate(&ctx.accounts, &args)?;
        
        
        let sovereign_coin = &mut ctx.accounts.sovereign_coin;
        sovereign_coin.bump = ctx.bumps.sovereign_coin;
        sovereign_coin.authority = ctx.accounts.authority.key();
        sovereign_coin.factory = ctx.accounts.factory.key();
        
       
        let name_bytes = args.name.as_bytes();
        let symbol_bytes = args.symbol.as_bytes();
        
        sovereign_coin.name = [0u8; 32];
        sovereign_coin.symbol = [0u8; 8];
        
        sovereign_coin.name[..name_bytes.len()].copy_from_slice(name_bytes);
        sovereign_coin.symbol[..symbol_bytes.len()].copy_from_slice(symbol_bytes);
        
        
        let uri_bytes = args.uri.as_bytes();
        sovereign_coin.uri = [0u8; 200];
        sovereign_coin.uri[..uri_bytes.len()].copy_from_slice(uri_bytes);

       
        let fiat_bytes = args.fiat_currency.as_bytes();
        sovereign_coin.target_fiat_currency = [0u8; 8];
        sovereign_coin.target_fiat_currency[..fiat_bytes.len()].copy_from_slice(fiat_bytes);
        
        
        let selected_mapping = {
            let factory = &ctx.accounts.factory;
            let fiat_bytes = args.fiat_currency.as_bytes();
            let mut selected: Option<&BondCurrencyMapping> = None;
            
            for i in 0..factory.bond_mappings_count as usize {
                let mapping = &factory.bond_mappings[i];
                if mapping.active {
                    let mapping_currency = &mapping.fiat_currency;
                    let mapping_len = mapping_currency.iter().take_while(|&&b| b != 0).count();
                    if mapping_len == fiat_bytes.len() {
                        let mapping_prefix = &mapping_currency[..mapping_len];
                        if mapping_prefix == fiat_bytes {
                            selected = Some(mapping);
                            break;
                        }
                    }
                }
            }
            
            selected.map(|m| m.clone()).ok_or(StablecoinError::NoBondMappingForCurrency)?
        };
        
       
        require!(
            ctx.accounts.bond_token_mint.key() == selected_mapping.bond_mint,
            StablecoinError::InvalidBondMint
        );
        
        
        let (bond_account, _) = find_bond_pda(selected_mapping.bond_mint);
        sovereign_coin.bond_mint = selected_mapping.bond_mint;
        sovereign_coin.bond_account = bond_account;
        sovereign_coin.bond_rating = selected_mapping.bond_rating;
        
        
        sovereign_coin.required_reserve_percentage = calculate_required_reserve(
            ctx.accounts.factory.min_fiat_reserve_percentage,
            selected_mapping.bond_rating,
            ctx.accounts.factory.bond_reserve_numerator,
            ctx.accounts.factory.bond_reserve_denominator
        )?
        .try_into()
        .map_err(|_| StablecoinError::ReservePercentageOverflow)?;
        
        
        sovereign_coin.decimals = 6;
        sovereign_coin.total_supply = 0;
        sovereign_coin.fiat_amount = 0;
        sovereign_coin.bond_amount = 0;
        sovereign_coin.interest_rate = 0;
        sovereign_coin.is_interest_bearing = false;
        sovereign_coin.is_compressed = false;
        sovereign_coin.merkle_tree = None;

        
        let clock = Clock::get()?;
        emit_cpi!(SovereignCoinInitializedEvent {
            authority: ctx.accounts.authority.key(),
            sovereign_coin: sovereign_coin.key(),
            name: args.name,
            symbol: args.symbol,
            fiat_currency: args.fiat_currency,
            bond_mint: sovereign_coin.bond_mint,
            bond_account: sovereign_coin.bond_account,
            bond_rating: sovereign_coin.bond_rating,
            decimals: sovereign_coin.decimals,
            total_supply: sovereign_coin.total_supply,
            required_reserve_percentage: sovereign_coin.required_reserve_percentage,
            fiat_amount: sovereign_coin.fiat_amount,
            bond_amount: sovereign_coin.bond_amount,
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
}