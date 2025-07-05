use anchor_lang::prelude::*;
use switchboard_on_demand::PullFeedAccountData;
use anchor_lang::system_program::{CreateAccount, create_account};
use anchor_lang::solana_program::{
    self, 
    // system_program, 
    msg
};
use anchor_spl::metadata::{
    create_metadata_accounts_v3, 
    mpl_token_metadata::types::DataV2, 
    CreateMetadataAccountsV3,
    Metadata,
};
use anchor_spl::{
    associated_token::{
        AssociatedToken, 
        spl_associated_token_account, 
        // get_associated_token_address_with_program_id, 
        get_associated_token_address
    },
    token_interface::{
        self, 
        Burn, 
        Mint, 
        MintTo, 
        TokenInterface, 
        TokenAccount, 
        TransferChecked, 
        TransferCheckedWithFee,
        spl_token_2022, 
        spl_pod::optional_keys::OptionalNonZeroPubkey, 
        transfer_checked_with_fee,
        transfer_checked, 
        transfer_fee_initialize,
        harvest_withheld_tokens_to_mint,
        withdraw_withheld_tokens_from_mint,
        HarvestWithheldTokensToMint,
        WithdrawWithheldTokensFromMint,
        Token2022, 
        TransferFeeInitialize,
        // TransferFeeSetTransferFee,
        InterestBearingMintInitialize,
        InterestBearingMintUpdateRate,
        interest_bearing_mint_initialize, 
        interest_bearing_mint_update_rate,
    },
    token::{self, spl_token},
    token_2022::{
        spl_token_2022::{
            extension::{
                interest_bearing_mint::InterestBearingConfig,
                transfer_fee::TransferFeeConfig, 
                BaseStateWithExtensions, 
                ExtensionType, 
                StateWithExtensions,
            },
            pod::PodMint,
            state::Mint as MintState,
        },
        initialize_mint2,
        InitializeMint2,
    },
};
use stablebond_sdk::{
    instructions::{
        PurchaseBondV2, 
        PurchaseBondV2InstructionArgs, 
        RedeemBond, 
        InstantBondRedemption, 
        InstantBondRedemptionInstructionArgs
    }, 
    types::PaymentFeedType, 
    accounts::Bond,
    find_bond_pda, 
    find_issuance_pda, 
    find_payment_pda, 
    find_kyc_pda, 
    find_payment_feed_pda, 
    find_nft_issuance_vault_pda,
    find_payout_pda,
    find_sell_liquidity_pda
};
// use spl_math::precise_number::PreciseNumber;
use mpl_token_metadata::accounts::{
    MasterEdition as MasterEditionMpl, 
    Metadata as MetadataMpl
};
use std::panic::Location;
use static_assertions::const_assert_eq;


pub mod error;
pub mod instructions;
pub mod state;
pub mod events;
pub mod constants;
pub mod math;

pub use error::StablecoinError;
pub use instructions::*;
pub use state::*;
pub use events::*;
pub use constants::*;
pub use math::*;

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "lz_stablecoin_factory",
    project_url: "https://stable.fun",
    contacts: "email:team@stable.fun",
    policy: "Build quality.",
    source_code: "https://github.com/donjne/stablecoin_factory",
    source_release: "v1.0.0",
    auditors: "CD Security",
    acknowledgements: "
    Etherfuse
    Neodyme
    Light Protocol
    Solana Labs
    Anza
    Metaplex
    Meteora
    MetaDAO
    "
}

declare_id!("4GEUx2ACQTHWAqn5VDB98z1LDpN1tzspqQhnhguTGbGK");
// declare_id!("HEpq3mrVzjWcBksSSVHWwQPWGDhhWJCbiA6AXMKCUBiN");

#[program]
pub mod lz_stablecoin_factory {
    use super::*;

    pub fn initialize_factory(
        ctx: Context<InitializeFactory>,
        min_usdc_reserve: u16,           
        bond_reserve_numerator: u8,       
        bond_reserve_denominator: u8,    
        yield_share_protocol: u16,       
        yield_share_issuer: u16,         
        yield_share_holders: u16,
    ) -> Result<()> {
        let bump = ctx.bumps.factory;
        
        InitializeFactory::handler(
            ctx,
            bump,
            min_usdc_reserve,
            bond_reserve_numerator,
            bond_reserve_denominator,
            yield_share_protocol,
            yield_share_issuer,
            yield_share_holders,
        )
    }

    pub fn register_bond_maps(
        ctx: Context<RegisterBondMapping>, 
        fiat_currency: String, 
        bond_mint: Pubkey, 
        bond_rating: u8, 
    ) -> Result<()> {
        RegisterBondMapping::handler(ctx, fiat_currency, bond_mint, bond_rating)
    }

    // pub fn preview_exchange(
    //     ctx: Context<PreviewExchange>,
    //     args: PreviewExchangeArgs,
    // ) -> Result<PreviewExchangeResult> {
    //     PreviewExchange::handler(ctx, args)
    // }

    pub fn update_interest_rate(
        ctx: Context<UpdateInterestRate>,
        manual_rate: Option<i16>,
    ) -> Result<()> {
        handle_update_interest_rate(ctx, manual_rate)
    }

    pub fn create_fee_operator(ctx: Context<CreateFeeOperatorCtx>) -> Result<()> {
        handle_create_fee_operator(ctx)
    }
    
    pub fn close_fee_operator(ctx: Context<CloseFeeOperatorCtx>) -> Result<()> {
        handle_close_fee_operator(ctx)
    }
    
    pub fn harvest_fees<'info>(ctx: Context<'_, '_, 'info, 'info, HarvestFees<'info>>) -> Result<()> {
        handle_harvest_fees(ctx)
    }
    
    pub fn withdraw_fees(ctx: Context<WithdrawFees>) -> Result<()> {
        handle_withdraw_fees(ctx)
    }
    
    pub fn update_transfer_fee(
        ctx: Context<UpdateTransferFee>,
        transfer_fee_basis_points: u16,
        maximum_fee: u64,
    ) -> Result<()> {
        handle_update_transfer_fee(ctx, transfer_fee_basis_points, maximum_fee)
    }
    
    pub fn withdraw_from_protocol_account(
        ctx: Context<WithdrawFromProtocolAccount>,
        amount: u64,
    ) -> Result<()> {
        handle_withdraw_from_protocol_account(ctx, amount)
    }

        pub fn withdraw_from_sovereign_coin_account(
        ctx: Context<WithdrawFromSovereignCoinProtocolVault>,
        amount: u64,
    ) -> Result<()> {
        handle_withdraw_from_sovereign_coin_account(ctx, amount)
    }

    /// Initialize the sovereign coin account
    #[access_control(InitSovereignCoin::validate(&ctx.accounts, &args))]
    pub fn init_sovereign_coin(ctx: Context<InitSovereignCoin>, args: SovereignCoinArgs) -> Result<()> {
        InitSovereignCoin::handler(ctx, args)
    }

    pub fn setup_bond_info(ctx: Context<SetupBondInfo>) -> Result<()> {
        SetupBondInfo::handler(ctx)
    }

    pub fn setup_ibt_with_transfer_fee(ctx: Context<SetupInterestBearingMintWithTransferFee>, initial_rate: i16, transfer_fee_basis_points: u16, maximum_fee: u64,) -> Result<()> {
        SetupInterestBearingMintWithTransferFee::handler(ctx, initial_rate, transfer_fee_basis_points, maximum_fee)
    }

    pub fn setup_usdc_accounts(ctx: Context<SetupGlobalUsdcAccounts>) -> Result<()> {
        SetupGlobalUsdcAccounts::handler(ctx)
    }

    pub fn setup_bond_holding(ctx: Context<SetupBondHolding>) -> Result<()> {
        SetupBondHolding::handler(ctx)
    }

    pub fn finalize_setup(ctx: Context<FinalizeSetup>) -> Result<()> {
        FinalizeSetup::handler(ctx)
    }

    pub fn initialize_mint_sovereign_coin(ctx: Context<InitializeMintSovereignCoin>, args: InitializeMintSovereignCoinArgs) -> Result<()> {
        InitializeMintSovereignCoin::handler(ctx, args)
    }

    pub fn execute_mint_sovereign_coin(ctx: Context<ExecuteMintSovereignCoin>) -> Result<()> {
        ExecuteMintSovereignCoin::handler(ctx)
    }

    pub fn initialize_redeem_sovereign_coin(ctx: Context<InitializeRedeemStablecoin>, args: InitializeRedeemStablecoinArgs) -> Result<()> {
        InitializeRedeemStablecoin::handler(ctx, args)
    }

    pub fn execute_redeem_from_fiat(ctx: Context<ExecuteRedeemFromFiat>) -> Result<()> {
        ExecuteRedeemFromFiat::handler(ctx)
    }

    pub fn execute_redeem_from_fiat_and_protocol(ctx: Context<ExecuteRedeemFromFiatAndProtocol>) -> Result<()> {
        ExecuteRedeemFromFiatAndProtocol::handler(ctx)
    }

    pub fn execute_instant_redeem(ctx: Context<ExecuteInstantRedemption>) -> Result<()> {
        ExecuteInstantRedemption::handler(ctx)
    }

    pub fn initialize_nft_redemption(ctx: Context<InitializeNFTRedemption>) -> Result<()> {
        InitializeNFTRedemption::handler(ctx)
    }

    pub fn complete_nft_redemption(ctx: Context<CompleteNFTRedemption>) -> Result<()> {
        CompleteNFTRedemption::handler(ctx)
    }

    pub fn update_price_feed(ctx: Context<UpdatePriceFeeds>, args: PriceFeedsArgs) -> Result<()> {
        UpdatePriceFeeds::handler(ctx, args)
    }
}

