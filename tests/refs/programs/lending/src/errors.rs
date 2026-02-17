use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCodes {
    /***********************************|
    |               fToken              |
    |__________________________________*/
    #[msg(F_TOKEN_DEPOSIT_INSIGNIFICANT)]
    FTokenDepositInsignificant,

    #[msg(F_TOKEN_MIN_AMOUNT_OUT)]
    FTokenMinAmountOut,

    #[msg(F_TOKEN_MAX_AMOUNT)]
    FTokenMaxAmount,

    #[msg(F_TOKEN_INVALID_PARAMS)]
    FTokenInvalidParams,

    #[msg(F_TOKEN_REWARDS_RATE_MODEL_ALREADY_SET)]
    FTokenRewardsRateModelAlreadySet,

    #[msg(F_TOKEN_MAX_AUTH_COUNT)]
    FTokenMaxAuthCountReached,

    #[msg(F_TOKEN_LIQUIDITY_EXCHANGE_PRICE_UNEXPECTED)]
    FTokenLiquidityExchangePriceUnexpected,

    #[msg(F_TOKEN_CPI_TO_LIQUIDITY_FAILED)]
    FTokenCpiToLiquidityFailed,

    #[msg(F_TOKEN_ONLY_AUTH)]
    FTokenOnlyAuth,

    #[msg(F_TOKEN_ONLY_AUTHORITY)]
    FTokenOnlyAuthority,

    #[msg(F_TOKEN_ONLY_REBALANCER)]
    FTokenOnlyRebalancer,

    #[msg(F_TOKEN_USER_SUPPLY_POSITION_REQUIRED)]
    FTokenUserSupplyPositionRequired,

    #[msg(F_TOKEN_LIQUIDITY_PROGRAM_MISMATCH)]
    FTokenLiquidityProgramMismatch,
}
