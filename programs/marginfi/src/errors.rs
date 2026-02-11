use anchor_lang::prelude::*;

#[error_code]
pub enum MarginfiError {
    #[msg("Internal Marginfi logic error")] // 6000
    InternalLogicError,
    #[msg("Invalid bank index")] // 6001
    BankNotFound,
    #[msg("Lending account balance not found")] // 6002
    LendingAccountBalanceNotFound,
    #[msg("Bank deposit capacity exceeded")] // 6003
    BankAssetCapacityExceeded,
    #[msg("Invalid transfer")] // 6004
    InvalidTransfer,
    #[msg("Missing Oracle, Bank, LST mint, or Sol Pool")] // 6005
    MissingPythOrBankAccount,
    #[msg("Missing Pyth account")] // 6006
    MissingPythAccount,
    #[msg("Missing Bank account")] // 6007
    MissingBankAccount,
    #[msg("Invalid Bank account")] // 6008
    InvalidBankAccount,
    #[msg("RiskEngine rejected due to either bad health or stale oracles")] // 6009
    RiskEngineInitRejected,
    #[msg("Lending account balance slots are full")] // 6010
    LendingAccountBalanceSlotsFull,
    #[msg("Bank already exists")] // 6011
    BankAlreadyExists,
    #[msg("Amount to liquidate must be positive")] // 6012
    ZeroLiquidationAmount,
    #[msg("Account is not bankrupt")] // 6013
    AccountNotBankrupt,
    #[msg("Account balance is not bad debt")] // 6014
    BalanceNotBadDebt,
    #[msg("Invalid group config")] // 6015
    InvalidConfig,
    #[msg("Bank paused")] // 6016
    BankPaused,
    #[msg("Bank is ReduceOnly mode")] // 6017
    BankReduceOnly,
    #[msg("Bank is missing")] // 6018
    BankAccountNotFound,
    #[msg("Operation is deposit-only")] // 6019
    OperationDepositOnly,
    #[msg("Operation is withdraw-only")] // 6020
    OperationWithdrawOnly,
    #[msg("Operation is borrow-only")] // 6021
    OperationBorrowOnly,
    #[msg("Operation is repay-only")] // 6022
    OperationRepayOnly,
    #[msg("No asset found")] // 6023
    NoAssetFound,
    #[msg("No liability found")] // 6024
    NoLiabilityFound,
    #[msg("Invalid oracle setup")] // 6025
    InvalidOracleSetup,
    #[msg("Invalid bank utilization ratio")] // 6026
    IllegalUtilizationRatio,
    #[msg("Bank borrow cap exceeded")] // 6027
    BankLiabilityCapacityExceeded,
    #[msg("Invalid Price")] // 6028
    InvalidPrice,
    #[msg("Account can have only one liability when account is under isolated risk")] // 6029
    IsolatedAccountIllegalState,
    #[msg("Emissions already setup")] // 6030
    EmissionsAlreadySetup,
    #[msg("Oracle is not set")] // 6031
    OracleNotSetup,
    #[msg("Invalid switchboard decimal conversion")] // 6032
    InvalidSwitchboardDecimalConversion,
    #[msg("Cannot close balance because of outstanding emissions")] // 6033
    CannotCloseOutstandingEmissions,
    #[msg("Update emissions error")] //6034
    EmissionsUpdateError,
    #[msg("Account disabled")] // 6035
    AccountDisabled,
    #[msg("Account can't temporarily open 3 balances, please close a balance first")] // 6036
    AccountTempActiveBalanceLimitExceeded,
    #[msg("Illegal action during flashloan")] // 6037
    AccountInFlashloan,
    #[msg("Illegal flashloan")] // 6038
    IllegalFlashloan,
    #[msg("Illegal flag")] // 6039
    IllegalFlag,
    #[msg("Illegal balance state")] // 6040
    IllegalBalanceState,
    #[msg("Illegal account authority transfer")] // 6041
    IllegalAccountAuthorityTransfer,
    #[msg("Unauthorized")] // 6042
    Unauthorized,
    #[msg("Invalid account authority")] // 6043
    IllegalAction,
    #[msg("Token22 Banks require mint account as first remaining account")] // 6044
    T22MintRequired,
    #[msg("Invalid ATA for global fee account")] // 6045
    InvalidFeeAta,
    #[msg("Use add pool permissionless instead")] // 6046
    AddedStakedPoolManually,
    #[msg("Staked SOL accounts can only deposit staked assets and borrow SOL")] // 6047
    AssetTagMismatch,
    #[msg("Stake pool validation failed: check the stake pool, mint, or sol pool")] // 6048
    StakePoolValidationFailed,
    #[msg("Switchboard oracle: stale price")] // 6049
    SwitchboardStalePrice,
    #[msg("Pyth Push oracle: stale price")] // 6050
    PythPushStalePrice,
    #[msg("Oracle error: wrong number of accounts")] // 6051
    WrongNumberOfOracleAccounts,
    #[msg("Oracle error: wrong account keys")] // 6052
    WrongOracleAccountKeys,
    #[msg("Pyth Push oracle: wrong account owner")] // 6053
    PythPushWrongAccountOwner,
    #[msg("Staked Pyth Push oracle: wrong account owner")] // 6054
    StakedPythPushWrongAccountOwner,
    #[msg("Oracle max confidence exceeded: try again later")] // 6055
    OracleMaxConfidenceExceeded,
    #[msg("Pyth Push oracle: insufficient verification level")] // 6056
    PythPushInsufficientVerificationLevel,
    #[msg("Zero asset price")] // 6057
    ZeroAssetPrice,
    #[msg("Zero liability price")] // 6058
    ZeroLiabilityPrice,
    #[msg("Switchboard oracle: wrong account owner")] // 6059
    SwitchboardWrongAccountOwner,
    #[msg("Pyth Push oracle: invalid account")] // 6060
    PythPushInvalidAccount,
    #[msg("Switchboard oracle: invalid account")] // 6061
    SwitchboardInvalidAccount,
    #[msg("Math error")] // 6062
    MathError,
    #[msg("Invalid emissions destination account")] // 6063
    InvalidEmissionsDestinationAccount,
    #[msg("Asset and liability bank cannot be the same")] // 6064
    SameAssetAndLiabilityBanks,
    #[msg("Trying to withdraw more assets than available")] // 6065
    OverliquidationAttempt,
    #[msg("Liability bank has no liabilities")] // 6066
    NoLiabilitiesInLiabilityBank,
    #[msg("Liability bank has assets")] // 6067
    AssetsInLiabilityBank,
    #[msg("Account is healthy and cannot be liquidated")] // 6068
    HealthyAccount,
    #[msg("Liability payoff too severe, exhausted liability")] // 6069
    ExhaustedLiability,
    #[msg("Liability payoff too severe, liability balance has assets")] // 6070
    TooSeverePayoff,
    #[msg("Liquidation too severe, account above maintenance requirement")] // 6071
    TooSevereLiquidation,
    #[msg("Liquidation would worsen account health")] // 6072
    WorseHealthPostLiquidation,
    #[msg("Exceeded the maximum allowed integration positions")] // 6073
    IntegrationPositionLimitExceeded,
    #[msg("Maximum initial leverage exceeded")] // 6074
    MaxInitLeverageExceeded,
    #[msg("The Emode config was invalid")] // 6075
    BadEmodeConfig,
    #[msg("TWAP window size does not match expected duration")] // 6076
    PythPushInvalidWindowSize,
    #[msg("Invalid fees destination account")] // 6077
    InvalidFeesDestinationAccount,
    #[msg("Banks cannot close when they have open positions or emissions outstanding")] // 6078
    BankCannotClose,
    #[msg("Account already migrated")] // 6079
    AccountAlreadyMigrated,
    #[msg("Protocol is paused")] // 6080
    ProtocolPaused,
    #[msg("Metadata is too long")] // 6081
    MetadataTooLong,
    #[msg("Pause limit exceeded")] // 6082
    PauseLimitExceeded,
    #[msg("Protocol is not paused")] // 6083
    ProtocolNotPaused,
    #[msg("Bank killed by bankruptcy: bank shutdown and value of all holdings is zero")] // 6084
    BankKilledByBankruptcy,
    #[msg("Liquidation state issue. Check start before end, end last, and both unique")] // 6085
    UnexpectedLiquidationState,
    #[msg("Liquidation start must be first instruction (other than compute program ixes)")] // 6086
    StartNotFirst,
    #[msg("Only one liquidation event allowed per tx")] // 6087
    StartRepeats,
    #[msg("The end instruction must be the last ix in the tx")] // 6088
    EndNotLast,
    #[msg("Tried to call an instruction that is forbidden during liquidation")] // 6089
    ForbiddenIx,
    #[msg("Seized too much of the asset relative to liability repaid")] // 6090
    LiquidationPremiumTooHigh,
    #[msg("Start and end liquidation and flashloan must be top-level instructions")] // 6091
    NotAllowedInCPI,
    #[msg("Stake pool supply is zero: cannot compute price")] // 6092
    ZeroSupplyInStakePool,
    #[msg("Invalid group: account constraint violated")] // 6093
    InvalidGroup,
    #[msg("Invalid liquidity vault: account constraint violated")] // 6094
    InvalidLiquidityVault,
    #[msg("Invalid liquidation record: account constraint violated")] // 6095
    InvalidLiquidationRecord,
    #[msg("Invalid liquidation receiver: account constraint violated")] // 6096
    InvalidLiquidationReceiver,
    #[msg("Invalid emissions mint: account constraint violated")] // 6097
    InvalidEmissionsMint,
    #[msg("Invalid mint: account constraint violated")] // 6098
    InvalidMint,
    #[msg("Invalid fee wallet: account constraint violated")] // 6099
    InvalidFeeWallet,
    #[msg("Fixed oracle price must be zero or greater")] // 6100
    FixedOraclePriceNegative,
    #[msg("Daily withdrawal limit exceeded: try again later")] // 6101
    DailyWithdrawalLimitExceeded,
    #[msg("Cannot set daily withdrawal limit to zero")] // 6102
    ZeroWithdrawalLimit,
    #[msg("Account is frozen by the group admin")] // 6103
    AccountFrozen,

    // ************** BEGIN KAMINO ERRORS (starting at 6200)
    #[msg("Wrong asset tag for standard instructions, expected DEFAULT, SOL, or STAKED asset tag")]
    WrongAssetTagForStandardInstructions = 200, // 6200
    #[msg("Wrong asset tag for Kamino instructions, expected KAMINO asset tag")]
    WrongAssetTagForKaminoInstructions, // 6201
    #[msg("Cannot create a kamino bank with this instruction, use add_bank_kamino")]
    CantAddPool, // 6202
    #[msg("Kamino reserve mint address doesn't match the bank mint address")]
    KaminoReserveMintAddressMismatch, // 6203
    #[msg("Deposit failed: obligation deposit amount increase did not match the expected increase, left - actual, right - expected")]
    KaminoDepositFailed, // 6204
    #[msg("Withdraw failed: token vault increase did not match the expected increase, left - actual, right - expected")]
    KaminoWithdrawFailed, // 6205
    #[msg("Kamino Reserve data is stale - run refresh_reserve on kamino program first")]
    ReserveStale, // 6206
    #[msg("Kamino obligation must have exactly one active deposit, at index 0")]
    InvalidObligationDepositCount, // 6207
    #[msg("Kamino obligation deposit doesn't match the expected reserve")]
    ObligationDepositReserveMismatch, // 6208
    #[msg("Failed to meet minimum deposit amount requirement for init obligation")]
    ObligationInitDepositInsufficient, // 6209
    #[msg("Kamino reserve validation failed")]
    KaminoReserveValidationFailed, // 6210
    #[msg("Invalid oracle setup: only KaminoPythPush and KaminoSwitchboardPull are supported")]
    KaminoInvalidOracleSetup, // 6211
    #[msg("Maximum Maintenance leverage exceeded")]
    MaxMaintLeverageExceeded, // 6212
    #[msg("Invalid Kamino reserve: account constraint violated")]
    InvalidKaminoReserve, // 6213
    #[msg("Invalid Kamino obligation: account constraint violated")]
    InvalidKaminoObligation, // 6214
    // **************END KAMINO ERRORS

    // ************** BEGIN DRIFT ERRORS (starting at 6300)
    #[msg("Invalid oracle setup: only DriftPythPull and DriftSwitchboardPull are supported")]
    DriftInvalidOracleSetup = 300, // 6300
    #[msg("Drift spot market mint does not match bank mint")]
    DriftSpotMarketMintMismatch, // 6301
    #[msg("Wrong bank asset tag for Drift operation")]
    WrongBankAssetTagForDriftOperation, // 6302
    #[msg("Cannot use standard operations on Drift assets")]
    CantUseStandardOperationsOnDriftAssets, // 6303
    #[msg("Drift spot market validation failed")]
    DriftSpotMarketValidationFailed, // 6304
    #[msg("Drift user has invalid spot positions (only first position can have balance)")]
    DriftInvalidSpotPositions, // 6305
    #[msg("Drift spot position market does not match bank's configured market")]
    DriftSpotPositionMarketMismatch, // 6306
    #[msg("Drift position has invalid balance type (must be deposit)")]
    DriftInvalidPositionType, // 6307
    #[msg("Drift scaled balance change does not match expected amount")]
    DriftScaledBalanceMismatch, // 6308
    #[msg("Drift withdrawal failed - token amount mismatch")]
    DriftWithdrawFailed, // 6309
    #[msg("Drift user initial deposit insufficient (minimum 10 units required)")]
    DriftUserInitDepositInsufficient, // 6310
    #[msg("Invalid drift account")]
    InvalidDriftAccount, // 6311
    #[msg("Drift authority mismatch")]
    DriftAuthorityMismatch, // 6312
    #[msg("Invalid harvest position index - must be between 2 and 7")]
    DriftInvalidHarvestPositionIndex, // 6313
    #[msg("Drift position is empty")]
    DriftPositionEmpty, // 6314
    #[msg("Drift position has invalid balance type")]
    DriftInvalidBalanceType, // 6315
    #[msg("No admin deposits found in Drift positions 2-7 for this market")]
    DriftNoAdminDeposit, // 6316
    #[msg("Cannot harvest from the same market as the bank's main drift spot market")]
    DriftHarvestSameMarket, // 6317
    #[msg("Drift account bricked: too many active deposits from admin operations")]
    DriftBrickedAccount, // 6318
    #[msg("Drift reward oracle required when 2+ active deposits exist")]
    DriftMissingRewardOracle, // 6319
    #[msg("Drift reward spot market required when 2+ active deposits exist")]
    DriftMissingRewardSpotMarket, // 6320
    #[msg("Drift account has admin deposits that require reward accounts to be provided")]
    DriftMissingRewardAccounts, // 6321
    #[msg("Drift spot market is stale, interest needs to be updated")]
    DriftSpotMarketStale, // 6322
    #[msg("Invalid Drift spot market: account constraint violated")]
    InvalidDriftSpotMarket, // 6323
    #[msg("Invalid Drift user: account constraint violated")]
    InvalidDriftUser, // 6324
    #[msg("Invalid Drift user stats: account constraint violated")]
    InvalidDriftUserStats, // 6325
    #[msg("Drift cannot support tokens with more than 19 decimals")]
    DriftUnsupportedTokenDecimals, // 6326
    // **************END DRIFT ERRORS

    // ************** BEGIN SOLEND ERRORS (starting at 6400)
    #[msg("Invalid oracle setup: only SolendPythPull and SolendSwitchboardPull are supported")]
    SolendInvalidOracleSetup = 400, // 6400
    #[msg("Solend reserve validation failed")]
    SolendReserveValidationFailed, // 6401
    #[msg("Solend obligation owner mismatch")]
    SolendObligationOwnerMismatch, // 6402
    #[msg("Wrong bank asset tag for Solend operation")]
    WrongBankAssetTagForSolendOperation, // 6403
    #[msg("Cannot use standard operations on Solend assets")]
    CantUseStandardOperationsOnSolendAssets, // 6404
    #[msg("Solend reserve mismatch")]
    SolendReserveMismatch, // 6405
    #[msg("Solend reserve mint mismatch")]
    SolendReserveMintMismatch, // 6406
    #[msg("Solend obligation has invalid deposits (only first position can have balance)")]
    SolendInvalidDepositPositions, // 6407
    #[msg("Solend deposit position reserve does not match bank's configured reserve")]
    SolendDepositPositionReserveMismatch, // 6408
    #[msg("Solend cToken balance change does not match expected amount")]
    SolendCTokenBalanceMismatch, // 6409
    #[msg("Solend withdrawal failed - token amount mismatch")]
    SolendWithdrawFailed, // 6410
    #[msg("Solend reserve is stale")]
    SolendReserveStale, // 6411
    #[msg("Solend deposit failed - collateral amount mismatch")]
    SolendDepositFailed, // 6412
    #[msg("Invalid Solend account owner")]
    InvalidSolendAccount, // 6413
    #[msg("Invalid Solend account version")]
    InvalidSolendAccountVersion, // 6414
    #[msg("Invalid Solend reserve: account constraint violated")]
    InvalidSolendReserve, // 6415
    #[msg("Invalid Solend obligation: account constraint violated")]
    InvalidSolendObligation, // 6416
                             // **************END SOLEND ERRORS
}

impl From<MarginfiError> for ProgramError {
    fn from(e: MarginfiError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl From<pyth_solana_receiver_sdk::error::GetPriceError> for MarginfiError {
    fn from(e: pyth_solana_receiver_sdk::error::GetPriceError) -> Self {
        use pyth_solana_receiver_sdk::error::GetPriceError::*;
        match e {
            PriceTooOld => MarginfiError::PythPushStalePrice,
            InsufficientVerificationLevel => MarginfiError::PythPushInsufficientVerificationLevel,
            InvalidWindowSize => MarginfiError::PythPushInvalidWindowSize,
            MismatchedFeedId | FeedIdMustBe32Bytes | FeedIdNonHexCharacter => {
                MarginfiError::PythPushInvalidAccount
            }
        }
    }
}
impl From<u32> for MarginfiError {
    fn from(value: u32) -> Self {
        match value {
            6001 => MarginfiError::BankNotFound,
            6002 => MarginfiError::LendingAccountBalanceNotFound,
            6003 => MarginfiError::BankAssetCapacityExceeded,
            6004 => MarginfiError::InvalidTransfer,
            6005 => MarginfiError::MissingPythOrBankAccount,
            6006 => MarginfiError::MissingPythAccount,
            6007 => MarginfiError::MissingBankAccount,
            6008 => MarginfiError::InvalidBankAccount,
            6009 => MarginfiError::RiskEngineInitRejected,
            6010 => MarginfiError::LendingAccountBalanceSlotsFull,
            6011 => MarginfiError::BankAlreadyExists,
            6012 => MarginfiError::ZeroLiquidationAmount,
            6013 => MarginfiError::AccountNotBankrupt,
            6014 => MarginfiError::BalanceNotBadDebt,
            6015 => MarginfiError::InvalidConfig,
            6016 => MarginfiError::BankPaused,
            6017 => MarginfiError::BankReduceOnly,
            6018 => MarginfiError::BankAccountNotFound,
            6019 => MarginfiError::OperationDepositOnly,
            6020 => MarginfiError::OperationWithdrawOnly,
            6021 => MarginfiError::OperationBorrowOnly,
            6022 => MarginfiError::OperationRepayOnly,
            6023 => MarginfiError::NoAssetFound,
            6024 => MarginfiError::NoLiabilityFound,
            6025 => MarginfiError::InvalidOracleSetup,
            6026 => MarginfiError::IllegalUtilizationRatio,
            6027 => MarginfiError::BankLiabilityCapacityExceeded,
            6028 => MarginfiError::InvalidPrice,
            6029 => MarginfiError::IsolatedAccountIllegalState,
            6030 => MarginfiError::EmissionsAlreadySetup,
            6031 => MarginfiError::OracleNotSetup,
            6032 => MarginfiError::InvalidSwitchboardDecimalConversion,
            6033 => MarginfiError::CannotCloseOutstandingEmissions,
            6034 => MarginfiError::EmissionsUpdateError,
            6035 => MarginfiError::AccountDisabled,
            6036 => MarginfiError::AccountTempActiveBalanceLimitExceeded,
            6037 => MarginfiError::AccountInFlashloan,
            6038 => MarginfiError::IllegalFlashloan,
            6039 => MarginfiError::IllegalFlag,
            6040 => MarginfiError::IllegalBalanceState,
            6041 => MarginfiError::IllegalAccountAuthorityTransfer,
            6042 => MarginfiError::Unauthorized,
            6043 => MarginfiError::IllegalAction,
            6044 => MarginfiError::T22MintRequired,
            6045 => MarginfiError::InvalidFeeAta,
            6046 => MarginfiError::AddedStakedPoolManually,
            6047 => MarginfiError::AssetTagMismatch,
            6048 => MarginfiError::StakePoolValidationFailed,
            6049 => MarginfiError::SwitchboardStalePrice,
            6050 => MarginfiError::PythPushStalePrice,
            6051 => MarginfiError::WrongNumberOfOracleAccounts,
            6052 => MarginfiError::WrongOracleAccountKeys,
            6053 => MarginfiError::PythPushWrongAccountOwner,
            6054 => MarginfiError::StakedPythPushWrongAccountOwner,
            6055 => MarginfiError::OracleMaxConfidenceExceeded,
            6056 => MarginfiError::PythPushInsufficientVerificationLevel,
            6057 => MarginfiError::ZeroAssetPrice,
            6058 => MarginfiError::ZeroLiabilityPrice,
            6059 => MarginfiError::SwitchboardWrongAccountOwner,
            6060 => MarginfiError::PythPushInvalidAccount,
            6061 => MarginfiError::SwitchboardInvalidAccount,
            6062 => MarginfiError::MathError,
            6063 => MarginfiError::InvalidEmissionsDestinationAccount,
            6064 => MarginfiError::SameAssetAndLiabilityBanks,
            6065 => MarginfiError::OverliquidationAttempt,
            6066 => MarginfiError::NoLiabilitiesInLiabilityBank,
            6067 => MarginfiError::AssetsInLiabilityBank,
            6068 => MarginfiError::HealthyAccount,
            6069 => MarginfiError::ExhaustedLiability,
            6070 => MarginfiError::TooSeverePayoff,
            6071 => MarginfiError::TooSevereLiquidation,
            6072 => MarginfiError::WorseHealthPostLiquidation,
            6073 => MarginfiError::IntegrationPositionLimitExceeded,
            6074 => MarginfiError::MaxInitLeverageExceeded,
            6075 => MarginfiError::BadEmodeConfig,
            6076 => MarginfiError::PythPushInvalidWindowSize,
            6077 => MarginfiError::InvalidFeesDestinationAccount,
            6078 => MarginfiError::BankCannotClose,
            6079 => MarginfiError::AccountAlreadyMigrated,
            6080 => MarginfiError::ProtocolPaused,
            6081 => MarginfiError::MetadataTooLong,
            6082 => MarginfiError::PauseLimitExceeded,
            6083 => MarginfiError::ProtocolNotPaused,
            6084 => MarginfiError::BankKilledByBankruptcy,
            6085 => MarginfiError::UnexpectedLiquidationState,
            6086 => MarginfiError::StartNotFirst,
            6087 => MarginfiError::StartRepeats,
            6088 => MarginfiError::EndNotLast,
            6089 => MarginfiError::ForbiddenIx,
            6090 => MarginfiError::LiquidationPremiumTooHigh,
            6091 => MarginfiError::NotAllowedInCPI,
            6092 => MarginfiError::ZeroSupplyInStakePool,
            6093 => MarginfiError::InvalidGroup,
            6094 => MarginfiError::InvalidLiquidityVault,
            6095 => MarginfiError::InvalidLiquidationRecord,
            6096 => MarginfiError::InvalidLiquidationReceiver,
            6097 => MarginfiError::InvalidEmissionsMint,
            6098 => MarginfiError::InvalidMint,
            6099 => MarginfiError::InvalidFeeWallet,
            6100 => MarginfiError::FixedOraclePriceNegative,
            6101 => MarginfiError::DailyWithdrawalLimitExceeded,
            6102 => MarginfiError::ZeroWithdrawalLimit,

            // Kamino-specific errors (starting at 6200)
            6200 => MarginfiError::WrongAssetTagForStandardInstructions,
            6201 => MarginfiError::WrongAssetTagForKaminoInstructions,
            6202 => MarginfiError::CantAddPool,
            6203 => MarginfiError::KaminoReserveMintAddressMismatch,
            6204 => MarginfiError::KaminoDepositFailed,
            6205 => MarginfiError::KaminoWithdrawFailed,
            6206 => MarginfiError::ReserveStale,
            6207 => MarginfiError::InvalidObligationDepositCount,
            6208 => MarginfiError::ObligationDepositReserveMismatch,
            6209 => MarginfiError::ObligationInitDepositInsufficient,
            6210 => MarginfiError::KaminoReserveValidationFailed,
            6211 => MarginfiError::KaminoInvalidOracleSetup,
            6212 => MarginfiError::MaxMaintLeverageExceeded,
            6213 => MarginfiError::InvalidKaminoReserve,
            6214 => MarginfiError::InvalidKaminoObligation,

            // Drift-specific errors (starting at 6300)
            6300 => MarginfiError::DriftInvalidOracleSetup,
            6301 => MarginfiError::DriftSpotMarketMintMismatch,
            6302 => MarginfiError::WrongBankAssetTagForDriftOperation,
            6303 => MarginfiError::CantUseStandardOperationsOnDriftAssets,
            6304 => MarginfiError::DriftSpotMarketValidationFailed,
            6305 => MarginfiError::DriftInvalidSpotPositions,
            6306 => MarginfiError::DriftSpotPositionMarketMismatch,
            6307 => MarginfiError::DriftInvalidPositionType,
            6308 => MarginfiError::DriftScaledBalanceMismatch,
            6309 => MarginfiError::DriftWithdrawFailed,
            6310 => MarginfiError::DriftUserInitDepositInsufficient,
            6311 => MarginfiError::InvalidDriftAccount,
            6312 => MarginfiError::DriftAuthorityMismatch,
            6313 => MarginfiError::DriftInvalidHarvestPositionIndex,
            6314 => MarginfiError::DriftPositionEmpty,
            6315 => MarginfiError::DriftInvalidBalanceType,
            6316 => MarginfiError::DriftNoAdminDeposit,
            6317 => MarginfiError::DriftHarvestSameMarket,
            6318 => MarginfiError::DriftBrickedAccount,
            6319 => MarginfiError::DriftMissingRewardOracle,
            6320 => MarginfiError::DriftMissingRewardSpotMarket,
            6321 => MarginfiError::DriftMissingRewardAccounts,
            6322 => MarginfiError::DriftSpotMarketStale,
            6323 => MarginfiError::InvalidDriftSpotMarket,
            6324 => MarginfiError::InvalidDriftUser,
            6325 => MarginfiError::InvalidDriftUserStats,
            6326 => MarginfiError::DriftUnsupportedTokenDecimals,

            // Solend-specific errors (starting at 6400)
            6400 => MarginfiError::SolendInvalidOracleSetup,
            6401 => MarginfiError::SolendReserveValidationFailed,
            6402 => MarginfiError::SolendObligationOwnerMismatch,
            6403 => MarginfiError::WrongBankAssetTagForSolendOperation,
            6404 => MarginfiError::CantUseStandardOperationsOnSolendAssets,
            6405 => MarginfiError::SolendReserveMismatch,
            6406 => MarginfiError::SolendReserveMintMismatch,
            6407 => MarginfiError::SolendInvalidDepositPositions,
            6408 => MarginfiError::SolendDepositPositionReserveMismatch,
            6409 => MarginfiError::SolendCTokenBalanceMismatch,
            6410 => MarginfiError::SolendWithdrawFailed,
            6411 => MarginfiError::SolendReserveStale,
            6412 => MarginfiError::SolendDepositFailed,
            6413 => MarginfiError::InvalidSolendAccount,
            6414 => MarginfiError::InvalidSolendAccountVersion,
            6415 => MarginfiError::InvalidSolendReserve,
            6416 => MarginfiError::InvalidSolendObligation,

            _ => MarginfiError::InternalLogicError,
        }
    }
}

impl PartialEq for MarginfiError {
    fn eq(&self, other: &Self) -> bool {
        (*self as u32) == (*other as u32)
    }
}

impl Eq for MarginfiError {}

impl MarginfiError {
    pub fn is_oracle_error(&self) -> bool {
        matches!(
            self,
            MarginfiError::WrongNumberOfOracleAccounts
                | MarginfiError::SwitchboardInvalidAccount
                | MarginfiError::PythPushInvalidAccount
                | MarginfiError::SwitchboardWrongAccountOwner
                | MarginfiError::PythPushInsufficientVerificationLevel
                | MarginfiError::StakedPythPushWrongAccountOwner
                | MarginfiError::PythPushWrongAccountOwner
                | MarginfiError::WrongOracleAccountKeys
                | MarginfiError::PythPushStalePrice
                | MarginfiError::SwitchboardStalePrice
                | MarginfiError::StakePoolValidationFailed
                | MarginfiError::InvalidBankAccount
                | MarginfiError::MissingBankAccount
                | MarginfiError::MissingPythAccount
                | MarginfiError::MissingPythOrBankAccount
                | MarginfiError::PythPushInvalidWindowSize
                | MarginfiError::OracleMaxConfidenceExceeded
                | MarginfiError::ZeroSupplyInStakePool
        )
    }

    pub fn is_risk_engine_rejection(&self) -> bool {
        matches!(self, MarginfiError::RiskEngineInitRejected)
    }
}
