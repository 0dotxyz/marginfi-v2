pub mod allocator;
pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod ix_utils;
pub mod macros;
pub mod prelude;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;
use instructions::*;
use marginfi_type_crate::types::{
    BankConfigCompact, BankConfigOpt, EmodeEntry, InterestRateConfigOpt, WrappedI80F48,
    MAX_EMODE_ENTRIES,
};
use prelude::*;

pub use id_crate::ID;

#[program]
pub mod marginfi {
    use super::*;

    pub fn marginfi_group_initialize(ctx: Context<MarginfiGroupInitialize>) -> MarginfiResult {
        marginfi_group::initialize_group(ctx)
    }

    pub fn marginfi_group_configure(
        ctx: Context<MarginfiGroupConfigure>,
        new_admin: Pubkey,
        new_emode_admin: Pubkey,
        new_curve_admin: Pubkey,
        new_limit_admin: Pubkey,
        new_emissions_admin: Pubkey,
        new_metadata_admin: Pubkey,
        new_risk_admin: Pubkey,
        emode_max_init_leverage: Option<WrappedI80F48>,
        emode_max_maint_leverage: Option<WrappedI80F48>,
    ) -> MarginfiResult {
        marginfi_group::configure(
            ctx,
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        )
    }

    pub fn lending_pool_add_bank(
        ctx: Context<LendingPoolAddBank>,
        bank_config: BankConfigCompact,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_add_bank(ctx, bank_config)
    }

    /// A copy of lending_pool_add_bank with an additional bank seed.
    /// This seed is used to create a PDA for the bank's signature.
    /// lending_pool_add_bank is preserved for backwards compatibility.
    pub fn lending_pool_add_bank_with_seed(
        ctx: Context<LendingPoolAddBankWithSeed>,
        bank_config: BankConfigCompact,
        bank_seed: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_add_bank_with_seed(ctx, bank_config, bank_seed)
    }

    /// Staging or localnet only, panics on mainnet
    pub fn lending_pool_clone_bank(
        ctx: Context<LendingPoolCloneBank>,
        bank_seed: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_clone_bank(ctx, bank_seed)
    }

    pub fn lending_pool_add_bank_permissionless(
        ctx: Context<LendingPoolAddBankPermissionless>,
        bank_seed: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_add_bank_permissionless(ctx, bank_seed)
    }

    /// (admin only)
    pub fn lending_pool_configure_bank(
        ctx: Context<LendingPoolConfigureBank>,
        bank_config_opt: BankConfigOpt,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_configure_bank(ctx, bank_config_opt)
    }

    /// (delegate_curve_admin only)
    pub fn lending_pool_configure_bank_interest_only(
        ctx: Context<LendingPoolConfigureBankInterestOnly>,
        interest_rate_config: InterestRateConfigOpt,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_configure_bank_interest_only(ctx, interest_rate_config)
    }

    /// (delegate_limits_admin only)
    pub fn lending_pool_configure_bank_limits_only(
        ctx: Context<LendingPoolConfigureBankLimitsOnly>,
        deposit_limit: Option<u64>,
        borrow_limit: Option<u64>,
        total_asset_value_init_limit: Option<u64>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_configure_bank_limits_only(
            ctx,
            deposit_limit,
            borrow_limit,
            total_asset_value_init_limit,
        )
    }

    /// (risk_admin only) - Signals all of a bank's liability have been deleveraged. Used if a bank
    /// still has liability dust after the risk admin has completed deleveraging all debts. The
    /// risk admin is trusted not to execute this until all non-dust debts have been deleveraged.
    pub fn lending_pool_force_tokenless_repay_complete(
        ctx: Context<LendingPoolForceTokenlessRepayComplete>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_force_tokenless_repay_complete(ctx)
    }

    /// (admin only)
    pub fn lending_pool_configure_bank_oracle(
        ctx: Context<LendingPoolConfigureBankOracle>,
        setup: u8,
        oracle: Pubkey,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_configure_bank_oracle(ctx, setup, oracle)
    }

    /// (admin only)
    pub fn lending_pool_set_fixed_oracle_price(
        ctx: Context<LendingPoolSetFixedOraclePrice>,
        price: WrappedI80F48,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_set_fixed_oracle_price(ctx, price)
    }

    /// (emode_admin only)
    pub fn lending_pool_configure_bank_emode(
        ctx: Context<LendingPoolConfigureBankEmode>,
        emode_tag: u16,
        entries: [EmodeEntry; MAX_EMODE_ENTRIES],
    ) -> MarginfiResult {
        marginfi_group::lending_pool_configure_bank_emode(ctx, emode_tag, entries)
    }

    /// (admin or emode_admin) Copies emode settings from one bank to another. Useful when applying
    /// emode settings from e.g. one LST to another.
    pub fn lending_pool_clone_emode(ctx: Context<LendingPoolCloneEmode>) -> MarginfiResult {
        marginfi_group::lending_pool_clone_emode(ctx)
    }

    /// (delegate_emissions_admin only)
    pub fn lending_pool_setup_emissions(
        ctx: Context<LendingPoolSetupEmissions>,
        flags: u64,
        rate: u64,
        total_emissions: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_setup_emissions(ctx, flags, rate, total_emissions)
    }

    /// (delegate_emissions_admin only)
    pub fn lending_pool_update_emissions_parameters(
        ctx: Context<LendingPoolUpdateEmissionsParameters>,
        emissions_flags: Option<u64>,
        emissions_rate: Option<u64>,
        additional_emissions: Option<u64>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_update_emissions_parameters(
            ctx,
            emissions_flags,
            emissions_rate,
            additional_emissions,
        )
    }

    /// Handle bad debt of a bankrupt marginfi account for a given bank.
    pub fn lending_pool_handle_bankruptcy<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolHandleBankruptcy<'info>>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_handle_bankruptcy(ctx)
    }

    // User instructions

    /// Initialize a marginfi account for a given group. The account is a fresh keypair, and must
    /// sign. If you are a CPI caller, consider using `marginfi_account_initialize_pda` instead, or
    /// create the account manually and use `transfer_to_new_account` to gift it to the owner you
    /// wish.
    pub fn marginfi_account_initialize(ctx: Context<MarginfiAccountInitialize>) -> MarginfiResult {
        marginfi_account::initialize_account(ctx)
    }

    pub fn marginfi_account_init_liq_record(ctx: Context<InitLiquidationRecord>) -> MarginfiResult {
        marginfi_account::initialize_liquidation_record(ctx)
    }

    /// The same as `marginfi_account_initialize`, except the created marginfi account uses a PDA
    /// (Program Derived Address)
    ///
    /// seeds:
    /// - marginfi_group
    /// - authority: The account authority (owner)  
    /// - account_index: A u16 value to allow multiple accounts per authority
    /// - third_party_id: Optional u16 for third-party tagging. Seeds < PDA_FREE_THRESHOLD can be
    ///   used freely. For a dedicated seed used by just your program (via CPI), contact us.
    pub fn marginfi_account_initialize_pda(
        ctx: Context<MarginfiAccountInitializePda>,
        account_index: u16,
        third_party_id: Option<u16>,
    ) -> MarginfiResult {
        marginfi_account::initialize_account_pda(ctx, account_index, third_party_id)
    }

    pub fn lending_account_deposit<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountDeposit<'info>>,
        amount: u64,
        deposit_up_to_limit: Option<bool>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_deposit(ctx, amount, deposit_up_to_limit)
    }

    pub fn lending_account_repay<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountRepay<'info>>,
        amount: u64,
        repay_all: Option<bool>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_repay(ctx, amount, repay_all)
    }

    pub fn lending_account_withdraw<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdraw<'info>>,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_withdraw(ctx, amount, withdraw_all)
    }

    pub fn lending_account_borrow<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountBorrow<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_account::lending_account_borrow(ctx, amount)
    }

    pub fn lending_account_close_balance(
        ctx: Context<LendingAccountCloseBalance>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_close_balance(ctx)
    }

    pub fn lending_account_withdraw_emissions<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdrawEmissions<'info>>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_withdraw_emissions(ctx)
    }

    pub fn lending_account_settle_emissions(
        ctx: Context<LendingAccountSettleEmissions>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_settle_emissions(ctx)
    }

    /// Liquidate a lending account balance of an unhealthy marginfi account
    pub fn lending_account_liquidate<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountLiquidate<'info>>,
        asset_amount: u64,
        liquidatee_accounts: u8,
        liquidator_accounts: u8,
    ) -> MarginfiResult {
        marginfi_account::lending_account_liquidate(
            ctx,
            asset_amount,
            liquidatee_accounts,
            liquidator_accounts,
        )
    }

    pub fn lending_account_start_flashloan(
        ctx: Context<LendingAccountStartFlashloan>,
        end_index: u64,
    ) -> MarginfiResult {
        marginfi_account::lending_account_start_flashloan(ctx, end_index)
    }

    pub fn lending_account_end_flashloan<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountEndFlashloan<'info>>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_end_flashloan(ctx)
    }

    pub fn marginfi_account_update_emissions_destination_account<'info>(
        ctx: Context<'_, '_, 'info, 'info, MarginfiAccountUpdateEmissionsDestinationAccount<'info>>,
    ) -> MarginfiResult {
        marginfi_account::marginfi_account_update_emissions_destination_account(ctx)
    }

    // Operational instructions
    pub fn lending_pool_accrue_bank_interest(
        ctx: Context<LendingPoolAccrueBankInterest>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_accrue_bank_interest(ctx)
    }

    pub fn lending_pool_collect_bank_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolCollectBankFees<'info>>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_collect_bank_fees(ctx)
    }

    pub fn lending_pool_withdraw_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolWithdrawFees<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_withdraw_fees(ctx, amount)
    }

    pub fn lending_pool_withdraw_fees_permissionless<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolWithdrawFeesPermissionless<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_withdraw_fees_permissionless(ctx, amount)
    }

    pub fn lending_pool_update_fees_destination_account<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolUpdateFeesDestinationAccount<'info>>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_update_fees_destination_account(ctx)
    }

    pub fn lending_pool_withdraw_insurance<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolWithdrawInsurance<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_withdraw_insurance(ctx, amount)
    }

    pub fn lending_pool_close_bank(ctx: Context<LendingPoolCloseBank>) -> MarginfiResult {
        marginfi_group::lending_pool_close_bank(ctx)
    }

    pub fn transfer_to_new_account(ctx: Context<TransferToNewAccount>) -> MarginfiResult {
        marginfi_account::transfer_to_new_account(ctx)
    }

    /// Same as `transfer_to_new_account` except the resulting account is a PDA
    ///
    /// seeds:
    /// - marginfi_group
    /// - authority: The account authority (owner)  
    /// - account_index: A u32 value to allow multiple accounts per authority
    /// - third_party_id: Optional u32 for third-party tagging. Seeds < PDA_FREE_THRESHOLD can be
    ///   used freely. For a dedicated seed used by just your program (via CPI), contact us.
    pub fn transfer_to_new_account_pda(
        ctx: Context<TransferToNewAccountPda>,
        account_index: u16,
        third_party_id: Option<u16>,
    ) -> MarginfiResult {
        marginfi_account::transfer_to_new_account_pda(ctx, account_index, third_party_id)
    }

    pub fn marginfi_account_set_freeze(
        ctx: Context<SetAccountFreeze>,
        frozen: bool,
    ) -> MarginfiResult {
        marginfi_account::set_account_freeze(ctx, frozen)
    }

    pub fn marginfi_account_close(ctx: Context<MarginfiAccountClose>) -> MarginfiResult {
        marginfi_account::close_account(ctx)
    }

    pub fn lending_account_withdraw_emissions_permissionless<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdrawEmissionsPermissionless<'info>>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_withdraw_emissions_permissionless(ctx)
    }

    /// (Permissionless) Refresh the internal risk engine health cache. Useful for liquidators and
    /// other consumers that want to see the internal risk state of a user account. This cache is
    /// read-only and serves no purpose except being populated by this ix.
    /// * remaining accounts expected in the same order as borrow, etc. I.e., for each balance the
    ///   user has, pass bank and oracle: <bank1, oracle1, bank2, oracle2>
    pub fn lending_account_pulse_health<'info>(
        ctx: Context<'_, '_, 'info, 'info, PulseHealth<'info>>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_pulse_health(ctx)
    }

    /// (Permissionless) Refresh the cached oracle price for a bank.
    pub fn lending_pool_pulse_bank_price_cache<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolPulseBankPriceCache<'info>>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_pulse_bank_price_cache(ctx)
    }

    /// (Runs once per program) Configures the fee state account, where the global admin sets fees
    /// that are assessed to the protocol
    pub fn init_global_fee_state(
        ctx: Context<InitFeeState>,
        admin: Pubkey,
        fee_wallet: Pubkey,
        bank_init_flat_sol_fee: u32,
        liquidation_flat_sol_fee: u32,
        program_fee_fixed: WrappedI80F48,
        program_fee_rate: WrappedI80F48,
        liquidation_max_fee: WrappedI80F48,
    ) -> MarginfiResult {
        marginfi_group::initialize_fee_state(
            ctx,
            admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            liquidation_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
            liquidation_max_fee,
        )
    }

    /// (global fee admin only) Adjust fees, admin, or the destination wallet
    pub fn edit_global_fee_state(
        ctx: Context<EditFeeState>,
        admin: Pubkey,
        fee_wallet: Pubkey,
        bank_init_flat_sol_fee: u32,
        liquidation_flat_sol_fee: u32,
        program_fee_fixed: WrappedI80F48,
        program_fee_rate: WrappedI80F48,
        liquidation_max_fee: WrappedI80F48,
    ) -> MarginfiResult {
        marginfi_group::edit_fee_state(
            ctx,
            admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            liquidation_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
            liquidation_max_fee,
        )
    }

    /// (Permissionless) Force any group to adopt the current FeeState settings
    pub fn propagate_fee_state(ctx: Context<PropagateFee>) -> MarginfiResult {
        marginfi_group::propagate_fee(ctx)
    }

    /// (global fee admin only) Enable or disable program fees for any group. Does not require the
    /// group admin to sign: the global fee state admin can turn program fees on or off for any
    /// group
    pub fn config_group_fee(
        ctx: Context<ConfigGroupFee>,
        enable_program_fee: bool,
    ) -> MarginfiResult {
        marginfi_group::config_group_fee(ctx, enable_program_fee)
    }

    /// (group admin only) Init the Staked Settings account, which is used to create staked
    /// collateral banks, and must run before any staked collateral bank can be created with
    /// `add_pool_permissionless`. Running this ix effectively opts the group into the staked
    /// collateral feature.
    pub fn init_staked_settings(
        ctx: Context<InitStakedSettings>,
        settings: StakedSettingsConfig,
    ) -> MarginfiResult {
        marginfi_group::initialize_staked_settings(ctx, settings)
    }

    pub fn edit_staked_settings(
        ctx: Context<EditStakedSettings>,
        settings: StakedSettingsEditConfig,
    ) -> MarginfiResult {
        marginfi_group::edit_staked_settings(ctx, settings)
    }

    pub fn propagate_staked_settings(ctx: Context<PropagateStakedSettings>) -> MarginfiResult {
        marginfi_group::propagate_staked_settings(ctx)
    }

    pub fn start_liquidation<'info>(
        ctx: Context<'_, '_, 'info, 'info, StartLiquidation<'info>>,
    ) -> MarginfiResult {
        marginfi_account::start_liquidation(ctx)
    }

    pub fn end_liquidation<'info>(
        ctx: Context<'_, '_, 'info, 'info, EndLiquidation<'info>>,
    ) -> MarginfiResult {
        marginfi_account::end_liquidation(ctx)
    }

    pub fn start_deleverage<'info>(
        ctx: Context<'_, '_, 'info, 'info, StartDeleverage<'info>>,
    ) -> MarginfiResult {
        marginfi_account::start_deleverage(ctx)
    }

    pub fn end_deleverage<'info>(
        ctx: Context<'_, '_, 'info, 'info, EndDeleverage<'info>>,
    ) -> MarginfiResult {
        marginfi_account::end_deleverage(ctx)
    }

    pub fn panic_pause(ctx: Context<PanicPause>) -> MarginfiResult {
        marginfi_group::panic_pause(ctx)
    }

    pub fn panic_unpause(ctx: Context<PanicUnpause>) -> MarginfiResult {
        marginfi_group::panic_unpause(ctx)
    }

    /// (permissionless) Unpause the protocol when pause time has expired
    pub fn panic_unpause_permissionless(
        ctx: Context<PanicUnpausePermissionless>,
    ) -> MarginfiResult {
        marginfi_group::panic_unpause_permissionless(ctx)
    }

    // TODO deprecate in 1.7
    /// (Permissionless) Convert a bank from the legacy curve setup to the new setup, with no effect
    /// on how interest accrues.
    pub fn migrate_curve(ctx: Context<MigrateCurve>) -> MarginfiResult {
        marginfi_group::migrate_curve(ctx)
    }

    /// (permissionless) pay the rent to open a bank's metadata.
    pub fn init_bank_metadata(ctx: Context<InitBankMetadata>) -> MarginfiResult {
        marginfi_group::init_bank_metadata(ctx)
    }

    /// (metadata admin only) Write ticker/descrption information for a bank on-chain. Optional, not
    /// all Banks are guranteed to have metadata.
    pub fn write_bank_metadata(
        ctx: Context<WriteBankMetadata>,
        ticker: Option<Vec<u8>>,
        description: Option<Vec<u8>>,
    ) -> MarginfiResult {
        marginfi_group::write_bank_metadata(ctx, ticker, description)
    }

    /// (group admin only) Set the daily withdrawal limit for deleverages per group.
    pub fn configure_deleverage_withdrawal_limit(
        ctx: Context<ConfigureDeleverageWithdrawalLimit>,
        limit: u32,
    ) -> MarginfiResult {
        marginfi_group::configure_deleverage_withdrawal_limit(ctx, limit)
    }

    /// (admin only) Configure bank-level rate limits for withdraw/borrow.
    /// Rate limits track net outflow in native tokens. Deposits offset withdraws.
    /// Set to 0 to disable. Hourly and daily windows are independent.
    pub fn configure_bank_rate_limits(
        ctx: Context<ConfigureBankRateLimits>,
        hourly_max_outflow: Option<u64>,
        daily_max_outflow: Option<u64>,
    ) -> MarginfiResult {
        marginfi_group::configure_bank_rate_limits(ctx, hourly_max_outflow, daily_max_outflow)
    }

    /// (admin only) Configure group-level rate limits for withdraw/borrow.
    /// Rate limits track aggregate net outflow in USD.
    /// Example: $10M = 10_000_000. Set to 0 to disable.
    pub fn configure_group_rate_limits(
        ctx: Context<ConfigureGroupRateLimits>,
        hourly_max_outflow_usd: Option<u64>,
        daily_max_outflow_usd: Option<u64>,
    ) -> MarginfiResult {
        marginfi_group::configure_group_rate_limits(
            ctx,
            hourly_max_outflow_usd,
            daily_max_outflow_usd,
        )
    }

    // TODO deprecate and incorporate this functionality into forced-withdraw in 1.7+
    /// (risk admin only) Purge a user's lending balance without withdrawing anything. Only usable
    /// after all the debt has been settled on a bank in deleveraging mode, e.g. when
    /// `TOKENLESS_REPAYMENTS_ALLOWED` and `TOKENLESS_REPAYMENTS_COMPLETE`. used to purge remaining
    /// lending assets in a now-worthless bank before it is fully sunset.
    pub fn purge_deleverage_balance(
        ctx: Context<LendingAccountPurgeDelevBalance>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_purge_delev_balance(ctx)
    }

    /****** Kamino integration instructions *****/

    /// (permissionless) Initialize a Kamino obligation for a marginfi bank
    /// * amount - In token, in native decimals. Must be >10 (i.e. 10 lamports, not 10 tokens). Lost
    ///   forever. Generally, try to make this the equivalent of around $1, in case Kamino ever
    ///   rounds small balances down to zero.
    pub fn kamino_init_obligation(
        ctx: Context<KaminoInitObligation>,
        amount: u64,
    ) -> MarginfiResult {
        kamino::kamino_init_obligation(ctx, amount)
    }

    /// (user) Deposit into a Kamino pool through a marginfi account
    /// * amount - in the liquidity token (e.g. if there is a Kamino USDC bank, pass the amount of
    ///   USDC desired), in native decimals.
    pub fn kamino_deposit<'info>(
        ctx: Context<'_, '_, 'info, 'info, KaminoDeposit<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        kamino::kamino_deposit(ctx, amount)
    }

    /// (user) Withdraw from a Kamino pool through a marginfi account
    /// * amount - in the collateral token (NOT liquidity token), in native decimals. Must convert
    ///     from collateral to liquidity token amounts using the current exchange rate.
    /// * withdraw_all - if true, withdraw the entire mrgn balance (Note: due to rounding down, a
    ///   deposit and withdraw back to back may result in several lamports less)
    pub fn kamino_withdraw<'info>(
        ctx: Context<'_, '_, 'info, 'info, KaminoWithdraw<'info>>,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> MarginfiResult {
        kamino::kamino_withdraw(ctx, amount, withdraw_all)
    }

    /// (group admin only) Add a Kamino bank to the group. Pass the oracle and reserve in remaining
    /// accounts 0 and 1 respectively.
    pub fn lending_pool_add_bank_kamino(
        ctx: Context<LendingPoolAddBankKamino>,
        bank_config: state::kamino::KaminoConfigCompact,
        bank_seed: u64,
    ) -> MarginfiResult {
        kamino::lending_pool_add_bank_kamino(ctx, bank_config, bank_seed)
    }

    /// (fee admin only) Harvest the specified reward index from the Kamino Farm attached to this bank.
    ///
    /// * `reward_index` — index of the reward token in the Kamino Farm's reward list
    pub fn kamino_harvest_reward(
        ctx: Context<KaminoHarvestReward>,
        reward_index: u64,
    ) -> MarginfiResult {
        kamino::kamino_harvest_reward(ctx, reward_index)
    }

    // Drift integration instructions

    /// (group admin only) Add a Drift bank to the group.
    pub fn lending_pool_add_bank_drift(
        ctx: Context<LendingPoolAddBankDrift>,
        bank_config: state::drift::DriftConfigCompact,
        bank_seed: u64,
    ) -> MarginfiResult {
        drift::lending_pool_add_bank_drift(ctx, bank_config, bank_seed)
    }

    /// (permissionless) Initialize a Drift user and user stats for a marginfi bank
    /// Creates user with sub_account_id = 0 and empty name
    /// Requires a minimum deposit to ensure the account remains active
    /// * amount - minimum deposit amount (at least 10 units) in native decimals
    pub fn drift_init_user(ctx: Context<DriftInitUser>, amount: u64) -> MarginfiResult {
        drift::drift_init_user(ctx, amount)
    }

    /// (user) Deposit into a Drift spot market through a marginfi account
    /// * amount - in the underlying token (e.g., USDC), in native decimals
    pub fn drift_deposit<'info>(
        ctx: Context<'_, '_, 'info, 'info, DriftDeposit<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        drift::drift_deposit(ctx, amount)
    }

    /// (user) Withdraw from a Drift spot market through a marginfi account
    /// * amount - in the underlying token (e.g., USDC), in native decimals
    /// * withdraw_all - if true, withdraws entire position
    pub fn drift_withdraw<'info>(
        ctx: Context<'_, '_, 'info, 'info, DriftWithdraw<'info>>,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> MarginfiResult {
        drift::drift_withdraw(ctx, amount, withdraw_all)
    }

    /// (fee admin only) Harvest rewards from admin deposits in Drift spot markets
    /// The harvest spot market must be different from the bank's main drift spot market
    pub fn drift_harvest_reward<'info>(
        ctx: Context<'_, '_, 'info, 'info, DriftHarvestReward<'info>>,
    ) -> MarginfiResult {
        drift::drift_harvest_reward(ctx)
    }

    // Solend integration instructions

    /// (admin) Add a Solend bank to the marginfi group
    pub fn lending_pool_add_bank_solend(
        ctx: Context<LendingPoolAddBankSolend>,
        bank_config: state::solend::SolendConfigCompact,
        bank_seed: u64,
    ) -> MarginfiResult {
        solend::lending_pool_add_bank_solend(ctx, bank_config, bank_seed)
    }

    /// (permissionless) Initialize a Solend obligation for a marginfi bank
    /// Requires a minimum deposit to ensure the obligation remains active
    /// * amount - minimum deposit amount (at least 10 units) in native decimals
    pub fn solend_init_obligation(
        ctx: Context<SolendInitObligation>,
        amount: u64,
    ) -> MarginfiResult {
        solend::solend_init_obligation(ctx, amount)
    }

    /// (user) Deposit into a Solend reserve through a marginfi account
    /// * amount - in the underlying token (e.g., USDC), in native decimals
    pub fn solend_deposit<'info>(
        ctx: Context<'_, '_, 'info, 'info, SolendDeposit<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        solend::solend_deposit(ctx, amount)
    }

    /// (user) Withdraw from a Solend reserve through a marginfi account
    /// * amount - in collateral tokens (cTokens), in native decimals  
    /// * withdraw_all - withdraw entire position if true
    pub fn solend_withdraw<'info>(
        ctx: Context<'_, '_, 'info, 'info, SolendWithdraw<'info>>,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> MarginfiResult {
        solend::solend_withdraw(ctx, amount, withdraw_all)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;
#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "marginfi v2",
    project_url: "https://app.marginfi.com/",
    contacts: "email:security@mrgn.group",
    policy: "https://github.com/mrgnlabs/marginfi-v2/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/mrgnlabs/marginfi-v2"
}
