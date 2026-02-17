use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCodes {
    /***********************************|
    |               Flashloan             |
    |__________________________________*/
    #[msg(FLASHLOAN_INVALID_AUTHORITY)]
    FlashloanInvalidAuthority,

    #[msg(FLASHLOAN_FEE_TOO_HIGH)]
    FlashloanFeeTooHigh,

    #[msg(FLASHLOAN_INVALID_PARAMS)]
    FlashloanInvalidParams,

    #[msg(FLASHLOAN_ALREADY_ACTIVE)]
    FlashloanAlreadyActive,

    #[msg(FLASHLOAN_ALREADY_INACTIVE)]
    FlashloanAlreadyInactive,

    #[msg(FLASHLOAN_CPI_TO_LIQUIDITY_FAILED)]
    FlashloanCpiToLiquidityFailed,

    #[msg(FLASHLOAN_NOT_ALLOWED_IN_THIS_SLOT)]
    FlashloanNotAllowedInThisSlot,

    #[msg(FLASHLOAN_INVALID_INSTRUCTION_SYSVAR)]
    FlashloanInvalidInstructionSysvar,

    #[msg(FLASHLOAN_INVALID_INSTRUCTION_DATA)]
    FlashloanInvalidInstructionData,

    #[msg(FLASHLOAN_PAYBACK_NOT_FOUND)]
    FlashloanPaybackNotFound,

    #[msg(FLASHLOAN_INVALID_INSTRUCTION)]
    FlashloanInvalidInstruction,

    #[msg(FLASHLOAN_PAUSED)]
    FlashloanPaused,

    #[msg(FLASHLOAN_CPICALL_NOT_ALLOWED)]
    FlashloanCPICallNotAllowed,

    #[msg(FLASHLOAN_MULTIPLE_PAYBACKS_FOUND)]
    FlashloanMultiplePaybacksFound,
}
