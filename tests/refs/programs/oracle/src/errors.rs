use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCodes {
    #[msg(PRICE_NOT_VALID)]
    PriceNotValid,

    #[msg(PRICE_TOO_OLD)]
    PriceTooOld,

    #[msg(RATE_ZERO)]
    RateZero,

    #[msg(INVALID_PARAMS)]
    InvalidParams,

    #[msg(INVALID_PYTH_SOURCE_MULTIPLIER_AND_DIVISOR)]
    InvalidPythSourceMultiplierAndDivisor,

    #[msg(INVALID_SOURCE)]
    InvalidSource,

    #[msg(INVALID_SOURCES_LENGTH)]
    InvalidSourcesLength,

    #[msg(ORACLE_ADMIN_ONLY_AUTHORITY)]
    OracleAdminOnlyAuthority,

    #[msg(ORACLE_ADMIN_ONLY_AUTH)]
    OracleAdminOnlyAuth,

    #[msg(ORACLE_ADMIN_MAX_AUTH_COUNT_REACHED)]
    OracleAdminMaxAuthCountReached,

    #[msg(ORACLE_ADMIN_INVALID_PARAMS)]
    OracleAdminInvalidParams,

    #[msg(ORACLE_NONCE_MISMATCH)]
    OracleNonceMismatch,

    #[msg(PRICE_CONFIDENCE_NOT_SUFFICIENT)]
    PriceConfidenceNotSufficient,

    #[msg(STAKE_POOL_NOT_REFRESHED)]
    StakePoolNotRefreshed,

    #[msg(INVALID_PRICE)]
    InvalidPrice,

    #[msg(FEE_TOO_HIGH)]
    FeeTooHigh,

    #[msg(REDSTONE_PRICE_OVERFLOW)]
    RedstonePriceOverflow,

    #[msg(TIMESTAMP_EXPECTED)]
    TimestampExpected,

    #[msg(CPI_TO_STAKE_PROGRAM_FAILED)]
    CpiToStakeProgramFailed,

    #[msg(INVALID_STAKE_POOL_RETURN_PARAMS)]
    InvalidStakePoolReturnParams,

    #[msg(CHAINLINK_PRICE_READ_ERROR)]
    ChainlinkPriceReadError,

    #[msg(SINGLE_POOL_TOKEN_SUPPLY_ZERO)]
    SinglePoolTokenSupplyZero,

    #[msg(SINGLE_POOL_INVALID_STAKE_ACCOUNT)]
    SinglePoolInvalidStakeAccount,

    #[msg(SINGLE_POOL_INVALID_MINT)]
    SinglePoolInvalidMint,

    #[msg(JUP_LEND_ACCOUNT_MISMATCH)]
    JupLendAccountMismatch,
}
