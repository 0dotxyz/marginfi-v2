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
    BankConfigCompact, BankConfigOpt, EmodeEntry, InterestRateConfigOpt, OrderTrigger,
    WrappedI80F48, MAX_EMODE_ENTRIES,
};
use prelude::*;

pub use id_crate::ID;

#[program]
pub mod marginfi {
    use super::*;

    /// (admin only) Initialize a new marginfi group. The signer becomes the group admin.
    pub fn marginfi_group_initialize(ctx: Context<MarginfiGroupInitialize>) -> MarginfiResult {
        marginfi_group::initialize_group(ctx)
    }

    /// (admin only) Configure group admin keys and emode leverage caps. All admin keys must be
    /// provided on every call. Emode leverage caps are set if provided, otherwise the existing
    /// (non-zero) values are kept. Pass `Some(value)` to update, `None` to leave unchanged.
    pub fn marginfi_group_configure(
        ctx: Context<MarginfiGroupConfigure>,
        new_admin: Option<Pubkey>,
        new_emode_admin: Option<Pubkey>,
        new_curve_admin: Option<Pubkey>,
        new_limit_admin: Option<Pubkey>,
        new_flow_admin: Option<Pubkey>,
        new_emissions_admin: Option<Pubkey>,
        new_metadata_admin: Option<Pubkey>,
        new_risk_admin: Option<Pubkey>,
        emode_max_init_leverage: Option<WrappedI80F48>,
        emode_max_maint_leverage: Option<WrappedI80F48>,
    ) -> MarginfiResult {
        marginfi_group::configure(
            ctx,
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            new_flow_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        )
    }

    /// (admin only) Add a new bank to the lending pool
    pub fn lending_pool_add_bank(
        ctx: Context<LendingPoolAddBank>,
        bank_config: BankConfigCompact,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_add_bank(ctx, bank_config)
    }

    /// (admin only) A copy of lending_pool_add_bank with an additional bank seed.
    /// This seed is used to create a PDA for the bank's signature.
    /// lending_pool_add_bank is preserved for backwards compatibility.
    pub fn lending_pool_add_bank_with_seed(
        ctx: Context<LendingPoolAddBankWithSeed>,
        bank_config: BankConfigCompact,
        bank_seed: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_add_bank_with_seed(ctx, bank_config, bank_seed)
    }

    /// (admin only) Staging or localnet only, panics on mainnet
    /// This instruction is used to clone a bank to a new PDA.
    pub fn lending_pool_clone_bank(
        ctx: Context<LendingPoolCloneBank>,
        bank_seed: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_clone_bank(ctx, bank_seed)
    }

    /// (permissionless) Add a staked collateral bank. Requires a valid SPL single-pool LST mint.
    pub fn lending_pool_add_bank_permissionless(
        ctx: Context<LendingPoolAddBankPermissionless>,
        bank_seed: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_add_bank_permissionless(ctx, bank_seed)
    }

    /// (admin only) Configure bank parameters. If the bank has `FREEZE_SETTINGS`, only
    /// deposit/borrow limits are updated and all other config changes are silently ignored.
    pub fn lending_pool_configure_bank(
        ctx: Context<LendingPoolConfigureBank>,
        bank_config_opt: BankConfigOpt,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_configure_bank(ctx, bank_config_opt)
    }

    /// (delegate_curve_admin only) Update interest rate config. Does nothing if bank has
    /// `FREEZE_SETTINGS`.
    pub fn lending_pool_configure_bank_interest_only(
        ctx: Context<LendingPoolConfigureBankInterestOnly>,
        interest_rate_config: InterestRateConfigOpt,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_configure_bank_interest_only(ctx, interest_rate_config)
    }

    /// (delegate_limit_admin only) Update deposit/borrow/init limits only.
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

    /// (permissionless) Reclaim all remaining tokens from the emissions vault
    /// to the global fee wallet ATA, and disable emissions on the bank.
    pub fn lending_pool_reclaim_emissions_vault(
        ctx: Context<LendingPoolReclaimEmissionsVault>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_reclaim_emissions_vault(ctx)
    }

    /// (permissionless) Deposit same-bank emissions directly into liquidity vault and increase
    /// depositors' value via `asset_share_value`.
    pub fn lending_pool_emissions_deposit(
        ctx: Context<LendingPoolEmissionsDeposit>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_emissions_deposit(ctx, amount)
    }

    /// (risk_admin or admin, unless `PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG` is set on the bank)
    /// Handle bad debt of a bankrupt marginfi account for a given bank. Covers bad debt from the
    /// insurance fund and socializes any remainder among depositors.
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

    /// (permissionless) Initialize a liquidation record PDA for a marginfi account. The fee_payer
    /// pays rent; the record is required for receivership liquidation.
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

    /// (user) Create a new Order.
    /// * bank_keys - Currently only two keys: the lending position and borrowing position in the
    ///   users's Balances for which the order is being placed
    /// * trigger - the type of order (stop loss, take profit, or both), and the threshold at which
    ///   to trigger the order, in dollars
    pub fn marginfi_account_place_order(
        ctx: Context<PlaceOrder>,
        bank_keys: Vec<Pubkey>,
        trigger: OrderTrigger,
    ) -> MarginfiResult {
        marginfi_account::place_order(ctx, bank_keys, trigger)
    }

    /// (user) Close an existing Order, returning rent to the user
    pub fn marginfi_account_close_order(ctx: Context<CloseOrder>) -> MarginfiResult {
        marginfi_account::close_order(ctx)
    }

    /// (permissionless keeper) Close an existing Order after the user account was closed, or it no
    /// longer has the associated positions, or the user has executed
    /// `marginfi_account_set_keeper_close_flags`. Keeper keeps the rent.
    pub fn marginfi_account_keeper_close_order(ctx: Context<KeeperCloseOrder>) -> MarginfiResult {
        marginfi_account::keeper_close_order(ctx)
    }

    /// (user) Purge flags from some balances, enabling a Keeper to call
    /// `marginfi_account_keeper_close_order` on associated Orders. Typically, use
    /// `marginfi_account_close_order` instead if trying to close an Order.
    pub fn marginfi_account_set_keeper_close_flags(
        ctx: Context<SetKeeperCloseFlags>,
        bank_keys_opt: Option<Vec<Pubkey>>,
    ) -> MarginfiResult {
        marginfi_account::set_keeper_close_flags(ctx, bank_keys_opt)
    }

    /// (permissionless keeper) Begin Order execution
    /// * Enables the Keeper to withdraw/repay associated positions until the end of the tx
    /// * Only one `StartExecuteOrder` is allowed per tx
    /// * Must appear before `EndExecuteOrder` in the tx, and before any instructions except certain
    ///   allowed ones (compute budget, kamino refresh, etc)
    /// * `EndExecuteOrder` must also appear in the tx
    /// * CPI is forbidden
    /// * Costs a small amount of rent, which is returned at the end of the tx, make sure you have
    ///   enough SOL to start the tx.
    pub fn marginfi_account_start_execute_order<'info>(
        ctx: Context<'_, '_, 'info, 'info, StartExecuteOrder<'info>>,
    ) -> MarginfiResult {
        marginfi_account::start_execute_order(ctx)
    }

    /// (permissionless keeper) End Order execution
    /// * Closes the Order (keeper keeps the rent)
    /// * Closes the borrow position involved in the Order, the lending position remains open
    /// * User health must be "unchanged" (within Order requirements i.e. minus slippage). Keeper
    ///   may keep any slippage in excess of what was needed to complete the Order as profit.
    /// * `StartExecuteOrder` must appear earlier in the tx
    /// * Must appear last in the tx
    /// * CPI is forbidden
    /// * Returns rent for ephemeral accounts created during `StartExecuteOrder`
    pub fn marginfi_account_end_execute_order<'info>(
        ctx: Context<'_, '_, 'info, 'info, EndExecuteOrder<'info>>,
    ) -> MarginfiResult {
        marginfi_account::end_execute_order(ctx)
    }
    /// (account authority) Deposit assets into a bank. Accrues interest, records deposit, and
    /// transfers tokens from the signer's token account to the bank's liquidity vault.
    pub fn lending_account_deposit<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountDeposit<'info>>,
        amount: u64,
        deposit_up_to_limit: Option<bool>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_deposit(ctx, amount, deposit_up_to_limit)
    }

    /// (account authority, or any signer during receivership) Repay borrowed assets. Accrues
    /// interest, records repayment, and transfers tokens to the bank's liquidity vault.
    pub fn lending_account_repay<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountRepay<'info>>,
        amount: u64,
        repay_all: Option<bool>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_repay(ctx, amount, repay_all)
    }

    /// (account authority, or any signer during receivership) Withdraw assets from a bank. Accrues
    /// interest, records withdrawal, transfers tokens, and runs a health check (skipped during
    /// receivership).
    pub fn lending_account_withdraw<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdraw<'info>>,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_withdraw(ctx, amount, withdraw_all)
    }

    /// (account authority) Borrow assets from a bank. Accrues interest, records liability, applies
    /// origination fee, transfers tokens, and runs a health check.
    pub fn lending_account_borrow<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountBorrow<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_account::lending_account_borrow(ctx, amount)
    }

    /// (account authority) Close a balance position with dust-level amounts.
    pub fn lending_account_close_balance(
        ctx: Context<LendingAccountCloseBalance>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_close_balance(ctx)
    }

    /// (permissionless) Liquidate a lending account balance of an unhealthy marginfi account.
    /// The liquidator takes on the liability and receives discounted collateral (2.5% liquidator
    /// fee + 2.5% insurance fee).
    /// * `asset_amount` - amount of collateral to liquidate
    /// * `liquidatee_accounts` - number of remaining accounts for the liquidatee
    /// * `liquidator_accounts` - number of remaining accounts for the liquidator
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

    /// (account authority) Start a flash loan. Must have a corresponding `end_flashloan` ix in the
    /// same tx. Health checks are skipped until the flash loan ends.
    pub fn lending_account_start_flashloan(
        ctx: Context<LendingAccountStartFlashloan>,
        end_index: u64,
    ) -> MarginfiResult {
        marginfi_account::lending_account_start_flashloan(ctx, end_index)
    }

    /// (account authority) End a flash loan and run the health check.
    pub fn lending_account_end_flashloan<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountEndFlashloan<'info>>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_end_flashloan(ctx)
    }

    /// (account authority) Set the wallet whose canonical ATA will receive off-chain emissions.
    pub fn marginfi_account_update_emissions_destination_account(
        ctx: Context<MarginfiAccountUpdateEmissionsDestinationAccount>,
    ) -> MarginfiResult {
        marginfi_account::marginfi_account_update_emissions_destination_account(ctx)
    }

    // Operational instructions

    /// (permissionless) Accrue interest on a bank, updating share values and collecting fees.
    pub fn lending_pool_accrue_bank_interest(
        ctx: Context<LendingPoolAccrueBankInterest>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_accrue_bank_interest(ctx)
    }

    /// (permissionless) Transfer accrued fees from the liquidity vault to insurance/fee/program
    /// vaults.
    pub fn lending_pool_collect_bank_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolCollectBankFees<'info>>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_collect_bank_fees(ctx)
    }

    /// (admin only) Withdraw collected group fees from the fee vault.
    pub fn lending_pool_withdraw_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolWithdrawFees<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_withdraw_fees(ctx, amount)
    }

    /// (permissionless) Withdraw group fees to the pre-configured `fees_destination_account`.
    pub fn lending_pool_withdraw_fees_permissionless<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolWithdrawFeesPermissionless<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_withdraw_fees_permissionless(ctx, amount)
    }

    /// (admin only) Set the destination wallet for permissionless fee withdrawals.
    pub fn lending_pool_update_fees_destination_account<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolUpdateFeesDestinationAccount<'info>>,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_update_fees_destination_account(ctx)
    }

    /// (admin only) Withdraw from the insurance vault.
    pub fn lending_pool_withdraw_insurance<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolWithdrawInsurance<'info>>,
        amount: u64,
    ) -> MarginfiResult {
        marginfi_group::lending_pool_withdraw_insurance(ctx, amount)
    }

    /// (admin only) Close a bank. Requires CLOSE_ENABLED_FLAG and zero positions/shares.
    pub fn lending_pool_close_bank(ctx: Context<LendingPoolCloseBank>) -> MarginfiResult {
        marginfi_group::lending_pool_close_bank(ctx)
    }

    /// (account authority) Transfer all positions to a new account under a new authority. The old
    /// account is disabled. Pays a flat SOL fee to the protocol.
    pub fn transfer_to_new_account(ctx: Context<TransferToNewAccount>) -> MarginfiResult {
        marginfi_account::transfer_to_new_account(ctx)
    }

    /// (account authority) Same as `transfer_to_new_account` except the resulting account is a PDA
    ///
    /// seeds:
    /// - marginfi_group
    /// - authority: The account authority (owner)  
    /// - account_index: A u16 value to allow multiple accounts per authority
    /// - third_party_id: Optional u16 for third-party tagging. Seeds < PDA_FREE_THRESHOLD can be
    ///   used freely. For a dedicated seed used by just your program (via CPI), contact us.
    pub fn transfer_to_new_account_pda(
        ctx: Context<TransferToNewAccountPda>,
        account_index: u16,
        third_party_id: Option<u16>,
    ) -> MarginfiResult {
        marginfi_account::transfer_to_new_account_pda(ctx, account_index, third_party_id)
    }

    /// (admin only) Freeze or unfreeze a marginfi account. Frozen accounts can only be operated on
    /// by the group admin.
    pub fn marginfi_account_set_freeze(
        ctx: Context<SetAccountFreeze>,
        frozen: bool,
    ) -> MarginfiResult {
        marginfi_account::set_account_freeze(ctx, frozen)
    }

    /// (account authority) Close a marginfi account. Requires all balances to be empty and no
    /// active flags (disabled, flashloan, receivership).
    pub fn marginfi_account_close(ctx: Context<MarginfiAccountClose>) -> MarginfiResult {
        marginfi_account::close_account(ctx)
    }

    /// (permissionless) Zero out `emissions_outstanding` on a balance after emissions are disabled
    /// on the bank.
    pub fn lending_account_clear_emissions(
        ctx: Context<LendingAccountClearEmissions>,
    ) -> MarginfiResult {
        marginfi_account::lending_account_clear_emissions(ctx)
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
        order_init_flat_sol_fee: u32,
        program_fee_fixed: WrappedI80F48,
        program_fee_rate: WrappedI80F48,
        liquidation_max_fee: WrappedI80F48,
        order_execution_max_fee: WrappedI80F48,
    ) -> MarginfiResult {
        marginfi_group::initialize_fee_state(
            ctx,
            admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            liquidation_flat_sol_fee,
            order_init_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
            liquidation_max_fee,
            order_execution_max_fee,
        )
    }

    /// (global fee admin only) Adjust fees, admin, or the destination wallet
    pub fn edit_global_fee_state(
        ctx: Context<EditFeeState>,
        admin: Pubkey,
        fee_wallet: Pubkey,
        bank_init_flat_sol_fee: u32,
        liquidation_flat_sol_fee: u32,
        order_init_flat_sol_fee: u32,
        program_fee_fixed: WrappedI80F48,
        program_fee_rate: WrappedI80F48,
        liquidation_max_fee: WrappedI80F48,
        order_execution_max_fee: WrappedI80F48,
    ) -> MarginfiResult {
        marginfi_group::edit_fee_state(
            ctx,
            admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            liquidation_flat_sol_fee,
            order_init_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
            liquidation_max_fee,
            order_execution_max_fee,
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

    /// (admin only) Edit the staked collateral settings for the group.
    pub fn edit_staked_settings(
        ctx: Context<EditStakedSettings>,
        settings: StakedSettingsEditConfig,
    ) -> MarginfiResult {
        marginfi_group::edit_staked_settings(ctx, settings)
    }

    /// (permissionless) Propagate updated staked settings to a staked collateral bank.
    pub fn propagate_staked_settings(ctx: Context<PropagateStakedSettings>) -> MarginfiResult {
        marginfi_group::propagate_staked_settings(ctx)
    }

    /// (permissionless) Begin receivership liquidation on an unhealthy account. Snapshots health
    /// and marks the account in receivership. Must have `end_liquidation` as the last ix in the tx.
    pub fn start_liquidation<'info>(
        ctx: Context<'_, '_, 'info, 'info, StartLiquidation<'info>>,
    ) -> MarginfiResult {
        marginfi_account::start_liquidation(ctx)
    }

    /// (liquidation_receiver, set in start_liquidation) End receivership liquidation. Validates
    /// health improved and seized assets are within fee limits. Charges a flat SOL fee.
    pub fn end_liquidation<'info>(
        ctx: Context<'_, '_, 'info, 'info, EndLiquidation<'info>>,
    ) -> MarginfiResult {
        marginfi_account::end_liquidation(ctx)
    }

    /// (risk_admin only) Begin forced deleverage on an account. Similar to start_liquidation but
    /// does not require the account to be unhealthy.
    pub fn start_deleverage<'info>(
        ctx: Context<'_, '_, 'info, 'info, StartDeleverage<'info>>,
    ) -> MarginfiResult {
        marginfi_account::start_deleverage(ctx)
    }

    /// (risk_admin only) End forced deleverage. Validates health did not worsen.
    pub fn end_deleverage<'info>(
        ctx: Context<'_, '_, 'info, 'info, EndDeleverage<'info>>,
    ) -> MarginfiResult {
        marginfi_account::end_deleverage(ctx)
    }

    /// (global_fee_admin only) Pause the protocol. Auto-expires after 30 minutes. Limited to 3
    /// pauses per day and 2 consecutive pauses.
    pub fn panic_pause(ctx: Context<PanicPause>) -> MarginfiResult {
        marginfi_group::panic_pause(ctx)
    }

    /// (global_fee_admin only) Unpause the protocol before the auto-expiry.
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

    /// (metadata admin only) Write ticker/description information for a bank on-chain. Optional, not
    /// all Banks are guaranteed to have metadata.
    pub fn write_bank_metadata(
        ctx: Context<WriteBankMetadata>,
        ticker: Option<Vec<u8>>,
        description: Option<Vec<u8>>,
    ) -> MarginfiResult {
        marginfi_group::write_bank_metadata(ctx, ticker, description)
    }

    /// (admin or delegate_limit_admin) Set the daily withdrawal limit for deleverages per group.
    pub fn configure_deleverage_withdrawal_limit(
        ctx: Context<ConfigureDeleverageWithdrawalLimit>,
        limit: u32,
    ) -> MarginfiResult {
        marginfi_group::configure_deleverage_withdrawal_limit(ctx, limit)
    }

    /// (delegate_flow_admin only) Update the deleverage daily withdraw outflow with
    /// aggregated data. The delegate flow admin aggregates
    /// `DeleverageWithdrawFlowEvent` events off-chain and calls this instruction at intervals.
    pub fn update_deleverage_withdrawals(
        ctx: Context<UpdateDeleverageWithdrawals>,
        outflow_usd: u32,
        update_seq: u64,
        event_start_slot: u64,
        event_end_slot: u64,
    ) -> MarginfiResult {
        marginfi_group::update_deleverage_withdrawals(
            ctx,
            outflow_usd,
            update_seq,
            event_start_slot,
            event_end_slot,
        )
    }

    /// (admin or delegate_limit_admin) Configure bank-level rate limits for withdraw/borrow.
    /// Rate limits track net outflow in native tokens. Deposits offset withdraws.
    /// Set to 0 to disable. Hourly and daily windows are independent.
    pub fn configure_bank_rate_limits(
        ctx: Context<ConfigureBankRateLimits>,
        hourly_max_outflow: Option<u64>,
        daily_max_outflow: Option<u64>,
    ) -> MarginfiResult {
        marginfi_group::configure_bank_rate_limits(ctx, hourly_max_outflow, daily_max_outflow)
    }

    /// (admin or delegate_limit_admin) Configure group-level rate limits for withdraw/borrow.
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

    /// (delegate_flow_admin only) Update the group rate limiter with aggregated
    /// inflow/outflow. The delegate flow admin aggregates
    /// `RateLimitFlowEvent` events off-chain, converts to USD, and calls this instruction at
    /// intervals to update group rate limiter state.
    pub fn update_group_rate_limiter(
        ctx: Context<UpdateGroupRateLimiter>,
        outflow_usd: Option<u64>,
        inflow_usd: Option<u64>,
        update_seq: u64,
        event_start_slot: u64,
        event_end_slot: u64,
    ) -> MarginfiResult {
        marginfi_group::update_group_rate_limiter(
            ctx,
            outflow_usd,
            inflow_usd,
            update_seq,
            event_start_slot,
            event_end_slot,
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
        refresh_reserve: Option<bool>,
    ) -> MarginfiResult {
        kamino::kamino_deposit(ctx, amount, refresh_reserve)
    }

    /// (user) Withdraw from a Kamino pool through a marginfi account
    /// * amount - in the collateral token (NOT liquidity token), in native decimals. Must convert
    ///     from collateral to liquidity token amounts using the current exchange rate.
    /// * flags - optional bitflags:
    ///   - bit 0 (`0x01`): withdraw all
    ///   - bit 1 (`0x02`): refresh reserve via batch refresh
    pub fn kamino_withdraw<'info>(
        ctx: Context<'_, '_, 'info, 'info, KaminoWithdraw<'info>>,
        amount: u64,
        flags: Option<u8>,
    ) -> MarginfiResult {
        kamino::kamino_withdraw(ctx, amount, flags)
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

    /// (permissionless) Harvest the specified reward index from the Kamino Farm attached to this
    /// bank. Rewards are always sent to the global fee wallet's canonical ATA.
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

    /// (permissionless) Harvest rewards from admin deposits in Drift spot markets.
    /// Rewards are always sent to the global fee wallet's canonical ATA.
    /// The harvest spot market must be different from the bank's main drift spot market.
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

    // Juplend integration instructions

    /// (admin) Add a JupLend bank to the marginfi group.
    ///
    /// Remaining accounts (for oracle validation):
    /// 0. underlying oracle feed (pyth push or switchboard pull)
    /// 1. JupLend `Lending` state
    pub fn lending_pool_add_bank_juplend(
        ctx: Context<LendingPoolAddBankJuplend>,
        bank_config: state::juplend::JuplendConfigCompact,
        bank_seed: u64,
    ) -> MarginfiResult {
        juplend::lending_pool_add_bank_juplend(ctx, bank_config, bank_seed)
    }

    /// (permissionless) Initialize the bank-level JupLend position.
    ///
    /// This creates the bank's fToken ATA (owned by the bank liquidity vault authority) and
    /// performs a nominal seed deposit into JupLend, then flips the bank from `Paused` to
    /// `Operational`.
    pub fn juplend_init_position(ctx: Context<JuplendInitPosition>, amount: u64) -> MarginfiResult {
        juplend::juplend_init_position(ctx, amount)
    }

    /// (user) Deposit into a JupLend lending pool through a marginfi account.
    /// * amount - in the underlying token (e.g., USDC), in native decimals
    pub fn juplend_deposit(ctx: Context<JuplendDeposit>, amount: u64) -> MarginfiResult {
        juplend::juplend_deposit(ctx, amount)
    }

    /// (user) Withdraw from a JupLend lending pool through a marginfi account.
    /// * amount - in the underlying token (e.g., USDC), in native decimals
    pub fn juplend_withdraw<'info>(
        ctx: Context<'_, '_, 'info, 'info, JuplendWithdraw<'info>>,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> MarginfiResult {
        juplend::juplend_withdraw(ctx, amount, withdraw_all)
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
