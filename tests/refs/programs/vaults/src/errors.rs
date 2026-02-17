use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCodes {
    /***********************************|
    |            Vault                  |
    |__________________________________*/
    #[msg(VAULT_NEXT_TICK_NOT_FOUND)]
    VaultNextTickNotFound,

    #[msg(VAULT_INVALID_POSITION_MINT)]
    VaultInvalidPositionMint,

    #[msg(VAULT_TICK_ID_LIQUIDATION_MISMATCH)]
    VaultTickIdLiquidationMismatch,

    #[msg(VAULT_INVALID_POSITION_TOKEN_AMOUNT)]
    VaultInvalidPositionTokenAmount,

    #[msg(VAULT_INVALID_REMAINING_ACCOUNTS_INDICES)]
    VaultInvalidRemainingAccountsIndices,

    #[msg(VAULT_TICK_HAS_DEBT_VAULT_ID_MISMATCH)]
    VaultTickHasDebtVaultIdMismatch,

    #[msg(VAULT_BRANCH_VAULT_ID_MISMATCH)]
    VaultBranchVaultIdMismatch,

    #[msg(VAULT_TICK_VAULT_ID_MISMATCH)]
    VaultTickVaultIdMismatch,

    #[msg(VAULT_INVALID_DECIMALS)]
    VaultInvalidDecimals,

    #[msg(VAULT_INVALID_OPERATE_AMOUNT)]
    VaultInvalidOperateAmount,

    #[msg(VAULT_TICK_IS_EMPTY)]
    VaultTickIsEmpty,

    #[msg(VAULT_POSITION_ABOVE_CF)]
    VaultPositionAboveCF,

    #[msg(VAULT_TOP_TICK_DOES_NOT_EXIST)]
    VaultTopTickDoesNotExist,

    #[msg(VAULT_EXCESS_SLIPPAGE_LIQUIDATION)]
    VaultExcessSlippageLiquidation,

    #[msg(VAULT_NOT_REBALANCER)]
    VaultNotRebalancer,

    #[msg(VAULT_TOKEN_NOT_INITIALIZED)]
    VaultTokenNotInitialized,

    #[msg(VAULT_USER_COLLATERAL_DEBT_EXCEED)]
    VaultUserCollateralDebtExceed,

    #[msg(VAULT_EXCESS_COLLATERAL_WITHDRAWAL)]
    VaultExcessCollateralWithdrawal,

    #[msg(VAULT_EXCESS_DEBT_PAYBACK)]
    VaultExcessDebtPayback,

    #[msg(VAULT_WITHDRAW_MORE_THAN_OPERATE_LIMIT)]
    VaultWithdrawMoreThanOperateLimit,

    #[msg(VAULT_INVALID_LIQUIDATION_AMT)]
    VaultInvalidLiquidationAmt,

    #[msg(VAULT_LIQUIDATION_RESULT)]
    VaultLiquidationResult,

    #[msg(VAULT_BRANCH_DEBT_TOO_LOW)]
    VaultBranchDebtTooLow,

    #[msg(VAULT_TICK_DEBT_TOO_LOW)]
    VaultTickDebtTooLow,

    #[msg(VAULT_LIQUIDITY_EXCHANGE_PRICE_UNEXPECTED)]
    VaultLiquidityExchangePriceUnexpected,

    #[msg(VAULT_USER_DEBT_TOO_LOW)]
    VaultUserDebtTooLow,

    #[msg(VAULT_INVALID_PAYBACK_OR_DEPOSIT)]
    VaultInvalidPaybackOrDeposit,

    #[msg(VAULT_INVALID_LIQUIDATION)]
    VaultInvalidLiquidation,

    #[msg(VAULT_NOTHING_TO_REBALANCE)]
    VaultNothingToRebalance,

    #[msg(VAULT_LIQUIDATION_REVERTS)]
    VaultLiquidationReverts,

    #[msg(VAULT_INVALID_ORACLE_PRICE)]
    VaultInvalidOraclePrice,

    #[msg(VAULT_BRANCH_NOT_FOUND)]
    VaultBranchNotFound,

    #[msg(VAULT_TICK_NOT_FOUND)]
    VaultTickNotFound,

    #[msg(VAULT_TICK_HAS_DEBT_NOT_FOUND)]
    VaultTickHasDebtNotFound,

    #[msg(VAULT_TICK_MISMATCH)]
    VaultTickMismatch,

    #[msg(VAULT_INVALID_VAULT_ID)]
    VaultInvalidVaultId,

    #[msg(VAULT_INVALID_NEXT_POSITION_ID)]
    VaultInvalidNextPositionId,

    #[msg(VAULT_INVALID_POSITION_ID)]
    VaultInvalidPositionId,

    #[msg(VAULT_POSITION_NOT_EMPTY)]
    VaultPositionNotEmpty,

    #[msg(VAULT_INVALID_SUPPLY_MINT)]
    VaultInvalidSupplyMint,

    #[msg(VAULT_INVALID_BORROW_MINT)]
    VaultInvalidBorrowMint,

    #[msg(VAULT_INVALID_ORACLE)]
    VaultInvalidOracle,

    #[msg(VAULT_INVALID_TICK)]
    VaultInvalidTick,

    #[msg(VAULT_INVALID_LIQUIDITY_PROGRAM)]
    VaultInvalidLiquidityProgram,

    #[msg(VAULT_INVALID_POSITION_AUTHORITY)]
    VaultInvalidPositionAuthority,

    #[msg(VAULT_ORACLE_NOT_VALID)]
    VaultOracleNotValid,

    #[msg(VAULT_BRANCH_OWNER_NOT_VALID)]
    VaultBranchOwnerNotValid,

    #[msg(VAULT_TICK_HAS_DEBT_OWNER_NOT_VALID)]
    VaultTickHasDebtOwnerNotValid,

    #[msg(VAULT_TICK_DATA_OWNER_NOT_VALID)]
    VaultTickOwnerNotValid,

    #[msg(VAULT_LIQUIDATE_REMAINING_ACCOUNTS_TOO_SHORT)]
    VaultLiquidateRemainingAccountsTooShort,

    #[msg(VAULT_OPERATE_REMAINING_ACCOUNTS_TOO_SHORT)]
    VaultOperateRemainingAccountsTooShort,

    #[msg(VAULT_INVALID_ZEROTH_BRANCH)]
    VaultInvalidZerothBranch,

    #[msg(VAULT_CPI_TO_LIQUIDITY_FAILED)]
    VaultCpiToLiquidityFailed,

    #[msg(VAULT_CPI_TO_ORACLE_FAILED)]
    VaultCpiToOracleFailed,

    #[msg(VAULT_ONLY_AUTHORITY)]
    VaultOnlyAuthority,

    #[msg(VAULT_NEW_BRANCH_INVALID)]
    VaultNewBranchInvalid,

    #[msg(VAULT_TICK_HAS_DEBT_INDEX_MISMATCH)]
    VaultTickHasDebtIndexMismatch,

    #[msg(VAULT_TICK_HAS_DEBT_OUT_OF_RANGE)]
    VaultTickHasDebtOutOfRange,

    #[msg(VAULT_USER_SUPPLY_POSITION_REQUIRED)]
    VaultUserSupplyPositionRequired,

    #[msg(VAULT_CLAIM_ACCOUNT_REQUIRED)]
    VaultClaimAccountRequired,

    #[msg(VAULT_RECIPIENT_WITHDRAW_ACCOUNT_REQUIRED)]
    VaultRecipientWithdrawAccountRequired,

    #[msg(VAULT_RECIPIENT_BORROW_ACCOUNT_REQUIRED)]
    VaultRecipientBorrowAccountRequired,

    #[msg(VAULT_POSITION_ABOVE_LIQUIDATION_THRESHOLD)]
    VaultPositionAboveLiquidationThreshold,

    /***********************************|
    |            Vault Admin            |
    |__________________________________*/
    #[msg(VAULT_ADMIN_VALUE_ABOVE_LIMIT)]
    VaultAdminValueAboveLimit,

    #[msg(VAULT_ADMIN_ONLY_AUTH_ACCOUNTS)]
    VaultAdminOnlyAuths,

    #[msg(VAULT_ADMIN_ADDRESS_ZERO_NOT_ALLOWED)]
    VaultAdminAddressZeroNotAllowed,

    #[msg(VAULT_ADMIN_VAULT_ID_MISMATCH)]
    VaultAdminVaultIdMismatch,

    #[msg(VAULT_ADMIN_TOTAL_IDS_MISMATCH)]
    VaultAdminTotalIdsMismatch,

    #[msg(VAULT_ADMIN_TICK_MISMATCH)]
    VaultAdminTickMismatch,

    #[msg(VAULT_ADMIN_LIQUIDITY_PROGRAM_MISMATCH)]
    VaultAdminLiquidityProgramMismatch,

    #[msg(VAULT_ADMIN_MAX_AUTH_COUNT_REACHED)]
    VaultAdminMaxAuthCountReached,

    #[msg(VAULT_ADMIN_INVALID_PARAMS)]
    VaultAdminInvalidParams,

    #[msg(VAULT_ADMIN_ONLY_AUTHORITY)]
    VaultAdminOnlyAuthority,

    #[msg(VAULT_ADMIN_ORACLE_PROGRAM_MISMATCH)]
    VaultAdminOracleProgramMismatch,
}
