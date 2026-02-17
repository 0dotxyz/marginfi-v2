use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCodes {
    /***********************************|
    |         Admin Module              |
    |__________________________________*/
    #[msg(ADMIN_MODULE_USER_CLASS_NOT_PAUSABLE)]
    UserClassNotPausable,

    #[msg(ADMIN_MODULE_USER_CLASS_NOT_FOUND)]
    UserClassNotFound,

    #[msg(ADMIN_MODULE_USER_ALREADY_PAUSED)]
    UserAlreadyPaused,

    #[msg(ADMIN_MODULE_USER_ALREADY_UNPAUSED)]
    UserAlreadyUnpaused,

    #[msg(ADMIN_MODULE_ONLY_LIQUIDITY_AUTHORITY)]
    OnlyLiquidityAuthority,

    #[msg(ADMIN_MODULE_ONLY_AUTH)]
    OnlyAuth,

    #[msg(ADMIN_MODULE_ONLY_GUARDIANS)]
    OnlyGuardians,

    #[msg(ADMIN_MODULE_INVALID_PARAMS)]
    InvalidParams,

    #[msg(ADMIN_MODULE_INVALID_CONFIG_ORDER)]
    InvalidConfigOrder,

    #[msg(ADMIN_MODULE_STATUS_ALREADY_SET)]
    StatusAlreadySet,

    #[msg(ADMIN_MODULE_LIMITS_CAN_NOT_BE_ZERO)]
    LimitsCannotBeZero,

    #[msg(ADMIN_MODULE_MAX_AUTH_COUNT)]
    MaxAuthCountReached,

    #[msg(ADMIN_MODULE_MAX_USER_CLASSES)]
    MaxUserClassesReached,

    /***********************************|
    |          User Module              |
    |__________________________________*/
    #[msg(USER_MODULE_INSUFFICIENT_BALANCE)]
    InsufficientBalance,

    #[msg(USER_MODULE_USER_SUPPLY_POSITION_REQUIRED)]
    UserSupplyPositionRequired,

    #[msg(USER_MODULE_USER_BORROW_POSITION_REQUIRED)]
    UserBorrowPositionRequired,

    #[msg(USER_MODULE_CLAIM_ACCOUNT_REQUIRED)]
    ClaimAccountRequired,

    #[msg(USER_MODULE_WITHDRAW_TO_ACCOUNT_REQUIRED)]
    WithdrawToAccountRequired,

    #[msg(USER_MODULE_BORROW_TO_ACCOUNT_REQUIRED)]
    BorrowToAccountRequired,

    #[msg(USER_MODULE_INVALID_CLAIM_AMOUNT)]
    InvalidClaimAmount,

    #[msg(USER_MODULE_NO_AMOUNT_TO_CLAIM)]
    NoAmountToClaim,

    #[msg(USER_MODULE_AMOUNT_NOT_ZERO)]
    AmountNotZero,

    #[msg(USER_MODULE_VALUE_OVERFLOW)]
    ValueOverflow,

    #[msg(USER_MODULE_INVALID_TRANSFER_TYPE)]
    InvalidTransferType,

    #[msg(USER_MODULE_MINT_MISMATCH)]
    MintMismatch,

    #[msg(USER_MODULE_USER_NOT_DEFINED)]
    UserNotDefined,

    #[msg(USER_MODULE_INVALID_USER_CLAIM)]
    InvalidUserClaim,

    /// @notice thrown when user operations are paused for an interacted token
    #[msg(USER_MODULE_USER_PAUSED)]
    UserPaused,

    /// @notice thrown when user's try to withdraw below withdrawal limit
    #[msg(USER_MODULE_WITHDRAWAL_LIMIT_REACHED)]
    WithdrawalLimitReached,

    /// @notice thrown when user's try to borrow above borrow limit
    #[msg(USER_MODULE_BORROW_LIMIT_REACHED)]
    BorrowLimitReached,

    /// @notice thrown when user sent supply/withdraw and borrow/payback both are nearly MIN_OPERATE_AMOUNT
    #[msg(USER_MODULE_OPERATE_AMOUNTS_ZERO)]
    OperateAmountsNearlyZero,

    /// @notice thrown when user sent supply/withdraw and borrow/payback both are nearly MIN_OPERATE_AMOUNT
    #[msg(USER_MODULE_OPERATE_AMOUNTS_TOO_BIG)]
    OperateAmountTooBig,

    /// @notice thrown when user sent supply/withdraw and borrow/payback both are nearly MIN_OPERATE_AMOUNT
    #[msg(USER_MODULE_OPERATE_AMOUNTS_INSUFFICIENT)]
    OperateAmountsInsufficient,

    /// @notice thrown when user did send excess or insufficient amount (beyond rounding issues)
    #[msg(USER_MODULE_TRANSFER_AMOUNT_OUT_OF_BOUNDS)]
    TransferAmountOutOfBounds,

    /// @notice thrown when there is supply / payback in Liquidity, but last instruction is not transfer
    #[msg(FORBIDDEN_OPERATE_CALL)]
    ForbiddenOperateCall,

    /// @notice thrown when user's try to borrow above max utilization
    #[msg(USER_MODULE_MAX_UTILIZATION_REACHED)]
    MaxUtilizationReached,

    /// @notice all ValueOverflow errors below are thrown if a certain input param or calc result overflows the allowed
    #[msg(USER_MODULE_VALUE_OVERFLOW_TOTAL_SUPPLY)]
    ValueOverflowTotalSupply,
    #[msg(USER_MODULE_VALUE_OVERFLOW_TOTAL_BORROW)]
    ValueOverflowTotalBorrow,

    #[msg(USER_MODULE_DEPOSIT_EXPECTED)]
    DepositExpected,

    /***********************************|
    |         LiquidityHelpers          |
    |__________________________________*/
    // LiquidityCalcs errors
    #[msg(LIQUIDITY_CALCS_EXCHANGE_PRICE_ZERO)]
    ExchangePriceZero,

    #[msg(LIQUIDITY_CALCS_UNSUPPORTED_RATE_VERSION)]
    UnsupportedRateVersion,

    #[msg(LIQUIDITY_CALCS_BORROW_RATE_NEGATIVE)]
    BorrowRateNegative,

    // Protocol lockdown
    #[msg(PROTOCOL_LOCKDOWN)]
    ProtocolLockdown,
}
