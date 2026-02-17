use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AddressU8 {
    pub addr: Pubkey,
    pub value: u8,
}

/// @notice struct to set borrow rate data for version 1
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RateDataV1Params {
    ///
    /// @param kink in borrow rate. in 1e2: 100% = 10_000; 1% = 100
    pub kink: u128,
    ///
    /// @param rateAtUtilizationZero desired borrow rate when utilization is zero. in 1e2: 100% = 10_000; 1% = 100
    /// i.e. constant minimum borrow rate
    /// e.g. at utilization = 0.01% rate could still be at least 4% (rateAtUtilizationZero would be 400 then)
    pub rate_at_utilization_zero: u128,
    ///
    /// @param rateAtUtilizationKink borrow rate when utilization is at kink. in 1e2: 100% = 10_000; 1% = 100
    /// e.g. when rate should be 7% at kink then rateAtUtilizationKink would be 700
    pub rate_at_utilization_kink: u128,
    ///
    /// @param rateAtUtilizationMax borrow rate when utilization is maximum at 100%. in 1e2: 100% = 10_000; 1% = 100
    /// e.g. when rate should be 125% at 100% then rateAtUtilizationMax would be 12_500
    pub rate_at_utilization_max: u128,
}

/// @notice struct to set borrow rate data for version 2
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RateDataV2Params {
    ///
    /// @param kink1 first kink in borrow rate. in 1e2: 100% = 10_000; 1% = 100
    /// utilization below kink 1 usually means slow increase in rate, once utilization is above kink 1 borrow rate increases faster
    pub kink1: u128,
    ///
    /// @param kink2 second kink in borrow rate. in 1e2: 100% = 10_000; 1% = 100
    /// utilization below kink 2 usually means slow / medium increase in rate, once utilization is above kink 2 borrow rate increases fast
    pub kink2: u128,
    ///
    /// @param rateAtUtilizationZero desired borrow rate when utilization is zero. in 1e2: 100% = 10_000; 1% = 100
    /// i.e. constant minimum borrow rate
    /// e.g. at utilization = 0.01% rate could still be at least 4% (rateAtUtilizationZero would be 400 then)
    pub rate_at_utilization_zero: u128,
    ///
    /// @param rateAtUtilizationKink1 desired borrow rate when utilization is at first kink. in 1e2: 100% = 10_000; 1% = 100
    /// e.g. when rate should be 7% at first kink then rateAtUtilizationKink would be 700
    pub rate_at_utilization_kink1: u128,
    ///
    /// @param rateAtUtilizationKink2 desired borrow rate when utilization is at second kink. in 1e2: 100% = 10_000; 1% = 100
    /// e.g. when rate should be 7% at second kink then rateAtUtilizationKink would be 1_200
    pub rate_at_utilization_kink2: u128,
    ///
    /// @param rateAtUtilizationMax desired borrow rate when utilization is maximum at 100%. in 1e2: 100% = 10_000; 1% = 100
    /// e.g. when rate should be 125% at 100% then rateAtUtilizationMax would be 12_500
    pub rate_at_utilization_max: u128,
}

/// @notice struct to set token config
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TokenConfig {
    ///
    /// @param token address
    pub token: Pubkey,
    ///
    /// @param fee charges on borrower's interest. in 1e2: 100% = 10_000; 1% = 100
    pub fee: u128,
    ///
    /// @param maxUtilization maximum allowed utilization. in 1e2: 100% = 10_000; 1% = 100
    ///                       set to 100% to disable and have default limit of 100% (avoiding SLOAD).
    pub max_utilization: u128,
}

/// @notice struct to set user supply & withdrawal config
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UserSupplyConfig {
    ///
    /// @param mode: 0 = without interest. 1 = with interest
    pub mode: u8,
    ///
    /// @param expandPercent withdrawal limit expand percent. in 1e2: 100% = 10_000; 1% = 100
    /// Also used to calculate rate at which withdrawal limit should decrease (instant).
    pub expand_percent: u128,
    ///
    /// @param expandDuration withdrawal limit expand duration in seconds.
    /// used to calculate rate together with expandPercent
    pub expand_duration: u128,
    ///
    /// @param baseWithdrawalLimit base limit, below this, user can withdraw the entire amount.
    /// amount in raw (to be multiplied with exchange price) or normal depends on configured mode in user config for the token:
    /// with interest -> raw, without interest -> normal
    pub base_withdrawal_limit: u128,
}

/// @notice struct to set user borrow & payback config
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UserBorrowConfig {
    ///
    /// @param mode: 0 = without interest. 1 = with interest
    pub mode: u8,
    ///
    /// @param expandPercent debt limit expand percent. in 1e2: 100% = 10_000; 1% = 100
    /// Also used to calculate rate at which debt limit should decrease (instant).
    pub expand_percent: u128,
    ///
    /// @param expandDuration debt limit expand duration in seconds.
    /// used to calculate rate together with expandPercent
    pub expand_duration: u128,
    ///
    /// @param baseDebtCeiling base borrow limit. until here, borrow limit remains as baseDebtCeiling
    /// (user can borrow until this point at once without stepped expansion). Above this, automated limit comes in place.
    /// amount in raw (to be multiplied with exchange price) or normal depends on configured mode in user config for the token:
    /// with interest -> raw, without interest -> normal
    pub base_debt_ceiling: u128,
    ///
    /// @param maxDebtCeiling max borrow ceiling, maximum amount the user can borrow.
    /// amount in raw (to be multiplied with exchange price) or normal depends on configured mode in user config for the token:
    /// with interest -> raw, without interest -> normal
    pub max_debt_ceiling: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Debug)]
pub enum TransferType {
    SKIP,   // skip transfer
    DIRECT, // transfer directly to the user (no claim)
    CLAIM,  // transfer to claim account and then can be claimed by user later
}
