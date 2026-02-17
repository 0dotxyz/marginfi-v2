use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCodes {
    #[msg(LENDING_REWARD_RATE_MODEL_INVALID_PARAMS)]
    InvalidParams,

    #[msg(LENDING_REWARD_RATE_MODEL_ALREADY_STOPPED)]
    AlreadyStopped,

    #[msg(LENDING_REWARD_RATE_MODEL_NEXT_REWARDS_QUEUED)]
    NextRewardsQueued,

    #[msg(LENDING_REWARD_RATE_MODEL_NOT_ENDED)]
    NotEnded,

    #[msg(LENDING_REWARD_RATE_MODEL_NO_QUEUED_REWARDS)]
    NoQueuedRewards,

    #[msg(LENDING_REWARD_RATE_MODEL_MUST_TRANSITION_TO_NEXT)]
    MustTransitionToNext,

    #[msg(LENDING_REWARD_RATE_MODEL_NO_REWARDS_STARTED)]
    NoRewardsStarted,

    #[msg(LENDING_REWARD_RATE_MODEL_MAX_AUTH_COUNT_REACHED)]
    MaxAuthCountReached,

    #[msg(LENDING_REWARD_RATE_MODEL_ONLY_AUTHORITY)]
    OnlyAuthority,

    #[msg(LENDING_REWARD_RATE_MODEL_ONLY_AUTH)]
    OnlyAuths,

    #[msg(LENDING_REWARD_RATE_MODEL_CPI_TO_LENDING_PROGRAM_FAILED)]
    CpiToLendingProgramFailed,

    #[msg(LENDING_REWARD_RATE_MODEL_INVALID_LENDING_PROGRAM)]
    InvalidLendingProgram,

    #[msg(LENDING_REWARD_RATE_MODEL_INVALID_MINT)]
    InvalidMint,
}
