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
    #[msg("The bond holding account does not match the expected one for this sovereign coin")]
    InvalidBondHolding,
    #[msg("The provided bond mint does not match the expected one for this currency")]
    InvalidBondMint,
    #[msg("The protocol bond account has insufficient funds")]
    InsufficientBondBalance,
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
    InvalidRateAuthority,
    #[msg("The bond holding account already exists for this sovereign coin")]
    BondHoldingAlreadyExists,
    #[msg("The bond account data is invalid or corrupted")]
    InvalidBondAccountData,
    // Additional missing error codes
    #[msg("Invalid sovereign coin mint")]
    InvalidSovereignCoinMint,
    #[msg("Invalid mint authority")]
    InvalidMintAuthority,
    #[msg("Invalid name length")]
    InvalidNameLength,
    #[msg("Invalid symbol length")]
    InvalidSymbolLength,
    #[msg("Invalid URI length")]
    InvalidUriLength,
    #[msg("State update failed")]
    StateUpdateFailed,
    #[msg("Invalid global USDC reserve")]
    InvalidGlobalUsdcReserve,
    #[msg("Invalid protocol vault")]
    InvalidProtocolVault,
    #[msg("Invalid global USDC account")]
    InvalidGlobalUsdcAccount,
    #[msg("Redeem state has expired")]
    RedeemStateExpired,
    #[msg("Redemption verification failed")]
    RedemptionVerificationFailed,
    #[msg("Insufficient redemption payout")]
    InsufficientRedemptionPayout,
    #[msg("Mint state has expired")]
    MintStateExpired,
    #[msg("Invalid preview input")]
    InvalidPreviewInput,
    #[msg("Invalid bond account")]
    InvalidBondAccount,
    #[msg("Token is not interest bearing")]
    NotInterestBearing,
    #[msg("Invalid fee basis points")]
    InvalidFeeBasisPoints,
    #[msg("Division by zero")]
    DivisionByZero,
    #[msg("Invalid USDC Reserve")]
    InvalidUSDCReserve,
    // LayerZero specific errors
    #[msg("Unauthorized LayerZero operation")]
    LzUnauthorized = 7000,
    #[msg("Invalid LayerZero sender")]
    LzInvalidSender = 7001,
    #[msg("Invalid LayerZero decimals")]
    LzInvalidDecimals = 7002,
    #[msg("LayerZero slippage exceeded")]
    LzSlippageExceeded = 7003,
    #[msg("Invalid LayerZero token destination")]
    LzInvalidTokenDest = 7004,
    #[msg("LayerZero rate limit exceeded")]
    LzRateLimitExceeded = 7005,
    #[msg("Invalid LayerZero fee")]
    LzInvalidFee = 7006,
    #[msg("LayerZero operations paused")]
    LzPaused = 7007,
    #[msg("LayerZero not enabled for this sovereign coin")]
    LzNotEnabled = 7100,
    #[msg("LayerZero already enabled for this sovereign coin")]
    LzAlreadyEnabled = 7101,
    #[msg("Invalid LayerZero OFT store")]
    LzInvalidOftStore = 7102,
    #[msg("Invalid LayerZero peer configuration")]
    LzInvalidPeer = 7103,
    #[msg("Invalid LayerZero rate limit configuration")]
    LzInvalidRateLimit = 7104,
    #[msg("Insufficient liquidity for cross-chain transfer")]
    InsufficientLiquidity = 7105,
}