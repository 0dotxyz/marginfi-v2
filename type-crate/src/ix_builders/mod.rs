//! Instruction builders for the marginfi program.
//!
//! Each instruction has an accounts struct mirroring its on-chain `Accounts` layout and a
//! builder returning a ready-to-send [`Instruction`]. Builders emit only the fixed accounts.
//! Many instructions additionally read `remaining_accounts`; each builder's doc states the
//! layout it expects, and [`crate::pdas::bank_observation_keys`] derives the oracle set a bank
//! contributes to a health check.
//!
//! Every instruction is addressed to [`crate::ID`], which the network feature selects; clients
//! that pick a cluster at runtime retarget with [`with_program_id`]. Struct names follow the
//! instruction rather than the program's `Accounts` struct, so the IDL's `PulseHealth` is
//! [`lending::LendingAccountPulseHealth`] here.
//!
//! Coverage is 96 of the program's 103 instructions; `misc::ix_parity` fails if a new one lands
//! without a builder or an entry in that test's `NO_BUILDER` allowlist. The omissions are
//! governance and migration entrypoints driven by an admin runbook rather than a client: group
//! fee config, permissionless bank creation, staked-bank vote-account backfill, bank clone and
//! close, tokenless repay completion, and deleverage purge.
//!
//! [`Instruction`]: solana_instruction::Instruction

pub mod account;
pub mod admin;
pub mod drift;
pub mod juplend;
pub mod kamino;
pub mod lending;
pub mod liquidation;
pub mod order;
pub mod pool;
pub mod solend;

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Account metas in the exact order the program expects them.
pub trait ToAccountMetas {
    fn to_account_metas(&self) -> Vec<AccountMeta>;
}

/// Retargets a built instruction at another deployment of the program, for clients that pick
/// their cluster at runtime rather than through the network feature.
///
/// An omitted optional account is encoded as a meta addressed to the program itself, and anchor
/// only reads it as `None` when it matches the invoked program, so those metas move too.
pub fn with_program_id(mut ix: Instruction, program_id: Pubkey) -> Instruction {
    let previous = ix.program_id;
    for meta in ix.accounts.iter_mut() {
        if meta.pubkey == previous {
            meta.pubkey = program_id;
        }
    }
    ix.program_id = program_id;
    ix
}

/// Every instruction that has a builder here. The `use` in the generated module makes each name
/// resolve to a real function, so an entry cannot outlive the builder it names.
macro_rules! declare_builders {
    ($($module:ident :: $name:ident),* $(,)?) => {
        pub const BUILDERS: &[&str] = &[$(stringify!($name)),*];

        #[allow(unused_imports)]
        mod builder_exists {
            $(pub use super::$module::$name;)*
        }
    };
}

declare_builders! {
    account::admin_close_account,
    account::marginfi_account_close,
    account::marginfi_account_close_liq_record,
    account::marginfi_account_init_liq_record,
    account::marginfi_account_initialize,
    account::marginfi_account_initialize_pda,
    account::marginfi_account_set_freeze,
    account::marginfi_account_update_emissions_destination_account,
    account::transfer_to_new_account,
    account::transfer_to_new_account_pda,
    admin::configure_bank_rate_limits,
    admin::configure_deleverage_withdrawal_limit,
    admin::configure_group_rate_limits,
    admin::disable_staked_oracles,
    admin::edit_global_fee_state,
    admin::edit_staked_settings,
    admin::enable_staked_oracle_onramp,
    admin::init_global_fee_state,
    admin::init_staked_settings,
    admin::panic_pause,
    admin::panic_unpause,
    admin::panic_unpause_permissionless,
    admin::propagate_fee_state,
    admin::propagate_staked_settings,
    admin::resize_global_fee_state,
    admin::super_admin_deposit,
    admin::super_admin_withdraw,
    admin::update_deleverage_withdrawals,
    admin::update_group_rate_limiter,
    drift::drift_claim_bad_debt,
    drift::drift_deposit,
    drift::drift_harvest_reward,
    drift::drift_init_user,
    drift::drift_withdraw,
    drift::lending_pool_add_bank_drift,
    juplend::juplend_deposit,
    juplend::juplend_init_position,
    juplend::juplend_withdraw,
    juplend::lending_pool_add_bank_juplend,
    kamino::kamino_deposit,
    kamino::kamino_harvest_reward,
    kamino::kamino_init_obligation,
    kamino::kamino_withdraw,
    kamino::lending_pool_add_bank_kamino,
    lending::lending_account_borrow,
    lending::lending_account_close_balance,
    lending::lending_account_deposit,
    lending::lending_account_end_flashloan,
    lending::lending_account_liquidate,
    lending::lending_account_pulse_health,
    lending::lending_account_repay,
    lending::lending_account_start_flashloan,
    lending::lending_account_withdraw,
    liquidation::end_deleverage,
    liquidation::end_liquidation,
    liquidation::start_deleverage,
    liquidation::start_liquidation,
    order::marginfi_account_close_order,
    order::marginfi_account_end_execute_order,
    order::marginfi_account_keeper_close_order,
    order::marginfi_account_place_order,
    order::marginfi_account_set_keeper_close_flags,
    order::marginfi_account_start_execute_order,
    pool::init_bank_metadata,
    pool::lending_pool_accrue_bank_interest,
    pool::lending_pool_add_bank,
    pool::lending_pool_add_bank_with_seed,
    pool::lending_pool_backfill_bank_is_t22_flag,
    pool::lending_pool_clear_circuit_breaker,
    pool::lending_pool_clone_emode,
    pool::lending_pool_collect_bank_fees,
    pool::lending_pool_configure_bank,
    pool::lending_pool_configure_bank_emode,
    pool::lending_pool_configure_bank_interest_only,
    pool::lending_pool_configure_bank_limits_only,
    pool::lending_pool_configure_bank_oracle,
    pool::lending_pool_emissions_deposit,
    pool::lending_pool_handle_bankruptcy,
    pool::lending_pool_init_same_asset_emode_registry,
    pool::lending_pool_pulse_bank_price_cache,
    pool::lending_pool_resize_group_account,
    pool::lending_pool_set_bank_same_asset_emode_eligibility,
    pool::lending_pool_set_fixed_oracle_price,
    pool::lending_pool_update_fees_destination_account,
    pool::lending_pool_withdraw_fees,
    pool::lending_pool_withdraw_fees_permissionless,
    pool::lending_pool_withdraw_insurance,
    pool::marginfi_group_configure,
    pool::marginfi_group_initialize,
    pool::sync_indexer_flags,
    pool::write_bank_metadata,
    pool::write_bank_metadata_pre_init,
    solend::lending_pool_add_bank_solend,
    solend::solend_deposit,
    solend::solend_init_obligation,
    solend::solend_withdraw,
}
