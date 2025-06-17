use super::*;

#[error_code]
pub enum StablecoinError {
    #[msg("An assertion failed")]
    AssertFailed,
    #[msg("Invalid yield distribution. Must sum to 100%")]
    InvalidYieldDistribution,
    #[msg("Please enter a valid amount greater than zero")]
    InvalidAmount,
    #[msg("The amount calculated does not equal the original amount")]
    InvalidCalculatedAmount,
    #[msg("Invalid reserve percentage. Must sum to 100%")]
    InvalidReservePercentage,
    #[msg("Miscalculation")]
    MathError,
    #[msg("The bond reserve ratio is invalid")]
    InvalidBondReserveRatio,
    #[msg("Reserve exceeds 100%")]
    ReserveExceeds100Percent,
    #[msg("Math Overflow Error")]
    MathOverflow,
    #[msg("The mint final verification did not pass")]
    MintVerificationFailed,
    #[msg("Reserve percentage overflow occurred")]
    ReservePercentageOverflow,
    #[msg("Bump not found")]
    BumpNotFound,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Name is too long")]
    NameTooLong,
    #[msg("Symbol is too long")]
    SymbolTooLong,
    #[msg("URI is too long")]
    UriTooLong,
    #[msg("There has been an arithmetic overflow error")]
    ArithmeticOverflow,
    #[msg("Bond Rating is Invalid")]
    InvalidBondRating,
    #[msg("USDC mint is invalid")]
    InvalidUSDCMint,
    #[msg("Fiat currency is wayyyy too long bruh relax")]
    FiatCurrencyTooLong,
    #[msg("The maximum bond mapping limit has been reached")]
    MaxBondMappingsReached,
    #[msg("No bond mapping found for the specified fiat currency")]
    NoBondMappingForCurrency,
    #[msg("Invalid fiat currency")]
    InvalidFiatCurrency,
    #[msg("The provided bond mint does not match the expected one for this currency")]
    InvalidBondMint,
    #[msg("Invalid price feed address")]
    InvalidPriceFeed,
    #[msg("Insufficient balance to redeem")]
    InsufficientBalance,
    #[msg("Bond ownership account required for this redemption")]
    BondOwnershipAccountRequired,
    #[msg("NFT token account required for bond redemption")]
    NFTTokenAccountRequired,
    #[msg("Instant bond redemption failed")]
    InstantRedemptionFailed,
    #[msg("NFT redemption Failed")]
    NFTRedemptionFailed,
    #[msg("There are no token accounts to harvest")]
    NoTokenAccountsToHarvest,
    #[msg("An invalid rate authority exists in the program")]
    InvalidRateAuthority
}