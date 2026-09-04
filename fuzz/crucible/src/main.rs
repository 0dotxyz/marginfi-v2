// SCOUT:TESTS:BEGIN
// Dev-time reachability/guard/regression tests removed for the client deliverable:
//   18478 lines / 561 #[test] fns (action-reaches-handler, negative-guard, and throwaway
//   zz_* diagnostics). All were #[cfg(test)]-only, so the release fuzzer binary is byte-identical.
//   Recover from git history if needed; they are NOT part of the shipped invariant harness.
// SCOUT:TESTS:END
// Generated harness: one action per instruction. Setup glue and TODOs are hand-edited.
use crucible_test_context::*;
use crucible_fuzzer::anchor_lang::system_program;
use crucible_fuzzer::*;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::rc::Rc;

// SCOUT:CHECK-CONTRACT:BEGIN sha256=c4b20795d13638b9cbca54acc8669b4394eb8494fe1116eb26b75f0b968aaf9e
// Semantic invariant checks have two modes:
//   default / SCOUT_CHECK_MODE=enforce: record a real Crucible fuzz violation;
//   SCOUT_CHECK_MODE=observe: emit nonce-bound reachability markers, never a violation.
// This exact alias is part of the trusted contract.  Generated setup and the
// macros below use `crate::`/`$crate` paths so a mutable prelude cannot replace
// Crucible's TestContext or violation/session functions with local lookalikes.
#[doc(hidden)]
extern crate crucible_test_context as __scout_crucible_test_context;

fn __scout_check_observe_mode() -> bool {
    // Cached: these audit switches are fixed for the process lifetime, but this runs on EVERY
    // property check (~89/action). Uncached getenv here cost ~1.2% CPU + a shared libc lock
    // (__findenv_locked) that serialises multicore workers — profiled 2026-08-19.
    static OBSERVE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OBSERVE.get_or_init(|| std::env::var("SCOUT_CHECK_MODE").as_deref() == Ok("observe"))
}

// Mute a property whose finding is already investigated and written up. Such a property keeps
// firing on the SAME known defect and floods the objective, hiding every other property's first
// finding behind thousands of duplicates -- observed at ~160 crashes per 25s on one target.
//
// Muting is ALWAYS announced on stderr, once per process. A silently disabled check is the exact
// false-negative trap this pipeline exists to avoid: a muted property is indistinguishable from a
// passing one unless the run says so out loud. `SCOUT_CHECK_MUTE` is also stripped from ordinary
// fuzz subprocesses alongside the other audit switches, so a stray shell variable can never
// quietly disable a check -- a caller must pass it explicitly.
fn __scout_check_announce_mutes(list: &str) {
    static MUTE_ONCE: std::sync::Once = std::sync::Once::new();
    MUTE_ONCE.call_once(|| {
        eprintln!("[SCOUT_CHECK_MUTED] {}", list);
    });
}

fn __scout_check_muted(property: &str) -> bool {
    // Cached parse (see __scout_check_observe_mode): O(1) set lookup instead of getenv + split on
    // every check. Behaviour-identical — SCOUT_CHECK_MUTE is constant for the process.
    static MUTED: std::sync::OnceLock<Option<(String, std::collections::HashSet<String>)>> =
        std::sync::OnceLock::new();
    let cached = MUTED.get_or_init(|| {
        std::env::var("SCOUT_CHECK_MUTE").ok().map(|list| {
            let set = list.split(',').map(|e| e.trim().to_string()).collect();
            (list, set)
        })
    });
    match cached {
        Some((list, set)) => {
            let muted = set.contains(property);
            if muted {
                __scout_check_announce_mutes(list);
            }
            muted
        }
        None => false,
    }
}

fn __scout_check_selected(property: &str) -> bool {
    if __scout_check_muted(property) {
        return false;
    }
    static ONLY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    match ONLY.get_or_init(|| std::env::var("SCOUT_CHECK_ONLY").ok()) {
        Some(selected) => selected == property,
        None => true,
    }
}

fn __scout_check_nonce() -> Result<String, &'static str> {
    let nonce = std::env::var("SCOUT_CHECK_RUN")
        .map_err(|_| "missing or non-Unicode SCOUT_CHECK_RUN")?;
    if nonce.is_empty() {
        return Err("empty SCOUT_CHECK_RUN");
    }
    if !nonce.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-')
    }) {
        return Err("SCOUT_CHECK_RUN contains unsafe characters");
    }
    Ok(nonce)
}

fn __scout_check_emit_error(reason: &str) {
    static ERROR_ONCE: std::sync::Once = std::sync::Once::new();
    ERROR_ONCE.call_once(|| {
        // Never echo an invalid value: whitespace/newlines would forge protocol fields.
        eprintln!("[SCOUT_CHECK_ERROR] INVALID {}", reason);
    });
}

macro_rules! scout_check_session {
    () => {{
        if $crate::__scout_check_observe_mode() {
            // Coverage-only replay runs before Crucible's stateful initializer.  Set
            // this per-thread flag here so failed actions terminate accumulated chains
            // exactly as they did in the stateful campaign that produced the corpus.
            $crate::__scout_crucible_test_context::set_stateful_chain_mode(true);
            static SESSION_ONCE: std::sync::Once = std::sync::Once::new();
            SESSION_ONCE.call_once(|| {
                match $crate::__scout_check_nonce() {
                    Ok(nonce) => eprintln!("[SCOUT_CHECK_SESSION] {}", nonce),
                    Err(reason) => $crate::__scout_check_emit_error(reason),
                }
            });
        }
    }};
}

// Gate the *entire* property computation, not only its final predicate.  This
// prevents another property's fallible reads, eligibility logic, or shadow-hook
// arithmetic from panicking/starving an isolated SCOUT_CHECK_ONLY replay.
macro_rules! scout_run_property {
    ($property:literal, $expression:expr $(,)?) => {{
        if $crate::__scout_check_selected($property) {
            let _ = $expression;
        }
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __scout_check_impl {
    ($property:literal, $site:literal, $predicate:expr, $message:expr) => {{
        let __scout_observe = $crate::__scout_check_observe_mode();
        if !$crate::__scout_check_selected($property) {
            true
        } else {
            let __scout_nonce = if __scout_observe {
                Some($crate::__scout_check_nonce())
            } else {
                None
            };
            if let Some(Err(ref __scout_error)) = __scout_nonce {
                // An invalid session can never produce an EVALUATED marker.  The
                // mechanical verifier therefore cannot mistake it for sound evidence.
                $crate::__scout_check_emit_error(__scout_error);
                false
            } else {
                // Keep the predicate in one lexical/runtime position.  Expressions
                // with reads or counters are evaluated exactly once per selected check.
                let __scout_check_result: bool = $predicate;
                if let Some(Ok(ref __scout_run)) = __scout_nonce {
                    eprintln!(
                        "[SCOUT_CHECK_EVALUATED] {} {} {} {}:{}",
                        __scout_run, $property, $site, file!(), line!()
                    );
                    if !__scout_check_result {
                        eprintln!(
                            "[SCOUT_CHECK_WOULD_VIOLATE] {} {} {} {}:{}",
                            __scout_run, $property, $site, file!(), line!()
                        );
                    }
                } else if !__scout_check_result {
                    $crate::__scout_crucible_test_context::record_violation($message);
                }
                __scout_check_result
            }
        }
    }};
}

macro_rules! scout_check {
    ($property:literal, $site:literal, $predicate:expr $(,)?) => {{
        $crate::__scout_check_impl!(
            $property,
            $site,
            $predicate,
            format!(
                "Invariant {} check {} failed at {}:{}",
                $property, $site, file!(), line!()
            )
        )
    }};
    ($property:literal, $site:literal, $predicate:expr, $($arg:tt)+) => {{
        $crate::__scout_check_impl!($property, $site, $predicate, format!($($arg)+))
    }};
}
// SCOUT:CHECK-CONTRACT:END

const SCOUT_TARGET_PROGRAM_ARTIFACT: &str = "programs/marginfi_program.so";

// SCOUT:BINDINGS:BEGIN
// fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0
// MarginfiGroupInitialize.marginfi_group = signer:Keypair::new()
// marginfi_group = self.marginfi_group

// MarginfiAccountInitialize.marginfi_account = signer:Keypair::new()
// MarginfiAccountInitializePda.marginfi_account = Pubkey::find_program_address(&[&[], marginfi_group.as_ref(), authority.as_ref(), &[], &[]], &self.program_id).0
// marginfi_account = self.marginfi_account

// staked_settings = Pubkey::find_program_address(&[STAKED_SETTINGS_SEED, self.marginfi_group.as_ref()], &self.program_id).0
// InitStakedSettings.settings = marginfi::types::StakedSettingsConfig { oracle: Pubkey::new_unique(), asset_weight_init: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.5)), asset_weight_maint: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.6)), deposit_limit: 1_000_000_000, total_asset_value_init_limit: 10_000_000_000, oracle_max_age: 60, risk_tier: marginfi::types::RiskTier::Collateral }

// InitGlobalFeeState.admin = self.payer.pubkey()
// EditGlobalFeeState.admin = self.payer.pubkey()

// bank_mint = self.bank_mint

// global_fee_wallet = self.global_fee_wallet
// InitGlobalFeeState.fee_wallet = self.global_fee_wallet

// LendingPoolAddBank.bank = signer:Keypair::new()
// bank = self.bank

// liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0
// liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0
// insurance_vault_authority = Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0
// insurance_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &self.program_id).0
// fee_vault_authority = Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0
// fee_vault = Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &self.program_id).0

// token_program = spl_token::id()

// LendingPoolAddBank.bank_config = scout_valid_bank_config(10)

// group = self.marginfi_group

// LendingAccountDeposit.signer_token_account = self.signer_token_account

// LendingPoolConfigureBank.bank_config_opt = scout_valid_bank_config_opt()

// old_marginfi_account = self.marginfi_account

// PropagateStakedSettings.marginfi_group = self.staked_group
// PropagateStakedSettings.staked_settings = self.staked_settings
// PropagateStakedSettings.bank = self.staked_bank

// LendingPoolCloneBank.bank = Pubkey::find_program_address(&[marginfi_group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0

// LendingPoolAddBankWithSeed.bank = Pubkey::find_program_address(&[marginfi_group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0
// LendingPoolAddBankWithSeed.bank_config = scout_valid_bank_config(10)

// LendingPoolCloneEmode.copy_from_bank = self.bank
// LendingPoolCloneEmode.copy_to_bank = self.clone_emode_bank

// LendingPoolAddBankWithSeed.bank = Pubkey::find_program_address(&[marginfi_group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0
// LendingPoolAddBankWithSeed.bank_config = scout_valid_bank_config(10)

// LendingPoolForceTokenlessRepayComplete.bank = self.tokenless_bank

// emissions_mint = self.emissions_mint
// emissions_auth = Pubkey::find_program_address(&[EMISSIONS_AUTH_SEED, bank.as_ref(), emissions_mint.as_ref()], &self.program_id).0
// emissions_token_account = Pubkey::find_program_address(&[EMISSIONS_TOKEN_ACCOUNT_SEED, bank.as_ref(), emissions_mint.as_ref()], &self.program_id).0
// emissions_funding_account = self.emissions_funding_account

// LendingAccountWithdraw.marginfi_account = self.withdraw_marginfi_account
// LendingAccountWithdraw.bank = self.withdraw_bank
// LendingAccountWithdraw.destination_token_account = self.signer_token_account
// LendingAccountWithdraw.bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0
// LendingAccountWithdraw.amount = SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT

// LendingPoolWithdrawFees.dst_token_account = self.fee_withdraw_dst_token_account
// LendingPoolWithdrawFees.amount = 0



// LendingAccountBorrow.destination_token_account = self.signer_token_account
// LendingAccountBorrow.bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0
// LendingAccountBorrow.amount = 0

// LendingPoolUpdateFeesDestinationAccount.destination_account = self.fee_withdraw_dst_token_account
// LendingPoolWithdrawFeesPermissionless.fees_destination_account = self.fee_withdraw_dst_token_account

// LendingPoolWithdrawInsurance.dst_token_account = self.fee_withdraw_dst_token_account
// LendingPoolWithdrawInsurance.amount = 0

// MarginfiAccountClose.marginfi_account = match self.scout_prepare_close_marginfi_account() { Some(v) => v, None => return false }

// EditGlobalFeeState.fee_wallet = self.global_fee_wallet

// MarginfiAccountClose.marginfi_account = match self.scout_prepare_close_marginfi_account() { Some(v) => v, None => return false }
// MarginfiAccountInitLiqRecord.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// StartLiquidation.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// StartLiquidation.liquidation_receiver = self.payer.pubkey()

// MarginfiAccountInitLiqRecord.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// StartDeleverage.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// EndDeleverage.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)

// InitBankMetadata.metadata = Pubkey::find_program_address(&[METADATA_SEED, bank.as_ref()], &self.program_id).0
// WriteBankMetadata.metadata = Pubkey::find_program_address(&[METADATA_SEED, bank.as_ref()], &self.program_id).0
// WriteBankMetadata.ticker = Some(b"SCOUT".to_vec())
// WriteBankMetadata.description = Some(b"Scout bank metadata".to_vec())

// MarginfiAccountClose.marginfi_account = match self.scout_prepare_close_marginfi_account() { Some(v) => v, None => return false }

// MarginfiAccountInitLiqRecord.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// StartDeleverage.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// EndDeleverage.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)

// LendingPoolAddBankKamino.bank = scout_seeded_bank_pda(self.program_id, group, bank_mint, bank_seed)
// LendingPoolAddBankKamino.integration_acc_1 = self.scout_ensure_kamino_reserve()
// LendingPoolAddBankKamino.integration_acc_2 = scout_kamino_obligation_pda(liquidity_vault_authority, scout_kamino_lending_market_pda(self.program_id))
// LendingPoolAddBankKamino.bank_config = scout_valid_kamino_config(self.scout_ensure_kamino_oracle())
// LendingPoolAddBankKamino.integration_acc_2 = scout_kamino_obligation_pda(Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0, scout_kamino_lending_market_pda(self.program_id))

// LendingPoolAddBankDrift.bank = Pubkey::find_program_address(&[group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0

// DriftDeposit.bank = self.scout_prepare_drift_deposit_bank()
// DriftDeposit.signer_token_account = self.signer_token_account
// DriftDeposit.integration_acc_1 = self.scout_drift_spot_market_for_bank(bank)
// DriftDeposit.integration_acc_2 = self.scout_drift_user_for_bank(bank)

// LendingPoolAddBankDrift.bank = Pubkey::find_program_address(&[group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0

// LendingPoolAddBankSolend.bank = scout_seeded_bank_pda(self.program_id, group, bank_mint, bank_seed)

// SolendDeposit.bank = self.scout_ensure_solend_deposit_fixture()
// SolendDeposit.amount = 0
// SolendDeposit.signer_token_account = self.signer_token_account
// SolendDeposit.integration_acc_2 = scout_solend_obligation(self.program_id, bank)
// SolendDeposit.lending_market = scout_solend_lending_market(self.program_id)
// SolendDeposit.lending_market_authority = scout_solend_lending_market_authority(self.program_id)
// SolendDeposit.integration_acc_1 = scout_solend_reserve(self.program_id)
// SolendDeposit.mint = self.bank_mint
// SolendDeposit.reserve_liquidity_supply = scout_solend_reserve_liquidity_supply(self.program_id)
// SolendDeposit.reserve_collateral_mint = scout_solend_reserve_collateral_mint(self.program_id)
// SolendDeposit.reserve_collateral_supply = scout_solend_reserve_collateral_supply(self.program_id)
// SolendDeposit.user_collateral = scout_solend_user_collateral(self.program_id)
// SolendDeposit.pyth_price = scout_solend_pyth_price(self.program_id)
// SolendDeposit.switchboard_feed = scout_solend_switchboard_feed(self.program_id)

// MarginfiAccountInitLiqRecord.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// StartDeleverage.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// EndDeleverage.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// KaminoDeposit.reserve_destination_deposit_collateral = scout_kamino_reserve_destination_deposit_collateral_fixed(self.program_id)
// DriftWithdraw.marginfi_account = self.scout_prepare_drift_withdraw_marginfi_account()
// DriftWithdraw.bank = self.scout_prepare_drift_withdraw_bank()
// DriftWithdraw.amount = 0

// LendingPoolSetFixedOraclePrice.price = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(1))

// MarginfiAccountUpdateEmissionsDestinationAccount.destination_account = self.global_fee_wallet


// TransferToNewAccountPda.third_party_id = None
// TransferToNewAccountPda.new_marginfi_account = Pubkey::find_program_address(&[SCOUT_MARGINFI_ACCOUNT_SEED, group.as_ref(), new_authority.as_ref(), &account_index.to_le_bytes(), &third_party_id.unwrap_or(0).to_le_bytes()], &self.program_id).0
// TransferToNewAccountPda.instructions_sysvar = anchor_lang::solana_program::sysvar::instructions::id()


// EndLiquidation.liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account)
// EndLiquidation.liquidation_receiver = self.payer.pubkey()
// EndLiquidation.global_fee_wallet = self.global_fee_wallet

// KaminoWithdraw.amount = 0

// KaminoHarvestReward.bank = self.scout_prepare_kamino_harvest_bank()
// KaminoHarvestReward.reward_index = 0
// KaminoHarvestReward.destination_token_account = self.scout_ensure_kamino_harvest_destination_token_account()
// KaminoHarvestReward.user_state = self.scout_ensure_kamino_harvest_system_account(b"scout_kamino_harvest_user_state")
// KaminoHarvestReward.farm_state = self.scout_ensure_kamino_harvest_system_account(b"scout_kamino_harvest_farm_state")
// KaminoHarvestReward.global_config = self.scout_ensure_kamino_harvest_system_account(b"scout_kh_global_config")
// KaminoHarvestReward.reward_mint = self.scout_ensure_kamino_harvest_reward_mint()
// KaminoHarvestReward.user_reward_ata = self.scout_ensure_kamino_harvest_user_reward_ata(bank)
// KaminoHarvestReward.rewards_vault = self.scout_ensure_kamino_harvest_rewards_vault()
// KaminoHarvestReward.rewards_treasury_vault = self.scout_ensure_kamino_harvest_rewards_treasury_vault()
// KaminoHarvestReward.farm_vaults_authority = self.scout_ensure_kamino_harvest_farm_vaults_authority()
// KaminoHarvestReward.scope_prices = None
// KaminoHarvestReward.farms_program = self.scout_ensure_kamino_harvest_farms_program()

// DriftHarvestReward.bank = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.bank).unwrap_or(self.bank)
// DriftHarvestReward.fee_state = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.fee_state).unwrap_or(Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0)
// DriftHarvestReward.liquidity_vault_authority = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.liquidity_vault_authority).unwrap_or(Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0)
// DriftHarvestReward.intermediary_token_account = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.intermediary_token_account).unwrap_or(self.signer_token_account)
// DriftHarvestReward.destination_token_account = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.destination_token_account).unwrap_or(self.fee_withdraw_dst_token_account)
// DriftHarvestReward.drift_state = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.drift_state).unwrap_or(scout_drift_state())
// DriftHarvestReward.integration_acc_2 = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.integration_acc_2).unwrap_or(scout_drift_user(scout_drift_program_id(), bank))
// DriftHarvestReward.integration_acc_3 = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.integration_acc_3).unwrap_or(scout_drift_user_stats(scout_drift_program_id(), bank))
// DriftHarvestReward.harvest_drift_spot_market = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.harvest_drift_spot_market).unwrap_or(scout_drift_spot_market())
// DriftHarvestReward.harvest_drift_spot_market_vault = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.harvest_drift_spot_market_vault).unwrap_or(scout_drift_spot_market_vault())
// DriftHarvestReward.drift_signer = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.drift_signer).unwrap_or(scout_drift_signer())
// DriftHarvestReward.reward_mint = self.scout_prepare_drift_harvest_reward_accounts(true).map(|a| a.reward_mint).unwrap_or(self.emissions_mint)
// DriftHarvestReward.drift_program = scout_drift_program_id()

// SolendWithdraw.bank = self.scout_prepare_solend_withdraw_bank()
// SolendWithdraw.withdraw_all = Some(true)
// SolendWithdraw.destination_token_account = self.scout_ensure_solend_withdraw_destination_token_account()
// SolendWithdraw.integration_acc_2 = scout_solend_obligation(self.program_id, bank)
// SolendWithdraw.lending_market = scout_solend_lending_market(self.program_id)
// SolendWithdraw.lending_market_authority = scout_solend_lending_market_authority(self.program_id)
// SolendWithdraw.integration_acc_1 = scout_solend_reserve(self.program_id)
// SolendWithdraw.mint = self.bank_mint
// SolendWithdraw.reserve_liquidity_supply = scout_solend_reserve_liquidity_supply(self.program_id)
// SolendWithdraw.reserve_collateral_mint = scout_solend_reserve_collateral_mint(self.program_id)
// SolendWithdraw.reserve_collateral_supply = scout_solend_reserve_collateral_supply(self.program_id)
// SolendWithdraw.user_collateral = scout_solend_user_collateral(self.program_id)

// LendingAccountRepay.signer_token_account = self.signer_token_account
// LendingAccountRepay.repay_all = Some(false)

// EndDeleverage.marginfi_account = match self.scout_prepare_end_deleverage_marginfi_account() { Some(v) => v, None => return false }

// LendingAccountCloseBalance.marginfi_account = match self.scout_prepare_lending_account_close_balance_marginfi_account() { Some(v) => v, None => return false }


// LendingAccountRepay.marginfi_account = match self.scout_create_lending_account_repay_marginfi_account() { Some(v) => v, None => return false }
// LendingAccountRepay.bank = match self.scout_create_lending_account_repay_bank_with_liability(marginfi_account, false, amount) { Some(v) => v, None => return false }

// LendingAccountWithdraw.withdraw_all = Some(true)

// LendingPoolHandleBankruptcy.marginfi_account = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.0).unwrap_or(self.marginfi_account)
// LendingPoolHandleBankruptcy.bank = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.1).unwrap_or(self.bank)
// LendingPoolHandleBankruptcy.liquidity_vault = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.2).unwrap_or(Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0)
// LendingPoolHandleBankruptcy.insurance_vault = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.3).unwrap_or(Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &self.program_id).0)
// LendingPoolHandleBankruptcy.insurance_vault_authority = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.4).unwrap_or(Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0)

// EndLiquidation.marginfi_account = match self.scout_prepare_end_liquidation_marginfi_account_receivership() { Some(v) => v, None => return false }

// LendingPoolCollectBankFees.fee_ata = self.scout_prepare_collect_bank_fees(),


// LendingPoolCloseBank.bank = match self.scout_mint_lending_pool_close_bank_guard_bank(true, 0, fixed::types::I80F48::ZERO, fixed::types::I80F48::ZERO) { Some(v) => v, None => return false }

// EditGlobalFeeState.program_fee_rate = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.1))

// LendingAccountBorrow.amount = SCOUT_BORROW_AMOUNT
// LendingAccountBorrow.marginfi_account = self.borrow_marginfi_account
// LendingAccountBorrow.bank = self.borrow_liab_bank
// LendingAccountBorrow.remaining_accounts = self.borrow_remaining_accounts.clone()

// LendingAccountRepay.repay_all = Some(true)
// LendingAccountRepay.marginfi_account = self.borrow_marginfi_account
// LendingAccountRepay.bank = match self.scout_borrow_scenario_ensure_liability() { Some(v) => v, None => return false }

// LendingPoolHandleBankruptcy.remaining_accounts = vec![bank]

// LendingPoolAddBankPermissionless.marginfi_group = self.staked_group
// LendingPoolAddBankPermissionless.staked_settings = self.staked_settings
// LendingPoolAddBankPermissionless.bank_mint = match self.scout_prepare_add_bank_permissionless() { Some(v) => v, None => return false }
// LendingPoolAddBankPermissionless.sol_pool = Pubkey::find_program_address(&[b"stake", self.perm_stake_pool.as_ref()], &spl_single_pool_id()).0
// LendingPoolAddBankPermissionless.stake_pool = self.perm_stake_pool
// LendingPoolAddBankPermissionless.bank = Pubkey::find_program_address(&[marginfi_group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0
// LendingPoolAddBankPermissionless.remaining_accounts = vec![self.staked_oracle, bank_mint, sol_pool]

// SCOUT:BINDINGS:END

// SCOUT:PRELUDE:BEGIN
extern crate anchor_lang as __scout_anchor_lang;

mod anchor_lang {
    pub use crate::__scout_anchor_lang::*;

    pub mod solana_program {
        pub use crate::__scout_anchor_lang::solana_program::*;

        pub mod sysvar {
            pub use crate::__scout_anchor_lang::solana_program::sysvar::*;

            pub mod instructions {
                pub use crate::__scout_anchor_lang::solana_program::sysvar::instructions::*;

                pub fn id() -> solana_pubkey::Pubkey {
                    <crate::__scout_anchor_lang::prelude::Instructions as crate::__scout_anchor_lang::solana_program::sysvar::SysvarId>::id()
                }
            }
        }
    }
}

const MAX_EMODE_ENTRIES: usize = 10;
const FEE_STATE_SEED: &[u8] = b"feestate";
const STAKED_SETTINGS_SEED: &[u8] = b"staked_settings";
const LIQUIDITY_VAULT_AUTHORITY_SEED: &[u8] = b"liquidity_vault_auth";
const LIQUIDITY_VAULT_SEED: &[u8] = b"liquidity_vault";
const INSURANCE_VAULT_AUTHORITY_SEED: &[u8] = b"insurance_vault_auth";
const INSURANCE_VAULT_SEED: &[u8] = b"insurance_vault";
const FEE_VAULT_AUTHORITY_SEED: &[u8] = b"fee_vault_auth";
const FEE_VAULT_SEED: &[u8] = b"fee_vault";

fn scout_valid_bank_config(oracle_max_age: u16) -> marginfi::types::BankConfigCompact {
    let zero = || marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ZERO);
    let one = || marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ONE);
    marginfi::types::BankConfigCompact {
        asset_weight_init: zero(),
        asset_weight_maint: zero(),
        liability_weight_init: one(),
        liability_weight_maint: one(),
        deposit_limit: 0,
        interest_rate_config: marginfi::types::InterestRateConfigCompact {
            insurance_fee_fixed_apr: zero(),
            insurance_ir_fee: zero(),
            protocol_fixed_fee_apr: zero(),
            protocol_ir_fee: zero(),
            protocol_origination_fee: zero(),
            zero_util_rate: 0,
            hundred_util_rate: 0,
            points: [
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
            ],
        },
        operational_state: marginfi::types::BankOperationalState::Operational,
        borrow_limit: 0,
        risk_tier: marginfi::types::RiskTier::Isolated,
        asset_tag: 0,
        config_flags: 0,
        _pad0: [0u8; 5],
        total_asset_value_init_limit: 0,
        oracle_max_age,
        oracle_max_confidence: 0,
    }
}

fn scout_valid_bank_config_opt() -> marginfi::types::BankConfigOpt {
    let zero = || marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ZERO);
    let one = || marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ONE);
    marginfi::types::BankConfigOpt {
        asset_weight_init: Some(zero()),
        asset_weight_maint: Some(zero()),
        liability_weight_init: Some(one()),
        liability_weight_maint: Some(one()),
        deposit_limit: Some(1_000_000_000),
        borrow_limit: Some(500_000_000),
        operational_state: Some(marginfi::types::BankOperationalState::Operational),
        interest_rate_config: Some(marginfi::types::InterestRateConfigOpt {
            insurance_fee_fixed_apr: Some(zero()),
            insurance_ir_fee: Some(zero()),
            protocol_fixed_fee_apr: Some(zero()),
            protocol_ir_fee: Some(zero()),
            protocol_origination_fee: Some(zero()),
            zero_util_rate: Some(0),
            hundred_util_rate: Some(0),
            points: Some([
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
            ]),
        }),
        risk_tier: Some(marginfi::types::RiskTier::Isolated),
        asset_tag: Some(0),
        total_asset_value_init_limit: Some(10_000_000_000),
        oracle_max_confidence: Some(500_000_000),
        oracle_max_age: Some(15),
        permissionless_bad_debt_settlement: Some(false),
        freeze_settings: Some(false),
        tokenless_repayments_allowed: Some(false),
        liquidation_liquidator_fee: None,
        liquidation_insurance_fee: None,
        circuit_breaker_enabled: None,
        cb_deviation_bps_tiers: None,
        cb_tier_durations_seconds: None,
        cb_escalation_window_mult: None,
        cb_ema_alpha_bps: None,
        cb_window_seconds: None,
        cb_window_max_up_bps: None,
        cb_window_max_down_bps: None,
    }
}

// MarginfiAccount.account_flags offset: 8 (discriminator) + group(32) + authority(32) + lending_account(1728).
const MARGINFI_ACCOUNT_FLAGS_OFFSET: usize = 8 + 32 + 32 + 1728;
const SCOUT_ACCOUNT_IN_FLASHLOAN: u64 = 1 << 1;
const SCOUT_ACCOUNT_IN_RECEIVERSHIP: u64 = 1 << 4;

fn spl_single_pool_id() -> Pubkey {
    "SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE".parse().unwrap()
}
fn vote_program_id() -> Pubkey {
    "Vote111111111111111111111111111111111111111".parse().unwrap()
}
fn native_stake_id() -> Pubkey {
    "Stake11111111111111111111111111111111111111".parse().unwrap()
}
fn pyth_receiver_program_id() -> Pubkey {
    "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ".parse().unwrap()
}

fn scout_price_update_v2_bytes(feed_id: [u8; 32], price: i64, publish_time: i64) -> Vec<u8> {
    // Anchor discriminator: sha256("account:PriceUpdateV2")[..8]
    const DISCRIMINATOR: [u8; 8] = [34, 241, 35, 99, 157, 126, 244, 205];
    let mut bytes = Vec::with_capacity(8 + 32 + 1 + 32 + 8 + 8 + 4 + 8 + 8 + 8 + 8 + 8);
    bytes.extend_from_slice(&DISCRIMINATOR);
    bytes.extend_from_slice(&Pubkey::default().to_bytes()); // write_authority
    bytes.push(1u8); // verification_level: VerificationLevel::Full
    bytes.extend_from_slice(&feed_id); // price_message.feed_id
    bytes.extend_from_slice(&price.to_le_bytes()); // price_message.price
    bytes.extend_from_slice(&1u64.to_le_bytes()); // price_message.conf
    bytes.extend_from_slice(&(-8i32).to_le_bytes()); // price_message.exponent
    bytes.extend_from_slice(&publish_time.to_le_bytes()); // price_message.publish_time
    bytes.extend_from_slice(&publish_time.to_le_bytes()); // price_message.prev_publish_time
    bytes.extend_from_slice(&price.to_le_bytes()); // price_message.ema_price
    bytes.extend_from_slice(&1u64.to_le_bytes()); // price_message.ema_conf
    bytes.extend_from_slice(&0u64.to_le_bytes()); // posted_slot
    bytes
}
// EMISSION_FLAGS: bitmask lending_pool_setup_emissions's `flags` arg must be a subset of.
const EMISSIONS_AUTH_SEED: &[u8] = b"emissions_auth_seed";
const EMISSIONS_TOKEN_ACCOUNT_SEED: &[u8] = b"emissions_token_account_seed";
const EMISSION_FLAGS: u64 = 0b11;
const SCOUT_ACCOUNT_IN_DELEVERAGE: u64 = 1 << 5;
// Bank.flags offset: 8 (discriminator) + 832.
const SCOUT_BANK_FLAGS_OFFSET: usize = 8 + 832;
const SCOUT_TOKENLESS_REPAYMENTS_COMPLETE: u64 = 1 << 6;
const SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT: u64 = 1_000_000;
const SCOUT_ACCOUNT_DISABLED: u64 = 1 << 0;
const SCOUT_ACCOUNT_FROZEN: u64 = 1 << 6;

fn scout_anchor_instruction<I, A>(
    program_id: Pubkey,
    instruction: I,
    accounts: A,
) -> anchor_lang::solana_program::instruction::Instruction
where
    I: anchor_lang::InstructionData,
    A: anchor_lang::ToAccountMetas,
{
    anchor_lang::solana_program::instruction::Instruction {
        program_id,
        accounts: accounts.to_account_metas(None),
        data: instruction.data(),
    }
}

fn scout_lending_account_start_flashloan_ix(
    program_id: Pubkey,
    marginfi_account: Pubkey,
    authority: Pubkey,
    end_index: u64,
) -> anchor_lang::solana_program::instruction::Instruction {
    scout_anchor_instruction(
        program_id,
        instruction::LendingAccountStartFlashloan { end_index },
        accounts::LendingAccountStartFlashloan {
            marginfi_account,
            authority,
        },
    )
}

fn scout_lending_account_end_flashloan_ix(
    program_id: Pubkey,
    marginfi_account: Pubkey,
    group: Pubkey,
    authority: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    scout_anchor_instruction(
        program_id,
        instruction::LendingAccountEndFlashloan {},
        accounts::LendingAccountEndFlashloan {
            marginfi_account,
            group,
            authority,
        },
    )
}
const SCOUT_BANK_EMODE_OFFSET: usize = 920;
const SCOUT_EMODE_SETTINGS_LEN: usize = 424;
const SCOUT_LIQUIDATE_LIQUIDITY_DEPOSIT_AMOUNT: u64 = 2_000_000;
const SCOUT_LIQUIDATE_COLLATERAL_DEPOSIT_AMOUNT: u64 = 2_000_000;
const SCOUT_LIQUIDATE_BORROW_AMOUNT: u64 = 1_000_000;
const SCOUT_LIQUIDATE_ASSET_AMOUNT: u64 = 500_000;
const SCOUT_LIQUIDATE_MAX_ASSET_AMOUNT: u64 = 1_000_000;

fn scout_liquidation_bank_config() -> marginfi::types::BankConfigCompact {
    let mut config = scout_valid_bank_config(10);
    config.asset_weight_init =
        marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.8));
    config.asset_weight_maint =
        marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.9));
    config.risk_tier = marginfi::types::RiskTier::Collateral;
    config.deposit_limit = u64::MAX;
    config.borrow_limit = u64::MAX;
    config.total_asset_value_init_limit = u64::MAX;
    config
}

fn scout_liquidation_remaining_accounts(first_bank: Pubkey, second_bank: Pubkey) -> Vec<Pubkey> {
    let mut banks = vec![first_bank, second_bank];
    banks.sort();
    banks
}

fn scout_bank_vault_pdas(program_id: Pubkey, bank: Pubkey) -> (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey) {
    (
        Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &program_id).0,
        Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &program_id).0,
        Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank.as_ref()], &program_id).0,
        Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &program_id).0,
        Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank.as_ref()], &program_id).0,
        Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &program_id).0,
    )
}

fn scout_forged_bank_pdas(program_id: Pubkey) -> [Pubkey; 4] {
    [
        Pubkey::find_program_address(&[b"scout_handle_bankruptcy_bank"], &program_id).0,
        Pubkey::find_program_address(&[b"scout_hb_zero_debt_bank_target"], &program_id).0,
        Pubkey::find_program_address(&[b"scout_hb_zero_debt_bank_debt"], &program_id).0,
        Pubkey::find_program_address(&[b"scout_hb_missing_balance_bank"], &program_id).0,
    ]
}

fn scout_forged_bank_and_account_pdas(program_id: Pubkey) -> [Pubkey; 6] {
    [
        Pubkey::find_program_address(&[b"scout_handle_bankruptcy_bank"], &program_id).0,
        Pubkey::find_program_address(&[b"scout_handle_bankruptcy_account"], &program_id).0,
        Pubkey::find_program_address(&[b"scout_hb_zero_debt_bank_target"], &program_id).0,
        Pubkey::find_program_address(&[b"scout_hb_zero_debt_bank_debt"], &program_id).0,
        Pubkey::find_program_address(&[b"scout_hb_zero_debt_account"], &program_id).0,
        Pubkey::find_program_address(&[b"scout_hb_missing_balance_bank"], &program_id).0,
    ]
}

fn scout_lending_pool_add_bank_accounts(
    marginfi_group: Pubkey,
    admin: Pubkey,
    fee_payer: Pubkey,
    fee_state: Pubkey,
    global_fee_wallet: Pubkey,
    bank_mint: Pubkey,
    bank: Pubkey,
    liquidity_vault_authority: Pubkey,
    liquidity_vault: Pubkey,
    insurance_vault_authority: Pubkey,
    insurance_vault: Pubkey,
    fee_vault_authority: Pubkey,
    fee_vault: Pubkey,
    token_program: Pubkey,
) -> accounts::LendingPoolAddBank {
    accounts::LendingPoolAddBank {
        marginfi_group,
        admin,
        fee_payer,
        fee_state,
        global_fee_wallet,
        bank_mint,
        bank,
        liquidity_vault_authority,
        liquidity_vault,
        insurance_vault_authority,
        insurance_vault,
        fee_vault_authority,
        fee_vault,
        token_program,
    }
}
const SCOUT_COLLECT_BANK_FEES_LIQUIDITY_AMOUNT: u64 = 9_000;
const SCOUT_COLLECT_BANK_FEES_INSURANCE_AMOUNT: u64 = 1_000;
const SCOUT_COLLECT_BANK_FEES_GROUP_AMOUNT: u64 = 2_000;
const SCOUT_COLLECT_BANK_FEES_PROGRAM_AMOUNT: u64 = 3_000;

fn scout_associated_token_program_id() -> Pubkey {
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".parse().unwrap()
}

fn scout_associated_token_address(wallet: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &scout_associated_token_program_id(),
    )
    .0
}
fn scout_liquidation_record_pda(program_id: Pubkey, marginfi_account: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"liq_record", marginfi_account.as_ref()], &program_id).0
}

fn scout_start_liquidation_ix(
    program_id: Pubkey,
    marginfi_account: Pubkey,
    liquidation_record: Pubkey,
    group: Pubkey,
    liquidation_receiver: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    scout_anchor_instruction(
        program_id,
        instruction::StartLiquidation {},
        accounts::StartLiquidation {
            marginfi_account,
            liquidation_record,
            group,
            liquidation_receiver,
        },
    )
}

fn scout_end_liquidation_ix(
    program_id: Pubkey,
    marginfi_account: Pubkey,
    liquidation_record: Pubkey,
    group: Pubkey,
    liquidation_receiver: Pubkey,
    fee_state: Pubkey,
    global_fee_wallet: Pubkey,
    fee_payer: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    scout_anchor_instruction(
        program_id,
        instruction::EndLiquidation {},
        accounts::EndLiquidation {
            marginfi_account,
            liquidation_record,
            group,
            liquidation_receiver,
            fee_state,
            global_fee_wallet,
            fee_payer: Some(fee_payer),
        },
    )
}
const LIQUIDATION_RECORD_SEED: &[u8] = b"liq_record";

fn scout_start_deleverage_ix(
    program_id: Pubkey,
    marginfi_account: Pubkey,
    liquidation_record: Pubkey,
    group: Pubkey,
    risk_admin: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    scout_anchor_instruction(
        program_id,
        instruction::StartDeleverage {},
        accounts::StartDeleverage {
            marginfi_account,
            liquidation_record,
            group,
            risk_admin,
        },
    )
}

fn scout_end_deleverage_ix(
    program_id: Pubkey,
    marginfi_account: Pubkey,
    liquidation_record: Pubkey,
    group: Pubkey,
    risk_admin: Pubkey,
) -> anchor_lang::solana_program::instruction::Instruction {
    scout_anchor_instruction(
        program_id,
        instruction::EndDeleverage {},
        accounts::EndDeleverage {
            marginfi_account,
            liquidation_record,
            group,
            risk_admin,
        },
    )
}
const METADATA_SEED: &[u8] = b"metadata";
// First Balance.liability_shares in MarginfiAccount bytes: 8 + group(32) + authority(32) + offset 56.
const SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET: usize = 8 + 32 + 32 + 56;
const SCOUT_KAMINO_BANK_SEED: u64 = 4_020_240_001;
const SCOUT_KAMINO_RESERVE_ACCOUNT_LEN: usize = 8 + 8616;
const SCOUT_KAMINO_OBLIGATION_ACCOUNT_LEN: usize = 8 + 3336;
const SCOUT_KAMINO_RESERVE_MINT_OFFSET: usize = 8 + 128;
const SCOUT_KAMINO_RESERVE_SUPPLY_OFFSET: usize = 8 + 160;
const SCOUT_KAMINO_RESERVE_AVAILABLE_OFFSET: usize = 8 + 224;
const SCOUT_KAMINO_RESERVE_DECIMALS_OFFSET: usize = 8 + 272;
const SCOUT_KAMINO_RESERVE_COLLATERAL_MINT_OFFSET: usize = 8 + 2560;
const SCOUT_KAMINO_RESERVE_COLLATERAL_SUPPLY_OFFSET: usize = 8 + 2600;
const SCOUT_KAMINO_RESERVE_COLLATERAL_VAULT_OFFSET: usize = 8 + 2608;
const SCOUT_KAMINO_OBLIGATION_DEPOSITS_OFFSET: usize = 8 + 96;
const SCOUT_KAMINO_OBLIGATION_DEPOSIT_STRIDE: usize = 136;

fn scout_kamino_program_id() -> Pubkey {
    "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD".parse().unwrap()
}

fn scout_kamino_farms_program_id() -> Pubkey {
    "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr".parse().unwrap()
}

fn scout_kamino_lending_nocpi_artifact() -> &'static str {
    "programs/kamino_lending_nocpi.so"
}

fn scout_kamino_farms_artifact() -> &'static str {
    "programs/kamino_farms.so"
}

fn scout_kamino_lending_market(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_lending_market"], &program_id).0
}

fn scout_kamino_lending_market_authority(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_market_authority"], &program_id).0
}

fn scout_kamino_reserve(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_reserve"], &program_id).0
}

fn scout_kamino_reserve_liquidity_supply(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_liquidity_supply"], &program_id).0
}

fn scout_kamino_reserve_collateral_mint(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_collateral_mint"], &program_id).0
}

fn scout_kamino_reserve_destination_deposit_collateral(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kam_dest_collateral"], &program_id).0
}

// Seed exceeds Solana's 32-byte MAX_SEED_LEN; this is the fixed replacement.
fn scout_kamino_reserve_destination_deposit_collateral_fixed(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_dest_collat"], &program_id).0
}

fn scout_kamino_obligation_farm_user_state(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_obligation_farm"], &program_id).0
}

fn scout_kamino_reserve_farm_state(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_reserve_farm"], &program_id).0
}

fn scout_kamino_pyth_oracle(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_pyth_oracle"], &program_id).0
}

fn scout_kamino_bank(program_id: Pubkey, group: Pubkey, mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[group.as_ref(), mint.as_ref(), &SCOUT_KAMINO_BANK_SEED.to_le_bytes()],
        &program_id,
    )
    .0
}

fn scout_kamino_obligation(program_id: Pubkey, bank: Pubkey) -> Pubkey {
    let liquidity_vault_authority =
        Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &program_id).0;
    let lending_market = scout_kamino_lending_market(program_id);
    Pubkey::find_program_address(
        &[
            &[0u8],
            &[0u8],
            liquidity_vault_authority.as_ref(),
            lending_market.as_ref(),
            system_program::ID.as_ref(),
            system_program::ID.as_ref(),
        ],
        &scout_kamino_program_id(),
    )
    .0
}

fn scout_write_pubkey(data: &mut [u8], offset: usize, key: Pubkey) {
    data[offset..offset + 32].copy_from_slice(&key.to_bytes());
}

fn scout_write_u64(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn scout_kamino_reserve_data(
    program_id: Pubkey,
    mint: Pubkey,
    liquidity_supply: Pubkey,
    collateral_mint: Pubkey,
    collateral_supply: Pubkey,
) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_KAMINO_RESERVE_ACCOUNT_LEN];
    data[..8].copy_from_slice(&[43, 242, 204, 202, 26, 247, 59, 127]);
    scout_write_u64(&mut data, 8, 1);
    scout_write_u64(&mut data, 16, u64::MAX);
    data[25] = 63;
    scout_write_pubkey(&mut data, 32, scout_kamino_lending_market(program_id));
    scout_write_pubkey(&mut data, SCOUT_KAMINO_RESERVE_MINT_OFFSET, mint);
    scout_write_pubkey(&mut data, SCOUT_KAMINO_RESERVE_SUPPLY_OFFSET, liquidity_supply);
    scout_write_u64(&mut data, SCOUT_KAMINO_RESERVE_AVAILABLE_OFFSET, 1_000_000_000);
    scout_write_u64(&mut data, SCOUT_KAMINO_RESERVE_DECIMALS_OFFSET, 6);
    scout_write_pubkey(&mut data, 8 + 408, spl_token::id());
    scout_write_pubkey(&mut data, SCOUT_KAMINO_RESERVE_COLLATERAL_MINT_OFFSET, collateral_mint);
    scout_write_u64(&mut data, SCOUT_KAMINO_RESERVE_COLLATERAL_SUPPLY_OFFSET, 1_000_000_000);
    scout_write_pubkey(&mut data, SCOUT_KAMINO_RESERVE_COLLATERAL_VAULT_OFFSET, collateral_supply);
    data
}

fn scout_kamino_obligation_data(
    program_id: Pubkey,
    bank: Pubkey,
    deposit_reserve: Pubkey,
    deposited_amount: u64,
    second_deposited_amount: u64,
) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_KAMINO_OBLIGATION_ACCOUNT_LEN];
    data[..8].copy_from_slice(&[168, 206, 141, 106, 88, 76, 172, 167]);
    scout_write_u64(&mut data, 8, 1);
    scout_write_u64(&mut data, 16, u64::MAX);
    data[25] = 63;
    let liquidity_vault_authority =
        Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &program_id).0;
    scout_write_pubkey(&mut data, 32, scout_kamino_lending_market(program_id));
    scout_write_pubkey(&mut data, 64, liquidity_vault_authority);
    scout_write_pubkey(&mut data, SCOUT_KAMINO_OBLIGATION_DEPOSITS_OFFSET, deposit_reserve);
    scout_write_u64(&mut data, SCOUT_KAMINO_OBLIGATION_DEPOSITS_OFFSET + 32, deposited_amount);
    scout_write_u64(
        &mut data,
        SCOUT_KAMINO_OBLIGATION_DEPOSITS_OFFSET + SCOUT_KAMINO_OBLIGATION_DEPOSIT_STRIDE + 32,
        second_deposited_amount,
    );
    data
}
const SCOUT_KAMINO_RESERVE_DISCRIMINATOR: [u8; 8] = [43, 242, 204, 202, 26, 247, 59, 127];
const SCOUT_KAMINO_RESERVE_LENDING_MARKET_OFFSET: usize = 8 + 24;
const SCOUT_KAMINO_RESERVE_MINT_PUBKEY_OFFSET: usize = 8 + 120;
const SCOUT_KAMINO_RESERVE_SLOT_OFFSET: usize = 8 + 8;
const SCOUT_KAMINO_RESERVE_STALE_OFFSET: usize = 8 + 16;
const SCOUT_KAMINO_RESERVE_PRICE_STATUS_OFFSET: usize = 8 + 17;
const SCOUT_KAMINO_RESERVE_AVAILABLE_AMOUNT_OFFSET: usize = 8 + 216;
const SCOUT_KAMINO_RESERVE_MINT_DECIMALS_OFFSET: usize = 8 + 264;

fn scout_seeded_bank_pda(program_id: Pubkey, group: Pubkey, bank_mint: Pubkey, bank_seed: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()],
        &program_id,
    )
    .0
}

fn scout_kamino_reserve_pda(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_reserve"], &program_id).0
}

fn scout_kamino_oracle_pda(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_oracle"], &program_id).0
}

fn scout_kamino_lending_market_pda(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_kamino_lending_market"], &program_id).0
}

fn scout_kamino_obligation_pda(liquidity_vault_authority: Pubkey, lending_market: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            &[0u8],
            &[0u8],
            liquidity_vault_authority.as_ref(),
            lending_market.as_ref(),
            system_program::ID.as_ref(),
            system_program::ID.as_ref(),
        ],
        &scout_kamino_program_id(),
    )
    .0
}

fn scout_kamino_reserve_bytes(lending_market: Pubkey, mint: Pubkey) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_KAMINO_RESERVE_ACCOUNT_LEN];
    data[0..8].copy_from_slice(&SCOUT_KAMINO_RESERVE_DISCRIMINATOR);
    data[SCOUT_KAMINO_RESERVE_SLOT_OFFSET..SCOUT_KAMINO_RESERVE_SLOT_OFFSET + 8]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    data[SCOUT_KAMINO_RESERVE_STALE_OFFSET] = 0;
    data[SCOUT_KAMINO_RESERVE_PRICE_STATUS_OFFSET] = 63;
    data[SCOUT_KAMINO_RESERVE_LENDING_MARKET_OFFSET..SCOUT_KAMINO_RESERVE_LENDING_MARKET_OFFSET + 32]
        .copy_from_slice(&lending_market.to_bytes());
    data[SCOUT_KAMINO_RESERVE_MINT_PUBKEY_OFFSET..SCOUT_KAMINO_RESERVE_MINT_PUBKEY_OFFSET + 32]
        .copy_from_slice(&mint.to_bytes());
    data[SCOUT_KAMINO_RESERVE_AVAILABLE_AMOUNT_OFFSET..SCOUT_KAMINO_RESERVE_AVAILABLE_AMOUNT_OFFSET + 8]
        .copy_from_slice(&1_000_000u64.to_le_bytes());
    data[SCOUT_KAMINO_RESERVE_MINT_DECIMALS_OFFSET..SCOUT_KAMINO_RESERVE_MINT_DECIMALS_OFFSET + 8]
        .copy_from_slice(&6u64.to_le_bytes());
    data
}

fn scout_valid_kamino_config(oracle: Pubkey) -> marginfi::types::KaminoConfigCompact {
    marginfi::types::KaminoConfigCompact {
        oracle,
        asset_weight_init: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.8)),
        asset_weight_maint: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.9)),
        deposit_limit: 1_000_000,
        oracle_setup: marginfi::types::OracleSetup::KaminoPythPush,
        operational_state: marginfi::types::BankOperationalState::Operational,
        risk_tier: marginfi::types::RiskTier::Collateral,
        config_flags: 1,
        total_asset_value_init_limit: 1_000_000,
        oracle_max_age: 10,
        oracle_max_confidence: 0,
    }
}
const DRIFT_USER_SEED: &[u8] = b"user";
const DRIFT_USER_STATS_SEED: &[u8] = b"user_stats";
const SCOUT_DRIFT_SPOT_MARKET_DISCRIMINATOR: [u8; 8] = [100, 177, 8, 107, 168, 65, 65, 39];
const SCOUT_DRIFT_SPOT_MARKET_ACCOUNT_LEN: usize = 8 + 768;
const SCOUT_DRIFT_SPOT_MARKET_PUBKEY_OFFSET: usize = 8;
const SCOUT_DRIFT_SPOT_MARKET_ORACLE_OFFSET: usize = 8 + 32;
const SCOUT_DRIFT_SPOT_MARKET_MINT_OFFSET: usize = 8 + 64;
const SCOUT_DRIFT_SPOT_MARKET_VAULT_OFFSET: usize = 8 + 96;
const SCOUT_DRIFT_SPOT_MARKET_CUMULATIVE_DEPOSIT_INTEREST_OFFSET: usize = 8 + 456;
const SCOUT_DRIFT_SPOT_MARKET_CUMULATIVE_BORROW_INTEREST_OFFSET: usize = 8 + 472;
const SCOUT_DRIFT_SPOT_MARKET_LAST_INTEREST_TS_OFFSET: usize = 8 + 560;
const SCOUT_DRIFT_SPOT_MARKET_DECIMALS_OFFSET: usize = 8 + 672;
const SCOUT_DRIFT_SPOT_MARKET_MARKET_INDEX_OFFSET: usize = 8 + 676;
const SCOUT_DRIFT_SPOT_MARKET_POOL_ID_OFFSET: usize = 8 + 727;
const SCOUT_DRIFT_CUMULATIVE_INTEREST_PRECISION: u128 = 10_000_000_000;

fn scout_drift_program_id() -> Pubkey {
    "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH".parse().unwrap()
}

fn scout_drift_spot_market_bytes(
    spot_market: Pubkey,
    oracle: Pubkey,
    mint: Pubkey,
    vault: Pubkey,
    decimals: u32,
    market_index: u16,
    pool_id: u8,
    last_interest_ts: u64,
) -> Vec<u8> {
    let mut bytes = vec![0u8; SCOUT_DRIFT_SPOT_MARKET_ACCOUNT_LEN];
    bytes[..8].copy_from_slice(&SCOUT_DRIFT_SPOT_MARKET_DISCRIMINATOR);
    bytes[SCOUT_DRIFT_SPOT_MARKET_PUBKEY_OFFSET..SCOUT_DRIFT_SPOT_MARKET_PUBKEY_OFFSET + 32]
        .copy_from_slice(spot_market.as_ref());
    bytes[SCOUT_DRIFT_SPOT_MARKET_ORACLE_OFFSET..SCOUT_DRIFT_SPOT_MARKET_ORACLE_OFFSET + 32]
        .copy_from_slice(oracle.as_ref());
    bytes[SCOUT_DRIFT_SPOT_MARKET_MINT_OFFSET..SCOUT_DRIFT_SPOT_MARKET_MINT_OFFSET + 32]
        .copy_from_slice(mint.as_ref());
    bytes[SCOUT_DRIFT_SPOT_MARKET_VAULT_OFFSET..SCOUT_DRIFT_SPOT_MARKET_VAULT_OFFSET + 32]
        .copy_from_slice(vault.as_ref());
    bytes[SCOUT_DRIFT_SPOT_MARKET_CUMULATIVE_DEPOSIT_INTEREST_OFFSET
        ..SCOUT_DRIFT_SPOT_MARKET_CUMULATIVE_DEPOSIT_INTEREST_OFFSET + 16]
        .copy_from_slice(&SCOUT_DRIFT_CUMULATIVE_INTEREST_PRECISION.to_le_bytes());
    bytes[SCOUT_DRIFT_SPOT_MARKET_CUMULATIVE_BORROW_INTEREST_OFFSET
        ..SCOUT_DRIFT_SPOT_MARKET_CUMULATIVE_BORROW_INTEREST_OFFSET + 16]
        .copy_from_slice(&SCOUT_DRIFT_CUMULATIVE_INTEREST_PRECISION.to_le_bytes());
    bytes[SCOUT_DRIFT_SPOT_MARKET_LAST_INTEREST_TS_OFFSET
        ..SCOUT_DRIFT_SPOT_MARKET_LAST_INTEREST_TS_OFFSET + 8]
        .copy_from_slice(&last_interest_ts.to_le_bytes());
    bytes[SCOUT_DRIFT_SPOT_MARKET_DECIMALS_OFFSET
        ..SCOUT_DRIFT_SPOT_MARKET_DECIMALS_OFFSET + 4]
        .copy_from_slice(&decimals.to_le_bytes());
    bytes[SCOUT_DRIFT_SPOT_MARKET_MARKET_INDEX_OFFSET
        ..SCOUT_DRIFT_SPOT_MARKET_MARKET_INDEX_OFFSET + 2]
        .copy_from_slice(&market_index.to_le_bytes());
    bytes[SCOUT_DRIFT_SPOT_MARKET_POOL_ID_OFFSET] = pool_id;
    bytes
}

fn scout_valid_drift_config(oracle: Pubkey) -> marginfi::types::DriftConfigCompact {
    marginfi::types::DriftConfigCompact {
        oracle,
        asset_weight_init: marginfi::types::WrappedI80F48::from_i80f48(
            fixed::types::I80F48::from_num(0.8),
        ),
        asset_weight_maint: marginfi::types::WrappedI80F48::from_i80f48(
            fixed::types::I80F48::from_num(0.9),
        ),
        deposit_limit: 1_000_000,
        oracle_setup: marginfi::types::OracleSetup::DriftPythPull,
        operational_state: marginfi::types::BankOperationalState::Operational,
        risk_tier: marginfi::types::RiskTier::Collateral,
        config_flags: 1,
        total_asset_value_init_limit: 1_000_000,
        oracle_max_age: 60,
        oracle_max_confidence: 0,
    }
}
const SCOUT_KAMINO_LENDING_NOCPI_ARTIFACT: &str = "programs/kamino_lending_nocpi.so";
const SCOUT_KAMINO_FARMS_ARTIFACT: &str = "programs/kamino_farms.so";
const SCOUT_KAMINO_OBLIGATION_DISCRIMINATOR: [u8; 8] = [168, 206, 141, 106, 88, 76, 172, 167];
const SCOUT_KAMINO_RESERVE_SIZE: usize = 8 + 8616;
const SCOUT_KAMINO_OBLIGATION_SIZE: usize = 8 + 3336;

#[derive(Clone, Copy)]
struct ScoutKaminoWithdrawAccounts {
    marginfi_account: Pubkey,
    bank: Pubkey,
    liquidity_vault_authority: Pubkey,
    liquidity_vault: Pubkey,
    reserve: Pubkey,
    obligation: Pubkey,
    oracle: Pubkey,
    lending_market: Pubkey,
    lending_market_authority: Pubkey,
    reserve_liquidity_supply: Pubkey,
    reserve_collateral_mint: Pubkey,
    reserve_source_collateral: Pubkey,
    destination_token_account: Pubkey,
}

fn scout_put_pubkey(data: &mut [u8], offset: usize, key: Pubkey) {
    data[offset..offset + 32].copy_from_slice(key.as_ref());
}

fn scout_put_u64(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn scout_kamino_obligation_bytes(
    lending_market: Pubkey,
    owner: Pubkey,
    reserve: Pubkey,
    slot: u64,
) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_KAMINO_OBLIGATION_SIZE];
    data[..8].copy_from_slice(&SCOUT_KAMINO_OBLIGATION_DISCRIMINATOR);
    scout_put_u64(&mut data, 8, 1);
    scout_put_u64(&mut data, 8 + 8, slot);
    data[8 + 16] = 0;
    data[8 + 17] = 63;
    scout_put_pubkey(&mut data, 8 + 24, lending_market);
    scout_put_pubkey(&mut data, 8 + 56, owner);
    scout_put_pubkey(&mut data, 8 + 88, reserve);
    scout_put_u64(&mut data, 8 + 88 + 32, 0);
    data
}
const SCOUT_DRIFT_PROGRAM_ARTIFACT: &str = "programs/drift_v2.so";
const SCOUT_DRIFT_DEPOSIT_BANK_SEED: u64 = 0x4452494654444550;
const SCOUT_DRIFT_CUMULATIVE_INTEREST: u128 = 10_000_000_000;

fn scout_anchor_zero_copy_bytes<T: bytemuck::Pod>(discriminator: &[u8], value: &T) -> Vec<u8> {
    let mut data = Vec::with_capacity(discriminator.len() + std::mem::size_of::<T>());
    data.extend_from_slice(discriminator);
    data.extend_from_slice(bytemuck::bytes_of(value));
    data
}

fn scout_minimal_user_stats_bytes(authority: Pubkey) -> Vec<u8> {
    let mut data = vec![0u8; 8 + 240];
    data[..8].copy_from_slice(&[176, 223, 136, 27, 122, 79, 32, 227]);
    data[8..40].copy_from_slice(authority.as_ref());
    data
}
const SOLEND_OBLIGATION_SEED: &[u8] = b"solend_obligation";
const SCOUT_SOLEND_RESERVE_ACCOUNT_LEN: usize = 619;
const SCOUT_SOLEND_RESERVE_DISCRIMINATOR: u8 = 1;
const SCOUT_SOLEND_RESERVE_LAST_UPDATE_SLOT_OFFSET: usize = 1;
const SCOUT_SOLEND_RESERVE_LENDING_MARKET_OFFSET: usize = 1 + 9;
const SCOUT_SOLEND_RESERVE_LIQUIDITY_MINT_OFFSET: usize = 1 + 41;
const SCOUT_SOLEND_RESERVE_LIQUIDITY_DECIMALS_OFFSET: usize = 1 + 73;
const SCOUT_SOLEND_RESERVE_LIQUIDITY_SUPPLY_OFFSET: usize = 1 + 74;
const SCOUT_SOLEND_RESERVE_PYTH_ORACLE_OFFSET: usize = 1 + 106;
const SCOUT_SOLEND_RESERVE_SWITCHBOARD_ORACLE_OFFSET: usize = 1 + 138;
const SCOUT_SOLEND_RESERVE_AVAILABLE_AMOUNT_OFFSET: usize = 1 + 170;
const SCOUT_SOLEND_RESERVE_COLLATERAL_MINT_OFFSET: usize = 1 + 226;
const SCOUT_SOLEND_RESERVE_COLLATERAL_SUPPLY_OFFSET: usize = 1 + 258;
const SCOUT_SOLEND_RESERVE_COLLATERAL_ACCOUNT_OFFSET: usize = 1 + 266;

fn scout_solend_program_id() -> Pubkey {
    "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo".parse().unwrap()
}

fn scout_solend_obligation_pda(program_id: Pubkey, bank: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[SOLEND_OBLIGATION_SEED, bank.as_ref()],
        &program_id,
    )
    .0
}

// Account set for a stood-up Solend-integration bank; deposit + withdraw share it.
#[derive(Clone, Copy)]
struct ScoutSolendAccounts {
    bank: Pubkey,
    lva: Pubkey,               // liquidity_vault_authority: obligation owner / transfer auth
    liquidity_vault: Pubkey,
    obligation: Pubkey,
    oracle: Pubkey,
    reserve: Pubkey,
    lending_market: Pubkey,
    lma: Pubkey,               // lending_market_authority: owns reserve_liquidity_supply
    liquidity_supply: Pubkey,
    collateral_mint: Pubkey,
    collateral_supply: Pubkey,
    user_collateral: Pubkey,
    switchboard: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
}

fn scout_solend_reserve_bytes(
    mint: Pubkey,
    lending_market: Pubkey,
    liquidity_supply: Pubkey,
    pyth_oracle: Pubkey,
    switchboard_oracle: Pubkey,
    collateral_mint: Pubkey,
    collateral_supply: Pubkey,
) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_SOLEND_RESERVE_ACCOUNT_LEN];
    data[0] = SCOUT_SOLEND_RESERVE_DISCRIMINATOR;
    scout_write_u64(&mut data, SCOUT_SOLEND_RESERVE_LAST_UPDATE_SLOT_OFFSET, u64::MAX);
    scout_write_pubkey(&mut data, SCOUT_SOLEND_RESERVE_LENDING_MARKET_OFFSET, lending_market);
    scout_write_pubkey(&mut data, SCOUT_SOLEND_RESERVE_LIQUIDITY_MINT_OFFSET, mint);
    data[SCOUT_SOLEND_RESERVE_LIQUIDITY_DECIMALS_OFFSET] = 6;
    scout_write_pubkey(&mut data, SCOUT_SOLEND_RESERVE_LIQUIDITY_SUPPLY_OFFSET, liquidity_supply);
    scout_write_pubkey(&mut data, SCOUT_SOLEND_RESERVE_PYTH_ORACLE_OFFSET, pyth_oracle);
    scout_write_pubkey(&mut data, SCOUT_SOLEND_RESERVE_SWITCHBOARD_ORACLE_OFFSET, switchboard_oracle);
    scout_write_u64(&mut data, SCOUT_SOLEND_RESERVE_AVAILABLE_AMOUNT_OFFSET, 1_000_000_000);
    scout_write_pubkey(&mut data, SCOUT_SOLEND_RESERVE_COLLATERAL_MINT_OFFSET, collateral_mint);
    scout_write_u64(&mut data, SCOUT_SOLEND_RESERVE_COLLATERAL_SUPPLY_OFFSET, 1_000_000_000);
    scout_write_pubkey(&mut data, SCOUT_SOLEND_RESERVE_COLLATERAL_ACCOUNT_OFFSET, collateral_supply);
    data
}

fn scout_valid_solend_config(oracle: Pubkey) -> marginfi::types::SolendConfigCompact {
    marginfi::types::SolendConfigCompact {
        oracle,
        asset_weight_init: marginfi::types::WrappedI80F48::from_i80f48(
            fixed::types::I80F48::from_num(0.8),
        ),
        asset_weight_maint: marginfi::types::WrappedI80F48::from_i80f48(
            fixed::types::I80F48::from_num(0.9),
        ),
        deposit_limit: 1_000_000,
        oracle_setup: marginfi::types::OracleSetup::SolendPythPull,
        operational_state: marginfi::types::BankOperationalState::Operational,
        risk_tier: marginfi::types::RiskTier::Collateral,
        config_flags: 1,
        total_asset_value_init_limit: 1_000_000,
        oracle_max_age: 60,
        oracle_max_confidence: 0,
    }
}
const SCOUT_SOLEND_DEPOSIT_BANK_SEED: u64 = 5_020_240_001;
const SCOUT_SOLEND_OBLIGATION_ACCOUNT_LEN: usize = 1300;

fn scout_solend_artifact() -> &'static str {
    "programs/solend.so"
}

// Minimal executable Solend mock; runs the integration deposit CPI (unlike scout_solend_artifact).
fn scout_solend_mocks_artifact() -> &'static str {
    "programs/solend_mocks.so"
}

fn scout_solend_lending_market(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_solend_lending_market"], &program_id).0
}

fn scout_solend_lending_market_authority(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[scout_solend_lending_market(program_id).as_ref()],
        &scout_solend_program_id(),
    )
    .0
}

fn scout_solend_reserve(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_solend_reserve"], &program_id).0
}

fn scout_solend_reserve_liquidity_supply(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_solend_liquidity_supply"], &program_id).0
}

fn scout_solend_reserve_collateral_mint(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_solend_collateral_mint"], &program_id).0
}

fn scout_solend_reserve_collateral_supply(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_solend_collateral_supply"], &program_id).0
}

fn scout_solend_user_collateral(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_solend_user_collateral"], &program_id).0
}

fn scout_solend_pyth_price(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_solend_pyth_price"], &program_id).0
}

fn scout_solend_switchboard_feed(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_solend_switchboard_feed"], &program_id).0
}

fn scout_solend_obligation(program_id: Pubkey, bank: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[SOLEND_OBLIGATION_SEED, bank.as_ref()], &program_id).0
}

fn scout_solend_obligation_bytes(lending_market: Pubkey, owner: Pubkey, reserve: Pubkey) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_SOLEND_OBLIGATION_ACCOUNT_LEN];
    data[0] = 1;
    scout_write_u64(&mut data, 1, u64::MAX);
    scout_write_pubkey(&mut data, 10, lending_market);
    scout_write_pubkey(&mut data, 42, owner);
    data[202] = 1;
    data[203] = 0;
    scout_write_pubkey(&mut data, 204, reserve);
    scout_write_u64(&mut data, 236, 10);
    data
}
const SCOUT_DRIFT_MAIN_MARKET_INDEX: u16 = 7;
const SCOUT_DRIFT_MARKET_INDEX_FLOOR: u16 = 32;
const SCOUT_DRIFT_SPOT_MARKET_LEN: usize = 8 + 768;
const SCOUT_DRIFT_USER_LEN: usize = 8 + 4368;
const SCOUT_DRIFT_USER_STATS_LEN: usize = 8 + 240;

#[derive(Clone, Copy)]
struct ScoutDriftHarvestAccounts {
    bank: Pubkey,
    fee_state: Pubkey,
    liquidity_vault_authority: Pubkey,
    intermediary_token_account: Pubkey,
    destination_token_account: Pubkey,
    drift_state: Pubkey,
    integration_acc_2: Pubkey,
    integration_acc_3: Pubkey,
    harvest_drift_spot_market: Pubkey,
    harvest_drift_spot_market_vault: Pubkey,
    drift_signer: Pubkey,
    reward_mint: Pubkey,
}

fn scout_drift_v2_artifact() -> &'static str {
    "programs/drift_v2.so"
}

fn scout_drift_state() -> Pubkey {
    Pubkey::find_program_address(&[b"drift_state"], &scout_drift_program_id()).0
}

fn scout_drift_signer() -> Pubkey {
    Pubkey::find_program_address(&[b"drift_signer"], &scout_drift_program_id()).0
}

fn scout_drift_spot_market() -> Pubkey {
    Pubkey::find_program_address(
        &[b"spot_market", &SCOUT_DRIFT_MARKET_INDEX.to_le_bytes()],
        &scout_drift_program_id(),
    )
    .0
}

fn scout_drift_spot_market_vault() -> Pubkey {
    Pubkey::find_program_address(
        &[b"spot_market_vault", &SCOUT_DRIFT_MARKET_INDEX.to_le_bytes()],
        &scout_drift_program_id(),
    )
    .0
}

fn scout_drift_user(program_id: Pubkey, authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user", authority.as_ref(), &0u16.to_le_bytes()], &program_id).0
}

fn scout_drift_user_stats(program_id: Pubkey, authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user_stats", authority.as_ref()], &program_id).0
}

fn scout_drift_market_index_from_mint(mint: Pubkey) -> u16 {
    let mint_bytes = mint.to_bytes();
    SCOUT_DRIFT_MARKET_INDEX_FLOOR + (u16::from_le_bytes([mint_bytes[0], mint_bytes[1]]) % 10_000)
}

fn scout_drift_bank_seed_from_mint(mint: Pubkey) -> u64 {
    let mint_bytes = mint.to_bytes();
    u64::from_le_bytes([
        mint_bytes[0],
        mint_bytes[1],
        mint_bytes[2],
        mint_bytes[3],
        mint_bytes[4],
        mint_bytes[5],
        mint_bytes[6],
        mint_bytes[7],
    ])
}

fn scout_drift_write_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn scout_drift_write_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn scout_drift_write_i64(data: &mut [u8], offset: usize, value: i64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn scout_drift_write_u128(data: &mut [u8], offset: usize, value: u128) {
    data[offset..offset + 16].copy_from_slice(&value.to_le_bytes());
}

fn scout_drift_spot_market_data(
    market: Pubkey,
    oracle: Pubkey,
    mint: Pubkey,
    vault: Pubkey,
    market_index: u16,
    decimals: u32,
) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_DRIFT_SPOT_MARKET_LEN];
    data[..8].copy_from_slice(&[100, 177, 8, 107, 168, 65, 65, 39]);
    scout_write_pubkey(&mut data, 8, market);
    scout_write_pubkey(&mut data, 8 + 32, oracle);
    scout_write_pubkey(&mut data, 8 + 64, mint);
    scout_write_pubkey(&mut data, 8 + 96, vault);
    scout_drift_write_u128(&mut data, 8 + 424, 1_000_000_000);
    scout_drift_write_u128(&mut data, 8 + 440, 0);
    scout_drift_write_u128(&mut data, 8 + 456, SCOUT_DRIFT_CUMULATIVE_INTEREST);
    scout_drift_write_u128(&mut data, 8 + 472, SCOUT_DRIFT_CUMULATIVE_INTEREST);
    scout_write_u64(&mut data, 568, 0);
    scout_drift_write_u32(&mut data, 8 + 672, decimals);
    scout_drift_write_u16(&mut data, 8 + 676, market_index);
    data[8 + 727] = 0;
    data
}

fn scout_drift_user_data(authority: Pubkey, admin_market_index: u16, with_admin_deposit: bool) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_DRIFT_USER_LEN];
    data[..8].copy_from_slice(&[159, 117, 95, 227, 239, 151, 58, 236]);
    scout_write_pubkey(&mut data, 8, authority);
    scout_write_pubkey(&mut data, 8 + 32, authority);
    if with_admin_deposit {
        let position_offset = 8 + 96 + 2 * 40;
        scout_write_u64(&mut data, position_offset, 10_000);
        scout_drift_write_i64(&mut data, position_offset + 24, 10_000);
        scout_drift_write_u16(&mut data, position_offset + 32, admin_market_index);
        data[position_offset + 34] = 0;
    }
    data
}

fn scout_drift_user_stats_data(authority: Pubkey) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_DRIFT_USER_STATS_LEN];
    data[..8].copy_from_slice(&[176, 223, 136, 27, 122, 79, 32, 227]);
    scout_write_pubkey(&mut data, 8, authority);
    data
}
const SCOUT_DRIFT_WITHDRAW_BANK_SEED: u64 = 8_609_202_601;
const SCOUT_DRIFT_MARKET_INDEX: u16 = 0;
const SCOUT_MARGINFI_ACCOUNT_LENDING_OFFSET: usize = 8 + 32 + 32;
const SCOUT_MARGINFI_ACCOUNT_LENDING_LEN: usize = 1728;
const SCOUT_BALANCE_BANK_OFFSET: usize = SCOUT_MARGINFI_ACCOUNT_LENDING_OFFSET + 1;
const SCOUT_BALANCE_ASSET_TAG_OFFSET: usize = SCOUT_MARGINFI_ACCOUNT_LENDING_OFFSET + 33;

fn scout_drift_artifact() -> &'static str {
    "programs/drift_v2.so"
}

fn scout_drift_withdraw_bank_pda(program_id: Pubkey, group: Pubkey, mint: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[group.as_ref(), mint.as_ref(), &SCOUT_DRIFT_WITHDRAW_BANK_SEED.to_le_bytes()],
        &program_id,
    )
    .0
}

fn scout_drift_liquidity_vault_authority(program_id: Pubkey, bank: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &program_id).0
}

fn scout_drift_withdraw_oracle(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_drift_withdraw_oracle"], &program_id).0
}

fn scout_drift_withdraw_marginfi_account(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_drift_wd_mfi_acct"], &program_id).0
}

fn scout_drift_withdraw_destination_token_account(program_id: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"scout_drift_withdraw_destination"], &program_id).0
}

fn scout_write_u128(data: &mut [u8], offset: usize, value: u128) {
    data[offset..offset + 16].copy_from_slice(&value.to_le_bytes());
}

fn scout_drift_state_data() -> Vec<u8> {
    let mut data = vec![0u8; 1024];
    data[..8].copy_from_slice(&[216, 146, 107, 94, 104, 75, 182, 177]);
    data
}
const SCOUT_BALANCE_ASSET_SHARES_OFFSET: usize = SCOUT_MARGINFI_ACCOUNT_LENDING_OFFSET + 40;
const SCOUT_BANK_TOTAL_ASSET_SHARES_OFFSET: usize = 8 + 264;
// Balance = 104B, LendingAccount.balances[16] (type-crate/src/types/user_account.rs:117,144).
const SCOUT_BALANCE_STRIDE: usize = 104;
const SCOUT_BALANCES_PER_ACCOUNT: usize = 16;
const SCOUT_SHARE_SUM_TOLERANCE: fixed::types::I80F48 =
    fixed::types::I80F48::lit("0.000000001");
// panic_unpause_permissionless requires elapsed >= PAUSE_DURATION_SECONDS (30min); +1s margin.
const SCOUT_PANIC_PAUSE_EXPIRY_SECONDS: i64 = 30 * 60 + 1;
// First Balance offsets in Anchor MarginfiAccount: disc(8)+group(32)+authority(32), then Balance
// (user_account.rs): active(0), bank_pk(1), bank_asset_tag(33), asset_shares(40), liability_shares(56).
const SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET: usize = 8 + 32 + 32;
const SCOUT_PULSE_FIRST_BALANCE_ACTIVE_OFFSET: usize = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET;
const SCOUT_PULSE_FIRST_BALANCE_BANK_OFFSET: usize = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + 1;
const SCOUT_PULSE_FIRST_BALANCE_BANK_ASSET_TAG_OFFSET: usize =
    SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + 33;
const SCOUT_PULSE_FIRST_BALANCE_ASSET_SHARES_OFFSET: usize =
    SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + 40;
const SCOUT_PULSE_FIRST_BALANCE_LIABILITY_SHARES_OFFSET: usize =
    SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + 56;
// Balance.emissions_outstanding offset = active(1)+bank_pk(32)+bank_asset_tag(1)+_pad0(6)+asset_shares(16)+liability_shares(16) = 72B from balances[0] start.
const SCOUT_BALANCE_EMISSIONS_OUTSTANDING_OFFSET: usize =
    SCOUT_MARGINFI_ACCOUNT_LENDING_OFFSET + 72;
const SCOUT_WITHDRAW_EMISSIONS_TOTAL: u64 = 1_000_000_000;
const SCOUT_WITHDRAW_EMISSIONS_OUTSTANDING: u64 = 1_000;
// Seed for marginfi_account_initialize_pda / transfer_to_new_account_pda (constants.rs:19).
const SCOUT_MARGINFI_ACCOUNT_SEED: &[u8] = b"marginfi_account";
// LiquidationRecord (512B): disc(8)+key(32)+marginfi_account(32)+record_payer(32)+liquidation_receiver(32)+entries[4](4*48)+LiquidationCache.
const SCOUT_LIQUIDATION_RECORD_RECEIVER_OFFSET: usize = 8 + 32 + 32 + 32;
const SCOUT_LIQUIDATION_RECORD_CACHE_OFFSET: usize = 8 + 32 + 32 + 32 + 32 + (4 * 48);
const SCOUT_LIQUIDATION_RECORD_ASSET_EQUITY_OFFSET: usize =
    SCOUT_LIQUIDATION_RECORD_CACHE_OFFSET + 32;
const SCOUT_LIQUIDATION_RECORD_LIABILITY_EQUITY_OFFSET: usize =
    SCOUT_LIQUIDATION_RECORD_CACHE_OFFSET + 48;
const SCOUT_FEE_STATE_LIQUIDATION_FLAT_SOL_FEE_OFFSET: usize = 208;
const SCOUT_KAMINO_WITHDRAW_BANK_SEED: u64 = 4_020_240_002;
const SCOUT_KAMINO_WITHDRAW_DUST_ASSET_BITS: i128 = 1i128 << 47;

fn scout_kamino_withdraw_bank(program_id: Pubkey, group: Pubkey, mint: Pubkey) -> Pubkey {
    scout_seeded_bank_pda(program_id, group, mint, SCOUT_KAMINO_WITHDRAW_BANK_SEED)
}

fn scout_kamino_withdraw_remaining_accounts(accounts: ScoutKaminoWithdrawAccounts) -> Vec<Pubkey> {
    vec![accounts.bank, accounts.oracle, accounts.reserve]
}
const SCOUT_BANK_CONFIG_OFFSET: usize = 8 + 288;
const SCOUT_BANK_INTEREST_RATE_CONFIG_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 72;
const SCOUT_INTEREST_CURVE_LEGACY: u8 = 0;
const SCOUT_INTEREST_CURVE_SEVEN_POINT: u8 = 1;
const SCOUT_SOLEND_WITHDRAW_BANK_SEED: u64 = 5_020_240_002;
const SCOUT_SOLEND_WITHDRAW_DEPOSIT_AMOUNT: u64 = 10;
const SCOUT_FIRST_BALANCE_OFFSET: usize = 8 + 32 + 32;
const SCOUT_FIRST_BALANCE_ACTIVE_OFFSET: usize = SCOUT_FIRST_BALANCE_OFFSET;
const SCOUT_FIRST_BALANCE_BANK_PK_OFFSET: usize = SCOUT_FIRST_BALANCE_OFFSET + 1;
const SCOUT_FIRST_BALANCE_BANK_ASSET_TAG_OFFSET: usize = SCOUT_FIRST_BALANCE_OFFSET + 33;
const SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET: usize = SCOUT_FIRST_BALANCE_OFFSET + 40;
const SCOUT_BANK_TOTAL_LIABILITY_SHARES_OFFSET: usize = 8 + 248;
// Bank tail (bank.rs): _padding_1 208->1648, integration_acc_1..3 96->1552, _padding_0 16->1536,
// borrowing_position_count 4->1532, lending_position_count 4->1528 (end of BankCache, both i32).
const SCOUT_BANK_BORROWING_POSITION_COUNT_OFFSET: usize = 8 + 1532;
const SCOUT_REPAY_SETUP_LIABILITY_AMOUNT: u64 = 1_000;
// Offsets = disc(8) + repr(C) payload offset (bank.rs, size 1856, align 8).
const SCOUT_CLOSE_ENABLED_FLAG: u64 = 1 << 4;
const SCOUT_BANK_EMISSIONS_REMAINING_OFFSET: usize = 8 + 848;
const SCOUT_BANK_LENDING_POSITION_COUNT_OFFSET: usize = 8 + 1528;
const SCOUT_HANDLE_BANKRUPTCY_BANK_ORACLE_SETUP_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 313;
const SCOUT_HANDLE_BANKRUPTCY_BANK_FIXED_PRICE_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 512;
const SCOUT_HANDLE_BANKRUPTCY_ORACLE_SETUP_FIXED: u8 = 8;
const SCOUT_HANDLE_BANKRUPTCY_LIABILITY_AMOUNT: i64 = 1_000;
const SCOUT_WITHDRAW_ACTION_BANK_ASSET_WEIGHT_INIT_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET;
const SCOUT_WITHDRAW_ACTION_BANK_ASSET_WEIGHT_MAINT_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 16;
const SCOUT_WITHDRAW_ACTION_BANK_ORACLE_SETUP_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 313;
const SCOUT_WITHDRAW_ACTION_BANK_RISK_TIER_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 488;
const SCOUT_WITHDRAW_ACTION_BANK_FIXED_PRICE_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 512;
const SCOUT_WITHDRAW_ACTION_ORACLE_SETUP_FIXED: u8 = 8;
const SCOUT_WITHDRAW_ACTION_RISK_TIER_COLLATERAL: u8 = 0;
const SCOUT_WITHDRAW_BANK_CONFIG_OFFSET_V2: usize = 8 + 288;
const SCOUT_WITHDRAW_BANK_ASSET_WEIGHT_INIT_OFFSET_V2: usize = SCOUT_WITHDRAW_BANK_CONFIG_OFFSET_V2;
const SCOUT_WITHDRAW_BANK_ASSET_WEIGHT_MAINT_OFFSET_V2: usize = SCOUT_WITHDRAW_BANK_CONFIG_OFFSET_V2 + 16;
const SCOUT_WITHDRAW_BANK_ORACLE_SETUP_OFFSET_V2: usize = SCOUT_WITHDRAW_BANK_CONFIG_OFFSET_V2 + 313;
const SCOUT_WITHDRAW_BANK_RISK_TIER_OFFSET_V2: usize = SCOUT_WITHDRAW_BANK_CONFIG_OFFSET_V2 + 488;
const SCOUT_WITHDRAW_BANK_FIXED_PRICE_OFFSET_V2: usize = SCOUT_WITHDRAW_BANK_CONFIG_OFFSET_V2 + 512;
const SCOUT_WITHDRAW_ORACLE_SETUP_FIXED_V2: u8 = 8;
const SCOUT_WITHDRAW_RISK_TIER_COLLATERAL_V2: u8 = 0;
const SCOUT_BALANCE_ACTIVE_OFFSET: usize = SCOUT_MARGINFI_ACCOUNT_LENDING_OFFSET;
type ScoutHandleBankruptcyAccounts = (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey);
type ScoutHandleBankruptcyVariantAccounts = (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Vec<Pubkey>);
const SCOUT_HANDLE_BANKRUPTCY_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
const SCOUT_HANDLE_BANKRUPTCY_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_HANDLE_BANKRUPTCY_BANK_LEN: usize = 8 + 1856;
const SCOUT_HANDLE_BANKRUPTCY_ACCOUNT_LEN: usize = 8 + 2304;
const SCOUT_HANDLE_BANKRUPTCY_BANK_MINT_OFFSET: usize = 8;
const SCOUT_HANDLE_BANKRUPTCY_BANK_MINT_DECIMALS_OFFSET: usize = 8 + 32;
const SCOUT_HANDLE_BANKRUPTCY_BANK_GROUP_OFFSET: usize = 8 + 33;
const SCOUT_HANDLE_BANKRUPTCY_BANK_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
const SCOUT_HANDLE_BANKRUPTCY_BANK_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
const SCOUT_HANDLE_BANKRUPTCY_BANK_LIQUIDITY_VAULT_OFFSET: usize = 8 + 104;
const SCOUT_HANDLE_BANKRUPTCY_BANK_LIQUIDITY_VAULT_BUMP_OFFSET: usize = 8 + 136;
const SCOUT_HANDLE_BANKRUPTCY_BANK_LIQUIDITY_VAULT_AUTHORITY_BUMP_OFFSET: usize = 8 + 137;
const SCOUT_HANDLE_BANKRUPTCY_BANK_INSURANCE_VAULT_OFFSET: usize = 8 + 138;
const SCOUT_HANDLE_BANKRUPTCY_BANK_INSURANCE_VAULT_BUMP_OFFSET: usize = 8 + 170;
const SCOUT_HANDLE_BANKRUPTCY_BANK_INSURANCE_VAULT_AUTHORITY_BUMP_OFFSET: usize = 8 + 171;
const SCOUT_HANDLE_BANKRUPTCY_BANK_FEE_VAULT_OFFSET: usize = 8 + 192;
const SCOUT_HANDLE_BANKRUPTCY_BANK_FEE_VAULT_BUMP_OFFSET: usize = 8 + 224;
const SCOUT_HANDLE_BANKRUPTCY_BANK_FEE_VAULT_AUTHORITY_BUMP_OFFSET: usize = 8 + 225;
const SCOUT_HANDLE_BANKRUPTCY_BANK_TOTAL_ASSET_SHARES_OFFSET: usize = 8 + 264;
const SCOUT_HANDLE_BANKRUPTCY_BANK_LAST_UPDATE_OFFSET: usize = 8 + 280;
const SCOUT_HANDLE_BANKRUPTCY_BANK_OPERATIONAL_STATE_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 312;
const SCOUT_HANDLE_BANKRUPTCY_BANK_LIABILITY_WEIGHT_INIT_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 32;
const SCOUT_HANDLE_BANKRUPTCY_BANK_LIABILITY_WEIGHT_MAINT_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 48;
const SCOUT_HANDLE_BANKRUPTCY_BANK_ORACLE_MAX_AGE_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 506;
const SCOUT_HANDLE_BANKRUPTCY_BANK_RISK_TIER_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 488;
const SCOUT_HANDLE_BANKRUPTCY_BANK_FLAGS_VALUE_OFFSET: usize = 8 + 832;
const SCOUT_HANDLE_BANKRUPTCY_BANK_CLOSE_ENABLED_FLAG: u64 = 1 << 4;
const SCOUT_HANDLE_BANKRUPTCY_TOKEN_ACCOUNT_AMOUNT_OFFSET: usize = 64;
const SCOUT_HANDLE_BANKRUPTCY_SECOND_BALANCE_OFFSET: usize = SCOUT_FIRST_BALANCE_OFFSET + 104;

fn scout_put_i80f48(data: &mut [u8], offset: usize, value: fixed::types::I80F48) {
    data[offset..offset + 16].copy_from_slice(&value.to_le_bytes());
}

fn scout_handle_bankruptcy_bank_bytes(
    group: Pubkey,
    mint: Pubkey,
    liquidity_vault: Pubkey,
    liquidity_vault_bump: u8,
    liquidity_vault_authority_bump: u8,
    insurance_vault: Pubkey,
    insurance_vault_bump: u8,
    insurance_vault_authority_bump: u8,
    fee_vault: Pubkey,
    fee_vault_bump: u8,
    fee_vault_authority_bump: u8,
    liability_shares: fixed::types::I80F48,
) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_HANDLE_BANKRUPTCY_BANK_LEN];
    data[..8].copy_from_slice(&SCOUT_HANDLE_BANKRUPTCY_BANK_DISCRIMINATOR);
    scout_put_pubkey(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_MINT_OFFSET, mint);
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_MINT_DECIMALS_OFFSET] = 6;
    scout_put_pubkey(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_GROUP_OFFSET, group);
    scout_put_i80f48(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_ASSET_SHARE_VALUE_OFFSET, fixed::types::I80F48::ONE);
    scout_put_i80f48(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_LIABILITY_SHARE_VALUE_OFFSET, fixed::types::I80F48::ONE);
    scout_put_pubkey(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_LIQUIDITY_VAULT_OFFSET, liquidity_vault);
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_LIQUIDITY_VAULT_BUMP_OFFSET] = liquidity_vault_bump;
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_LIQUIDITY_VAULT_AUTHORITY_BUMP_OFFSET] = liquidity_vault_authority_bump;
    scout_put_pubkey(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_INSURANCE_VAULT_OFFSET, insurance_vault);
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_INSURANCE_VAULT_BUMP_OFFSET] = insurance_vault_bump;
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_INSURANCE_VAULT_AUTHORITY_BUMP_OFFSET] = insurance_vault_authority_bump;
    scout_put_pubkey(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_FEE_VAULT_OFFSET, fee_vault);
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_FEE_VAULT_BUMP_OFFSET] = fee_vault_bump;
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_FEE_VAULT_AUTHORITY_BUMP_OFFSET] = fee_vault_authority_bump;
    scout_put_i80f48(&mut data, SCOUT_BANK_TOTAL_LIABILITY_SHARES_OFFSET, liability_shares);
    scout_put_i80f48(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_TOTAL_ASSET_SHARES_OFFSET, fixed::types::I80F48::ZERO);
    scout_put_i80f48(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_LIABILITY_WEIGHT_INIT_OFFSET, fixed::types::I80F48::ONE);
    scout_put_i80f48(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_LIABILITY_WEIGHT_MAINT_OFFSET, fixed::types::I80F48::ONE);
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_OPERATIONAL_STATE_OFFSET] = 1;
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_ORACLE_SETUP_OFFSET] = SCOUT_HANDLE_BANKRUPTCY_ORACLE_SETUP_FIXED;
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_RISK_TIER_OFFSET] = 1;
    data[SCOUT_HANDLE_BANKRUPTCY_BANK_ORACLE_MAX_AGE_OFFSET..SCOUT_HANDLE_BANKRUPTCY_BANK_ORACLE_MAX_AGE_OFFSET + 2]
        .copy_from_slice(&10u16.to_le_bytes());
    scout_put_i80f48(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_FIXED_PRICE_OFFSET, fixed::types::I80F48::ONE);
    scout_put_u64(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_FLAGS_VALUE_OFFSET, SCOUT_HANDLE_BANKRUPTCY_BANK_CLOSE_ENABLED_FLAG);
    data[SCOUT_BANK_BORROWING_POSITION_COUNT_OFFSET..SCOUT_BANK_BORROWING_POSITION_COUNT_OFFSET + 4]
        .copy_from_slice(&(if liability_shares > fixed::types::I80F48::ZERO { 1i32 } else { 0i32 }).to_le_bytes());
    scout_put_u64(&mut data, SCOUT_HANDLE_BANKRUPTCY_BANK_LAST_UPDATE_OFFSET, 0);
    data
}

fn scout_handle_bankruptcy_account_bytes(
    group: Pubkey,
    authority: Pubkey,
    first_bank: Pubkey,
    first_liability: fixed::types::I80F48,
    second_bank: Option<Pubkey>,
    second_liability: fixed::types::I80F48,
) -> Vec<u8> {
    let mut data = vec![0u8; SCOUT_HANDLE_BANKRUPTCY_ACCOUNT_LEN];
    data[..8].copy_from_slice(&SCOUT_HANDLE_BANKRUPTCY_ACCOUNT_DISCRIMINATOR);
    scout_put_pubkey(&mut data, 8, group);
    scout_put_pubkey(&mut data, 8 + 32, authority);
    data[SCOUT_FIRST_BALANCE_ACTIVE_OFFSET] = 1;
    scout_put_pubkey(&mut data, SCOUT_FIRST_BALANCE_BANK_PK_OFFSET, first_bank);
    data[SCOUT_FIRST_BALANCE_BANK_ASSET_TAG_OFFSET] = 0;
    scout_put_i80f48(&mut data, SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET, fixed::types::I80F48::ZERO);
    data[SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET..SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET + 16]
        .copy_from_slice(&first_liability.to_le_bytes());
    if let Some(second_bank) = second_bank {
        data[SCOUT_HANDLE_BANKRUPTCY_SECOND_BALANCE_OFFSET] = 1;
        scout_put_pubkey(&mut data, SCOUT_HANDLE_BANKRUPTCY_SECOND_BALANCE_OFFSET + 1, second_bank);
        data[SCOUT_HANDLE_BANKRUPTCY_SECOND_BALANCE_OFFSET + 33] = 0;
        data[SCOUT_HANDLE_BANKRUPTCY_SECOND_BALANCE_OFFSET + 40..SCOUT_HANDLE_BANKRUPTCY_SECOND_BALANCE_OFFSET + 56]
            .copy_from_slice(&fixed::types::I80F48::ZERO.to_le_bytes());
        data[SCOUT_HANDLE_BANKRUPTCY_SECOND_BALANCE_OFFSET + 56..SCOUT_HANDLE_BANKRUPTCY_SECOND_BALANCE_OFFSET + 72]
            .copy_from_slice(&second_liability.to_le_bytes());
    }
    data
}
const SCOUT_DRIFT_HARVEST_BANK_SEED: u64 = 8_609_202_778;
const SCOUT_DRIFT_WITHDRAW_ASSET_TAG: u8 = 4;
const SCOUT_DRIFT_WITHDRAW_ACTIVE_SCALED_BALANCE: u64 = 10_000;
const SCOUT_DRIFT_WITHDRAW_DUST_SCALED_BALANCE: u64 = 1;
const SCOUT_DRIFT_WITHDRAW_TOKEN_AMOUNT: u64 = 1;
const SCOUT_DRIFT_WITHDRAW_BANK_LENDING_POSITION_COUNT_OFFSET: usize = 8 + 1528;
const SCOUT_DRIFT_WITHDRAW_POSITION_BASE_OFFSET: usize = 8 + 96;
const SCOUT_DRIFT_WITHDRAW_POSITION_LEN: usize = 40;

#[derive(Clone, Copy)]
struct ScoutDriftWithdrawAccountsV6 {
    marginfi_account: Pubkey,
    bank: Pubkey,
    oracle: Pubkey,
    liquidity_vault_authority: Pubkey,
    liquidity_vault: Pubkey,
    destination_token_account: Pubkey,
    drift_state: Pubkey,
    integration_acc_2: Pubkey,
    integration_acc_3: Pubkey,
    integration_acc_1: Pubkey,
    drift_spot_market_vault: Pubkey,
    drift_reward_oracle: Option<Pubkey>,
    drift_reward_spot_market: Option<Pubkey>,
    drift_reward_mint: Option<Pubkey>,
    drift_reward_oracle_2: Option<Pubkey>,
    drift_reward_spot_market_2: Option<Pubkey>,
    drift_reward_mint_2: Option<Pubkey>,
    drift_signer: Pubkey,
}

fn scout_drift_withdraw_unique_seed() -> u64 {
    let bytes = Pubkey::new_unique().to_bytes();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn scout_drift_withdraw_spot_market_data_v6(
    spot_market: Pubkey,
    oracle: Pubkey,
    mint: Pubkey,
    vault: Pubkey,
    market_index: u16,
    cumulative_interest: u128,
    last_interest_ts: u64,
) -> Vec<u8> {
    let mut data = scout_drift_spot_market_bytes(
        spot_market,
        oracle,
        mint,
        vault,
        6,
        market_index,
        0,
        last_interest_ts,
    );
    data[SCOUT_DRIFT_SPOT_MARKET_CUMULATIVE_DEPOSIT_INTEREST_OFFSET
        ..SCOUT_DRIFT_SPOT_MARKET_CUMULATIVE_DEPOSIT_INTEREST_OFFSET + 16]
        .copy_from_slice(&cumulative_interest.to_le_bytes());
    data
}

fn scout_drift_withdraw_write_spot_position_v6(
    data: &mut [u8],
    position_index: usize,
    market_index: u16,
    scaled_balance: u64,
) {
    let offset = SCOUT_DRIFT_WITHDRAW_POSITION_BASE_OFFSET
        + position_index * SCOUT_DRIFT_WITHDRAW_POSITION_LEN;
    scout_write_u64(data, offset, scaled_balance);
    scout_drift_write_i64(data, offset + 24, scaled_balance as i64);
    scout_drift_write_u16(data, offset + 32, market_index);
    data[offset + 34] = 0;
}

fn scout_drift_withdraw_user_data_v6(
    authority: Pubkey,
    market_index: u16,
    scaled_balance: u64,
    reward_positions: u8,
) -> Vec<u8> {
    let mut data = scout_drift_user_data(authority, market_index, false);
    let position_index = if market_index == 0 { 0 } else { 1 };
    scout_drift_withdraw_write_spot_position_v6(
        &mut data,
        position_index,
        market_index,
        scaled_balance,
    );
    if reward_positions >= 1 {
        scout_drift_withdraw_write_spot_position_v6(&mut data, 2, market_index + 1, 1);
    }
    if reward_positions >= 2 {
        scout_drift_withdraw_write_spot_position_v6(&mut data, 3, market_index + 2, 1);
    }
    data
}
fn scout_handle_bankruptcy_token2022_id() -> Pubkey {
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse().unwrap()
}

fn scout_token_mint_bytes_append(decimals: u8, mint_authority: Pubkey, supply: u64) -> Vec<u8> {
    let mint = spl_token::state::Mint {
        mint_authority: spl_token::solana_program::program_option::COption::Some(mint_authority),
        supply,
        decimals,
        is_initialized: true,
        freeze_authority: spl_token::solana_program::program_option::COption::None,
    };
    let mut data = vec![0u8; <spl_token::state::Mint as spl_token::solana_program::program_pack::Pack>::LEN];
    <spl_token::state::Mint as spl_token::solana_program::program_pack::Pack>::pack(mint, &mut data).unwrap();
    data
}

fn scout_token_account_bytes_append(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
    let token_account = spl_token::state::Account {
        mint,
        owner,
        amount,
        delegate: spl_token::solana_program::program_option::COption::None,
        state: spl_token::state::AccountState::Initialized,
        is_native: spl_token::solana_program::program_option::COption::None,
        delegated_amount: 0,
        close_authority: spl_token::solana_program::program_option::COption::None,
    };
    let mut data = vec![0u8; <spl_token::state::Account as spl_token::solana_program::program_pack::Pack>::LEN];
    <spl_token::state::Account as spl_token::solana_program::program_pack::Pack>::pack(token_account, &mut data).unwrap();
    data
}
#[derive(Clone, Copy)]
struct ScoutDriftDepositAccountsAppendOnly {
    bank: Pubkey,
    oracle: Pubkey,
    liquidity_vault_authority: Pubkey,
    liquidity_vault: Pubkey,
    drift_state: Pubkey,
    integration_acc_2: Pubkey,
    integration_acc_3: Pubkey,
    integration_acc_1: Pubkey,
    drift_spot_market_vault: Pubkey,
}

const SCOUT_DRIFT_DEPOSIT_ORACLE_FEED_ID_APPEND_ONLY: [u8; 32] = [11u8; 32];
// Maintenance-weighted LiquidationCache offsets (type-crate/src/types/liquidation_record.rs:79-104).
const SCOUT_LIQUIDATION_RECORD_ASSET_MAINT_OFFSET: usize = SCOUT_LIQUIDATION_RECORD_CACHE_OFFSET;
const SCOUT_LIQUIDATION_RECORD_LIABILITY_MAINT_OFFSET: usize =
    SCOUT_LIQUIDATION_RECORD_CACHE_OFFSET + 16;
const SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET: usize = 184;
const SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET: usize = 240;
const SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET: usize = 904;
// SPL Token account: mint(32)+owner(32), amount:u64 at byte 64.
const SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET: usize = 64;
// InterestRateConfig field offsets (interest_rate.rs, repr(C), size 240).
const SCOUT_IRC_OPTIMAL_UTIL_OFFSET: usize = 0;
const SCOUT_IRC_PLATEAU_RATE_OFFSET: usize = 16;
const SCOUT_IRC_MAX_RATE_OFFSET: usize = 32;
const SCOUT_IRC_ZERO_UTIL_RATE_OFFSET: usize = 128;
const SCOUT_IRC_HUNDRED_UTIL_RATE_OFFSET: usize = 132;
const SCOUT_IRC_POINTS_OFFSET: usize = 136;
const SCOUT_IRC_CURVE_TYPE_OFFSET: usize = 176;
// MinimalReserve.mint_total_supply offset (kamino-mocks/src/state.rs ReserveCollateral @2584).
const SCOUT_KAMINO_RESERVE_MINT_TOTAL_SUPPLY_OFFSET: usize = 8 + 2584;
const SCOUT_ASSET_TAG_KAMINO_SHARED: u8 = 3;
// Anchor account discriminators (sha256("account:<Type>")[..8]) for Kamino Farms UserState/FarmState/GlobalConfig.
const SCOUT_KAMINO_FARMS_USER_STATE_DISCRIMINATOR: [u8; 8] = [72, 177, 85, 249, 76, 167, 186, 126];
const SCOUT_KAMINO_FARMS_FARM_STATE_DISCRIMINATOR: [u8; 8] = [198, 102, 216, 74, 63, 66, 163, 190];
const SCOUT_KAMINO_FARMS_GLOBAL_CONFIG_DISCRIMINATOR: [u8; 8] = [149, 8, 156, 202, 160, 252, 176, 217];
// FeeState.panic_state.pause_flags offset (fee_state.rs:23-60, panic_state_cache.rs:60-76).
const SCOUT_FEE_STATE_PANIC_PAUSE_FLAGS_OFFSET: usize = 168;
const SCOUT_PANIC_STATE_FLAG_PAUSED: u8 = 1;
// pause_start_timestamp is 8 bytes into panic_state.
const SCOUT_FEE_STATE_PANIC_PAUSE_START_TIMESTAMP_OFFSET: usize =
    SCOUT_FEE_STATE_PANIC_PAUSE_FLAGS_OFFSET + 8;

// --- Flashloan bracket (compound action) sizing -------------------------------------------------
const SCOUT_FLASHLOAN_VAULT_LIQUIDITY: u64 = 100_000_000;
const SCOUT_FLASHLOAN_SEED_DEPOSIT: u64 = 100_000;
const SCOUT_FLASHLOAN_SEED_BORROW: u64 = 1_000;
const SCOUT_FLASHLOAN_MIDDLE_AMOUNT_MODULUS: u64 = 1_000_000;

// --- shared, permanently borrowable scenario for lending_account_borrow / _repay -------------
const SCOUT_BORROW_COLLATERAL_DEPOSIT_AMOUNT: u64 = 1_000_000_000_000;
const SCOUT_BORROW_LIQUIDITY_DEPOSIT_AMOUNT: u64 = 1_000_000_000_000;
const SCOUT_BORROW_AMOUNT: u64 = 1_000_000;
const SCOUT_BORROW_REPAY_PARTIAL_AMOUNT: u64 = 250_000;
// ---------------------------------------------------------------------------------------------
// P-0004 shared readers: pure reads that degrade to `None` on unexpected layout.
// ---------------------------------------------------------------------------------------------
// Bank.asset_share_value / liability_share_value offsets (type-crate/src/types/bank.rs:22-73).
const SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
const SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
// One native token unit; absolute allowance over the whole ledger.
const SCOUT_P4_UNIT_TOLERANCE: fixed::types::I80F48 = fixed::types::I80F48::lit("1");
// Sub-unit slack for the per-borrow liability delta.
const SCOUT_P4_DUST_TOLERANCE: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000000001");
// All 16 balance slots as one contiguous run.
const SCOUT_P4_BALANCE_REGION_LEN: usize = SCOUT_BALANCES_PER_ACCOUNT * SCOUT_BALANCE_STRIDE;
// Typed empty-slice fallback for a hook's fallible account read (P-0004, P-0011).
const SCOUT_HOOK_NO_BYTES: &[u8] = &[];

/// (asset_share_value, liability_share_value) of a Bank as raw LE I80F48 bytes.
fn scout_p4_share_values(
    ctx: &TestContext,
    bank: &Pubkey,
) -> Option<([u8; 16], [u8; 16])> {
    if *bank == Pubkey::default() {
        return None;
    }
    let data = ctx.read_account(bank).ok()?.data;
    if data.len() < SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16 {
        return None;
    }
    let mut asv = [0u8; 16];
    let mut lsv = [0u8; 16];
    asv.copy_from_slice(
        &data[SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET..SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET + 16],
    );
    lsv.copy_from_slice(
        &data[SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET
            ..SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16],
    );
    Some((asv, lsv))
}

/// (asset_shares, liability_shares) a MarginfiAccount holds against one bank. Returns (0, 0) if no balance, None if unreadable.
fn scout_p4_balance_shares(
    ctx: &TestContext,
    account: &Pubkey,
    bank: &Pubkey,
) -> Option<(fixed::types::I80F48, fixed::types::I80F48)> {
    if *account == Pubkey::default() || *bank == Pubkey::default() {
        return None;
    }
    let data = ctx.read_account(account).ok()?.data;
    for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
        let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
        if data.len() < base + SCOUT_BALANCE_STRIDE {
            break;
        }
        if data[base] != 1 {
            continue;
        }
        if &data[base + 1..base + 33] != bank.as_ref() {
            continue;
        }
        let mut asset = [0u8; 16];
        asset.copy_from_slice(&data[base + 40..base + 56]);
        let mut liab = [0u8; 16];
        liab.copy_from_slice(&data[base + 56..base + 72]);
        return Some((
            fixed::types::I80F48::from_le_bytes(asset),
            fixed::types::I80F48::from_le_bytes(liab),
        ));
    }
    Some((fixed::types::I80F48::ZERO, fixed::types::I80F48::ZERO))
}

// P-0011 principal codes; hooks write into scout_p11_writer.
const SCOUT_P11_WRITER_EMISSIONS_DELEGATE: u8 = 1;
const SCOUT_P11_WRITER_RISK_ADMIN: u8 = 2;
const SCOUT_P11_WRITER_REPAY_PATH: u8 = 3;
const SCOUT_P11_WRITER_BANK_CREATION: u8 = 4;
const SCOUT_P11_WRITER_GROUP_ADMIN: u8 = 5;
const SCOUT_P11_WRITER_GLOBAL_FEE_ADMIN: u8 = 6;
// P-0011 masks: EMISSIONS = bits 0|1; TOKENLESS_COMPLETE = bit 6; ANY_OWNED = union of
// legitimate-writer bits (0,1,2,3,5,6) -- bit 4 and reserved 7-63 are owned by nobody.
const SCOUT_P11_MASK_EMISSIONS: u64 = 0b11;
const SCOUT_P11_MASK_TOKENLESS_COMPLETE: u64 = 1 << 6;
const SCOUT_P11_ANY_OWNED: u64 = 0b0110_1111;
// Bank/MarginfiGroup discriminators and lengths; group auth-field offsets accumulated over its
// repr(C) declaration order (admin, group_flags, fee_state_cache, ..., risk_admin, metadata_admin).
const SCOUT_P11_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_P11_GROUP_DISCRIMINATOR: [u8; 8] = [182, 23, 173, 240, 151, 206, 182, 67];
const SCOUT_P11_BANK_ACCOUNT_LEN: usize = 8 + 1856;
const SCOUT_P11_GROUP_ACCOUNT_LEN: usize = 8 + 1056;
const SCOUT_P11_GROUP_FLAGS_OFFSET: usize = 8 + 32;
const SCOUT_P11_GROUP_AUTH_OFFSETS: [usize; 7] =
    [8, 8 + 120, 8 + 152, 8 + 184, 8 + 216, 8 + 288, 8 + 320];
const SCOUT_P11_GROUP_AUTH_NAMES: [&str; 7] = [
    "admin",
    "emode_admin",
    "delegate_curve_admin",
    "delegate_limit_admin",
    "delegate_emissions_admin",
    "risk_admin",
    "metadata_admin",
];
// Capacity of P-0011's authorised-baseline registry (fixed array, saturating cursor).
const SCOUT_P11_SEED_CAP: usize = 64;
// Ring capacity for P-0002's known-account registry; fails open on overflow.
const SCOUT_KNOWN_CAP: usize = 256;
// Ring capacity for P-0006's created-bank registry (fixed array, hooks may only assign).
const SCOUT_P06_BANK_CAP: usize = 128;
// Ring capacity for per-property SUBJECT registries (P-0001/P-0017/P-0020/P-0022/P-0039).
const SCOUT_SUBJECT_CAP: usize = 96;
// Capacity of P-0013's registry of banks proven to have had CLOSE_ENABLED_FLAG set (cursor saturates).
const SCOUT_P13_BANK_CAP: usize = 64;
// Capacity of P-0030's fee-counter baseline registry (fixed array; written by an EXTRA-ACTIONS probe).
const SCOUT_P30_BANK_CAP: usize = 64;
// Bank discriminator/length (bank.rs:16, constants.rs:180-187).
const SCOUT_P30_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_P30_BANK_LEN: usize = 8 + 1856;

// --- P-0035 (no exposure growth while unhealthy) layout constants -------------------------
// MarginfiAccount (size 2304) / HealthCache (size 304) field layout, repr(C) declaration order.
const SCOUT_P35_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
const SCOUT_P35_ACCOUNT_LEN: usize = 8 + 2304;
const SCOUT_P35_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_P35_BANK_LEN: usize = 8 + 1856;
const SCOUT_P35_ACCOUNT_FLAGS_OFFSET: usize = 8 + 32 + 32 + 1728;
// ACCOUNT_IN_FLASHLOAN | ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_DELEVERAGE (user_account.rs:108-113).
const SCOUT_P35_BRACKET_FLAGS: u64 = (1 << 1) | (1 << 4) | (1 << 5);
const SCOUT_P35_HEALTH_CACHE_OFFSET: usize = 8 + 32 + 32 + 1728 + 8 + 32;
const SCOUT_P35_HC_ASSET_MAINT_OFFSET: usize = SCOUT_P35_HEALTH_CACHE_OFFSET + 32;
const SCOUT_P35_HC_LIAB_MAINT_OFFSET: usize = SCOUT_P35_HEALTH_CACHE_OFFSET + 48;
const SCOUT_P35_HC_TIMESTAMP_OFFSET: usize = SCOUT_P35_HEALTH_CACHE_OFFSET + 96;
const SCOUT_P35_HC_PRICES_OFFSET: usize = SCOUT_P35_HEALTH_CACHE_OFFSET + 112;
// Bank/BankConfig field offsets (bank.rs:22-140, bank_config.rs:28-97).
const SCOUT_P35_BANK_MINT_DECIMALS_OFFSET: usize = 8 + 32;
const SCOUT_P35_BANK_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
const SCOUT_P35_BANK_CONFIG_OFFSET: usize = 8 + 288;
const SCOUT_P35_BANK_LIABILITY_WEIGHT_MAINT_OFFSET: usize = SCOUT_P35_BANK_CONFIG_OFFSET + 48;
const SCOUT_P35_BANK_ORACLE_SETUP_OFFSET: usize = SCOUT_P35_BANK_CONFIG_OFFSET + 313;
const SCOUT_P35_BANK_ASSET_TAG_OFFSET: usize = SCOUT_P35_BANK_CONFIG_OFFSET + 489;
const SCOUT_P35_BANK_FIXED_PRICE_OFFSET: usize = SCOUT_P35_BANK_CONFIG_OFFSET + 512;
// OracleSetup::Fixed = variant 8; ASSET_TAG_DEFAULT = 0.
const SCOUT_P35_ORACLE_SETUP_FIXED: u8 = 8;
const SCOUT_P35_ASSET_TAG_DEFAULT: u8 = 0;
// Clock sysvar pubkey bytes.
const SCOUT_P35_CLOCK_SYSVAR_BYTES: [u8; 32] = [
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
];
// Clock sysvar layout: unix_timestamp follows slot/epoch_start_timestamp/epoch/leader_schedule_epoch.
const SCOUT_P35_CLOCK_UNIX_TIMESTAMP_OFFSET: usize = 32;
// Number of fixture-named MarginfiAccounts P-0035 judges.
const SCOUT_P35_SUBJECT_COUNT: usize = 8;
// mint_decimals cap; I80F48 saturates above ~6.04e23. Real SPL mints are <= 9.
const SCOUT_P35_MAX_DECIMALS: u8 = 20;
// Liability-value comparison tolerance.
const SCOUT_P35_VALUE_TOLERANCE: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000001");

// ---------------------------------------------------------------------------------------------
// P-0037 / P-0038 shared machinery: layout constants + probe-arm tags.
// ---------------------------------------------------------------------------------------------
const SCOUT_HP_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
const SCOUT_HP_ACCOUNT_LEN: usize = 8 + 2304;
const SCOUT_HP_ACCOUNT_GROUP_OFFSET: usize = 8;
const SCOUT_HP_ACCOUNT_FLAGS_OFFSET: usize = 8 + 32 + 32 + 1728;
const SCOUT_HP_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_HP_BANK_LEN: usize = 8 + 1856;
const SCOUT_HP_BANK_MINT_DECIMALS_OFFSET: usize = 8 + 32;
const SCOUT_HP_BANK_GROUP_OFFSET: usize = 8 + 33;
const SCOUT_HP_BANK_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
const SCOUT_HP_BANK_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
const SCOUT_HP_BANK_ASSET_WEIGHT_MAINT_OFFSET: usize = 8 + 288 + 16;
const SCOUT_HP_BANK_LIABILITY_WEIGHT_MAINT_OFFSET: usize = 8 + 288 + 48;
const SCOUT_HP_BANK_ORACLE_SETUP_OFFSET: usize = 8 + 288 + 313;
const SCOUT_HP_BANK_RISK_TIER_OFFSET: usize = 8 + 288 + 488;
const SCOUT_HP_BANK_ASSET_TAG_OFFSET: usize = 8 + 288 + 489;
const SCOUT_HP_BANK_FIXED_PRICE_OFFSET: usize = 8 + 288 + 512;
const SCOUT_HP_BANK_EMODE_TAG_OFFSET: usize = 8 + 912;
const SCOUT_HP_BANK_EMODE_FLAGS_OFFSET: usize = 8 + 912 + 16;
// OracleSetup::Fixed = variant 8, RiskTier::Isolated = variant 1; ASSET_TAG_DEFAULT = 0.
const SCOUT_HP_ORACLE_SETUP_FIXED: u8 = 8;
// EXP_10 lookup table (predicates can't call 10u128.pow(n)).
const SCOUT_HP_EXP_10: [u128; 25] = [
    1,
    10,
    100,
    1_000,
    10_000,
    100_000,
    1_000_000,
    10_000_000,
    100_000_000,
    1_000_000_000,
    10_000_000_000,
    100_000_000_000,
    1_000_000_000_000,
    10_000_000_000_000,
    100_000_000_000_000,
    1_000_000_000_000_000,
    10_000_000_000_000_000,
    100_000_000_000_000_000,
    1_000_000_000_000_000_000,
    10_000_000_000_000_000_000,
    100_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000,
];
const SCOUT_HP_RISK_TIER_ISOLATED: u8 = 1;
const SCOUT_HP_ASSET_TAG_DEFAULT: u8 = 0;
// Exempt flag bits (user_account.rs:108-114): DISABLED(1), IN_FLASHLOAN(2), IN_RECEIVERSHIP(16), IN_DELEVERAGE(32), FROZEN(64).
const SCOUT_HP_ACCOUNT_EXEMPT_FLAGS: u64 = (1 << 0) | (1 << 1) | (1 << 4) | (1 << 5) | (1 << 6);
// Probe arms: 1-3 = P-0037 gated set; 4-6 = P-0038 allowed self-initiated instructions.
const SCOUT_HP_KIND_NONE: u8 = 0;
const SCOUT_HP_KIND_WITHDRAW: u8 = 1;
const SCOUT_HP_KIND_WITHDRAW_ALL: u8 = 2;
const SCOUT_HP_KIND_BORROW: u8 = 3;
const SCOUT_HP_KIND_DEPOSIT: u8 = 4;
const SCOUT_HP_KIND_REPAY: u8 = 5;
const SCOUT_HP_KIND_PULSE_HEALTH: u8 = 6;
const SCOUT_HP_KIND_COUNT: u8 = 6;
// Probe scenario: collateral crashed 10 -> 0.1, liability priced 1. Post-crash maintenance health = -0.82.
const SCOUT_HP_LIQUIDITY_DEPOSIT_AMOUNT: u64 = 2_000_000;
const SCOUT_HP_COLLATERAL_DEPOSIT_AMOUNT: u64 = 2_000_000;
const SCOUT_HP_BORROW_AMOUNT: u64 = 1_000_000;
const SCOUT_HP_CRASHED_PRICE: f64 = 0.1;
const SCOUT_HP_PROBE_WITHDRAW_AMOUNT: u64 = 500_000;
const SCOUT_HP_PROBE_BORROW_AMOUNT: u64 = 1;
const SCOUT_HP_PROBE_DEPOSIT_AMOUNT: u64 = 1_000_000;
const SCOUT_HP_PROBE_REPAY_AMOUNT: u64 = 100_000;

// ---- P-0033 (cumulative extraction bound) --------------------------------------------------
// marginfi's liquidation fee schedule (constants.rs:24-25: both fees 0.025):
//     liquidator net value change = +f_l * G          (+0.025 * G)
//     liquidatee  net value change = -(f_l+f_i) * G    (-0.050 * G)
// where G = q_a * p_a.
const SCOUT_P33_LIQUIDATOR_FEE: fixed::types::I80F48 = fixed::types::I80F48::lit("0.025");
const SCOUT_P33_LIQUIDATOR_PLUS_INSURANCE_FEE: fixed::types::I80F48 =
    fixed::types::I80F48::lit("0.05");
const SCOUT_P33_PER_ROUND_SLACK: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000001");
// Probe scenario: collateral crashed 10 -> 0.1, liability priced 1.
const SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT: u64 = 2_000_000;
const SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT: u64 = 2_000_000;
const SCOUT_P33_BORROW_AMOUNT: u64 = 1_000_000;
const SCOUT_P33_CRASHED_PRICE: f64 = 0.1;
// Extra deposit so the liquidator holds ASSETS in the liability bank pre-liquidation, forcing
// `withdraw_ignore_borrow_cap`'s mixed asset-then-borrow path (liquidate.rs:263-271).
const SCOUT_P33_LIQUIDATOR_LIAB_DEPOSIT_AMOUNT: u64 = 200_000;
const SCOUT_P33_ASSET_AMOUNT: u64 = 400_000;
const SCOUT_P33_MAX_ROUNDS: u8 = 3;
// ---- P-0021 / P-0023 (single-liquidation value bounds) ------------------------------------
const SCOUT_P2123_ROUND_SLACK: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000001");
// Arm 2's liquidator liability-bank deposit, deliberately below the per-round liability so round 1
// drains the balance then borrows the remainder (mixed withdraw-then-borrow path, liquidate.rs:265-266).
const SCOUT_P2123_LIQUIDATOR_LIAB_PARTIAL_AMOUNT: u64 = 20_000;

// ---- P-0024 (no third-party-induced liquidation) -------------------------------------------
// Arms of `action_third_party_health_probe`: 1-4 act on the third party's own account in the
// victim's banks; 5-6 name the victim's account with the third party's signature; 7-10 are
// signer-less entry points any key may invoke.
const SCOUT_P24_ARM_NONE: u8 = 0;
const SCOUT_P24_ARM_THIRD_PARTY_DEPOSIT_OWN: u8 = 1;
const SCOUT_P24_ARM_THIRD_PARTY_WITHDRAW_OWN: u8 = 2;
const SCOUT_P24_ARM_THIRD_PARTY_BORROW_OWN: u8 = 3;
const SCOUT_P24_ARM_THIRD_PARTY_REPAY_OWN: u8 = 4;
const SCOUT_P24_ARM_WITHDRAW_VICTIM: u8 = 5;
const SCOUT_P24_ARM_BORROW_VICTIM: u8 = 6;
// `PulseHealth` (pulse_health.rs:105-109): one `mut` AccountLoader, no signer.
const SCOUT_P24_ARM_PULSE_VICTIM: u8 = 7;
// `LendingPoolAccrueBankInterest` (marginfi_group/accrue_bank_interest.rs:26-36): group + `mut` bank, no signer.
const SCOUT_P24_ARM_ACCRUE_COLLATERAL_BANK: u8 = 8;
const SCOUT_P24_ARM_ACCRUE_LIABILITY_BANK: u8 = 9;
const SCOUT_P24_ARM_COUNT: u8 = 9;

// Scenario: collateral 2.0 @ price 10, price moves to 8.5 -> maintenance health +0.3 (thin, not liquidatable).
const SCOUT_P24_LIQUIDITY_DEPOSIT_AMOUNT: u64 = 100_000_000;
const SCOUT_P24_VICTIM_COLLATERAL_AMOUNT: u64 = 2_000_000;
const SCOUT_P24_VICTIM_BORROW_AMOUNT: u64 = 15_000_000;
const SCOUT_P24_START_PRICE: f64 = 10.0;
const SCOUT_P24_SETTLED_PRICE: f64 = 8.5;
// Third party's own position, sized so arms 2-4 (withdraw/borrow/repay on its own account) pass their own risk checks.
const SCOUT_P24_THIRD_PARTY_COLLATERAL_AMOUNT: u64 = 4_000_000;
const SCOUT_P24_THIRD_PARTY_BORROW_AMOUNT: u64 = 1_000_000;
const SCOUT_P24_PROBE_DEPOSIT_AMOUNT: u64 = 1_000_000;
const SCOUT_P24_PROBE_WITHDRAW_AMOUNT: u64 = 500_000;
const SCOUT_P24_PROBE_BORROW_AMOUNT: u64 = 1_000_000;
const SCOUT_P24_PROBE_REPAY_AMOUNT: u64 = 100_000;
const SCOUT_P24_VICTIM_LAMPORTS: u64 = 10_000_000_000;
const SCOUT_P24_VICTIM_TOKENS: u64 = 1_000_000_000_000;

// ---- P-0025 (REPAY LIVENESS) probe constants -----------------------------------------------
// P-0025 is a LIVENESS claim ("a well-formed repay MUST succeed"): `action_repay_liveness_probe`
// degrades part of the account unrelated to the repaid bank X, then sends a partial repay against X.
// `lending_account_repay` (repay.rs:34-152) builds no RiskEngine and performs no health check,
// unlike withdraw/borrow -- which is what makes the property non-vacuous.
const SCOUT_RL_ARM_COUNT: u8 = 6;
// Arm 0: no degradation (CONTROL).
const SCOUT_RL_ARM_NONE: u8 = 0;
// Arm 1: oracle crash on the COLLATERAL bank (not the repaid bank).
const SCOUT_RL_ARM_CRASH_COLLATERAL_ORACLE: u8 = 1;
// Arm 2: pause the OTHER liability bank (bank Y).
const SCOUT_RL_ARM_PAUSE_OTHER_BANK: u8 = 2;
// Arm 3: ReduceOnly on bank Y.
const SCOUT_RL_ARM_REDUCE_ONLY_OTHER_BANK: u8 = 3;
// Arm 4: oracle crash to zero on the other liability bank (Y).
const SCOUT_RL_ARM_ZERO_OTHER_BANK_ORACLE: u8 = 4;
// Arm 5: handle_bankruptcy on bank Y, then a real price restore on bank C.
const SCOUT_RL_ARM_BANKRUPT_OTHER_BANK: u8 = 5;
// Initial health = 16 - 2 > 0.
const SCOUT_RL_LIQUIDITY_DEPOSIT_AMOUNT: u64 = 2_000_000;
const SCOUT_RL_COLLATERAL_DEPOSIT_AMOUNT: u64 = 2_000_000;
const SCOUT_RL_BORROW_AMOUNT: u64 = 1_000_000;
// Strictly smaller than the liability (RepayOnly guard, ordinary partial-repay path).
const SCOUT_RL_REPAY_AMOUNT: u64 = 100_000;
const SCOUT_RL_HEALTHY_COLLATERAL_PRICE: f64 = 10.0;
const SCOUT_RL_LIABILITY_PRICE: f64 = 1.0;
const SCOUT_RL_CRASHED_PRICE: f64 = 0.1;
// Zero price is valid (only negative rejected); zeroes collateral value.
const SCOUT_RL_ZERO_PRICE: f64 = 0.0;
// BankConfig.operational_state offset (BankConfig + 312).
const SCOUT_RL_BANK_OPERATIONAL_STATE_OFFSET: usize = 8 + 288 + 312;
// `BankOperationalState` discriminants, in declaration order.
const SCOUT_RL_BANK_STATE_OPERATIONAL: u8 = 1;
// SPL token account `amount` is a little-endian u64 at byte 64 of the 165-byte layout (mint(32)+owner(32)).
const SCOUT_RL_TOKEN_ACCOUNT_AMOUNT_OFFSET: usize = 64;
const SCOUT_RL_TOKEN_ACCOUNT_LEN: usize = 165;
// Exempt flags: IN_FLASHLOAN(2), IN_RECEIVERSHIP(16), IN_DELEVERAGE(32), FROZEN(64).
// ACCOUNT_DISABLED(1) is deliberately NOT exempt -- set by an operation on a DIFFERENT bank
// (handle_bankruptcy.rs:186) and never cleared, which is the entire point of the property.
const SCOUT_RL_ACCOUNT_EXEMPT_FLAGS: u64 = (1 << 1) | (1 << 4) | (1 << 5) | (1 << 6);

// ---------------------------------------------------------------------------------------------
// P-0026 / P-0027 shared machinery: Bank.cache (BankCache) and MarginfiAccount.health_cache
// (HealthCache) byte layout (bank.rs:16-17, bank_cache.rs:7-8, bank_config.rs:28-97).
const SCOUT_PC_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_PC_BANK_LEN: usize = 8 + 1856;
const SCOUT_PC_BANK_ORACLE_SETUP_OFFSET: usize = 8 + 288 + 313;
const SCOUT_PC_BANK_FIXED_PRICE_OFFSET: usize = 8 + 288 + 512;
const SCOUT_PC_BANK_CACHE_PRICE_OFFSET: usize = 8 + 1368 + 32;
const SCOUT_PC_BANK_CACHE_TIMESTAMP_OFFSET: usize = 8 + 1368 + 48;
const SCOUT_PC_BANK_CACHE_CONFIDENCE_OFFSET: usize = 8 + 1368 + 56;
// OracleSetup::Fixed is variant 8.
const SCOUT_PC_ORACLE_SETUP_FIXED: u8 = 8;
// MarginfiAccount.health_cache starts at 1832; HealthCache.timestamp follows six WrappedI80F48 fields at +96.
const SCOUT_PC_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
const SCOUT_PC_ACCOUNT_LEN: usize = 8 + 2304;
const SCOUT_PC_ACCOUNT_HEALTH_TIMESTAMP_OFFSET: usize = 8 + 1832 + 96;

// ---- P-0026 (freshness-stamp monotonicity) --------------------------------------------------
// Fixed list of fixture-named subjects (pure-predicate grammar only allows TestContext reads).
const SCOUT_P26_ACCOUNT_COUNT: usize = 5;
const SCOUT_P26_BANK_COUNT: usize = 4;

// ---- P-0027 (pulse == independent re-derivation) --------------------------------------------
// Distinct prices the probe cycles through: 0, 1, a binary-exact fraction, sub-1e-9 dust, and two
// values beyond f64 precision (the handler logs `price_i80.to_num::<f64>()`, :38).
const SCOUT_P27_PRICE_CHOICES: u8 = 6;

// Bounded oracle drift, capped at 5% per call.
const SCOUT_DRIFT_MAX_BPS: u64 = 500;             // 5.00%
const SCOUT_DRIFT_BPS_DENOMINATOR: u64 = 10_000;
// Floor so drift can never reach zero.
const SCOUT_DRIFT_MIN_PRICE: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000001");

// ---- P-0028 (crank non-interference) ---------------------------------------------------------
// A "crank" is an entry point whose Accounts context declares no Signer<'info> at all.
const SCOUT_P28_ARM_NONE: u8 = 0;
const SCOUT_P28_ARM_PULSE_HEALTH: u8 = 1;
const SCOUT_P28_ARM_ACCRUE_COLLATERAL: u8 = 2;
const SCOUT_P28_ARM_ACCRUE_LIABILITY: u8 = 3;
const SCOUT_P28_ARM_PULSE_PRICE_CACHE: u8 = 4;
const SCOUT_P28_ARM_COUNT: u8 = 4;

// Clock advance taken before the pre-reading (part of the scenario, not the crank).
const SCOUT_P28_WARP_SECONDS: i64 = 5;
const SCOUT_P28_WARP_SLOTS: u64 = 12;
// LEG B follow-up: deposit into the collateral bank by the third party.
const SCOUT_P28_FOLLOWUP_DEPOSIT_AMOUNT: u64 = 1_000;

// MarginfiAccount byte layout (size 2304). Balance = active(1) bank_pk(32) bank_asset_tag(1)
// _pad0(6) asset_shares(16) liability_shares(16) emissions_outstanding(16) last_update(8) _padding(8).
const SCOUT_P28_ACC_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
const SCOUT_P28_ACC_LEN: usize = 8 + 2304;
const SCOUT_P28_ACC_BALANCES_OFFSET: usize = 8 + 64;
const SCOUT_P28_BALANCE_LEN: usize = 104;
const SCOUT_P28_BALANCE_COUNT: usize = 16;
const SCOUT_P28_BALANCE_BANK_PK_OFFSET: usize = 1;
const SCOUT_P28_BALANCE_SHARES_OFFSET: usize = 40;
const SCOUT_P28_BALANCE_EMISSIONS_OFFSET: usize = 72;
const SCOUT_P28_BALANCE_LAST_UPDATE_OFFSET: usize = 88;
const SCOUT_P28_BALANCE_PAD_OFFSET: usize = 96;
const SCOUT_P28_ACC_LENDING_PAD_OFFSET: usize = 8 + 1728;
const SCOUT_P28_ACC_FLAGS_OFFSET: usize = 8 + 1792;
const SCOUT_P28_ACC_EMISSIONS_DEST_OFFSET: usize = 8 + 1800;
const SCOUT_P28_ACC_HEALTH_CACHE_OFFSET: usize = 8 + 1832;
const SCOUT_P28_ACC_MIGRATED_OFFSET: usize = 8 + 2136;
const SCOUT_P28_ACC_LAST_UPDATE_OFFSET: usize = 8 + 2200;
const SCOUT_P28_ACC_INDEX_OFFSET: usize = 8 + 2208;
const SCOUT_P28_ACC_LIQ_RECORD_OFFSET: usize = 8 + 2216;
const SCOUT_P28_ACC_TAIL_PAD_OFFSET: usize = 8 + 2248;

// Bank byte layout (size 1856).
const SCOUT_P28_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_P28_BANK_LEN: usize = 8 + 1856;
const SCOUT_P28_BANK_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
const SCOUT_P28_BANK_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
const SCOUT_P28_BANK_VAULTS_OFFSET: usize = 8 + 104;
const SCOUT_P28_BANK_INSURANCE_FEES_OFFSET: usize = 8 + 176;
const SCOUT_P28_BANK_FEE_VAULT_OFFSET: usize = 8 + 192;
const SCOUT_P28_BANK_GROUP_FEES_OFFSET: usize = 8 + 232;
const SCOUT_P28_BANK_TOTAL_SHARES_OFFSET: usize = 8 + 248;
const SCOUT_P28_BANK_LAST_UPDATE_OFFSET: usize = 8 + 280;
const SCOUT_P28_BANK_CONFIG_OFFSET: usize = 8 + 288;
const SCOUT_P28_BANK_FLAGS_OFFSET: usize = 8 + 832;
const SCOUT_P28_BANK_EMISSIONS_RATE_OFFSET: usize = 8 + 840;
const SCOUT_P28_BANK_EMISSIONS_REMAINING_OFFSET: usize = 8 + 848;
const SCOUT_P28_BANK_EMISSIONS_MINT_OFFSET: usize = 8 + 864;
const SCOUT_P28_BANK_PROGRAM_FEES_OFFSET: usize = 8 + 896;
const SCOUT_P28_BANK_EMODE_OFFSET: usize = 8 + 912;
const SCOUT_P28_BANK_FEES_DEST_OFFSET: usize = 8 + 1336;
const SCOUT_P28_BANK_CACHE_OFFSET: usize = 8 + 1368;
const SCOUT_P28_BANK_COUNTS_OFFSET: usize = 8 + 1528;
const SCOUT_P28_BANK_INTEGRATION_OFFSET: usize = 8 + 1552;
const SCOUT_P28_BANK_TAIL_PAD_OFFSET: usize = 8 + 1648;

// One bit per FORBIDDEN region of the victim's MarginfiAccount.
const SCOUT_P28_ACC_BIT_IDENTITY: u32 = 1 << 0;
const SCOUT_P28_ACC_BIT_BALANCE_SLOT: u32 = 1 << 1;
const SCOUT_P28_ACC_BIT_BALANCE_SHARES: u32 = 1 << 2;
const SCOUT_P28_ACC_BIT_BALANCE_EMISSIONS: u32 = 1 << 3;
const SCOUT_P28_ACC_BIT_BALANCE_LAST_UPDATE: u32 = 1 << 4;
const SCOUT_P28_ACC_BIT_BALANCE_PAD: u32 = 1 << 5;
const SCOUT_P28_ACC_BIT_LENDING_PAD: u32 = 1 << 6;
const SCOUT_P28_ACC_BIT_FLAGS: u32 = 1 << 7;
const SCOUT_P28_ACC_BIT_EMISSIONS_DEST: u32 = 1 << 8;
const SCOUT_P28_ACC_BIT_HEALTH_CACHE: u32 = 1 << 9;
const SCOUT_P28_ACC_BIT_MIGRATED: u32 = 1 << 10;
const SCOUT_P28_ACC_BIT_LAST_UPDATE: u32 = 1 << 11;
const SCOUT_P28_ACC_BIT_INDEX: u32 = 1 << 12;
const SCOUT_P28_ACC_BIT_LIQ_RECORD: u32 = 1 << 13;
const SCOUT_P28_ACC_BIT_TAIL_PAD: u32 = 1 << 14;
const SCOUT_P28_ACC_BIT_SHAPE: u32 = 1 << 15;

// One bit per forbidden region of a Bank (OR over both victim banks).
const SCOUT_P28_BANK_BIT_IDENTITY: u32 = 1 << 0;
const SCOUT_P28_BANK_BIT_ASSET_SHARE_VALUE: u32 = 1 << 1;
const SCOUT_P28_BANK_BIT_LIABILITY_SHARE_VALUE: u32 = 1 << 2;
const SCOUT_P28_BANK_BIT_VAULTS: u32 = 1 << 3;
const SCOUT_P28_BANK_BIT_INSURANCE_FEES: u32 = 1 << 4;
const SCOUT_P28_BANK_BIT_FEE_VAULT: u32 = 1 << 5;
const SCOUT_P28_BANK_BIT_GROUP_FEES: u32 = 1 << 6;
const SCOUT_P28_BANK_BIT_TOTAL_SHARES: u32 = 1 << 7;
const SCOUT_P28_BANK_BIT_LAST_UPDATE: u32 = 1 << 8;
const SCOUT_P28_BANK_BIT_CONFIG: u32 = 1 << 9;
const SCOUT_P28_BANK_BIT_FLAGS: u32 = 1 << 10;
const SCOUT_P28_BANK_BIT_EMISSIONS_RATE: u32 = 1 << 11;
const SCOUT_P28_BANK_BIT_EMISSIONS_REMAINING: u32 = 1 << 12;
const SCOUT_P28_BANK_BIT_EMISSIONS_MINT: u32 = 1 << 13;
const SCOUT_P28_BANK_BIT_PROGRAM_FEES: u32 = 1 << 14;
const SCOUT_P28_BANK_BIT_EMODE: u32 = 1 << 15;
const SCOUT_P28_BANK_BIT_FEES_DEST: u32 = 1 << 16;
const SCOUT_P28_BANK_BIT_CACHE: u32 = 1 << 17;
const SCOUT_P28_BANK_BIT_COUNTS: u32 = 1 << 18;
const SCOUT_P28_BANK_BIT_INTEGRATION: u32 = 1 << 19;
const SCOUT_P28_BANK_BIT_TAIL_PAD: u32 = 1 << 20;
const SCOUT_P28_BANK_BIT_SHAPE: u32 = 1 << 21;

// ---- P-0014 / P-0016 (share-value integrity) -------------------------------------------------
// Both read the same two share-value fields off the same subject registry (SCOUT_SV_* namespace).
const SCOUT_SV_BANK_CAP: usize = 64;
// Ring capacity for banks with a socialised loss since the last baseline.
const SCOUT_SV_SOCIALIZED_CAP: usize = 8;
// Bank discriminator/length (bank.rs:16).
const SCOUT_SV_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_SV_BANK_LEN: usize = 8 + 1856;
// asset_share_value @72 / liability_share_value @88.
const SCOUT_SV_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
const SCOUT_SV_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
// BankConfig.operational_state offset, same value as SCOUT_RL_BANK_OPERATIONAL_STATE_OFFSET.
const SCOUT_SV_OPERATIONAL_STATE_OFFSET: usize = 8 + 288 + 312;
// BankOperationalState::KilledByBankruptcy is variant 3.
const SCOUT_SV_KILLED_BY_BANKRUPTCY: u8 = 3;

// ---- P-0031 / P-0032 (liquidation two-leg conservation; start/end parity) --------------------
// final_discount = 1 - (f_l+f_i) = 0.95 (liquidatee credited); liquidator_discount = 1 - f_l = 0.975 (liquidator debited).
const SCOUT_P31_FINAL_DISCOUNT: fixed::types::I80F48 = fixed::types::I80F48::lit("0.95");
const SCOUT_P31_LIQUIDATOR_DISCOUNT: fixed::types::I80F48 = fixed::types::I80F48::lit("0.975");
// P-0031's per-leg slack, in native token units (not dollars).
const SCOUT_P31_LEG_SLACK: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000001");

// ---- P-0032's bracket probe ------------------------------------------------------------------
// LIQUIDATION_BONUS_FEE_MINIMUM (constants.rs:89) is end_liquidation's premium floor.
const SCOUT_P32_BONUS_FEE_MINIMUM: fixed::types::I80F48 = fixed::types::I80F48::lit("0.05");
const SCOUT_P32_VALUE_SLACK: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000001");
// Receiver strategies: arm 0 repays 0.95 * seized (faithful, must NOT fire); arm 1 repays the
// minimum that keeps maintenance health from falling.
const SCOUT_P32_SEIZE_AMOUNT: u64 = 1_000_000;
const SCOUT_P32_FAITHFUL_REPAY_AMOUNT: u64 = 95_000;
// P-0020-DELEV: seized/repaid ratio 1.67 >> the ~1.05 premium; health guard still passes.
const SCOUT_P20_DELEV_REPAY_AMOUNT: u64 = 60_000;
const SCOUT_P32_HEALTH_NEUTRAL_REPAY_AMOUNT: u64 = 91_000;
const SCOUT_P32_ARMS: u8 = 2;

// ---------------------------------------------------------------------------------------------
// P-0010 / P-0019 -- the interest-rate model, reimplemented independently.
// SCOUT_PIR_* = Property Interest Rate.
// Bank layout (size 1856; offsets below +8 for the discriminator); cache starts at 1368.
const SCOUT_PIR_BANK_LEN: usize = 8 + 1856;
const SCOUT_PIR_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_PIR_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
const SCOUT_PIR_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
const SCOUT_PIR_TOTAL_LIABILITY_SHARES_OFFSET: usize = 8 + 248;
const SCOUT_PIR_TOTAL_ASSET_SHARES_OFFSET: usize = 8 + 264;
const SCOUT_PIR_LAST_UPDATE_OFFSET: usize = 8 + 280;
const SCOUT_PIR_CACHE_OFFSET: usize = 8 + 1368;
// BankCache (160B): base_rate u32 @0, lending_rate u32 @4, borrowing_rate u32 @8, interest_accumulated_for u32 @12.
const SCOUT_PIR_CACHE_BASE_RATE_OFFSET: usize = SCOUT_PIR_CACHE_OFFSET;
const SCOUT_PIR_CACHE_LENDING_RATE_OFFSET: usize = SCOUT_PIR_CACHE_OFFSET + 4;
const SCOUT_PIR_CACHE_BORROWING_RATE_OFFSET: usize = SCOUT_PIR_CACHE_OFFSET + 8;

// InterestRateConfig (size 240) begins at BankConfig+72 = 368. Offsets below are relative to the struct start.
const SCOUT_PIR_IRC_OFFSET: usize = 8 + 288 + 72;
const SCOUT_PIR_IRC_LEN: usize = 240;
const SCOUT_PIR_IRC_INSURANCE_FIXED: usize = 48;
const SCOUT_PIR_IRC_INSURANCE_IR: usize = 64;
const SCOUT_PIR_IRC_PROTOCOL_FIXED: usize = 80;
const SCOUT_PIR_IRC_PROTOCOL_IR: usize = 96;
const SCOUT_PIR_IRC_ZERO_UTIL_RATE: usize = 128;
const SCOUT_PIR_IRC_HUNDRED_UTIL_RATE: usize = 132;
const SCOUT_PIR_IRC_POINTS: usize = 136;
const SCOUT_PIR_IRC_CURVE_TYPE: usize = 176;
// points: [RatePoint; 5], RatePoint = {util: u32, rate: u32} -> 40 bytes, stride 8.
const SCOUT_PIR_CURVE_POINTS: usize = 5;
const SCOUT_PIR_RATE_POINT_STRIDE: usize = 8;

// MarginfiGroup (size 1056): fee_state_cache @40.
const SCOUT_PIR_GROUP_LEN: usize = 8 + 1056;
const SCOUT_PIR_GROUP_FLAGS_OFFSET: usize = 8 + 32;
const SCOUT_PIR_GROUP_PROGRAM_FEE_FIXED_OFFSET: usize = 8 + 40 + 32;
const SCOUT_PIR_GROUP_PROGRAM_FEE_RATE_OFFSET: usize = 8 + 40 + 48;
// PROGRAM_FEES_ENABLED bit (marginfi_group.rs:9).
const SCOUT_PIR_PROGRAM_FEES_ENABLED: u64 = 1;

const SCOUT_PIR_CURVE_SEVEN_POINT: u8 = 1;
const SCOUT_PIR_SECONDS_PER_YEAR: fixed::types::I80F48 = fixed::types::I80F48::lit("31536000");
// rate_from_u32(r) scales 0-1000% APR over u32; util_from_u32(u) scales 0-100%.
const SCOUT_PIR_MILLI_MAX_PERCENT: fixed::types::I80F48 = fixed::types::I80F48::lit("10");

// Curve the probe installs, in the program's u32 encodings (rate_from_u32(r)=r/u32::MAX*10,
// util_from_u32(u)=u/u32::MAX). Two used points give three distinct linear segments.
const SCOUT_PIR_CURVE_ZERO_UTIL_RATE: u32 = 0;
const SCOUT_PIR_CURVE_HUNDRED_UTIL_RATE: u32 = 429_496_729; // 100% APR
const SCOUT_PIR_CURVE_P0_UTIL: u32 = 2_147_483_647; //  50% utilization
const SCOUT_PIR_CURVE_P0_RATE: u32 = 42_949_672; //  10% APR
const SCOUT_PIR_CURVE_P1_UTIL: u32 = 3_435_973_836; //  80% utilization
const SCOUT_PIR_CURVE_P1_RATE: u32 = 171_798_691; //  40% APR
// Non-zero rate and fixed fees on both legs.
const SCOUT_PIR_CURVE_INSURANCE_IR_FEE: fixed::types::I80F48 = fixed::types::I80F48::lit("0.01");
const SCOUT_PIR_CURVE_PROTOCOL_IR_FEE: fixed::types::I80F48 = fixed::types::I80F48::lit("0.02");
const SCOUT_PIR_CURVE_INSURANCE_FIXED_APR: fixed::types::I80F48 =
    fixed::types::I80F48::lit("0.005");
const SCOUT_PIR_CURVE_PROTOCOL_FIXED_APR: fixed::types::I80F48 =
    fixed::types::I80F48::lit("0.0025");

// 2e9 deposited / 1e9 borrowed => utilization exactly 0.5 (segment boundary).
const SCOUT_PIR_LIQUIDITY_AMOUNT: u64 = 2_000_000_000;
const SCOUT_PIR_COLLATERAL_AMOUNT: u64 = 2_000_000_000;
const SCOUT_PIR_BORROW_AMOUNT: u64 = 1_000_000_000;
const SCOUT_PIR_COLLATERAL_PRICE: f64 = 10.0;
// One day. Safe to warp: both probe banks use OracleSetup::Fixed (no staleness check).
const SCOUT_PIR_ACCRUAL_SECONDS: i64 = 86_400;

// P-0010 (u32 rate codes) tolerance: one ULP, absorbing to_num::<u32>() truncation asymmetry.
const SCOUT_PIR_RATE_CODE_TOLERANCE: u32 = 1;
// P-0019 (accrued token amounts) tolerance: one native unit.
const SCOUT_PIR_TOKEN_TOLERANCE: fixed::types::I80F48 = fixed::types::I80F48::lit("1");

// ---------------------------------------------------------------------------------------------
// P-0007 -- "Bank.last_update advances only in a transaction that also books interest for the
// elapsed interval, on any bank holding both assets and liabilities."
//
// Offsets from Bank's field order (bank.rs).
const SCOUT_P7_BANK_LEN: usize = 8 + 1856;
const SCOUT_P7_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
const SCOUT_P7_BANK_TOTAL_LIABILITY_SHARES_OFFSET: usize = 8 + 248;
const SCOUT_P7_BANK_TOTAL_ASSET_SHARES_OFFSET: usize = 8 + 264;
const SCOUT_P7_BANK_LAST_UPDATE_OFFSET: usize = 8 + 280;
// BankCache.interest_accumulated_for: witness that accrue_interest reached its booking body.
const SCOUT_P7_BANK_INTEREST_ACCUMULATED_FOR_OFFSET: usize = 8 + 1368 + 12;
const SCOUT_P7_BANK_COUNT: usize = 5;
const SCOUT_P7_WARP_SECONDS: i64 = 3_600;

// ---------------------------------------------------------------------------------------------
// P-0009 -- "Every write of MarginfiAccount.health_cache stamps timestamp to the current clock,
// program_version to PROGRAM_VERSION, and sets ENGINE_OK."
//
// HealthCache field offsets (size 304); timestamp @96.
const SCOUT_P9_ACCOUNT_LEN: usize = 8 + 2304;
const SCOUT_P9_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
const SCOUT_P9_HEALTH_CACHE_OFFSET: usize = 8 + 1832;
const SCOUT_P9_HEALTH_CACHE_LEN: usize = 304;
const SCOUT_P9_HC_TIMESTAMP_OFFSET: usize = SCOUT_P9_HEALTH_CACHE_OFFSET + 96;
const SCOUT_P9_HC_FLAGS_OFFSET: usize = SCOUT_P9_HEALTH_CACHE_OFFSET + 104;
const SCOUT_P9_HC_MRGN_ERR_OFFSET: usize = SCOUT_P9_HEALTH_CACHE_OFFSET + 108;
const SCOUT_P9_HC_INTERNAL_ERR_OFFSET: usize = SCOUT_P9_HEALTH_CACHE_OFFSET + 240;
const SCOUT_P9_HC_PROGRAM_VERSION_OFFSET: usize = SCOUT_P9_HEALTH_CACHE_OFFSET + 245;
// ENGINE_OK=2, PROGRAM_VERSION=3 (pinned literals, not imports).
const SCOUT_P9_ENGINE_OK: u32 = 2;
const SCOUT_P9_PROGRAM_VERSION: u8 = 3;
const SCOUT_P9_ACCOUNT_COUNT: usize = 4;
// P-0008 byte layout: MarginfiGroup's fee_state_cache and panic_state_cache fields.
const SCOUT_P8_GROUP_DISCRIMINATOR: [u8; 8] = [182, 23, 173, 240, 151, 206, 182, 67];
const SCOUT_P8_GROUP_LEN: usize = 8 + 1056;
const SCOUT_P8_GROUP_CACHED_WALLET_OFFSET: usize = 8 + 40;
const SCOUT_P8_GROUP_CACHED_FEE_FIXED_OFFSET: usize = 8 + 72;
const SCOUT_P8_GROUP_CACHED_FEE_RATE_OFFSET: usize = 8 + 88;
const SCOUT_P8_GROUP_FEE_CACHE_STAMP_OFFSET: usize = 8 + 104;
const SCOUT_P8_GROUP_PROPAGATION_STAMP_OFFSET: usize = 8 + 264;
// FeeState field layout (fee_state.rs).
const SCOUT_P8_FEE_STATE_DISCRIMINATOR: [u8; 8] = [63, 224, 16, 85, 193, 36, 235, 220];
const SCOUT_P8_FEE_STATE_LEN: usize = 8 + 256;
const SCOUT_P8_FEE_STATE_WALLET_OFFSET: usize = 72;
const SCOUT_P8_FEE_STATE_FEE_FIXED_OFFSET: usize = 136;
const SCOUT_P8_FEE_STATE_FEE_RATE_OFFSET: usize = 152;
const SCOUT_P8_ARMS: u8 = 3;

// ---- P-0015 (the deleverage daily-withdrawal window) ------------------------------------------
// MarginfiGroup.WithdrawWindowCache (group.rs:52,84-88): daily_limit, withdrawn_today (USD,
// approximate), last_daily_reset_timestamp. `configure_deleverage_withdrawal_limit` is the only
// admin-gated writer of daily_limit and does NOT clear withdrawn_today. `update_withdrawn_equity`
// is the only writer of withdrawn_today, called from four sites all gated on ACCOUNT_IN_DELEVERAGE.
const SCOUT_P15_GROUP_ACCOUNT_LEN: usize = 8 + 1056;
const SCOUT_P15_GROUP_DISCRIMINATOR: [u8; 8] = [182, 23, 173, 240, 151, 206, 182, 67];
const SCOUT_P15_GROUP_DAILY_LIMIT_OFFSET: usize = 8 + 272;
const SCOUT_P15_GROUP_WITHDRAWN_TODAY_OFFSET: usize = 8 + 276;
// DAILY_RESET_INTERVAL, pinned literal.
const SCOUT_P15_WINDOW_SECONDS: i64 = 24 * 60 * 60;
// Ledger capacity, lineage-scoped; recording stops at the cap (stays chronological).
const SCOUT_P15_CAP: usize = 32;
// Dollar slack on the window total.
const SCOUT_P15_VALUE_SLACK: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000001");
// Scenario: every individual withdrawal < $1 while the trailing-24h total clears limit=1 (the
// smallest legal configuration). Seizing 990_000 units = $0.99; end_deleverage requires repaying
// >= 0.9 * seized = $0.891, so 892_000 leaves a small margin. Three rounds stay well within the
// 4_000_000 borrowed / 40_000_000 deposited scenario.
const SCOUT_P15_DAILY_LIMIT: u32 = 1;
const SCOUT_P15_ASSET_DEPOSIT: u64 = 40_000_000;
const SCOUT_P15_LIQUIDITY_DEPOSIT: u64 = 8_000_000;
const SCOUT_P15_BORROW: u64 = 4_000_000;
const SCOUT_P15_SEIZE_AMOUNT: u64 = 990_000;
const SCOUT_P15_REPAY_AMOUNT: u64 = 892_000;
const SCOUT_P15_MAX_ROUNDS: u8 = 3;

// SCOUT:PRELUDE:END
crucible_idl_gen::declare_fuzz_program!("idls/marginfi_program.json");

use marginfi::{accounts, instruction};

#[derive(Clone)]
struct MarginfiFixture {
    ctx: crate::__scout_crucible_test_context::TestContext,
    program_id: Pubkey,
    payer: Rc<Keypair>,
    // SCOUT:FIELDS:BEGIN
    scout_known_accounts: [Pubkey; SCOUT_KNOWN_CAP],
    scout_known_next: usize,
    scout_p4_dep_account: Pubkey,
    scout_p4_dep_bank: Pubkey,
    scout_p4_dep_tokens: u128,
    scout_p4_dep_asv: [u8; 16],
    scout_p4_bor_account: Pubkey,
    scout_p4_bor_bank: Pubkey,
    scout_p4_bor_amount: u128,
    scout_p4_bor_prev_ok: bool,
    scout_p4_bor_prev_slots: [u8; SCOUT_P4_BALANCE_REGION_LEN],
    scout_p4_bor_prev_lsv: [u8; 16],
    scout_p4_bor_cur_ok: bool,
    scout_p4_bor_cur_slots: [u8; SCOUT_P4_BALANCE_REGION_LEN],
    scout_p4_bor_cur_lsv: [u8; 16],
    scout_p1_prev_ok: bool,
    scout_p1_prev_value: i128,
    scout_p1_prev_share_values: Vec<(Pubkey, [u8; 16], [u8; 16])>,
    scout_p1_prev_actor_count: usize,
    scout_p1_cur_ok: bool,
    scout_p1_cur_value: i128,
    scout_p1_cur_tokens: i128,
    scout_p1_cur_claim: i128,
    scout_p1_cur_share_values: Vec<(Pubkey, [u8; 16], [u8; 16])>,
    scout_p1_cur_actor_count: usize,
    scout_p1_accounts: [Pubkey; SCOUT_SUBJECT_CAP],
    scout_p1_accounts_next: usize,
    scout_p17_accounts: [Pubkey; SCOUT_SUBJECT_CAP],
    scout_p17_accounts_next: usize,
    scout_p12_transferred_accounts: [Pubkey; SCOUT_SUBJECT_CAP],
    scout_p12_transferred_next: usize,
    scout_p17_harness_flagged: Vec<Pubkey>,
    scout_p11_seed_bank: [Pubkey; SCOUT_P11_SEED_CAP],
    scout_p11_seed_flags: [u64; SCOUT_P11_SEED_CAP],
    scout_p11_seed_next: usize,
    scout_p11_group: Pubkey,
    scout_p11_group_auth: [Pubkey; 7],
    scout_p11_group_flags: u64,
    scout_p20_accounts: [Pubkey; SCOUT_SUBJECT_CAP],
    scout_p20_accounts_next: usize,
    scout_p39_subjects: [Pubkey; SCOUT_SUBJECT_CAP],
    scout_p39_subjects_next: usize,
    // actors / PDAs / shadow-ledger fields the setup glue needs
    marginfi_group: Pubkey,
    marginfi_account: Pubkey,
    global_fee_wallet: Pubkey,
    bank_mint: Pubkey,
    bank: Pubkey,
    signer_token_account: Pubkey,

    staked_group: Pubkey,
    staked_settings: Pubkey,
    staked_bank: Pubkey,
    staked_oracle: Pubkey,

    clone_emode_bank: Pubkey,

    tokenless_bank: Pubkey,

    emissions_mint: Pubkey,
    emissions_funding_account: Pubkey,
    withdraw_marginfi_account: Pubkey,
    withdraw_bank: Pubkey,
    fee_withdraw_dst_token_account: Pubkey,
    withdraw_emissions_marginfi_account: Pubkey,
    withdraw_emissions_bank: Pubkey,
    withdraw_emissions_destination_account: Pubkey,
    fee_bank: Pubkey,
    fee_borrow_marginfi_account: Pubkey,
    metadata_bank: Pubkey,
    kamino_withdraw_bank: Pubkey,
    kamino_withdraw_marginfi_account: Pubkey,
    kamino_withdraw_reserve: Pubkey,
    pulse_health_healthy_account: Pubkey,
    pulse_health_healthy_bank: Pubkey,
    pulse_health_risk_rejected_account: Pubkey,
    pulse_health_risk_rejected_remaining: Vec<Pubkey>,
    borrow_marginfi_account: Pubkey,
    borrow_liab_bank: Pubkey,
    borrow_asset_bank: Pubkey,
    borrow_remaining_accounts: Vec<Pubkey>,
    perm_stake_pool: Pubkey,
    scout_p22_accounts: [Pubkey; SCOUT_SUBJECT_CAP],
    scout_p22_accounts_next: usize,
    scout_p22_solvency: Vec<(Pubkey, bool)>,
    scout_p22_bank_marks: Vec<(Pubkey, [u8; 16], [u8; 16], [u8; 16])>,
    scout_p22_cur_valued: Vec<(Pubkey, i128, i128, bool)>,
    scout_p22_cur_marks: Vec<(Pubkey, [u8; 16], [u8; 16], [u8; 16])>,
    scout_p22_forged_accounts: Vec<Pubkey>,
    scout_p36_sorted_account: Pubkey,
    scout_p06_banks: [Pubkey; SCOUT_P06_BANK_CAP],
    scout_p06_bank_next: usize,
    scout_p13_banks: [Pubkey; SCOUT_P13_BANK_CAP],
    scout_p13_bank_next: usize,
    scout_p30_banks: [Pubkey; SCOUT_P30_BANK_CAP],
    scout_p30_insurance_bits: [i128; SCOUT_P30_BANK_CAP],
    scout_p30_group_bits: [i128; SCOUT_P30_BANK_CAP],
    scout_p30_program_bits: [i128; SCOUT_P30_BANK_CAP],
    scout_p30_collect_seq: u64,
    scout_p30_collect_seq_at_baseline: u64,
    scout_hp_subject: Pubkey,
    scout_hp_kind: u8,
    scout_hp_pre_valid: bool,
    scout_hp_pre_health: i128,
    scout_hp_succeeded: bool,
    scout_p33_valid: bool,
    scout_p33_rounds: u32,
    scout_p33_gross_bits: i128,
    scout_p33_gain_bits: i128,
    scout_p33_loss_bits: i128,
    scout_p33_worst_gain_bits: i128,
    scout_p33_worst_gross_bits: i128,
    scout_p33_liquidator: Pubkey,
    scout_p33_liquidatee: Pubkey,
    scout_p21_valid: bool,
    scout_p21_rounds: u32,
    scout_p21_loss_bits: i128,
    scout_p21_gross_bits: i128,
    scout_p21_arm: u8,
    scout_p21_liquidator: Pubkey,
    scout_p21_liquidatee: Pubkey,
    scout_p23_valid: bool,
    scout_p23_rounds: u32,
    scout_p23_gain_bits: i128,
    scout_p23_gross_bits: i128,
    scout_p23_arm: u8,
    scout_p23_liquidator: Pubkey,
    scout_p23_liquidatee: Pubkey,
    scout_p29_gen_valid: bool,
    scout_p29_gen_exact: bool,
    scout_p29_gen_expect_group: i128,
    scout_p29_gen_expect_program: i128,
    scout_p29_gen_delta_group: i128,
    scout_p29_gen_delta_program: i128,
    scout_p29_gen_delta_insurance: i128,
    scout_p29_pay_valid: bool,
    scout_p29_pay_succeeded: bool,
    scout_p29_pay_dec_insurance: i128,
    scout_p29_pay_dec_group: i128,
    scout_p29_pay_dec_program: i128,
    scout_p29_pay_out_insurance: u64,
    scout_p29_pay_out_group: u64,
    scout_p29_pay_out_program: u64,
    scout_p29_pay_liquidity_out: u64,
    scout_reoccupy_keypair: Option<Rc<Keypair>>,
    scout_p24_arm: u8,
    scout_p24_valid: bool,
    scout_p24_pinned: bool,
    scout_p24_succeeded: bool,
    scout_p24_pre_health: i128,
    scout_p24_post_health: i128,
    scout_p24_victim: Pubkey,
    scout_p24_victim_authority: Pubkey,
    scout_p24_actor: Pubkey,
    scout_rl_armed: bool,
    scout_rl_arm: u8,
    scout_rl_subject: Pubkey,
    scout_rl_bank: Pubkey,
    scout_rl_other_bank: Pubkey,
    scout_rl_liab_bits: i128,
    scout_rl_wallet: u64,
    scout_rl_amount: u64,
    scout_rl_bank_state: u8,
    scout_rl_bank_tag: u8,
    scout_rl_flags: u64,
    scout_rl_succeeded: bool,
    scout_p26_account_hw: [i64; SCOUT_P26_ACCOUNT_COUNT],
    scout_p26_bank_hw: [i64; SCOUT_P26_BANK_COUNT],
    scout_p26_probe_account: Pubkey,
    scout_p27_bank: Pubkey,
    scout_p27_valid: bool,
    scout_p27_asked_bits: i128,
    scout_p27_pre_ts: i64,
    scout_p27_post_ts: i64,
    scout_sv_banks: [Pubkey; SCOUT_SV_BANK_CAP],
    scout_sv_asset_bits: [i128; SCOUT_SV_BANK_CAP],
    scout_sv_liability_bits: [i128; SCOUT_SV_BANK_CAP],
    scout_sv_forged: [bool; SCOUT_SV_BANK_CAP],
    scout_sv_socialized: [Pubkey; SCOUT_SV_SOCIALIZED_CAP],
    scout_sv_socialized_next: usize,
    scout_p28_arm: u8,
    scout_p28_measured: bool,
    scout_p28_health_valid: bool,
    scout_p28_pinned: bool,
    scout_p28_succeeded: bool,
    scout_p28_account_mask: u32,
    scout_p28_bank_mask: u32,
    scout_p28_first_offset: u32,
    scout_p28_pre_health: i128,
    scout_p28_post_health: i128,
    scout_p28_followup_measured: bool,
    scout_p28_followup_ok: bool,
    scout_p28_victim: Pubkey,
    scout_p28_actor: Pubkey,
    scout_p31_valid: bool,
    scout_p31_rounds: u32,
    scout_p31_worst_bits: i128,
    scout_p31_collateral_residual_bits: i128,
    scout_p31_liability_residual_bits: i128,
    scout_p31_collateral_leg_bits: i128,
    scout_p31_liquidator_liab_leg_bits: i128,
    scout_p31_liquidatee_liab_leg_bits: i128,
    scout_p31_arm: u8,
    scout_p31_liquidator: Pubkey,
    scout_p31_liquidatee: Pubkey,
    scout_p32_valid: bool,
    scout_p32_brackets: u32,
    scout_p32_seized_bits: i128,
    scout_p32_repaid_bits: i128,
    scout_p32_max_fee_bits: i128,
    scout_p32_pre_assets_bits: i128,
    scout_p32_arm: u8,
    scout_p32_account: Pubkey,
    scout_pir_bank: Pubkey,
    scout_pir_ready: bool,
    scout_pir_acc_valid: bool,
    scout_pir_acc_bank: Pubkey,
    scout_pir_acc_delta: u64,
    scout_pir_acc_pre_asv: i128,
    scout_pir_acc_pre_lsv: i128,
    scout_pir_acc_post_asv: i128,
    scout_pir_acc_post_lsv: i128,
    scout_pir_acc_pre_asset_shares: i128,
    scout_pir_acc_pre_liability_shares: i128,
    scout_pir_acc_irc: [u8; SCOUT_PIR_IRC_LEN],
    scout_pir_acc_program_fee_fixed: i128,
    scout_pir_acc_program_fee_rate: i128,
    scout_pir_acc_program_fees: bool,
    scout_p7_bank: Pubkey,
    scout_p7_ready: bool,
    scout_p7_prev_valid: [bool; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_bank: [Pubkey; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_last_update: [i64; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_both_sided: [bool; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_valid: [bool; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_bank: [Pubkey; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_last_update: [i64; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_both_sided: [bool; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_asset_sv: [[u8; 16]; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_liab_sv: [[u8; 16]; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_asset_sv: [[u8; 16]; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_liab_sv: [[u8; 16]; SCOUT_P7_BANK_COUNT],
    scout_p9_clock: i64,
    scout_p9_prev_valid: [bool; SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_prev_account: [Pubkey; SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_prev_digest: [u64; SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_cur_valid: [bool; SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_cur_account: [Pubkey; SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_cur_digest: [u64; SCOUT_P9_ACCOUNT_COUNT],
    scout_p15_next: usize,
    scout_p15_ts: [i64; SCOUT_P15_CAP],
    scout_p15_value_bits: [i128; SCOUT_P15_CAP],
    scout_p15_limit: [u32; SCOUT_P15_CAP],
    // --- generated `action_lending_account_liquidate` scenario -------------------------------
    scout_liq_asset_bank: Pubkey,
    scout_liq_liab_bank: Pubkey,
    scout_liq_liquidator: Pubkey,
    scout_liq_liquidatee: Pubkey,
    scout_liq_remaining: Vec<Pubkey>,
    // SCOUT:FIELDS:END
}

#[fuzz_fixture]
impl MarginfiFixture {
    fn scout_placeholder(&self) -> Pubkey { Pubkey::new_unique() }

    pub fn setup() -> Self {
        let mut ctx = crate::__scout_crucible_test_context::TestContext::new();
        let program_id = Pubkey::new_from_array(marginfi::ID.to_bytes());
        // SCOUT:TARGET-PROGRAM:BEGIN
        crate::__scout_crucible_test_context::TestContext::add_program(&mut ctx, &program_id, SCOUT_TARGET_PROGRAM_ARTIFACT).unwrap();
        // SCOUT:TARGET-PROGRAM:END
        let payer = Rc::new(Keypair::new());
        ctx.create_account().pubkey(payer.pubkey()).lamports(1_000_000_000)
            .owner(system_program::ID).create().unwrap();
        // SCOUT:SETUP-GLUE:BEGIN
        let bank_mint_pubkey = ctx
            .create_mint()
            .pubkey(Pubkey::new_unique())
            .decimals(6)
            .mint_authority(payer.pubkey())
            .create()
            .unwrap();

        let global_fee_wallet_pubkey = Pubkey::new_unique();
        ctx.create_account()
            .pubkey(global_fee_wallet_pubkey)
            .lamports(1_000_000_000)
            .owner(system_program::ID)
            .create()
            .unwrap();

        let signer_token_account_pubkey = ctx
            .create_token_account()
            .pubkey(Pubkey::new_unique())
            .mint(bank_mint_pubkey)
            .token_owner(payer.pubkey())
            .amount(u64::MAX)
            .create()
            .unwrap();

        let mut fixture = Self {
    scout_known_accounts: [Pubkey::default(); SCOUT_KNOWN_CAP],
    scout_known_next: 0,
    scout_p1_prev_ok: false,
    scout_p1_prev_value: 0,
    scout_p1_prev_share_values: Vec::new(),
    scout_p1_prev_actor_count: 0,
    scout_p1_cur_ok: false,
    scout_p1_cur_value: 0,
    scout_p1_cur_tokens: 0,
    scout_p1_cur_claim: 0,
    scout_p1_cur_share_values: Vec::new(),
    scout_p1_cur_actor_count: 0,
    scout_p1_accounts: [Pubkey::default(); SCOUT_SUBJECT_CAP],
    scout_p1_accounts_next: 0,
    scout_p4_dep_account: Pubkey::default(),
    scout_p4_dep_bank: Pubkey::default(),
    scout_p4_dep_tokens: 0,
    scout_p4_dep_asv: [0u8; 16],
    scout_p4_bor_account: Pubkey::default(),
    scout_p4_bor_bank: Pubkey::default(),
    scout_p4_bor_amount: 0,
    scout_p4_bor_prev_ok: false,
    scout_p4_bor_prev_slots: [0u8; SCOUT_P4_BALANCE_REGION_LEN],
    scout_p4_bor_prev_lsv: [0u8; 16],
    scout_p4_bor_cur_ok: false,
    scout_p4_bor_cur_slots: [0u8; SCOUT_P4_BALANCE_REGION_LEN],
    scout_p4_bor_cur_lsv: [0u8; 16],
    scout_p11_seed_bank: [Pubkey::default(); SCOUT_P11_SEED_CAP],
    scout_p11_seed_flags: [0u64; SCOUT_P11_SEED_CAP],
    scout_p11_seed_next: 0,
    scout_p11_group: Pubkey::default(),
    scout_p11_group_auth: [Pubkey::default(); 7],
    scout_p11_group_flags: 0,
    scout_p17_accounts: [Pubkey::default(); SCOUT_SUBJECT_CAP],
    scout_p17_accounts_next: 0,
    scout_p12_transferred_accounts: [Pubkey::default(); SCOUT_SUBJECT_CAP],
    scout_p12_transferred_next: 0,
    scout_p17_harness_flagged: Vec::new(),
    scout_p20_accounts: [Pubkey::default(); SCOUT_SUBJECT_CAP],
    scout_p20_accounts_next: 0,
    scout_p39_subjects: [Pubkey::default(); SCOUT_SUBJECT_CAP],
    scout_p39_subjects_next: 0,
    scout_hp_subject: Pubkey::default(),
    scout_hp_kind: SCOUT_HP_KIND_NONE,
    scout_hp_pre_valid: false,
    scout_hp_pre_health: 0,
    scout_hp_succeeded: false,
    scout_p33_valid: false,
    scout_p33_rounds: 0,
    scout_p33_gross_bits: 0,
    scout_p33_gain_bits: 0,
    scout_p33_loss_bits: 0,
    scout_p33_worst_gain_bits: 0,
    scout_p33_worst_gross_bits: 0,
    scout_p33_liquidator: Pubkey::default(),
    scout_p33_liquidatee: Pubkey::default(),
    scout_p21_valid: false,
    scout_p21_rounds: 0,
    scout_p21_loss_bits: 0,
    scout_p21_gross_bits: 0,
    scout_p21_arm: 0,
    scout_p21_liquidator: Pubkey::default(),
    scout_p21_liquidatee: Pubkey::default(),
    scout_p23_valid: false,
    scout_p23_rounds: 0,
    scout_p23_gain_bits: 0,
    scout_p23_gross_bits: 0,
    scout_p23_arm: 0,
    scout_p23_liquidator: Pubkey::default(),
    scout_p23_liquidatee: Pubkey::default(),
    scout_p31_valid: false,
    scout_p31_rounds: 0,
    scout_p31_worst_bits: 0,
    scout_p31_collateral_residual_bits: 0,
    scout_p31_liability_residual_bits: 0,
    scout_p31_collateral_leg_bits: 0,
    scout_p31_liquidator_liab_leg_bits: 0,
    scout_p31_liquidatee_liab_leg_bits: 0,
    scout_p31_arm: 0,
    scout_p31_liquidator: Pubkey::default(),
    scout_p31_liquidatee: Pubkey::default(),
    scout_p32_valid: false,
    scout_p32_brackets: 0,
    scout_p32_seized_bits: 0,
    scout_p32_repaid_bits: 0,
    scout_p32_max_fee_bits: 0,
    scout_p32_pre_assets_bits: 0,
    scout_p32_arm: 0,
    scout_p32_account: Pubkey::default(),
    scout_p29_gen_valid: false,
    scout_p29_gen_exact: false,
    scout_p29_gen_expect_group: 0,
    scout_p29_gen_expect_program: 0,
    scout_p29_gen_delta_group: 0,
    scout_p29_gen_delta_program: 0,
    scout_p29_gen_delta_insurance: 0,
    scout_p29_pay_valid: false,
    scout_p29_pay_succeeded: false,
    scout_p29_pay_dec_insurance: 0,
    scout_p29_pay_dec_group: 0,
    scout_p29_pay_dec_program: 0,
    scout_p29_pay_out_insurance: 0,
    scout_p29_pay_out_group: 0,
    scout_p29_pay_out_program: 0,
    scout_p29_pay_liquidity_out: 0,
    scout_reoccupy_keypair: None,
    scout_p26_account_hw: [0i64; SCOUT_P26_ACCOUNT_COUNT],
    scout_p26_bank_hw: [0i64; SCOUT_P26_BANK_COUNT],
    scout_p26_probe_account: Pubkey::default(),
    scout_p27_bank: Pubkey::default(),
    scout_p27_valid: false,
    scout_p27_asked_bits: 0,
    scout_p27_pre_ts: 0,
    scout_pir_bank: Pubkey::default(),
    scout_pir_ready: false,
    scout_pir_acc_valid: false,
    scout_pir_acc_bank: Pubkey::default(),
    scout_pir_acc_delta: 0,
    scout_pir_acc_pre_asv: 0,
    scout_pir_acc_pre_lsv: 0,
    scout_pir_acc_post_asv: 0,
    scout_pir_acc_post_lsv: 0,
    scout_pir_acc_pre_asset_shares: 0,
    scout_pir_acc_pre_liability_shares: 0,
    scout_pir_acc_irc: [0u8; SCOUT_PIR_IRC_LEN],
    scout_pir_acc_program_fee_fixed: 0,
    scout_pir_acc_program_fee_rate: 0,
    scout_pir_acc_program_fees: false,
    scout_p27_post_ts: 0,
    scout_p28_arm: SCOUT_P28_ARM_NONE,
    scout_p28_measured: false,
    scout_p28_health_valid: false,
    scout_p28_pinned: false,
    scout_p28_succeeded: false,
    scout_p28_account_mask: 0,
    scout_p28_bank_mask: 0,
    scout_p28_first_offset: 0,
    scout_p28_pre_health: 0,
    scout_p28_post_health: 0,
    scout_p28_followup_measured: false,
    scout_p28_followup_ok: false,
    scout_p28_victim: Pubkey::default(),
    scout_p28_actor: Pubkey::default(),
    scout_sv_banks: [Pubkey::default(); SCOUT_SV_BANK_CAP],
    scout_sv_asset_bits: [0i128; SCOUT_SV_BANK_CAP],
    scout_sv_liability_bits: [0i128; SCOUT_SV_BANK_CAP],
    scout_sv_forged: [false; SCOUT_SV_BANK_CAP],
    scout_sv_socialized: [Pubkey::default(); SCOUT_SV_SOCIALIZED_CAP],
    scout_sv_socialized_next: 0,
    scout_rl_armed: false,
    scout_rl_arm: SCOUT_RL_ARM_NONE,
    scout_rl_subject: Pubkey::default(),
    scout_rl_bank: Pubkey::default(),
    scout_rl_other_bank: Pubkey::default(),
    scout_rl_liab_bits: 0,
    scout_rl_wallet: 0,
    scout_rl_amount: 0,
    scout_rl_bank_state: 0,
    scout_rl_bank_tag: 0,
    scout_rl_flags: 0,
    scout_rl_succeeded: false,
    ctx: ctx,
    program_id: program_id,
    payer: payer,
    marginfi_group: Pubkey::default(),
    marginfi_account: Pubkey::default(),
    global_fee_wallet: global_fee_wallet_pubkey,
    bank_mint: bank_mint_pubkey,
    bank: Pubkey::default(),
    signer_token_account: signer_token_account_pubkey,
    staked_group: Pubkey::default(),
    staked_settings: Pubkey::default(),
    staked_bank: Pubkey::default(),
    staked_oracle: Pubkey::default(),
    clone_emode_bank: Pubkey::default(),
    tokenless_bank: Pubkey::default(),
    emissions_funding_account: Pubkey::default(),
    emissions_mint: Pubkey::default(),
    withdraw_bank: Pubkey::default(),
    withdraw_marginfi_account: Pubkey::default(),
    fee_withdraw_dst_token_account: Pubkey::default(),
    withdraw_emissions_bank: Pubkey::default(),
    withdraw_emissions_destination_account: Pubkey::default(),
    withdraw_emissions_marginfi_account: Pubkey::default(),
    fee_bank: Pubkey::default(),
    fee_borrow_marginfi_account: Pubkey::default(),
    metadata_bank: Pubkey::default(),
    kamino_withdraw_bank: Pubkey::default(),
    kamino_withdraw_marginfi_account: Pubkey::default(),
    kamino_withdraw_reserve: Pubkey::default(),
    pulse_health_healthy_account: Pubkey::default(),
    pulse_health_healthy_bank: Pubkey::default(),
    pulse_health_risk_rejected_account: Pubkey::default(),
    pulse_health_risk_rejected_remaining: Vec::new(),
    borrow_marginfi_account: Pubkey::default(),
    borrow_liab_bank: Pubkey::default(),
    borrow_asset_bank: Pubkey::default(),
    borrow_remaining_accounts: Vec::new(),
    scout_liq_asset_bank: Pubkey::default(),
    scout_liq_liab_bank: Pubkey::default(),
    scout_liq_liquidator: Pubkey::default(),
    scout_liq_liquidatee: Pubkey::default(),
    scout_liq_remaining: Vec::new(),
    perm_stake_pool: Pubkey::default(),
    scout_p22_accounts: [Pubkey::default(); SCOUT_SUBJECT_CAP],
    scout_p22_accounts_next: 0,
    scout_p22_solvency: Vec::new(),
    scout_p22_bank_marks: Vec::new(),
    scout_p22_cur_valued: Vec::new(),
    scout_p22_cur_marks: Vec::new(),
    scout_p22_forged_accounts: Vec::new(),
    scout_p36_sorted_account: Pubkey::default(),
    scout_p06_banks: [Pubkey::default(); SCOUT_P06_BANK_CAP],
    scout_p06_bank_next: 0,
    scout_p13_banks: [Pubkey::default(); SCOUT_P13_BANK_CAP],
    scout_p13_bank_next: 0,
    scout_p30_banks: [Pubkey::default(); SCOUT_P30_BANK_CAP],
    scout_p30_insurance_bits: [0i128; SCOUT_P30_BANK_CAP],
    scout_p30_group_bits: [0i128; SCOUT_P30_BANK_CAP],
    scout_p30_program_bits: [0i128; SCOUT_P30_BANK_CAP],
    scout_p30_collect_seq: 0,
    scout_p30_collect_seq_at_baseline: 0,
    scout_p24_arm: SCOUT_P24_ARM_NONE,
    scout_p24_valid: false,
    scout_p24_pinned: false,
    scout_p24_succeeded: false,
    scout_p24_pre_health: 0,
    scout_p24_post_health: 0,
    scout_p24_victim: Pubkey::default(),
    scout_p24_victim_authority: Pubkey::default(),
    scout_p24_actor: Pubkey::default(),
    scout_p7_bank: Pubkey::default(),
    scout_p7_ready: false,
    scout_p7_prev_valid: [false; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_bank: [Pubkey::default(); SCOUT_P7_BANK_COUNT],
    scout_p7_prev_last_update: [0i64; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_both_sided: [false; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_valid: [false; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_bank: [Pubkey::default(); SCOUT_P7_BANK_COUNT],
    scout_p7_cur_last_update: [0i64; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_both_sided: [false; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_asset_sv: [[0u8; 16]; SCOUT_P7_BANK_COUNT],
    scout_p7_prev_liab_sv: [[0u8; 16]; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_asset_sv: [[0u8; 16]; SCOUT_P7_BANK_COUNT],
    scout_p7_cur_liab_sv: [[0u8; 16]; SCOUT_P7_BANK_COUNT],
    scout_p9_clock: 0,
    scout_p9_prev_valid: [false; SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_prev_account: [Pubkey::default(); SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_prev_digest: [0u64; SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_cur_valid: [false; SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_cur_account: [Pubkey::default(); SCOUT_P9_ACCOUNT_COUNT],
    scout_p9_cur_digest: [0u64; SCOUT_P9_ACCOUNT_COUNT],
    scout_p15_next: 0,
    scout_p15_ts: [0i64; SCOUT_P15_CAP],
    scout_p15_value_bits: [0i128; SCOUT_P15_CAP],
    scout_p15_limit: [0u32; SCOUT_P15_CAP],
};
        assert!(
            fixture.action_init_global_fee_state(0, 0),
            "setup: init_global_fee_state prerequisite for marginfi_group_initialize failed"
        );

        let marginfi_group_keypair = Keypair::new();
        let marginfi_group_pubkey = marginfi_group_keypair.pubkey();
        let fee_state_pda = Pubkey::find_program_address(&[FEE_STATE_SEED], &fixture.program_id).0;
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::MarginfiGroupInitialize {})
                .accounts(accounts::MarginfiGroupInitialize {
                    marginfi_group: marginfi_group_pubkey,
                    admin: fixture.payer.pubkey(),
                    fee_state: fee_state_pda,
                })
                .signers(&[&*fixture.payer, &marginfi_group_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: marginfi_group_initialize prerequisite for marginfi_group_configure (and \
             every other instruction needing an existing group) failed"
        );
        fixture.marginfi_group = marginfi_group_pubkey;

        let marginfi_account_keypair = Keypair::new();
        let marginfi_account_pubkey = marginfi_account_keypair.pubkey();
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::MarginfiAccountInitialize {})
                .accounts(accounts::MarginfiAccountInitialize {
                    marginfi_group: fixture.marginfi_group,
                    marginfi_account: marginfi_account_pubkey,
                    authority: fixture.payer.pubkey(),
                    fee_payer: fixture.payer.pubkey(),
                })
                .signers(&[&*fixture.payer, &marginfi_account_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: marginfi_account_initialize prerequisite for the lending_account_* family \
             (and every other instruction needing an existing marginfi_account) failed"
        );
        fixture.marginfi_account = marginfi_account_pubkey;
        fixture.scout_register_subject_account(marginfi_account_pubkey);
        fixture.scout_known_accounts[fixture.scout_known_next % SCOUT_KNOWN_CAP] =
            marginfi_account_pubkey;
        fixture.scout_known_next += 1;

        let bank_keypair = Keypair::new();
        let bank_pubkey = bank_keypair.pubkey();
        let (liquidity_vault_authority, _) = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, bank_pubkey.as_ref()],
            &fixture.program_id,
        );
        let (liquidity_vault, _) = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, bank_pubkey.as_ref()],
            &fixture.program_id,
        );
        let (insurance_vault_authority, _) = Pubkey::find_program_address(
            &[INSURANCE_VAULT_AUTHORITY_SEED, bank_pubkey.as_ref()],
            &fixture.program_id,
        );
        let (insurance_vault, _) = Pubkey::find_program_address(
            &[INSURANCE_VAULT_SEED, bank_pubkey.as_ref()],
            &fixture.program_id,
        );
        let (fee_vault_authority, _) = Pubkey::find_program_address(
            &[FEE_VAULT_AUTHORITY_SEED, bank_pubkey.as_ref()],
            &fixture.program_id,
        );
        let (fee_vault, _) = Pubkey::find_program_address(
            &[FEE_VAULT_SEED, bank_pubkey.as_ref()],
            &fixture.program_id,
        );
        let mut bank_config = scout_valid_bank_config(10);
        bank_config.deposit_limit = u64::MAX;
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBank { bank_config })
                .accounts(scout_lending_pool_add_bank_accounts(fixture.marginfi_group, fixture.payer.pubkey(), fixture.payer.pubkey(), fee_state_pda, fixture.global_fee_wallet, fixture.bank_mint, bank_pubkey, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, spl_token::id()))
                .signers(&[&*fixture.payer, &bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank prerequisite for the lending_account_*/\
             lending_pool_configure_bank* family failed"
        );
        fixture.bank = bank_pubkey;

        let staked_group_keypair = Keypair::new();
        let staked_group_pubkey = staked_group_keypair.pubkey();
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::MarginfiGroupInitialize {})
                .accounts(accounts::MarginfiGroupInitialize {
                    marginfi_group: staked_group_pubkey,
                    admin: fixture.payer.pubkey(),
                    fee_state: fee_state_pda,
                })
                .signers(&[&*fixture.payer, &staked_group_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: marginfi_group_initialize (second instance, for the staked-collateral \
             chain) prerequisite for propagate_staked_settings failed"
        );
        fixture.staked_group = staked_group_pubkey;

        let staked_oracle_pubkey = Pubkey::new_unique();
        let oracle_bytes = scout_price_update_v2_bytes([7u8; 32], 100_000_000, 1);
        fixture.ctx.create_account()
            .pubkey(staked_oracle_pubkey)
            .owner(pyth_receiver_program_id())
            .data(&oracle_bytes)
            .create()
            .unwrap();
        fixture.staked_oracle = staked_oracle_pubkey;

        let (staked_settings_pda, _) = Pubkey::find_program_address(
            &[STAKED_SETTINGS_SEED, staked_group_pubkey.as_ref()],
            &fixture.program_id,
        );
        let staked_settings_config = marginfi::types::StakedSettingsConfig {
            oracle: staked_oracle_pubkey,
            asset_weight_init: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.5)),
            asset_weight_maint: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.6)),
            deposit_limit: 1_000_000_000,
            total_asset_value_init_limit: 10_000_000_000,
            oracle_max_age: 60,
            risk_tier: marginfi::types::RiskTier::Collateral,
        };
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::InitStakedSettings { settings: staked_settings_config })
                .accounts(accounts::InitStakedSettings {
                    marginfi_group: staked_group_pubkey,
                    admin: fixture.payer.pubkey(),
                    fee_payer: fixture.payer.pubkey(),
                    staked_settings: staked_settings_pda,
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: init_staked_settings (second/staked_group instance) prerequisite for \
             lending_pool_add_bank_permissionless/propagate_staked_settings failed"
        );
        fixture.staked_settings = staked_settings_pda;

        // add_pool_permissionless derives the ENTIRE single-pool chain from the validator
        // vote account and rejects anything that does not match
        // (staked_pool_utils.rs / type-crate pdas.rs:19-35), so build it in that direction:
        // vote -> stake_pool -> {mint, sol_pool, onramp}. Each leg also has an owner check.
        let staked_validator_vote_account = Pubkey::new_unique();
        fixture.ctx.create_account()
            .pubkey(staked_validator_vote_account)
            .owner(vote_program_id())
            .lamports(1_000_000)
            .create()
            .unwrap();
        let (stake_pool_pubkey, _) = Pubkey::find_program_address(
            &[b"pool", staked_validator_vote_account.as_ref()],
            &spl_single_pool_id(),
        );
        fixture.ctx.create_account()
            .pubkey(stake_pool_pubkey)
            .owner(spl_single_pool_id())
            .lamports(1_000_000)
            .create()
            .unwrap();
        let (lst_mint_pda, _) = Pubkey::find_program_address(
            &[b"mint", stake_pool_pubkey.as_ref()],
            &spl_single_pool_id(),
        );
        let (sol_pool_pda, _) = Pubkey::find_program_address(
            &[b"stake", stake_pool_pubkey.as_ref()],
            &spl_single_pool_id(),
        );
        let (staked_pool_onramp, _) = Pubkey::find_program_address(
            &[b"onramp", stake_pool_pubkey.as_ref()],
            &spl_single_pool_id(),
        );
        fixture.ctx.create_account()
            .pubkey(staked_pool_onramp)
            .owner(native_stake_id())
            .lamports(1_000_000)
            .create()
            .unwrap();
        fixture.ctx.create_mint()
            .pubkey(lst_mint_pda)
            .decimals(9)
            .mint_authority(fixture.payer.pubkey())
            .create()
            .unwrap();
        fixture.ctx.create_account()
            .pubkey(sol_pool_pda)
            .owner(native_stake_id())
            .lamports(1_000_000)
            .create()
            .unwrap();

        let staked_bank_seed: u64 = 0;
        let (staked_bank_pubkey, _) = Pubkey::find_program_address(
            &[
                staked_group_pubkey.as_ref(),
                lst_mint_pda.as_ref(),
                &staked_bank_seed.to_le_bytes(),
            ],
            &fixture.program_id,
        );
        let (staked_liquidity_vault_authority, staked_liquidity_vault, staked_insurance_vault_authority, staked_insurance_vault, staked_fee_vault_authority, staked_fee_vault) = scout_bank_vault_pdas(fixture.program_id, staked_bank_pubkey);
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBankPermissionless { bank_seed: staked_bank_seed })
                .accounts(accounts::LendingPoolAddBankPermissionless {
                    marginfi_group: staked_group_pubkey,
                    staked_settings: staked_settings_pda,
                    fee_payer: fixture.payer.pubkey(),
                    bank_mint: lst_mint_pda,
                    sol_pool: sol_pool_pda,
                    pool_onramp: staked_pool_onramp,
                    validator_vote_account: staked_validator_vote_account,
                    stake_pool: stake_pool_pubkey,
                    bank: staked_bank_pubkey,
                    liquidity_vault_authority: staked_liquidity_vault_authority,
                    liquidity_vault: staked_liquidity_vault,
                    insurance_vault_authority: staked_insurance_vault_authority,
                    insurance_vault: staked_insurance_vault,
                    fee_vault_authority: staked_fee_vault_authority,
                    fee_vault: staked_fee_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(vec![staked_oracle_pubkey, lst_mint_pda, sol_pool_pda, staked_pool_onramp])
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank_permissionless prerequisite for \
             propagate_staked_settings failed"
        );
        fixture.staked_bank = staked_bank_pubkey;

        let clone_emode_bank_keypair = Keypair::new();
        let clone_emode_bank_pubkey = clone_emode_bank_keypair.pubkey();
        let (clone_emode_liquidity_vault_authority, clone_emode_liquidity_vault, clone_emode_insurance_vault_authority, clone_emode_insurance_vault, clone_emode_fee_vault_authority, clone_emode_fee_vault) = scout_bank_vault_pdas(fixture.program_id, clone_emode_bank_pubkey);
        let mut clone_emode_bank_config = scout_valid_bank_config(10);
        clone_emode_bank_config.deposit_limit = u64::MAX;
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: clone_emode_bank_config })
                .accounts(scout_lending_pool_add_bank_accounts(fixture.marginfi_group, fixture.payer.pubkey(), fixture.payer.pubkey(), fee_state_pda, fixture.global_fee_wallet, fixture.bank_mint, clone_emode_bank_pubkey, clone_emode_liquidity_vault_authority, clone_emode_liquidity_vault, clone_emode_insurance_vault_authority, clone_emode_insurance_vault, clone_emode_fee_vault_authority, clone_emode_fee_vault, spl_token::id()))
                .signers(&[&*fixture.payer, &clone_emode_bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank (second instance, for lending_pool_clone_emode's \
             copy_to_bank) failed"
        );
        fixture.clone_emode_bank = clone_emode_bank_pubkey;

        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::MarginfiGroupConfigure {
                    new_admin: Some(fixture.payer.pubkey()),
                    new_emode_admin: Some(fixture.payer.pubkey()),
                    new_curve_admin: Some(fixture.payer.pubkey()),
                    new_limit_admin: Some(fixture.payer.pubkey()),
                    new_emissions_admin: Some(fixture.payer.pubkey()),
                    new_metadata_admin: Some(fixture.payer.pubkey()),
                    new_risk_admin: Some(fixture.payer.pubkey()),
                    new_flow_admin: Some(fixture.payer.pubkey()),
                    emode_max_init_leverage: None,
                    emode_max_maint_leverage: None,
                    same_asset_emode_init_leverage: None,
                    same_asset_emode_maint_leverage: None,
                })
                .accounts(accounts::MarginfiGroupConfigure {
                    marginfi_group: fixture.marginfi_group,
                    admin: fixture.payer.pubkey(),
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: marginfi_group_configure prerequisite for \
             lending_pool_force_tokenless_repay_complete (and every other admin-role-gated \
             instruction relying on a real, non-zero risk_admin/delegate_*_admin/emode_admin/ \
             metadata_admin) failed"
        );
        let tokenless_bank_keypair = Keypair::new();
        let tokenless_bank_pubkey = tokenless_bank_keypair.pubkey();
        let (tokenless_liquidity_vault_authority, tokenless_liquidity_vault, tokenless_insurance_vault_authority, tokenless_insurance_vault, tokenless_fee_vault_authority, tokenless_fee_vault) = scout_bank_vault_pdas(fixture.program_id, tokenless_bank_pubkey);
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: scout_valid_bank_config(10) })
                .accounts(scout_lending_pool_add_bank_accounts(fixture.marginfi_group, fixture.payer.pubkey(), fixture.payer.pubkey(), fee_state_pda, fixture.global_fee_wallet, fixture.bank_mint, tokenless_bank_pubkey, tokenless_liquidity_vault_authority, tokenless_liquidity_vault, tokenless_insurance_vault_authority, tokenless_insurance_vault, tokenless_fee_vault_authority, tokenless_fee_vault, spl_token::id()))
                .signers(&[&*fixture.payer, &tokenless_bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank (second/tokenless_bank instance) prerequisite for \
             lending_pool_force_tokenless_repay_complete failed"
        );
        fixture.tokenless_bank = tokenless_bank_pubkey;

        let mut tokenless_bank_config_opt = scout_valid_bank_config_opt();
        tokenless_bank_config_opt.tokenless_repayments_allowed = Some(true);
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolConfigureBank { bank_config_opt: tokenless_bank_config_opt })
                .accounts(accounts::LendingPoolConfigureBank {
                    group: fixture.marginfi_group,
                    admin: fixture.payer.pubkey(),
                    bank: tokenless_bank_pubkey,
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_configure_bank (flipping tokenless_repayments_allowed=true on \
             tokenless_bank) prerequisite for lending_pool_force_tokenless_repay_complete failed"
        );

        let emissions_mint_pubkey = fixture.ctx
            .create_mint()
            .pubkey(Pubkey::new_unique())
            .decimals(6)
            .mint_authority(fixture.payer.pubkey())
            .create()
            .unwrap();

        let emissions_funding_account_pubkey = fixture.ctx
            .create_token_account()
            .pubkey(Pubkey::new_unique())
            .mint(emissions_mint_pubkey)
            .token_owner(fixture.payer.pubkey())
            .amount(u64::MAX)
            .create()
            .unwrap();
        fixture.emissions_mint = emissions_mint_pubkey;
        fixture.emissions_funding_account = emissions_funding_account_pubkey;

        let withdraw_account_keypair = Keypair::new();
        let withdraw_account_pubkey = withdraw_account_keypair.pubkey();
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::MarginfiAccountInitialize {})
                .accounts(accounts::MarginfiAccountInitialize {
                    marginfi_group: fixture.marginfi_group,
                    marginfi_account: withdraw_account_pubkey,
                    authority: fixture.payer.pubkey(),
                    fee_payer: fixture.payer.pubkey(),
                })
                .signers(&[&*fixture.payer, &withdraw_account_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: marginfi_account_initialize prerequisite for lending_account_withdraw failed"
        );
        fixture.withdraw_marginfi_account = withdraw_account_pubkey;
        fixture.scout_register_subject_account(withdraw_account_pubkey);

        let withdraw_bank_keypair = Keypair::new();
        let withdraw_bank_pubkey = withdraw_bank_keypair.pubkey();
        let (withdraw_liquidity_vault_authority, withdraw_liquidity_vault, withdraw_insurance_vault_authority, withdraw_insurance_vault, withdraw_fee_vault_authority, withdraw_fee_vault) = scout_bank_vault_pdas(fixture.program_id, withdraw_bank_pubkey);
        let mut withdraw_bank_config = scout_valid_bank_config(10);
        withdraw_bank_config.deposit_limit = u64::MAX;
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: withdraw_bank_config })
                .accounts(scout_lending_pool_add_bank_accounts(fixture.marginfi_group, fixture.payer.pubkey(), fixture.payer.pubkey(), fee_state_pda, fixture.global_fee_wallet, fixture.bank_mint, withdraw_bank_pubkey, withdraw_liquidity_vault_authority, withdraw_liquidity_vault, withdraw_insurance_vault_authority, withdraw_insurance_vault, withdraw_fee_vault_authority, withdraw_fee_vault, spl_token::id()))
                .signers(&[&*fixture.payer, &withdraw_bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank prerequisite for lending_account_withdraw failed"
        );
        fixture.withdraw_bank = withdraw_bank_pubkey;

        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingAccountDeposit {
                    amount: SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT,
                    deposit_up_to_limit: None,
                })
                .accounts(accounts::LendingAccountDeposit {
                    group: fixture.marginfi_group,
                    marginfi_account: fixture.withdraw_marginfi_account,
                    authority: fixture.payer.pubkey(),
                    bank: fixture.withdraw_bank,
                    signer_token_account: fixture.signer_token_account,
                    liquidity_vault: withdraw_liquidity_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_account_deposit seed balance for lending_account_withdraw failed"
        );

        let fee_withdraw_dst_token_account_pubkey = fixture.ctx
            .create_token_account()
            .pubkey(Pubkey::new_unique())
            .mint(bank_mint_pubkey)
            .token_owner(fixture.payer.pubkey())
            .amount(0)
            .create()
            .unwrap();
        let ctx = &mut fixture.ctx;
        let payer = fixture.payer.clone();
        let _scout_lending_pool_withdraw_fees_preexisting_tail_token_account = ctx
            .create_token_account()
            .pubkey(Pubkey::new_unique())
            .mint(bank_mint_pubkey)
            .token_owner(payer.pubkey())
            .amount(0)
            .create()
            .unwrap();
        fixture.fee_withdraw_dst_token_account = fee_withdraw_dst_token_account_pubkey;

        let withdraw_emissions_account_keypair = Keypair::new();
        let withdraw_emissions_account_pubkey = withdraw_emissions_account_keypair.pubkey();
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::MarginfiAccountInitialize {})
                .accounts(accounts::MarginfiAccountInitialize {
                    marginfi_group: fixture.marginfi_group,
                    marginfi_account: withdraw_emissions_account_pubkey,
                    authority: fixture.payer.pubkey(),
                    fee_payer: fixture.payer.pubkey(),
                })
                .signers(&[&*fixture.payer, &withdraw_emissions_account_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: marginfi_account_initialize prerequisite for lending_account_withdraw_emissions failed"
        );
        fixture.withdraw_emissions_marginfi_account = withdraw_emissions_account_pubkey;
        fixture.scout_register_subject_account(withdraw_emissions_account_pubkey);

        let withdraw_emissions_bank_keypair = Keypair::new();
        let withdraw_emissions_bank_pubkey = withdraw_emissions_bank_keypair.pubkey();
        let (withdraw_emissions_liquidity_vault_authority, withdraw_emissions_liquidity_vault, withdraw_emissions_insurance_vault_authority, withdraw_emissions_insurance_vault, withdraw_emissions_fee_vault_authority, withdraw_emissions_fee_vault) = scout_bank_vault_pdas(fixture.program_id, withdraw_emissions_bank_pubkey);
        let mut withdraw_emissions_bank_config = scout_valid_bank_config(10);
        withdraw_emissions_bank_config.deposit_limit = u64::MAX;
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: withdraw_emissions_bank_config })
                .accounts(scout_lending_pool_add_bank_accounts(fixture.marginfi_group, fixture.payer.pubkey(), fixture.payer.pubkey(), fee_state_pda, fixture.global_fee_wallet, fixture.bank_mint, withdraw_emissions_bank_pubkey, withdraw_emissions_liquidity_vault_authority, withdraw_emissions_liquidity_vault, withdraw_emissions_insurance_vault_authority, withdraw_emissions_insurance_vault, withdraw_emissions_fee_vault_authority, withdraw_emissions_fee_vault, spl_token::id()))
                .signers(&[&*fixture.payer, &withdraw_emissions_bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank prerequisite for lending_account_withdraw_emissions failed"
        );
        fixture.withdraw_emissions_bank = withdraw_emissions_bank_pubkey;

        let withdraw_emissions_auth = Pubkey::find_program_address(
            &[
                EMISSIONS_AUTH_SEED,
                withdraw_emissions_bank_pubkey.as_ref(),
                fixture.emissions_mint.as_ref(),
            ],
            &fixture.program_id,
        )
        .0;
        let withdraw_emissions_vault = Pubkey::find_program_address(
            &[
                EMISSIONS_TOKEN_ACCOUNT_SEED,
                withdraw_emissions_bank_pubkey.as_ref(),
                fixture.emissions_mint.as_ref(),
            ],
            &fixture.program_id,
        )
        .0;
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingAccountDeposit {
                    amount: SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT,
                    deposit_up_to_limit: None,
                })
                .accounts(accounts::LendingAccountDeposit {
                    group: fixture.marginfi_group,
                    marginfi_account: fixture.withdraw_emissions_marginfi_account,
                    authority: fixture.payer.pubkey(),
                    bank: fixture.withdraw_emissions_bank,
                    signer_token_account: fixture.signer_token_account,
                    liquidity_vault: withdraw_emissions_liquidity_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_account_deposit seed balance for lending_account_withdraw_emissions failed"
        );

        let withdraw_emissions_destination_account = fixture.ctx
            .create_token_account()
            .pubkey(Pubkey::new_unique())
            .mint(fixture.emissions_mint)
            .token_owner(fixture.payer.pubkey())
            .amount(0)
            .create()
            .unwrap();
        fixture.withdraw_emissions_destination_account = withdraw_emissions_destination_account;
        fixture
            .ctx
            .update_account(&fixture.withdraw_emissions_marginfi_account, |data| {
                data[SCOUT_BALANCE_EMISSIONS_OUTSTANDING_OFFSET
                    ..SCOUT_BALANCE_EMISSIONS_OUTSTANDING_OFFSET + 16]
                    .copy_from_slice(
                        &fixed::types::I80F48::from_num(SCOUT_WITHDRAW_EMISSIONS_OUTSTANDING)
                            .to_le_bytes(),
                    );
            })
            .unwrap();

        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolUpdateFeesDestinationAccount {})
                .accounts(accounts::LendingPoolUpdateFeesDestinationAccount {
                    group: fixture.marginfi_group,
                    bank: fixture.bank,
                    admin: fixture.payer.pubkey(),
                    destination_account: fixture.fee_withdraw_dst_token_account,
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_update_fees_destination_account prerequisite for \
             lending_pool_withdraw_fees_permissionless failed"
        );

        fixture
            .ctx
            .create_token_account()
            .pubkey(scout_associated_token_address(
                &fixture.payer.pubkey(),
                &fixture.emissions_mint,
                &spl_token::id(),
            ))
            .mint(fixture.emissions_mint)
            .token_owner(fixture.payer.pubkey())
            .amount(0)
            .create()
            .unwrap();
        fixture.withdraw_emissions_destination_account = scout_associated_token_address(
            &fixture.payer.pubkey(),
            &fixture.emissions_mint,
            &spl_token::id(),
        );
        fixture
            .ctx
            .update_account(&fixture.withdraw_emissions_marginfi_account, |data| {
                let start = MARGINFI_ACCOUNT_FLAGS_OFFSET + 8;
                data[start..start + 32].copy_from_slice(fixture.payer.pubkey().as_ref());
            })
            .unwrap();

        if std::path::Path::new(scout_kamino_lending_nocpi_artifact()).exists() {
            TestContext::add_program(
                &mut fixture.ctx,
                &scout_kamino_program_id(),
                scout_kamino_lending_nocpi_artifact(),
            )
            .unwrap();
        }
        if std::path::Path::new(scout_kamino_farms_artifact()).exists() {
            TestContext::add_program(
                &mut fixture.ctx,
                &scout_kamino_farms_program_id(),
                scout_kamino_farms_artifact(),
            )
            .unwrap();
        }

        if std::path::Path::new(scout_solend_mocks_artifact()).exists() {
            TestContext::add_program(
                &mut fixture.ctx,
                &scout_solend_program_id(),
                scout_solend_mocks_artifact(),
            )
            .unwrap();
        }

        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolForceTokenlessRepayComplete {})
                .accounts(accounts::LendingPoolForceTokenlessRepayComplete {
                    group: fixture.marginfi_group,
                    risk_admin: fixture.payer.pubkey(),
                    bank: fixture.tokenless_bank,
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: force tokenless complete prerequisite for purge_deleverage_balance failed"
        );
        fixture
            .ctx
            .update_account(&fixture.marginfi_account, |data| {
                data[SCOUT_FIRST_BALANCE_ACTIVE_OFFSET] = 1;
                data[SCOUT_FIRST_BALANCE_BANK_PK_OFFSET
                    ..SCOUT_FIRST_BALANCE_BANK_PK_OFFSET + 32]
                    .copy_from_slice(fixture.tokenless_bank.as_ref());
                data[SCOUT_FIRST_BALANCE_BANK_ASSET_TAG_OFFSET] = 0;
                data[SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET
                    ..SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET + 16]
                    .copy_from_slice(&fixed::types::I80F48::ZERO.to_le_bytes());
                data[SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET
                    ..SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET + 16]
                    .copy_from_slice(&fixed::types::I80F48::ZERO.to_le_bytes());
            })
            .unwrap();
        &fixture;

        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolForceTokenlessRepayComplete {})
                .accounts(accounts::LendingPoolForceTokenlessRepayComplete {
                    group: fixture.marginfi_group,
                    risk_admin: fixture.payer.pubkey(),
                    bank: fixture.tokenless_bank,
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: force tokenless complete prerequisite for purge_deleverage_balance failed"
        );
        fixture
            .ctx
            .update_account(&fixture.marginfi_account, |data| {
                data[SCOUT_FIRST_BALANCE_ACTIVE_OFFSET] = 1;
                data[SCOUT_FIRST_BALANCE_BANK_PK_OFFSET
                    ..SCOUT_FIRST_BALANCE_BANK_PK_OFFSET + 32]
                    .copy_from_slice(fixture.tokenless_bank.as_ref());
                data[SCOUT_FIRST_BALANCE_BANK_ASSET_TAG_OFFSET] = 0;
                data[SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET
                    ..SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET + 16]
                    .copy_from_slice(&fixed::types::I80F48::ZERO.to_le_bytes());
                data[SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET
                    ..SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET + 16]
                    .copy_from_slice(&fixed::types::I80F48::ZERO.to_le_bytes());
            })
            .unwrap();
        &fixture;

        let fee_bank_keypair = Keypair::new();
        let fee_bank_pubkey = fee_bank_keypair.pubkey();
        let (fee_bank_liquidity_vault_authority, fee_bank_liquidity_vault, fee_bank_insurance_vault_authority, fee_bank_insurance_vault, fee_bank_fee_vault_authority, fee_bank_fee_vault) = scout_bank_vault_pdas(fixture.program_id, fee_bank_pubkey);
        let mut fee_bank_config = scout_valid_bank_config(10);
        fee_bank_config.deposit_limit = u64::MAX;
        fee_bank_config.borrow_limit = u64::MAX;
        fee_bank_config.interest_rate_config.protocol_origination_fee =
            marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ONE);
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: fee_bank_config })
                .accounts(scout_lending_pool_add_bank_accounts(fixture.marginfi_group, fixture.payer.pubkey(), fixture.payer.pubkey(), fee_state_pda, fixture.global_fee_wallet, fixture.bank_mint, fee_bank_pubkey, fee_bank_liquidity_vault_authority, fee_bank_liquidity_vault, fee_bank_insurance_vault_authority, fee_bank_insurance_vault, fee_bank_fee_vault_authority, fee_bank_fee_vault, spl_token::id()))
                .signers(&[&*fixture.payer, &fee_bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank for the dedicated fee_bank (origination-fee glue) failed"
        );
        fixture.fee_bank = fee_bank_pubkey;

        fixture.ctx
            .update_account(&fee_bank_liquidity_vault, |data| {
                const SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET: usize = 64;
                data[SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET..SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET + 8]
                    .copy_from_slice(&100_000_000u64.to_le_bytes());
            })
            .unwrap();

        fixture.ctx
            .update_account(&fee_bank_pubkey, |data| {
                data[SCOUT_BANK_TOTAL_ASSET_SHARES_OFFSET..SCOUT_BANK_TOTAL_ASSET_SHARES_OFFSET + 16]
                    .copy_from_slice(&fixed::types::I80F48::from_num(10_000_000).to_le_bytes());
            })
            .unwrap();

        let fee_borrow_marginfi_account_keypair = Keypair::new();
        let fee_borrow_marginfi_account_pubkey = fee_borrow_marginfi_account_keypair.pubkey();
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::MarginfiAccountInitialize {})
                .accounts(accounts::MarginfiAccountInitialize {
                    marginfi_group: fixture.marginfi_group,
                    marginfi_account: fee_borrow_marginfi_account_pubkey,
                    authority: fixture.payer.pubkey(),
                    fee_payer: fixture.payer.pubkey(),
                })
                .signers(&[&*fixture.payer, &fee_borrow_marginfi_account_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: marginfi_account_initialize for fee_borrow_marginfi_account failed"
        );
        fixture.scout_register_subject_account(fee_borrow_marginfi_account_pubkey);
        fixture.ctx
            .update_account(&fee_borrow_marginfi_account_pubkey, |data| {
                let start = MARGINFI_ACCOUNT_FLAGS_OFFSET;
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data[start..start + 8]);
                let flags = u64::from_le_bytes(bytes) | SCOUT_ACCOUNT_IN_FLASHLOAN;
                data[start..start + 8].copy_from_slice(&flags.to_le_bytes());
            })
            .unwrap();
        fixture.fee_borrow_marginfi_account = fee_borrow_marginfi_account_pubkey;

        let metadata_bank_keypair = Keypair::new();
        let metadata_bank_pubkey = metadata_bank_keypair.pubkey();
        let (metadata_bank_liquidity_vault_authority, metadata_bank_liquidity_vault, metadata_bank_insurance_vault_authority, metadata_bank_insurance_vault, metadata_bank_fee_vault_authority, metadata_bank_fee_vault) = scout_bank_vault_pdas(fixture.program_id, metadata_bank_pubkey);
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: scout_valid_bank_config(10) })
                .accounts(scout_lending_pool_add_bank_accounts(fixture.marginfi_group, fixture.payer.pubkey(), fixture.payer.pubkey(), fee_state_pda, fixture.global_fee_wallet, fixture.bank_mint, metadata_bank_pubkey, metadata_bank_liquidity_vault_authority, metadata_bank_liquidity_vault, metadata_bank_insurance_vault_authority, metadata_bank_insurance_vault, metadata_bank_fee_vault_authority, metadata_bank_fee_vault, spl_token::id()))
                .signers(&[&*fixture.payer, &metadata_bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank (dedicated metadata_bank instance) prerequisite for \
             write_bank_metadata failed"
        );
        fixture.metadata_bank = metadata_bank_pubkey;
        let metadata_bank_metadata_pda = Pubkey::find_program_address(
            &[METADATA_SEED, metadata_bank_pubkey.as_ref()],
            &fixture.program_id,
        ).0;
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::InitBankMetadata {})
                .accounts(accounts::InitBankMetadata {
                    bank: metadata_bank_pubkey,
                    fee_payer: fixture.payer.pubkey(),
                    metadata: metadata_bank_metadata_pda,
                })
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: init_bank_metadata (direct, real instruction call) prerequisite for \
             write_bank_metadata failed"
        );

        let kamino_deposit_lending_market = scout_kamino_lending_market(fixture.program_id);
        let kamino_deposit_reserve_pubkey = scout_kamino_reserve(fixture.program_id);
        let kamino_deposit_reserve_bytes = scout_kamino_reserve_bytes(
            kamino_deposit_lending_market,
            fixture.bank_mint,
        );
        fixture.ctx.create_account()
            .pubkey(kamino_deposit_reserve_pubkey)
            .owner(scout_kamino_program_id())
            .data(&kamino_deposit_reserve_bytes)
            .create()
            .unwrap();

        let kamino_withdraw_lending_market = scout_kamino_lending_market(fixture.program_id);
        let kamino_withdraw_reserve_pubkey = Pubkey::new_unique();
        let kamino_withdraw_reserve_bytes = scout_kamino_reserve_bytes(
            kamino_withdraw_lending_market,
            fixture.bank_mint,
        );
        fixture.ctx.create_account()
            .pubkey(kamino_withdraw_reserve_pubkey)
            .owner(scout_kamino_program_id())
            .data(&kamino_withdraw_reserve_bytes)
            .create()
            .unwrap();
        fixture.ctx
            .update_account(&kamino_withdraw_reserve_pubkey, |data| {
                scout_write_u64(
                    data,
                    SCOUT_KAMINO_RESERVE_MINT_TOTAL_SUPPLY_OFFSET,
                    1_000_000,
                );
            })
            .unwrap();
        let kamino_withdraw_bank_pda = scout_seeded_bank_pda(
            fixture.program_id,
            fixture.marginfi_group,
            fixture.bank_mint,
            SCOUT_KAMINO_WITHDRAW_BANK_SEED,
        );
        let (
            kamino_withdraw_liquidity_vault_authority,
            kamino_withdraw_liquidity_vault,
            kamino_withdraw_insurance_vault_authority,
            kamino_withdraw_insurance_vault,
            kamino_withdraw_fee_vault_authority,
            kamino_withdraw_fee_vault,
        ) = scout_bank_vault_pdas(fixture.program_id, kamino_withdraw_bank_pda);
        let kamino_withdraw_obligation_pubkey = scout_kamino_obligation_pda(
            kamino_withdraw_liquidity_vault_authority,
            kamino_withdraw_lending_market,
        );
        let kamino_withdraw_oracle_pubkey = Pubkey::new_unique();
        let kamino_withdraw_oracle_bytes =
            scout_price_update_v2_bytes([17u8; 32], 100_000_000, 1);
        fixture.ctx.create_account()
            .pubkey(kamino_withdraw_oracle_pubkey)
            .owner(pyth_receiver_program_id())
            .data(&kamino_withdraw_oracle_bytes)
            .create()
            .unwrap();
        let mut kamino_withdraw_bank_config = scout_valid_kamino_config(kamino_withdraw_oracle_pubkey);
        kamino_withdraw_bank_config.deposit_limit = u64::MAX;
        assert!(
            fixture.ctx
                .program(fixture.program_id)
                .call(instruction::LendingPoolAddBankKamino {
                    bank_config: kamino_withdraw_bank_config,
                    bank_seed: SCOUT_KAMINO_WITHDRAW_BANK_SEED,
                })
                .accounts(accounts::LendingPoolAddBankKamino {
                    group: fixture.marginfi_group,
                    admin: fixture.payer.pubkey(),
                    fee_payer: fixture.payer.pubkey(),
                    bank_mint: fixture.bank_mint,
                    bank: kamino_withdraw_bank_pda,
                    integration_acc_1: kamino_withdraw_reserve_pubkey,
                    integration_acc_2: kamino_withdraw_obligation_pubkey,
                    liquidity_vault_authority: kamino_withdraw_liquidity_vault_authority,
                    liquidity_vault: kamino_withdraw_liquidity_vault,
                    insurance_vault_authority: kamino_withdraw_insurance_vault_authority,
                    insurance_vault: kamino_withdraw_insurance_vault,
                    fee_vault_authority: kamino_withdraw_fee_vault_authority,
                    fee_vault: kamino_withdraw_fee_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(vec![kamino_withdraw_oracle_pubkey, kamino_withdraw_reserve_pubkey])
                .signers(&[&*fixture.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "setup: lending_pool_add_bank_kamino (dedicated kamino_withdraw fixture) failed"
        );
        let kamino_withdraw_obligation_bytes = scout_kamino_obligation_bytes(
            kamino_withdraw_lending_market,
            kamino_withdraw_liquidity_vault_authority,
            kamino_withdraw_reserve_pubkey,
            0,
        );
        fixture.ctx.create_account()
            .pubkey(kamino_withdraw_obligation_pubkey)
            .owner(scout_kamino_program_id())
            .data(&kamino_withdraw_obligation_bytes)
            .create()
            .unwrap();
        fixture.kamino_withdraw_bank = kamino_withdraw_bank_pda;
        fixture.kamino_withdraw_reserve = kamino_withdraw_reserve_pubkey;
        let kamino_withdraw_marginfi_account_pubkey = fixture
            .scout_create_initialized_marginfi_account()
            .expect("setup: marginfi_account_initialize must succeed on a fresh fixture");
        fixture.kamino_withdraw_marginfi_account = kamino_withdraw_marginfi_account_pubkey;
        fixture.ctx
            .update_account(&kamino_withdraw_marginfi_account_pubkey, |data| {
                data[SCOUT_FIRST_BALANCE_ACTIVE_OFFSET] = 1;
                data[SCOUT_FIRST_BALANCE_BANK_PK_OFFSET..SCOUT_FIRST_BALANCE_BANK_PK_OFFSET + 32]
                    .copy_from_slice(kamino_withdraw_bank_pda.as_ref());
                data[SCOUT_FIRST_BALANCE_BANK_ASSET_TAG_OFFSET] = SCOUT_ASSET_TAG_KAMINO_SHARED;
                let shares = fixed::types::I80F48::from_num(SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT);
                data[SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET
                    ..SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET + 16]
                    .copy_from_slice(&shares.to_le_bytes());
            })
            .unwrap();

        {
            let fee_state_pda =
                Pubkey::find_program_address(&[FEE_STATE_SEED], &fixture.program_id).0;
            let expired_pause_start: i64 = -(SCOUT_PANIC_PAUSE_EXPIRY_SECONDS);
            fixture
                .ctx
                .update_account(&fee_state_pda, |data| {
                    data[SCOUT_FEE_STATE_PANIC_PAUSE_FLAGS_OFFSET] |= SCOUT_PANIC_STATE_FLAG_PAUSED;
                    data[SCOUT_FEE_STATE_PANIC_PAUSE_START_TIMESTAMP_OFFSET
                        ..SCOUT_FEE_STATE_PANIC_PAUSE_START_TIMESTAMP_OFFSET + 8]
                        .copy_from_slice(&expired_pause_start.to_le_bytes());
                })
                .unwrap();
        }

        fixture.scout_setup_dedicated_pulse_health_accounts();

        let _ = fixture.scout_setup_borrow_scenario();

        // SCOUT:PROPERTY-HOOK:P-0013
        fixture.scout_p13_seed_close_enabled_banks();
        fixture.scout_p11_seed_authority_baseline();
        // SCOUT:PROPERTY-HOOK:P-0030
        let _ = fixture.scout_p30_record_fee_baselines();
        // SCOUT:PROPERTY-HOOK:P-0014
        let _ = fixture.scout_sv_record_share_baselines();

        fixture
        // SCOUT:SETUP-GLUE:END
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_marginfi_group_initialize(&mut self) -> bool {
        let __scout_signer_marginfi_group = Keypair::new();
        let marginfi_group = __scout_signer_marginfi_group.pubkey();
        let admin = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiGroupInitialize {  })
            .accounts(accounts::MarginfiGroupInitialize {
                marginfi_group: marginfi_group,
                admin: admin,
                fee_state: fee_state,
            })
            .signers(&[&*self.payer, &__scout_signer_marginfi_group])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:marginfi_group_initialize:BEGIN
            // SCOUT:ACTION-HOOK:marginfi_group_initialize:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_marginfi_group_configure(&mut self) -> bool {
        // TODO: arg emode_max_init_leverage: Option<marginfi::types::WrappedI80F48>; arg emode_max_maint_leverage: Option<marginfi::types::WrappedI80F48>
        let new_admin: Pubkey = self.payer.pubkey();
        let new_emode_admin: Pubkey = self.payer.pubkey();
        let new_curve_admin: Pubkey = self.payer.pubkey();
        let new_limit_admin: Pubkey = self.payer.pubkey();
        let new_emissions_admin: Pubkey = self.payer.pubkey();
        let new_metadata_admin: Pubkey = self.payer.pubkey();
        let new_risk_admin: Pubkey = self.payer.pubkey();
        let emode_max_init_leverage: Option<marginfi::types::WrappedI80F48> = Default::default(); // TODO: construct arg emode_max_init_leverage: Option<marginfi::types::WrappedI80F48>
        let emode_max_maint_leverage: Option<marginfi::types::WrappedI80F48> = Default::default(); // TODO: construct arg emode_max_maint_leverage: Option<marginfi::types::WrappedI80F48>
        let marginfi_group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiGroupConfigure { new_admin: Some(new_admin), new_emode_admin: Some(new_emode_admin), new_curve_admin: Some(new_curve_admin), new_limit_admin: Some(new_limit_admin), new_emissions_admin: Some(new_emissions_admin), new_metadata_admin: Some(new_metadata_admin), new_risk_admin: Some(new_risk_admin), new_flow_admin: None, emode_max_init_leverage, emode_max_maint_leverage, same_asset_emode_init_leverage: None, same_asset_emode_maint_leverage: None })
            .accounts(accounts::MarginfiGroupConfigure {
                marginfi_group: marginfi_group,
                admin: admin,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:marginfi_group_configure:BEGIN
            // SCOUT:ACTION-HOOK:marginfi_group_configure:END
        }
        __scout_success
    }

    pub fn action_lending_pool_add_bank(&mut self) -> bool {
        // TODO: 1 extra signer(s): ['fee_payer']
        let bank_config: marginfi::types::BankConfigCompact = scout_valid_bank_config(10);
        let marginfi_group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let global_fee_wallet = self.global_fee_wallet;
        let bank_mint = self.bank_mint;
        let __scout_signer_bank = Keypair::new();
        let bank = __scout_signer_bank.pubkey();
        let (liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault) = scout_bank_vault_pdas(self.program_id, bank);
        let token_program = spl_token::id();
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBank { bank_config })
            .accounts(scout_lending_pool_add_bank_accounts(marginfi_group, admin, fee_payer, fee_state, global_fee_wallet, bank_mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, token_program))
            .signers(&[&*self.payer, &__scout_signer_bank])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_add_bank:BEGIN
            scout_run_property!("P-0006", {
                self.scout_p06_banks[self.scout_p06_bank_next % SCOUT_P06_BANK_CAP] = bank;
                self.scout_p06_bank_next = self.scout_p06_bank_next.saturating_add(1);
            });
            // SCOUT:ACTION-HOOK:lending_pool_add_bank:END
        }
        __scout_success
    }

    pub fn action_lending_pool_add_bank_with_seed(&mut self, bank_seed: u64) -> bool {
        // TODO: 1 extra signer(s): ['fee_payer']
        let bank_config: marginfi::types::BankConfigCompact = scout_valid_bank_config(10);
        let marginfi_group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let global_fee_wallet = self.global_fee_wallet;
        let bank_mint = self.bank_mint;
        let bank = Pubkey::find_program_address(&[marginfi_group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0;
        let (liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault) = scout_bank_vault_pdas(self.program_id, bank);
        let token_program = spl_token::id();
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBankWithSeed { bank_config, bank_seed })
            .accounts(accounts::LendingPoolAddBankWithSeed {
                marginfi_group: marginfi_group,
                admin: admin,
                fee_payer: fee_payer,
                fee_state: fee_state,
                global_fee_wallet: global_fee_wallet,
                bank_mint: bank_mint,
                bank: bank,
                liquidity_vault_authority: liquidity_vault_authority,
                liquidity_vault: liquidity_vault,
                insurance_vault_authority: insurance_vault_authority,
                insurance_vault: insurance_vault,
                fee_vault_authority: fee_vault_authority,
                fee_vault: fee_vault,
                token_program: token_program,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_add_bank_with_seed:BEGIN
            scout_run_property!("P-0006", {
                self.scout_p06_banks[self.scout_p06_bank_next % SCOUT_P06_BANK_CAP] = bank;
                self.scout_p06_bank_next = self.scout_p06_bank_next.saturating_add(1);
            });
            // SCOUT:ACTION-HOOK:lending_pool_add_bank_with_seed:END
        }
        __scout_success
    }

    pub fn action_lending_pool_add_bank_permissionless(&mut self, bank_seed: u64) -> bool {
        let marginfi_group = self.staked_group;
        let staked_settings = self.staked_settings;
        let fee_payer = self.payer.pubkey();
        let bank_mint = match self.scout_prepare_add_bank_permissionless() { Some(v) => v, None => return false };
        let sol_pool = Pubkey::find_program_address(&[b"stake", self.perm_stake_pool.as_ref()], &spl_single_pool_id()).0;
        let perm_pool_onramp = self.scout_placeholder();
        let perm_validator_vote_account = self.scout_placeholder();
        let stake_pool = self.perm_stake_pool;
        let bank = Pubkey::find_program_address(&[marginfi_group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0;
        let (liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault) = scout_bank_vault_pdas(self.program_id, bank);
        let token_program = spl_token::id();
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBankPermissionless { bank_seed })
            .accounts(accounts::LendingPoolAddBankPermissionless {
                marginfi_group: marginfi_group,
                staked_settings: staked_settings,
                fee_payer: fee_payer,
                bank_mint: bank_mint,
                sol_pool: sol_pool,
                pool_onramp: perm_pool_onramp,
                validator_vote_account: perm_validator_vote_account,
                stake_pool: stake_pool,
                bank: bank,
                liquidity_vault_authority: liquidity_vault_authority,
                liquidity_vault: liquidity_vault,
                insurance_vault_authority: insurance_vault_authority,
                insurance_vault: insurance_vault,
                fee_vault_authority: fee_vault_authority,
                fee_vault: fee_vault,
                token_program: token_program,
            })
            .remaining_accounts(vec![self.staked_oracle, bank_mint, sol_pool, perm_pool_onramp])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_add_bank_permissionless:BEGIN
            scout_run_property!("P-0006", {
                self.scout_p06_banks[self.scout_p06_bank_next % SCOUT_P06_BANK_CAP] = bank;
                self.scout_p06_bank_next = self.scout_p06_bank_next.saturating_add(1);
            });
            // SCOUT:ACTION-HOOK:lending_pool_add_bank_permissionless:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_lending_pool_configure_bank(&mut self) -> bool {
        let bank_config_opt: marginfi::types::BankConfigOpt = scout_valid_bank_config_opt();
        let group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let bank = self.bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBank { bank_config_opt })
            .accounts(accounts::LendingPoolConfigureBank {
                group: group,
                admin: admin,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_configure_bank:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_configure_bank:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_lending_pool_configure_bank_interest_only(&mut self) -> bool {
        // TODO: arg interest_rate_config: marginfi::types::InterestRateConfigOpt
        let interest_rate_config: marginfi::types::InterestRateConfigOpt = Default::default(); // TODO: construct arg interest_rate_config: marginfi::types::InterestRateConfigOpt
        let group = self.marginfi_group;
        let delegate_curve_admin = self.payer.pubkey();
        let bank = self.bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankInterestOnly { interest_rate_config })
            .accounts(accounts::LendingPoolConfigureBankInterestOnly {
                group: group,
                delegate_curve_admin: delegate_curve_admin,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_configure_bank_interest_only:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_configure_bank_interest_only:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_lending_pool_configure_bank_limits_only(&mut self) -> bool {
        // TODO: arg deposit_limit: Option<u64>; arg borrow_limit: Option<u64>; arg total_asset_value_init_limit: Option<u64>
        let deposit_limit: Option<u64> = Default::default(); // TODO: construct arg deposit_limit: Option<u64>
        let borrow_limit: Option<u64> = Default::default(); // TODO: construct arg borrow_limit: Option<u64>
        let total_asset_value_init_limit: Option<u64> = Default::default(); // TODO: construct arg total_asset_value_init_limit: Option<u64>
        let group = self.marginfi_group;
        let delegate_limit_admin = self.payer.pubkey();
        let bank = self.bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankLimitsOnly { deposit_limit, borrow_limit, total_asset_value_init_limit })
            .accounts(accounts::LendingPoolConfigureBankLimitsOnly {
                group: group,
                delegate_limit_admin: delegate_limit_admin,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_configure_bank_limits_only:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_configure_bank_limits_only:END
        }
        __scout_success
    }

    pub fn action_lending_pool_force_tokenless_repay_complete(&mut self) -> bool {
        let group = self.marginfi_group;
        let risk_admin = self.payer.pubkey();
        let bank = self.tokenless_bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolForceTokenlessRepayComplete {  })
            .accounts(accounts::LendingPoolForceTokenlessRepayComplete {
                group: group,
                risk_admin: risk_admin,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_force_tokenless_repay_complete:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_force_tokenless_repay_complete:END
        }
        __scout_success
    }

    pub fn action_lending_pool_set_fixed_oracle_price(&mut self) -> bool {
        let price: marginfi::types::WrappedI80F48 = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(1));
        let group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let bank = self.bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolSetFixedOraclePrice { price })
            .accounts(accounts::LendingPoolSetFixedOraclePrice {
                group: group,
                admin: admin,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_set_fixed_oracle_price:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_set_fixed_oracle_price:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_lending_pool_configure_bank_emode(&mut self, emode_tag: u16) -> bool {
        let raw_weight = fixed::types::I80F48::from_num(emode_tag % 90)
            .checked_div(fixed::types::I80F48::from_num(100))
            .unwrap_or(fixed::types::I80F48::ZERO);
        let w = marginfi::types::WrappedI80F48::from_i80f48(raw_weight);
        let mut entries: [marginfi::types::EmodeEntry; MAX_EMODE_ENTRIES] = Default::default();
        entries[0] = marginfi::types::EmodeEntry {
            collateral_bank_emode_tag: if emode_tag == 0 { 1 } else { emode_tag },
            flags: 0,
            pad0: [0u8; 5],
            asset_weight_init: w,
            asset_weight_maint: w,
        };
        let group = self.marginfi_group;
        let emode_admin = self.payer.pubkey();
        let bank = self.bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankEmode { emode_tag, entries })
            .accounts(accounts::LendingPoolConfigureBankEmode {
                group: group,
                emode_admin: emode_admin,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_configure_bank_emode:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_configure_bank_emode:END
        }
        __scout_success
    }

    pub fn action_lending_pool_clone_emode(&mut self) -> bool {
        let group = self.marginfi_group;
        let signer = self.payer.pubkey();
        let copy_from_bank = self.bank;
        let copy_to_bank = self.clone_emode_bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolCloneEmode {  })
            .accounts(accounts::LendingPoolCloneEmode {
                group: group,
                signer: signer,
                copy_from_bank: copy_from_bank,
                copy_to_bank: copy_to_bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_clone_emode:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_clone_emode:END
        }
        __scout_success
    }



    pub fn action_lending_pool_handle_bankruptcy(&mut self) -> bool {
        let group = self.marginfi_group;
        let signer = self.payer.pubkey();
        let bank = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.1).unwrap_or(self.bank);
        let marginfi_account = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.0).unwrap_or(self.marginfi_account);
        let liquidity_vault = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.2).unwrap_or(Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0);
        let insurance_vault = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.3).unwrap_or(Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &self.program_id).0);
        let insurance_vault_authority = self.scout_prepare_lending_pool_handle_bankruptcy_accounts().map(|a| a.4).unwrap_or(Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0);
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolHandleBankruptcy {  })
            .accounts(accounts::LendingPoolHandleBankruptcy {
                group: group,
                signer: signer,
                bank: bank,
                marginfi_account: marginfi_account,
                liquidity_vault: liquidity_vault,
                insurance_vault: insurance_vault,
                insurance_vault_authority: insurance_vault_authority,
                token_program: token_program,
            })
            .remaining_accounts(vec![bank])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_handle_bankruptcy:BEGIN
            scout_run_property!("P-0014", {
                self.scout_sv_socialized_next = self.scout_sv_socialized_next.saturating_add(1);
                self.scout_sv_socialized[self.scout_sv_socialized_next % SCOUT_SV_SOCIALIZED_CAP] =
                    bank;
            });
            // SCOUT:ACTION-HOOK:lending_pool_handle_bankruptcy:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_marginfi_account_initialize(&mut self) -> bool {
        // TODO: 1 extra signer(s): ['fee_payer']
        let marginfi_group = self.marginfi_group;
        let __scout_signer_marginfi_account = Keypair::new();
        let marginfi_account = __scout_signer_marginfi_account.pubkey();
        let authority = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitialize {  })
            .accounts(accounts::MarginfiAccountInitialize {
                marginfi_group: marginfi_group,
                marginfi_account: marginfi_account,
                authority: authority,
                fee_payer: fee_payer,
            })
            .signers(&[&*self.payer, &__scout_signer_marginfi_account])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:marginfi_account_initialize:BEGIN
            scout_run_property!("P-0002", {
                self.scout_known_accounts[self.scout_known_next % SCOUT_KNOWN_CAP] = marginfi_account;
                self.scout_known_next = self.scout_known_next.saturating_add(1);
            });
            scout_run_property!("P-0001", {
                self.scout_p1_accounts[self.scout_p1_accounts_next % SCOUT_SUBJECT_CAP] = marginfi_account;
                self.scout_p1_accounts_next = self.scout_p1_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0017", {
                self.scout_p17_accounts[self.scout_p17_accounts_next % SCOUT_SUBJECT_CAP] = marginfi_account;
                self.scout_p17_accounts_next = self.scout_p17_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0020", {
                self.scout_p20_accounts[self.scout_p20_accounts_next % SCOUT_SUBJECT_CAP] = marginfi_account;
                self.scout_p20_accounts_next = self.scout_p20_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0022", {
                self.scout_p22_accounts[self.scout_p22_accounts_next % SCOUT_SUBJECT_CAP] = marginfi_account;
                self.scout_p22_accounts_next = self.scout_p22_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0039", {
                self.scout_p39_subjects[self.scout_p39_subjects_next % SCOUT_SUBJECT_CAP] = marginfi_account;
                self.scout_p39_subjects_next = self.scout_p39_subjects_next.saturating_add(1);
            });
            // SCOUT:ACTION-HOOK:marginfi_account_initialize:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_marginfi_account_init_liq_record(&mut self) -> bool {
        let marginfi_account = self.marginfi_account;
        let fee_payer = self.payer.pubkey();
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitLiqRecord {  })
            .accounts(accounts::MarginfiAccountInitLiqRecord {
                marginfi_account: marginfi_account,
                fee_payer: fee_payer,
                liquidation_record: liquidation_record,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:marginfi_account_init_liq_record:BEGIN
            scout_run_property!("P-0039", {
                self.scout_p39_subjects[self.scout_p39_subjects_next % SCOUT_SUBJECT_CAP] = liquidation_record;
                self.scout_p39_subjects_next = self.scout_p39_subjects_next.saturating_add(1);
            });
            // SCOUT:ACTION-HOOK:marginfi_account_init_liq_record:END
        }
        __scout_success
    }

    pub fn action_lending_account_deposit(&mut self, amount: u64) -> bool {
        // TODO: arg deposit_up_to_limit: Option<bool>; remaining_accounts: reads ctx.remaining_accounts (src/instructions/marginfi_account/deposit.rs:50)
        let deposit_up_to_limit: Option<bool> = Default::default(); // TODO: construct arg deposit_up_to_limit: Option<bool>
        let group = self.marginfi_group;
        let marginfi_account = self.marginfi_account;
        let authority = self.payer.pubkey();
        let bank = self.bank;
        let signer_token_account = self.signer_token_account;
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit { amount, deposit_up_to_limit })
            .accounts(accounts::LendingAccountDeposit {
                group: group,
                marginfi_account: marginfi_account,
                authority: authority,
                bank: bank,
                signer_token_account: signer_token_account,
                liquidity_vault: liquidity_vault,
                token_program: token_program,
            })
            // TODO: reads ctx.remaining_accounts (deposit.rs:50 +1 more); unbound, so validation fails before the handler runs (no logic, no coverage). Bind `LendingAccountDeposit.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>; prefix `metas:` for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_deposit:BEGIN
            scout_run_property!("P-0004", {
                let __p4_same = self.scout_p4_dep_account == marginfi_account
                    && self.scout_p4_dep_bank == bank;
                let __p4_bank_bytes = match self.ctx.account_data(&bank) {
                    Ok(d) => d,
                    Err(_) => SCOUT_HOOK_NO_BYTES,
                };
                let __p4_asv_now: [u8; 16] = __p4_bank_bytes
                    .get(
                        SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET
                            ..SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET + 16,
                    )
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or_default();
                self.scout_p4_dep_tokens = if __p4_same {
                    self.scout_p4_dep_tokens.saturating_add(amount as u128)
                } else {
                    amount as u128
                };
                self.scout_p4_dep_asv = if __p4_same {
                    self.scout_p4_dep_asv
                } else {
                    __p4_asv_now
                };
                self.scout_p4_dep_account = marginfi_account;
                self.scout_p4_dep_bank = bank;
            });
            scout_run_property!("P-0036", {
                self.scout_p36_sorted_account = marginfi_account;
            });
            // SCOUT:ACTION-HOOK:lending_account_deposit:END
        }
        __scout_success
    }

    pub fn action_lending_account_repay(&mut self, amount: u64) -> bool {
        // TODO: remaining_accounts: reads ctx.remaining_accounts (src/instructions/marginfi_account/repay.rs:51)
        let repay_all: Option<bool> = Some(true);
        let group = self.marginfi_group;
        let marginfi_account = self.borrow_marginfi_account;
        let authority = self.payer.pubkey();
        let bank = match self.scout_borrow_scenario_ensure_liability() { Some(v) => v, None => return false };
        let signer_token_account = self.signer_token_account;
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountRepay { amount, repay_all })
            .accounts(accounts::LendingAccountRepay {
                group: group,
                marginfi_account: marginfi_account,
                authority: authority,
                bank: bank,
                signer_token_account: signer_token_account,
                liquidity_vault: liquidity_vault,
                token_program: token_program,
            })
            // TODO: reads ctx.remaining_accounts (repay.rs:51 +1 more); unbound, so validation fails before the handler runs (no logic, no coverage). Bind `LendingAccountRepay.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>; prefix `metas:` for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_repay:BEGIN
            scout_run_property!("P-0004", {
                let __p4_same = self.scout_p4_bor_account == marginfi_account
                    && self.scout_p4_bor_bank == bank;
                let __p4_acct_bytes = match self.ctx.account_data(&marginfi_account) {
                    Ok(d) => d,
                    Err(_) => SCOUT_HOOK_NO_BYTES,
                };
                let __p4_slots_now: [u8; SCOUT_P4_BALANCE_REGION_LEN] = __p4_acct_bytes
                    .get(
                        SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET
                            ..SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + SCOUT_P4_BALANCE_REGION_LEN,
                    )
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or([0u8; SCOUT_P4_BALANCE_REGION_LEN]);
                let __p4_slots_ok = __p4_acct_bytes.len()
                    >= SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + SCOUT_P4_BALANCE_REGION_LEN;
                let __p4_bank_bytes = match self.ctx.account_data(&bank) {
                    Ok(d) => d,
                    Err(_) => SCOUT_HOOK_NO_BYTES,
                };
                let __p4_lsv_now: [u8; 16] = __p4_bank_bytes
                    .get(
                        SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET
                            ..SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16,
                    )
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or_default();
                let __p4_ok = __p4_slots_ok
                    && __p4_bank_bytes.len() >= SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16;
                self.scout_p4_bor_prev_ok = if __p4_same {
                    false
                } else {
                    self.scout_p4_bor_prev_ok
                };
                self.scout_p4_bor_cur_ok = if __p4_same {
                    __p4_ok
                } else {
                    self.scout_p4_bor_cur_ok
                };
                self.scout_p4_bor_cur_slots = if __p4_same {
                    __p4_slots_now
                } else {
                    self.scout_p4_bor_cur_slots
                };
                self.scout_p4_bor_cur_lsv = if __p4_same {
                    __p4_lsv_now
                } else {
                    self.scout_p4_bor_cur_lsv
                };
            });
            scout_run_property!("P-0036", {
                self.scout_p36_sorted_account = marginfi_account;
            });
            // SCOUT:ACTION-HOOK:lending_account_repay:END
        }
        __scout_success
    }

    pub fn action_lending_account_withdraw(&mut self) -> bool {
        // TODO: remaining_accounts: reads ctx.remaining_accounts (src/instructions/marginfi_account/withdraw.rs:75)
        let amount: u64 = SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT;
        let withdraw_all: Option<bool> = Some(true);
        let group = self.marginfi_group;
        let marginfi_account = self.withdraw_marginfi_account;
        let authority = self.payer.pubkey();
        let bank = self.withdraw_bank;
        let destination_token_account = self.signer_token_account;
        let bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountWithdraw { amount, withdraw_all })
            .accounts(accounts::LendingAccountWithdraw {
                group: group,
                marginfi_account: marginfi_account,
                authority: authority,
                bank: bank,
                destination_token_account: destination_token_account,
                bank_liquidity_vault_authority: bank_liquidity_vault_authority,
                liquidity_vault: liquidity_vault,
                token_program: token_program,
            })
            // TODO: reads ctx.remaining_accounts (withdraw.rs:75 +4 more); unbound, so validation fails before the handler runs (no logic, no coverage). Bind `LendingAccountWithdraw.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>; prefix `metas:` for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_withdraw:BEGIN
            scout_run_property!("P-0036", {
                self.scout_p36_sorted_account = marginfi_account;
            });
            // SCOUT:ACTION-HOOK:lending_account_withdraw:END
        }
        __scout_success
    }

    pub fn action_lending_account_borrow(&mut self) -> bool {
        let amount: u64 = SCOUT_BORROW_AMOUNT;
        let group = self.marginfi_group;
        let marginfi_account = self.borrow_marginfi_account;
        let authority = self.payer.pubkey();
        let bank = self.borrow_liab_bank;
        let destination_token_account = self.signer_token_account;
        let bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountBorrow { amount })
            .accounts(accounts::LendingAccountBorrow {
                group: group,
                marginfi_account: marginfi_account,
                authority: authority,
                bank: bank,
                destination_token_account: destination_token_account,
                bank_liquidity_vault_authority: bank_liquidity_vault_authority,
                liquidity_vault: liquidity_vault,
                token_program: token_program,
            })
            .remaining_accounts(self.borrow_remaining_accounts.clone())
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_borrow:BEGIN
            scout_run_property!("P-0004", {
                let __p4_same = self.scout_p4_bor_account == marginfi_account
                    && self.scout_p4_bor_bank == bank;
                let __p4_acct_bytes = match self.ctx.account_data(&marginfi_account) {
                    Ok(d) => d,
                    Err(_) => SCOUT_HOOK_NO_BYTES,
                };
                let __p4_slots_now: [u8; SCOUT_P4_BALANCE_REGION_LEN] = __p4_acct_bytes
                    .get(
                        SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET
                            ..SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + SCOUT_P4_BALANCE_REGION_LEN,
                    )
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or([0u8; SCOUT_P4_BALANCE_REGION_LEN]);
                let __p4_slots_ok = __p4_acct_bytes.len()
                    >= SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + SCOUT_P4_BALANCE_REGION_LEN;
                let __p4_bank_bytes = match self.ctx.account_data(&bank) {
                    Ok(d) => d,
                    Err(_) => SCOUT_HOOK_NO_BYTES,
                };
                let __p4_lsv_now: [u8; 16] = __p4_bank_bytes
                    .get(
                        SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET
                            ..SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16,
                    )
                    .unwrap_or_default()
                    .try_into()
                    .unwrap_or_default();
                let __p4_ok = __p4_slots_ok
                    && __p4_bank_bytes.len() >= SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16;
                self.scout_p4_bor_prev_ok = __p4_same && self.scout_p4_bor_cur_ok;
                self.scout_p4_bor_prev_slots = self.scout_p4_bor_cur_slots;
                self.scout_p4_bor_prev_lsv = self.scout_p4_bor_cur_lsv;
                self.scout_p4_bor_cur_ok = __p4_ok;
                self.scout_p4_bor_cur_slots = __p4_slots_now;
                self.scout_p4_bor_cur_lsv = __p4_lsv_now;
                self.scout_p4_bor_amount = amount as u128;
                self.scout_p4_bor_account = marginfi_account;
                self.scout_p4_bor_bank = bank;
            });
            scout_run_property!("P-0036", {
                self.scout_p36_sorted_account = marginfi_account;
            });
            // SCOUT:ACTION-HOOK:lending_account_borrow:END
        }
        __scout_success
    }

    pub fn action_lending_account_close_balance(&mut self) -> bool {
        let group = self.marginfi_group;
        let marginfi_account = match self.scout_prepare_lending_account_close_balance_marginfi_account() { Some(v) => v, None => return false };
        let authority = self.payer.pubkey();
        let bank = self.bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountCloseBalance {  })
            .accounts(accounts::LendingAccountCloseBalance {
                group: group,
                marginfi_account: marginfi_account,
                authority: authority,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_close_balance:BEGIN
            scout_run_property!("P-0036", {
                self.scout_p36_sorted_account = marginfi_account;
            });
            // SCOUT:ACTION-HOOK:lending_account_close_balance:END
        }
        __scout_success
    }



    pub fn action_lending_account_liquidate(&mut self, asset_amount: u64, liquidatee_accounts: u8, liquidator_accounts: u8) -> bool {
        let asset_amount: u64 = { if !self.scout_liquidate_scenario_refresh() { return false; } asset_amount % SCOUT_LIQUIDATE_MAX_ASSET_AMOUNT + 1 };
        let liquidatee_accounts: u8 = 2;
        let liquidator_accounts: u8 = 2;
        let group = self.marginfi_group;
        let asset_bank = self.scout_liq_asset_bank;
        let liab_bank = self.scout_liq_liab_bank;
        let liquidator_marginfi_account = self.scout_liq_liquidator;
        let authority = self.payer.pubkey();
        let liquidatee_marginfi_account = self.scout_liq_liquidatee;
        let bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, liab_bank.as_ref()], &self.program_id).0;
        let bank_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let bank_insurance_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountLiquidate { asset_amount, liquidatee_accounts, liquidator_accounts })
            .accounts(accounts::LendingAccountLiquidate {
                group: group,
                asset_bank: asset_bank,
                liab_bank: liab_bank,
                liquidator_marginfi_account: liquidator_marginfi_account,
                authority: authority,
                liquidatee_marginfi_account: liquidatee_marginfi_account,
                bank_liquidity_vault_authority: bank_liquidity_vault_authority,
                bank_liquidity_vault: bank_liquidity_vault,
                bank_insurance_vault: bank_insurance_vault,
                token_program: token_program,
            })
            .remaining_accounts(self.scout_liq_remaining.clone())
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_liquidate:BEGIN
            // SCOUT:ACTION-HOOK:lending_account_liquidate:END
        }
        __scout_success
    }

    pub fn action_lending_account_start_flashloan(&mut self, end_index: u64) -> bool {
        let marginfi_account = self.marginfi_account;
        let authority = self.payer.pubkey();
        let ixs_sysvar = self.scout_placeholder(); // TODO: real account for ixs_sysvar (unchecked)
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountStartFlashloan { end_index })
            .accounts(accounts::LendingAccountStartFlashloan {
                marginfi_account: marginfi_account,
                authority: authority,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_start_flashloan:BEGIN
            // SCOUT:ACTION-HOOK:lending_account_start_flashloan:END
        }
        __scout_success
    }

    pub fn action_lending_account_end_flashloan(&mut self) -> bool {
        let marginfi_account = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let authority = self.payer.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountEndFlashloan {  })
            .accounts(accounts::LendingAccountEndFlashloan {
                marginfi_account: marginfi_account,
                group: self.marginfi_group,
                authority: authority,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_end_flashloan:BEGIN
            // SCOUT:ACTION-HOOK:lending_account_end_flashloan:END
        }
        __scout_success
    }

    pub fn action_marginfi_account_update_emissions_destination_account(&mut self) -> bool {
        let marginfi_account = self.marginfi_account;
        let authority = self.payer.pubkey();
        let destination_account = self.global_fee_wallet;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountUpdateEmissionsDestinationAccount {  })
            .accounts(accounts::MarginfiAccountUpdateEmissionsDestinationAccount {
                marginfi_account: marginfi_account,
                authority: authority,
                destination_account: destination_account,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:marginfi_account_update_emissions_destination_account:BEGIN
            // SCOUT:ACTION-HOOK:marginfi_account_update_emissions_destination_account:END
        }
        __scout_success
    }

    pub fn action_lending_pool_accrue_bank_interest(&mut self) -> bool {
        let group = self.marginfi_group;
        let bank = self.bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAccrueBankInterest {  })
            .accounts(accounts::LendingPoolAccrueBankInterest {
                group: group,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_accrue_bank_interest:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_accrue_bank_interest:END
        }
        __scout_success
    }

    pub fn action_lending_pool_collect_bank_fees(&mut self) -> bool {
        let group = self.marginfi_group;
        let bank = self.bank;
        let liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let insurance_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let fee_vault = Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let fee_ata = self.scout_prepare_collect_bank_fees();
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolCollectBankFees {  })
            .accounts(accounts::LendingPoolCollectBankFees {
                group: group,
                bank: bank,
                liquidity_vault_authority: liquidity_vault_authority,
                liquidity_vault: liquidity_vault,
                insurance_vault: insurance_vault,
                fee_vault: fee_vault,
                fee_state: fee_state,
                fee_ata: fee_ata,
                token_program: token_program,
            })
            // TODO: LendingPoolCollectBankFees reads ctx.remaining_accounts (collect_bank_fees.rs:51 +3 more); unbound -> fails
            // account validation before the handler runs, covers no lines. Bind `LendingPoolCollectBankFees.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>, appended read-only; `metas:` prefix for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_collect_bank_fees:BEGIN
            scout_run_property!("P-0030", {
                self.scout_p30_collect_seq = self.scout_p30_collect_seq.saturating_add(1);
            });
            // SCOUT:ACTION-HOOK:lending_pool_collect_bank_fees:END
        }
        __scout_success
    }

    pub fn action_lending_pool_withdraw_fees(&mut self) -> bool {
        let amount: u64 = 0;
        let group = self.marginfi_group;
        let bank = self.bank;
        let admin = self.payer.pubkey();
        let fee_vault = Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let fee_vault_authority = Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let dst_token_account = self.fee_withdraw_dst_token_account;
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolWithdrawFees { amount })
            .accounts(accounts::LendingPoolWithdrawFees {
                group: group,
                bank: bank,
                admin: admin,
                fee_vault: fee_vault,
                fee_vault_authority: fee_vault_authority,
                dst_token_account: dst_token_account,
                token_program: token_program,
            })
            // TODO: LendingPoolWithdrawFees reads ctx.remaining_accounts (collect_bank_fees.rs:267 +1 more); unbound -> fails
            // account validation before the handler runs, covers no lines. Bind `LendingPoolWithdrawFees.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>, appended read-only; `metas:` prefix for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_withdraw_fees:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_withdraw_fees:END
        }
        __scout_success
    }

    pub fn action_lending_pool_withdraw_fees_permissionless(&mut self, amount: u64) -> bool {
        let group = self.marginfi_group;
        let bank = self.bank;
        let fee_vault = Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let fee_vault_authority = Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let fees_destination_account = self.fee_withdraw_dst_token_account;
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolWithdrawFeesPermissionless { amount })
            .accounts(accounts::LendingPoolWithdrawFeesPermissionless {
                group: group,
                bank: bank,
                fee_vault: fee_vault,
                fee_vault_authority: fee_vault_authority,
                fees_destination_account: fees_destination_account,
                token_program: token_program,
            })
            // TODO: LendingPoolWithdrawFeesPermissionless reads ctx.remaining_accounts (collect_bank_fees.rs:471 +1 more); unbound -> fails
            // account validation before the handler runs, covers no lines. Bind `LendingPoolWithdrawFeesPermissionless.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>, appended read-only; `metas:` prefix for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_withdraw_fees_permissionless:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_withdraw_fees_permissionless:END
        }
        __scout_success
    }

    pub fn action_lending_pool_update_fees_destination_account(&mut self) -> bool {
        let group = self.marginfi_group;
        let bank = self.bank;
        let admin = self.payer.pubkey();
        let destination_account = self.fee_withdraw_dst_token_account;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolUpdateFeesDestinationAccount {  })
            .accounts(accounts::LendingPoolUpdateFeesDestinationAccount {
                group: group,
                bank: bank,
                admin: admin,
                destination_account: destination_account,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_update_fees_destination_account:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_update_fees_destination_account:END
        }
        __scout_success
    }

    pub fn action_lending_pool_withdraw_insurance(&mut self) -> bool {
        let amount: u64 = 0;
        let group = self.marginfi_group;
        let bank = self.bank;
        let admin = self.payer.pubkey();
        let insurance_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let insurance_vault_authority = Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let dst_token_account = self.fee_withdraw_dst_token_account;
        let token_program = spl_token::id();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolWithdrawInsurance { amount })
            .accounts(accounts::LendingPoolWithdrawInsurance {
                group: group,
                bank: bank,
                admin: admin,
                insurance_vault: insurance_vault,
                insurance_vault_authority: insurance_vault_authority,
                dst_token_account: dst_token_account,
                token_program: token_program,
            })
            // TODO: LendingPoolWithdrawInsurance reads ctx.remaining_accounts (collect_bank_fees.rs:346 +1 more); unbound -> fails
            // account validation before the handler runs, covers no lines. Bind `LendingPoolWithdrawInsurance.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>, appended read-only; `metas:` prefix for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_withdraw_insurance:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_withdraw_insurance:END
        }
        __scout_success
    }

    pub fn action_lending_pool_close_bank(&mut self) -> bool {
        let group = self.marginfi_group;
        let bank = match self.scout_mint_lending_pool_close_bank_guard_bank(true, 0, fixed::types::I80F48::ZERO, fixed::types::I80F48::ZERO) { Some(v) => v, None => return false };
        let admin = self.payer.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolCloseBank { force_close: Some(false) })
            .accounts(accounts::LendingPoolCloseBank {
                group: group,
                bank: bank,
                admin: admin,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_close_bank:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_close_bank:END
        }
        __scout_success
    }

    pub fn action_transfer_to_new_account(&mut self) -> bool {
        // TODO: 1 extra signer(s): ['fee_payer']
        let group = self.marginfi_group;
        let old_marginfi_account = self.marginfi_account;
        let __scout_signer_new_marginfi_account = Keypair::new();
        let new_marginfi_account = __scout_signer_new_marginfi_account.pubkey();
        let authority = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let new_authority = Pubkey::new_unique();
        let global_fee_wallet = self.global_fee_wallet;
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::TransferToNewAccount {  })
            .accounts(accounts::TransferToNewAccount {
                group: group,
                fee_state: Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0,
                old_marginfi_account: old_marginfi_account,
                new_marginfi_account: new_marginfi_account,
                authority: authority,
                fee_payer: fee_payer,
                new_authority: new_authority,
                global_fee_wallet: global_fee_wallet,
            })
            .signers(&[&*self.payer, &__scout_signer_new_marginfi_account])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:transfer_to_new_account:BEGIN
            scout_run_property!("P-0002", {
                self.scout_known_accounts[self.scout_known_next % SCOUT_KNOWN_CAP] = new_marginfi_account;
                self.scout_known_next = self.scout_known_next.saturating_add(1);
            });
            scout_run_property!("P-0001", {
                self.scout_p1_accounts[self.scout_p1_accounts_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p1_accounts_next = self.scout_p1_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0017", {
                self.scout_p17_accounts[self.scout_p17_accounts_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p17_accounts_next = self.scout_p17_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0020", {
                self.scout_p20_accounts[self.scout_p20_accounts_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p20_accounts_next = self.scout_p20_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0022", {
                self.scout_p22_accounts[self.scout_p22_accounts_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p22_accounts_next = self.scout_p22_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0039", {
                self.scout_p39_subjects[self.scout_p39_subjects_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p39_subjects_next = self.scout_p39_subjects_next.saturating_add(1);
            });
            // SCOUT:ACTION-HOOK:transfer_to_new_account:END
        }
        __scout_success
    }

    pub fn action_transfer_to_new_account_pda(&mut self, account_index: u16) -> bool {
        // TODO: 1 extra signer(s): ['fee_payer']
        let third_party_id: Option<u16> = None;
        let group = self.marginfi_group;
        let old_marginfi_account = self.marginfi_account;
        let authority = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let new_authority = Pubkey::new_unique();
        let global_fee_wallet = self.global_fee_wallet;
        let instructions_sysvar = anchor_lang::solana_program::sysvar::instructions::id();
        let system_program = system_program::ID;
        let new_marginfi_account = Pubkey::find_program_address(&[SCOUT_MARGINFI_ACCOUNT_SEED, group.as_ref(), new_authority.as_ref(), &account_index.to_le_bytes(), &third_party_id.unwrap_or(0).to_le_bytes()], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::TransferToNewAccountPda { account_index, third_party_id })
            .accounts(accounts::TransferToNewAccountPda {
                group: group,
                fee_state: Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0,
                old_marginfi_account: old_marginfi_account,
                new_marginfi_account: new_marginfi_account,
                authority: authority,
                fee_payer: fee_payer,
                new_authority: new_authority,
                global_fee_wallet: global_fee_wallet,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:transfer_to_new_account_pda:BEGIN
            scout_run_property!("P-0002", {
                self.scout_known_accounts[self.scout_known_next % SCOUT_KNOWN_CAP] = new_marginfi_account;
                self.scout_known_next = self.scout_known_next.saturating_add(1);
            });
            scout_run_property!("P-0001", {
                self.scout_p1_accounts[self.scout_p1_accounts_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p1_accounts_next = self.scout_p1_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0017", {
                self.scout_p17_accounts[self.scout_p17_accounts_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p17_accounts_next = self.scout_p17_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0020", {
                self.scout_p20_accounts[self.scout_p20_accounts_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p20_accounts_next = self.scout_p20_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0022", {
                self.scout_p22_accounts[self.scout_p22_accounts_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p22_accounts_next = self.scout_p22_accounts_next.saturating_add(1);
            });
            scout_run_property!("P-0039", {
                self.scout_p39_subjects[self.scout_p39_subjects_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
                self.scout_p39_subjects_next = self.scout_p39_subjects_next.saturating_add(1);
            });
            // SCOUT:ACTION-HOOK:transfer_to_new_account_pda:END
        }
        __scout_success
    }

    pub fn action_marginfi_account_set_freeze(&mut self, frozen: bool) -> bool {
        let group = self.marginfi_group;
        let marginfi_account = self.marginfi_account;
        let admin = self.payer.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountSetFreeze { frozen })
            .accounts(accounts::MarginfiAccountSetFreeze {
                group: group,
                marginfi_account: marginfi_account,
                admin: admin,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:marginfi_account_set_freeze:BEGIN
            // SCOUT:ACTION-HOOK:marginfi_account_set_freeze:END
        }
        __scout_success
    }

    pub fn action_marginfi_account_close(&mut self) -> bool {
        // TODO: 1 extra signer(s): ['fee_payer']
        let marginfi_account = match self.scout_prepare_close_marginfi_account() { Some(v) => v, None => return false };
        let authority = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountClose {  })
            .accounts(accounts::MarginfiAccountClose {
                marginfi_account: marginfi_account,
                authority: authority,
                fee_payer: fee_payer,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:marginfi_account_close:BEGIN
            // SCOUT:ACTION-HOOK:marginfi_account_close:END
        }
        __scout_success
    }


    pub fn action_lending_account_pulse_health(&mut self) -> bool {
        let marginfi_account = self.marginfi_account;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountPulseHealth {  })
            .accounts(accounts::LendingAccountPulseHealth {
                marginfi_account: marginfi_account,
                group: self.marginfi_group,
            })
            // TODO: LendingAccountPulseHealth reads ctx.remaining_accounts (pulse_health.rs:24); unbound -> fails account
            // validation before the handler runs, covers no lines. Bind `LendingAccountPulseHealth.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>, appended read-only; `metas:` prefix for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_account_pulse_health:BEGIN
            // SCOUT:ACTION-HOOK:lending_account_pulse_health:END
        }
        __scout_success
    }

    pub fn action_lending_pool_pulse_bank_price_cache(&mut self) -> bool {
        let group = self.marginfi_group;
        let bank = match self.scout_pulse_bank_price_cache_target() { Some(v) => v, None => return false };
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolPulseBankPriceCache {  })
            .accounts(accounts::LendingPoolPulseBankPriceCache {
                group: group,
                bank: bank,
            })
            // TODO: LendingPoolPulseBankPriceCache reads ctx.remaining_accounts (pulse_bank_price_cache.rs:15); unbound -> fails
            // account validation before the handler runs, covers no lines. Bind `LendingPoolPulseBankPriceCache.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>, appended read-only; `metas:` prefix for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:lending_pool_pulse_bank_price_cache:BEGIN
            // SCOUT:ACTION-HOOK:lending_pool_pulse_bank_price_cache:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_init_global_fee_state(&mut self, bank_init_flat_sol_fee: u32, liquidation_flat_sol_fee: u32) -> bool {
        // TODO: arg program_fee_fixed: marginfi::types::WrappedI80F48; arg program_fee_rate: marginfi::types::WrappedI80F48; arg liquidation_max_fee: marginfi::types::WrappedI80F48
        let admin: Pubkey = self.payer.pubkey();
        let fee_wallet: Pubkey = self.global_fee_wallet;
        let program_fee_fixed: marginfi::types::WrappedI80F48 = Default::default(); // TODO: construct arg program_fee_fixed: marginfi::types::WrappedI80F48
        let program_fee_rate: marginfi::types::WrappedI80F48 = Default::default(); // TODO: construct arg program_fee_rate: marginfi::types::WrappedI80F48
        let liquidation_max_fee: marginfi::types::WrappedI80F48 = Default::default(); // TODO: construct arg liquidation_max_fee: marginfi::types::WrappedI80F48
        let payer = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::InitGlobalFeeState { admin, fee_wallet, bank_init_flat_sol_fee, liquidation_flat_sol_fee, order_init_flat_sol_fee: 0, program_fee_fixed, program_fee_rate, liquidation_max_fee, order_execution_max_fee: Default::default() })
            .accounts(accounts::InitGlobalFeeState {
                payer: payer,
                fee_state: fee_state,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:init_global_fee_state:BEGIN
            // SCOUT:ACTION-HOOK:init_global_fee_state:END
        }
        __scout_success
    }

    pub fn action_edit_global_fee_state(&mut self, bank_init_flat_sol_fee: u32, liquidation_flat_sol_fee: u32) -> bool {
        // TODO: arg program_fee_fixed: marginfi::types::WrappedI80F48; arg liquidation_max_fee: marginfi::types::WrappedI80F48
        let admin: Pubkey = self.payer.pubkey();
        let fee_wallet: Pubkey = self.global_fee_wallet;
        let program_fee_fixed: marginfi::types::WrappedI80F48 = Default::default(); // TODO: construct arg program_fee_fixed: marginfi::types::WrappedI80F48
        let program_fee_rate: marginfi::types::WrappedI80F48 = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.1));
        let liquidation_max_fee: marginfi::types::WrappedI80F48 = Default::default(); // TODO: construct arg liquidation_max_fee: marginfi::types::WrappedI80F48
        let global_fee_admin = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::EditGlobalFeeState { admin: Some(admin), fee_wallet: Some(fee_wallet), bank_init_flat_sol_fee: Some(bank_init_flat_sol_fee), liquidation_flat_sol_fee: Some(liquidation_flat_sol_fee), order_init_flat_sol_fee: None, program_fee_fixed: Some(program_fee_fixed), program_fee_rate: Some(program_fee_rate), liquidation_max_fee: Some(liquidation_max_fee), order_execution_max_fee: None, pause_delegate_admin: None, account_transfer_fee: None })
            .accounts(accounts::EditGlobalFeeState {
                global_fee_admin: global_fee_admin,
                fee_state: fee_state,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:edit_global_fee_state:BEGIN
            // SCOUT:ACTION-HOOK:edit_global_fee_state:END
        }
        __scout_success
    }

    pub fn action_propagate_fee_state(&mut self) -> bool {
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let marginfi_group = self.marginfi_group;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::PropagateFeeState {  })
            .accounts(accounts::PropagateFeeState {
                fee_state: fee_state,
                marginfi_group: marginfi_group,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:propagate_fee_state:BEGIN
            // SCOUT:ACTION-HOOK:propagate_fee_state:END
        }
        __scout_success
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_config_group_fee(&mut self, enable_program_fee: bool) -> bool {
        let marginfi_group = self.marginfi_group;
        let global_fee_admin = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::ConfigGroupFee { enable_program_fee })
            .accounts(accounts::ConfigGroupFee {
                marginfi_group: marginfi_group,
                global_fee_admin: global_fee_admin,
                fee_state: fee_state,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:config_group_fee:BEGIN
            // SCOUT:ACTION-HOOK:config_group_fee:END
        }
        __scout_success
    }

    pub fn action_init_staked_settings(&mut self) -> bool {
        // TODO: 1 extra signer(s): ['fee_payer']
        let settings: marginfi::types::StakedSettingsConfig = marginfi::types::StakedSettingsConfig { oracle: Pubkey::new_unique(), asset_weight_init: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.5)), asset_weight_maint: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.6)), deposit_limit: 1_000_000_000, total_asset_value_init_limit: 10_000_000_000, oracle_max_age: 60, risk_tier: marginfi::types::RiskTier::Collateral };
        let marginfi_group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let staked_settings = Pubkey::find_program_address(&[STAKED_SETTINGS_SEED, self.marginfi_group.as_ref()], &self.program_id).0;
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::InitStakedSettings { settings })
            .accounts(accounts::InitStakedSettings {
                marginfi_group: marginfi_group,
                admin: admin,
                fee_payer: fee_payer,
                staked_settings: staked_settings,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:init_staked_settings:BEGIN
            // SCOUT:ACTION-HOOK:init_staked_settings:END
        }
        __scout_success
    }

    pub fn action_edit_staked_settings(&mut self) -> bool {
        // TODO: arg settings: marginfi::types::StakedSettingsEditConfig
        let settings: marginfi::types::StakedSettingsEditConfig = Default::default(); // TODO: construct arg settings: marginfi::types::StakedSettingsEditConfig
        let marginfi_group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let staked_settings = Pubkey::find_program_address(&[STAKED_SETTINGS_SEED, self.marginfi_group.as_ref()], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::EditStakedSettings { settings })
            .accounts(accounts::EditStakedSettings {
                marginfi_group: marginfi_group,
                admin: admin,
                staked_settings: staked_settings,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:edit_staked_settings:BEGIN
            // SCOUT:ACTION-HOOK:edit_staked_settings:END
        }
        __scout_success
    }

    pub fn action_propagate_staked_settings(&mut self) -> bool {
        let marginfi_group = self.staked_group;
        let staked_settings = self.staked_settings;
        let bank = self.staked_bank;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::PropagateStakedSettings {  })
            .accounts(accounts::PropagateStakedSettings {
                marginfi_group: marginfi_group,
                staked_settings: staked_settings,
                bank: bank,
            })
            // TODO: PropagateStakedSettings reads ctx.remaining_accounts (propagate_staked_settings.rs:27); unbound -> fails
            // account validation before the handler runs, covers no lines. Bind `PropagateStakedSettings.remaining_accounts = vec![..]` in SCOUT:BINDINGS (Vec<Pubkey>, appended read-only; `metas:` prefix for Vec<AccountMeta>). Don't guess.
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:propagate_staked_settings:BEGIN
            // SCOUT:ACTION-HOOK:propagate_staked_settings:END
        }
        __scout_success
    }

    pub fn action_start_liquidation(&mut self) -> bool {
        // TODO: remaining_accounts: reads ctx.remaining_accounts (src/instructions/marginfi_account/liquidate_start.rs:40)
        let marginfi_account = self.marginfi_account;
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        let liquidation_receiver = self.payer.pubkey();
        let instruction_sysvar = self.scout_placeholder(); // TODO: real account for instruction_sysvar (unchecked)
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::StartLiquidation {  })
            .accounts(accounts::StartLiquidation {
                marginfi_account: marginfi_account,
                liquidation_record: liquidation_record,
                group: self.marginfi_group,
                liquidation_receiver: liquidation_receiver,
            })
            // TODO: reads ctx.remaining_accounts (liquidate_start.rs:40); unbound, this fails account
            // validation before the handler runs, covering no lines. Bind `StartLiquidation.remaining_accounts`
            // in SCOUT:BINDINGS (Vec<Pubkey>, or `metas:`-prefixed Vec<AccountMeta> for explicit metas).
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:start_liquidation:BEGIN
            // SCOUT:ACTION-HOOK:start_liquidation:END
        }
        __scout_success
    }

    pub fn action_end_liquidation(&mut self) -> bool {
        // TODO: remaining_accounts: reads ctx.remaining_accounts (src/instructions/marginfi_account/liquidate_end.rs:44)
        let marginfi_account = match self.scout_prepare_end_liquidation_marginfi_account_receivership() { Some(v) => v, None => return false };
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        let liquidation_receiver = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let global_fee_wallet = self.global_fee_wallet;
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::EndLiquidation {  })
            .accounts(accounts::EndLiquidation {
                marginfi_account: marginfi_account,
                liquidation_record: liquidation_record,
                group: self.marginfi_group,
                fee_payer: Some(self.payer.pubkey()),
                liquidation_receiver: liquidation_receiver,
                fee_state: fee_state,
                global_fee_wallet: global_fee_wallet,
            })
            // TODO: reads ctx.remaining_accounts (liquidate_end.rs:44); unbound, this fails account
            // validation before the handler runs, covering no lines. Bind `EndLiquidation.remaining_accounts`
            // in SCOUT:BINDINGS (Vec<Pubkey>, or `metas:`-prefixed Vec<AccountMeta> for explicit metas).
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:end_liquidation:BEGIN
            // SCOUT:ACTION-HOOK:end_liquidation:END
        }
        __scout_success
    }

    pub fn action_start_deleverage(&mut self) -> bool {
        // TODO: remaining_accounts: reads ctx.remaining_accounts (src/instructions/marginfi_account/liquidate_start.rs:69)
        let marginfi_account = self.marginfi_account;
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        let group = self.marginfi_group;
        let risk_admin = self.payer.pubkey();
        let instruction_sysvar = self.scout_placeholder(); // TODO: real account for instruction_sysvar (unchecked)
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::StartDeleverage {  })
            .accounts(accounts::StartDeleverage {
                marginfi_account: marginfi_account,
                liquidation_record: liquidation_record,
                group: group,
                risk_admin: risk_admin,
            })
            // TODO: reads ctx.remaining_accounts (liquidate_start.rs:69); unbound, this fails account
            // validation before the handler runs, covering no lines. Bind `StartDeleverage.remaining_accounts`
            // in SCOUT:BINDINGS (Vec<Pubkey>, or `metas:`-prefixed Vec<AccountMeta> for explicit metas).
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:start_deleverage:BEGIN
            // SCOUT:ACTION-HOOK:start_deleverage:END
        }
        __scout_success
    }

    pub fn action_end_deleverage(&mut self) -> bool {
        // TODO: remaining_accounts: reads ctx.remaining_accounts (src/instructions/marginfi_account/liquidate_end.rs:97)
        let marginfi_account = match self.scout_prepare_end_deleverage_marginfi_account() { Some(v) => v, None => return false };
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        let group = self.marginfi_group;
        let risk_admin = self.payer.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::EndDeleverage {  })
            .accounts(accounts::EndDeleverage {
                marginfi_account: marginfi_account,
                liquidation_record: liquidation_record,
                group: group,
                risk_admin: risk_admin,
            })
            // TODO: reads ctx.remaining_accounts (liquidate_end.rs:97); unbound, this fails account
            // validation before the handler runs, covering no lines. Bind `EndDeleverage.remaining_accounts`
            // in SCOUT:BINDINGS (Vec<Pubkey>, or `metas:`-prefixed Vec<AccountMeta> for explicit metas).
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:end_deleverage:BEGIN
            // SCOUT:ACTION-HOOK:end_deleverage:END
        }
        __scout_success
    }

    pub fn action_panic_pause(&mut self) -> bool {
        let global_fee_admin = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::PanicPause {  })
            .accounts(accounts::PanicPause {
                pause_authority: global_fee_admin,
                fee_state: fee_state,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:panic_pause:BEGIN
            // SCOUT:ACTION-HOOK:panic_pause:END
        }
        __scout_success
    }

    pub fn action_panic_unpause(&mut self) -> bool {
        let global_fee_admin = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::PanicUnpause {  })
            .accounts(accounts::PanicUnpause {
                global_fee_admin: global_fee_admin,
                fee_state: fee_state,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:panic_unpause:BEGIN
            // SCOUT:ACTION-HOOK:panic_unpause:END
        }
        __scout_success
    }

    pub fn action_panic_unpause_permissionless(&mut self) -> bool {
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::PanicUnpausePermissionless {  })
            .accounts(accounts::PanicUnpausePermissionless {
                fee_state: fee_state,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:panic_unpause_permissionless:BEGIN
            // SCOUT:ACTION-HOOK:panic_unpause_permissionless:END
        }
        __scout_success
    }


    #[cfg(feature = "admin_actions")]
    pub fn action_init_bank_metadata(&mut self) -> bool {
        let bank = self.bank;
        let fee_payer = self.payer.pubkey();
        let metadata = Pubkey::find_program_address(&[METADATA_SEED, bank.as_ref()], &self.program_id).0;
        let system_program = system_program::ID;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::InitBankMetadata {  })
            .accounts(accounts::InitBankMetadata {
                bank: bank,
                fee_payer: fee_payer,
                metadata: metadata,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:init_bank_metadata:BEGIN
            // SCOUT:ACTION-HOOK:init_bank_metadata:END
        }
        __scout_success
    }

    pub fn action_write_bank_metadata(&mut self) -> bool {
        let ticker: Option<Vec<u8>> = Some(b"SCOUT".to_vec());
        let description: Option<Vec<u8>> = Some(b"Scout bank metadata".to_vec());
        let group = self.marginfi_group;
        let bank = self.bank;
        let metadata_admin = self.payer.pubkey();
        let metadata = Pubkey::find_program_address(&[METADATA_SEED, bank.as_ref()], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::WriteBankMetadata { ticker, description })
            .accounts(accounts::WriteBankMetadata {
                group: group,
                bank: bank,
                metadata_admin: metadata_admin,
                metadata: metadata,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:write_bank_metadata:BEGIN
            // SCOUT:ACTION-HOOK:write_bank_metadata:END
        }
        __scout_success
    }

    pub fn action_configure_deleverage_withdrawal_limit(&mut self, limit: u32) -> bool {
        let marginfi_group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::ConfigureDeleverageWithdrawalLimit { limit })
            .accounts(accounts::ConfigureDeleverageWithdrawalLimit {
                marginfi_group: marginfi_group,
                admin: admin,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:configure_deleverage_withdrawal_limit:BEGIN
            // SCOUT:ACTION-HOOK:configure_deleverage_withdrawal_limit:END
        }
        __scout_success
    }

    pub fn action_purge_deleverage_balance(&mut self) -> bool {
        let group = self.marginfi_group;
        let marginfi_account = self.scout_prepare_purge_deleverage_balance_account();
        let risk_admin = self.payer.pubkey();
        let bank = self.scout_prepare_purge_deleverage_balance_bank(marginfi_account);
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::PurgeDeleverageBalance {  })
            .accounts(accounts::PurgeDeleverageBalance {
                group: group,
                marginfi_account: marginfi_account,
                risk_admin: risk_admin,
                bank: bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            // SCOUT:ACTION-HOOK:purge_deleverage_balance:BEGIN
            // SCOUT:ACTION-HOOK:purge_deleverage_balance:END
        }
        __scout_success
    }

    // SCOUT:EXTRA-ACTIONS:BEGIN

    fn scout_register_subject_account(&mut self, account: Pubkey) {
        self.scout_p1_accounts[self.scout_p1_accounts_next % SCOUT_SUBJECT_CAP] = account;
        self.scout_p1_accounts_next = self.scout_p1_accounts_next.saturating_add(1);
        self.scout_p17_accounts[self.scout_p17_accounts_next % SCOUT_SUBJECT_CAP] = account;
        self.scout_p17_accounts_next = self.scout_p17_accounts_next.saturating_add(1);
        self.scout_p20_accounts[self.scout_p20_accounts_next % SCOUT_SUBJECT_CAP] = account;
        self.scout_p20_accounts_next = self.scout_p20_accounts_next.saturating_add(1);
        self.scout_p22_accounts[self.scout_p22_accounts_next % SCOUT_SUBJECT_CAP] = account;
        self.scout_p22_accounts_next = self.scout_p22_accounts_next.saturating_add(1);
        self.scout_p39_subjects[self.scout_p39_subjects_next % SCOUT_SUBJECT_CAP] = account;
        self.scout_p39_subjects_next = self.scout_p39_subjects_next.saturating_add(1);
    }

    fn scout_register_subject_record(&mut self, record: Pubkey) {
        self.scout_p39_subjects[self.scout_p39_subjects_next % SCOUT_SUBJECT_CAP] = record;
        self.scout_p39_subjects_next = self.scout_p39_subjects_next.saturating_add(1);
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_marginfi_group_initialize(&mut self) -> bool {
        let __scout_signer_marginfi_group = Keypair::new();
        let marginfi_group = __scout_signer_marginfi_group.pubkey();
        let admin = self.payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiGroupInitialize {})
            .accounts(accounts::MarginfiGroupInitialize {
                marginfi_group,
                admin,
                fee_state,
            })
            .signers(&[&*self.payer, &__scout_signer_marginfi_group])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_marginfi_group_configure(&mut self) -> bool {
        let marginfi_group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiGroupConfigure {
                new_admin: Some(self.payer.pubkey()),
                new_emode_admin: Some(self.payer.pubkey()),
                new_curve_admin: Some(self.payer.pubkey()),
                new_limit_admin: Some(self.payer.pubkey()),
                new_emissions_admin: Some(self.payer.pubkey()),
                new_metadata_admin: Some(self.payer.pubkey()),
                new_risk_admin: Some(self.payer.pubkey()),
                new_flow_admin: Some(self.payer.pubkey()),
                emode_max_init_leverage: None,
                emode_max_maint_leverage: None,
                same_asset_emode_init_leverage: None,
                same_asset_emode_maint_leverage: None,
            })
            .accounts(accounts::MarginfiGroupConfigure {
                marginfi_group,
                admin,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            self.scout_p11_refresh_group_authority_expectation(marginfi_group);
        }
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_lending_pool_configure_bank(&mut self) -> bool {
        let bank_config_opt = scout_valid_bank_config_opt();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBank { bank_config_opt })
            .accounts(accounts::LendingPoolConfigureBank {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank: self.bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_lending_pool_configure_bank_interest_only(&mut self) -> bool {
        let interest_rate_config: marginfi::types::InterestRateConfigOpt = Default::default();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankInterestOnly { interest_rate_config })
            .accounts(accounts::LendingPoolConfigureBankInterestOnly {
                group: self.marginfi_group,
                delegate_curve_admin: self.payer.pubkey(),
                bank: self.bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_lending_pool_configure_bank_limits_only(&mut self) -> bool {
        let deposit_limit: Option<u64> = Default::default();
        let borrow_limit: Option<u64> = Default::default();
        let total_asset_value_init_limit: Option<u64> = Default::default();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankLimitsOnly {
                deposit_limit,
                borrow_limit,
                total_asset_value_init_limit,
            })
            .accounts(accounts::LendingPoolConfigureBankLimitsOnly {
                group: self.marginfi_group,
                delegate_limit_admin: self.payer.pubkey(),
                bank: self.bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_lending_pool_configure_bank_emode(&mut self, emode_tag: u16) -> bool {
        let entries: [marginfi::types::EmodeEntry; MAX_EMODE_ENTRIES] = Default::default();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankEmode { emode_tag, entries })
            .accounts(accounts::LendingPoolConfigureBankEmode {
                group: self.marginfi_group,
                emode_admin: self.payer.pubkey(),
                bank: self.bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_marginfi_account_initialize(&mut self) -> bool {
        let __scout_signer_marginfi_account = Keypair::new();
        let marginfi_account = __scout_signer_marginfi_account.pubkey();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitialize {})
            .accounts(accounts::MarginfiAccountInitialize {
                marginfi_group: self.marginfi_group,
                marginfi_account,
                authority: self.payer.pubkey(),
                fee_payer: self.payer.pubkey(),
            })
            .signers(&[&*self.payer, &__scout_signer_marginfi_account])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            self.scout_register_subject_account(marginfi_account);
        }
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_marginfi_account_init_liq_record(&mut self) -> bool {
        let liquidation_record = scout_liquidation_record_pda(self.program_id, self.marginfi_account);
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitLiqRecord {})
            .accounts(accounts::MarginfiAccountInitLiqRecord {
                marginfi_account: self.marginfi_account,
                fee_payer: self.payer.pubkey(),
                liquidation_record,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            self.scout_register_subject_record(liquidation_record);
        }
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_init_global_fee_state(&mut self, bank_init_flat_sol_fee: u32, liquidation_flat_sol_fee: u32) -> bool {
        let program_fee_fixed: marginfi::types::WrappedI80F48 = Default::default();
        let program_fee_rate: marginfi::types::WrappedI80F48 = Default::default();
        let liquidation_max_fee: marginfi::types::WrappedI80F48 = Default::default();
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::InitGlobalFeeState {
                admin: self.payer.pubkey(),
                fee_wallet: self.global_fee_wallet,
                bank_init_flat_sol_fee,
                liquidation_flat_sol_fee,
                order_init_flat_sol_fee: 0,
                order_execution_max_fee: Default::default(),
                program_fee_fixed,
                program_fee_rate,
                liquidation_max_fee,
            })
            .accounts(accounts::InitGlobalFeeState {
                payer: self.payer.pubkey(),
                fee_state: Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_config_group_fee(&mut self, enable_program_fee: bool) -> bool {
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::ConfigGroupFee { enable_program_fee })
            .accounts(accounts::ConfigGroupFee {
                marginfi_group: self.marginfi_group,
                global_fee_admin: self.payer.pubkey(),
                fee_state: Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            self.scout_p11_refresh_group_flags_expectation(self.marginfi_group);
        }
        __scout_success
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_init_bank_metadata(&mut self) -> bool {
        let metadata = Pubkey::find_program_address(&[METADATA_SEED, self.bank.as_ref()], &self.program_id).0;
        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::InitBankMetadata {})
            .accounts(accounts::InitBankMetadata {
                bank: self.bank,
                fee_payer: self.payer.pubkey(),
                metadata,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        __scout_success
    }

    fn scout_prepare_update_emissions_bank(&mut self) -> Pubkey {
        self.bank
    }

    fn scout_prepare_lending_pool_handle_bankruptcy_accounts(
        &mut self,
    ) -> Option<ScoutHandleBankruptcyAccounts> {
        let bank = Pubkey::find_program_address(&[b"scout_handle_bankruptcy_bank"], &self.program_id).0;
        let marginfi_account =
            Pubkey::find_program_address(&[b"scout_handle_bankruptcy_account"], &self.program_id).0;
        let (liquidity_vault_authority, liquidity_vault_authority_bump) =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id);
        let (liquidity_vault, liquidity_vault_bump) =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id);
        let (insurance_vault_authority, insurance_vault_authority_bump) =
            Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id);
        let (insurance_vault, insurance_vault_bump) =
            Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &self.program_id);
        let (fee_vault_authority, fee_vault_authority_bump) =
            Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id);
        let (fee_vault, fee_vault_bump) =
            Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &self.program_id);
        let liability = fixed::types::I80F48::from_num(SCOUT_HANDLE_BANKRUPTCY_LIABILITY_AMOUNT);

        self.ctx
            .create_account()
            .pubkey(bank)
            .owner(self.program_id)
            .data(&scout_handle_bankruptcy_bank_bytes(
                self.marginfi_group,
                self.bank_mint,
                liquidity_vault,
                liquidity_vault_bump,
                liquidity_vault_authority_bump,
                insurance_vault,
                insurance_vault_bump,
                insurance_vault_authority_bump,
                fee_vault,
                fee_vault_bump,
                fee_vault_authority_bump,
                liability,
            ))
            .create()
            .ok()?;
        self.ctx
            .create_account()
            .pubkey(marginfi_account)
            .owner(self.program_id)
            .data(&scout_handle_bankruptcy_account_bytes(
                self.marginfi_group,
                self.payer.pubkey(),
                bank,
                liability,
                None,
                fixed::types::I80F48::ZERO,
            ))
            .create()
            .ok()?;
        self.ctx
            .create_token_account()
            .pubkey(liquidity_vault)
            .mint(self.bank_mint)
            .token_owner(liquidity_vault_authority)
            .amount(0)
            .create()
            .ok()?;
        self.ctx
            .create_token_account()
            .pubkey(insurance_vault)
            .mint(self.bank_mint)
            .token_owner(insurance_vault_authority)
            .amount(SCOUT_HANDLE_BANKRUPTCY_LIABILITY_AMOUNT as u64)
            .create()
            .ok()?;

        Some((marginfi_account, bank, liquidity_vault, insurance_vault, insurance_vault_authority))
    }


    fn scout_ensure_collect_bank_fees_ata(&mut self) -> Pubkey {
        self.fee_withdraw_dst_token_account
    }

    fn scout_prepare_close_marginfi_account(&mut self) -> Option<Pubkey> {
        self.scout_create_initialized_marginfi_account()
    }

    fn scout_prepare_purge_deleverage_balance_account(&mut self) -> Pubkey {
        self.marginfi_account
    }

    fn scout_prepare_purge_deleverage_balance_bank(&mut self, _marginfi_account: Pubkey) -> Pubkey {
        self.tokenless_bank
    }

    fn scout_create_initialized_marginfi_account(&mut self) -> Option<Pubkey> {
        let marginfi_account_keypair = Keypair::new();
        let marginfi_account = marginfi_account_keypair.pubkey();
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::MarginfiAccountInitialize {})
                .accounts(accounts::MarginfiAccountInitialize {
                    marginfi_group: self.marginfi_group,
                    marginfi_account,
                    authority: self.payer.pubkey(),
                    fee_payer: self.payer.pubkey(),
                })
                .signers(&[&*self.payer, &marginfi_account_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return None;
        }
        self.scout_register_subject_account(marginfi_account);
        Some(marginfi_account)
    }

    fn scout_create_lending_account_repay_marginfi_account(&mut self) -> Option<Pubkey> {
        self.scout_create_initialized_marginfi_account()
    }

    fn scout_create_lending_account_repay_bank(&mut self, tokenless_allowed: bool) -> Option<Pubkey> {
        let bank_keypair = Keypair::new();
        let bank = bank_keypair.pubkey();
        let (
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank);
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAddBank {
                    bank_config: scout_valid_bank_config(10),
                })
                .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, spl_token::id()))
                .signers(&[&*self.payer, &bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return None;
        }
        if tokenless_allowed {
            let mut bank_config_opt = scout_valid_bank_config_opt();
            bank_config_opt.tokenless_repayments_allowed = Some(true);
            if !(self.ctx
                    .program(self.program_id)
                    .call(instruction::LendingPoolConfigureBank { bank_config_opt })
                    .accounts(accounts::LendingPoolConfigureBank {
                        group: self.marginfi_group,
                        admin: self.payer.pubkey(),
                        bank,
                    })
                    .signers(&[&*self.payer])
                    .send()
                    .map(|o| o.is_success())
                    .unwrap_or(false)) {
                return None;
            }
        }
        Some(bank)
    }

    fn scout_seed_lending_account_repay_liability(
        &mut self,
        marginfi_account: Pubkey,
        bank: Pubkey,
        amount: u64,
    ) -> bool {
        let liability_shares = fixed::types::I80F48::from_num(amount);
        let account_len_needed = SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET + 16;
        if !self
            .ctx
            .read_account(&marginfi_account)
            .map(|a| a.data.len() >= account_len_needed)
            .unwrap_or(false)
        {
            return false;
        }
        let bank_len_needed = SCOUT_BANK_BORROWING_POSITION_COUNT_OFFSET + 4;
        if !self
            .ctx
            .read_account(&bank)
            .map(|a| a.data.len() >= bank_len_needed)
            .unwrap_or(false)
        {
            return false;
        }
        if self
            .ctx
            .update_account(&marginfi_account, |data| {
                data[SCOUT_FIRST_BALANCE_ACTIVE_OFFSET] = 1;
                data[SCOUT_FIRST_BALANCE_BANK_PK_OFFSET
                    ..SCOUT_FIRST_BALANCE_BANK_PK_OFFSET + 32]
                    .copy_from_slice(bank.as_ref());
                data[SCOUT_FIRST_BALANCE_BANK_ASSET_TAG_OFFSET] = 0;
                data[SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET
                    ..SCOUT_FIRST_BALANCE_ASSET_SHARES_OFFSET + 16]
                    .copy_from_slice(&fixed::types::I80F48::ZERO.to_le_bytes());
                data[SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET
                    ..SCOUT_FIRST_BALANCE_LIABILITY_SHARES_OFFSET + 16]
                    .copy_from_slice(&liability_shares.to_le_bytes());
            })
            .is_err()
        {
            return false;
        }
        if self
            .ctx
            .update_account(&bank, |data| {
                data[SCOUT_BANK_TOTAL_LIABILITY_SHARES_OFFSET
                    ..SCOUT_BANK_TOTAL_LIABILITY_SHARES_OFFSET + 16]
                    .copy_from_slice(&liability_shares.to_le_bytes());
                data[SCOUT_BANK_BORROWING_POSITION_COUNT_OFFSET
                    ..SCOUT_BANK_BORROWING_POSITION_COUNT_OFFSET + 4]
                    .copy_from_slice(&1i32.to_le_bytes());
            })
            .is_err()
        {
            return false;
        }
        if !self.scout_p22_forged_accounts.contains(&marginfi_account) {
            self.scout_p22_forged_accounts.push(marginfi_account);
        }
        true
    }

    fn scout_create_lending_account_repay_bank_with_liability(
        &mut self,
        marginfi_account: Pubkey,
        tokenless_allowed: bool,
        amount: u64,
    ) -> Option<Pubkey> {
        let bank = self.scout_create_lending_account_repay_bank(tokenless_allowed)?;
        if !self.scout_seed_lending_account_repay_liability(marginfi_account, bank, amount) {
            return None;
        }
        Some(bank)
    }

    fn scout_prepare_lending_account_close_balance_marginfi_account(&mut self) -> Option<Pubkey> {
        let marginfi_account = self.scout_create_initialized_marginfi_account()?;
        let account_len_needed = SCOUT_PULSE_FIRST_BALANCE_LIABILITY_SHARES_OFFSET + 16;
        if !self
            .ctx
            .read_account(&marginfi_account)
            .map(|a| a.data.len() >= account_len_needed)
            .unwrap_or(false)
        {
            return None;
        }
        if self
            .ctx
            .update_account(&marginfi_account, |data| {
                data[SCOUT_PULSE_FIRST_BALANCE_ACTIVE_OFFSET] = 1;
                data[SCOUT_PULSE_FIRST_BALANCE_BANK_OFFSET
                    ..SCOUT_PULSE_FIRST_BALANCE_BANK_OFFSET + 32]
                    .copy_from_slice(self.bank.as_ref());
                data[SCOUT_PULSE_FIRST_BALANCE_BANK_ASSET_TAG_OFFSET] = 0;
                data[SCOUT_PULSE_FIRST_BALANCE_ASSET_SHARES_OFFSET
                    ..SCOUT_PULSE_FIRST_BALANCE_ASSET_SHARES_OFFSET + 16]
                    .copy_from_slice(&fixed::types::I80F48::ZERO.to_le_bytes());
                data[SCOUT_PULSE_FIRST_BALANCE_LIABILITY_SHARES_OFFSET
                    ..SCOUT_PULSE_FIRST_BALANCE_LIABILITY_SHARES_OFFSET + 16]
                    .copy_from_slice(&fixed::types::I80F48::ZERO.to_le_bytes());
            })
            .is_err()
        {
            return None;
        }
        Some(marginfi_account)
    }

    fn scout_prepare_end_deleverage_marginfi_account(&mut self) -> Option<Pubkey> {
        let marginfi_account = self.scout_create_initialized_marginfi_account()?;
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::MarginfiAccountInitLiqRecord {})
                .accounts(accounts::MarginfiAccountInitLiqRecord {
                    marginfi_account,
                    fee_payer: self.payer.pubkey(),
                    liquidation_record,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return None;
        }
        self.scout_register_subject_record(liquidation_record);
        let account_len_needed = MARGINFI_ACCOUNT_FLAGS_OFFSET + 8;
        if !self
            .ctx
            .read_account(&marginfi_account)
            .map(|a| a.data.len() >= account_len_needed)
            .unwrap_or(false)
        {
            return None;
        }
        let record_len_needed = SCOUT_LIQUIDATION_RECORD_RECEIVER_OFFSET + 32;
        if !self
            .ctx
            .read_account(&liquidation_record)
            .map(|a| a.data.len() >= record_len_needed)
            .unwrap_or(false)
        {
            return None;
        }
        if self
            .ctx
            .update_account(&marginfi_account, |data| {
                let flags = SCOUT_ACCOUNT_IN_RECEIVERSHIP | SCOUT_ACCOUNT_IN_DELEVERAGE;
                data[MARGINFI_ACCOUNT_FLAGS_OFFSET..MARGINFI_ACCOUNT_FLAGS_OFFSET + 8]
                    .copy_from_slice(&flags.to_le_bytes());
            })
            .is_err()
        {
            return None;
        }
        if !self.scout_p17_harness_flagged.contains(&marginfi_account) {
            self.scout_p17_harness_flagged.push(marginfi_account);
        }
        let receiver = self.payer.pubkey();
        if self
            .ctx
            .update_account(&liquidation_record, |data| {
                data[SCOUT_LIQUIDATION_RECORD_RECEIVER_OFFSET
                    ..SCOUT_LIQUIDATION_RECORD_RECEIVER_OFFSET + 32]
                    .copy_from_slice(receiver.as_ref());
            })
            .is_err()
        {
            return None;
        }
        Some(marginfi_account)
    }

    fn scout_prepare_kamino_deposit(&mut self) -> Pubkey {
        self.bank
    }

    fn scout_ensure_kamino_oracle(&mut self) -> Pubkey {
        scout_kamino_oracle_pda(self.program_id)
    }

    fn scout_ensure_kamino_reserve(&mut self) -> Pubkey {
        scout_kamino_reserve(self.program_id)
    }

    fn scout_create_drift_pyth_oracle(&mut self) -> Pubkey {
        Pubkey::find_program_address(&[b"scout_drift_pyth_oracle"], &self.program_id).0
    }

    fn scout_create_drift_spot_market(&mut self, _mint: Pubkey) -> Pubkey {
        scout_drift_spot_market()
    }

    fn scout_ensure_drift_user_account(&mut self, authority: Pubkey) -> Pubkey {
        scout_drift_user(scout_drift_program_id(), authority)
    }

    fn scout_ensure_drift_user_stats_account(&mut self, authority: Pubkey) -> Pubkey {
        scout_drift_user_stats(scout_drift_program_id(), authority)
    }

    fn scout_prepare_drift_deposit_bank(&mut self) -> Pubkey {
        self.bank
    }

    fn scout_drift_oracle_for_bank(&mut self, _bank: Pubkey) -> Option<Pubkey> {
        Some(self.scout_create_drift_pyth_oracle())
    }

    fn scout_prepare_drift_state(&mut self) -> Pubkey {
        scout_drift_state()
    }

    fn scout_drift_user_for_bank(&mut self, bank: Pubkey) -> Pubkey {
        scout_drift_user(scout_drift_program_id(), bank)
    }

    fn scout_drift_user_stats_for_bank(&mut self, bank: Pubkey) -> Pubkey {
        scout_drift_user_stats(scout_drift_program_id(), bank)
    }

    fn scout_drift_spot_market_for_bank(&mut self, _bank: Pubkey) -> Pubkey {
        scout_drift_spot_market()
    }

    fn scout_drift_spot_market_vault_for_bank(&mut self, _bank: Pubkey) -> Pubkey {
        scout_drift_spot_market_vault()
    }

    fn scout_prepare_drift_withdraw_all_marginfi_account(&mut self) -> Pubkey {
        self.marginfi_account
    }

    fn scout_prepare_drift_withdraw_bank(&mut self) -> Pubkey {
        self.bank
    }

    fn scout_create_solend_pyth_oracle(&mut self) -> Pubkey {
        scout_solend_pyth_price(self.program_id)
    }

    fn scout_create_solend_reserve(&mut self, _mint: Pubkey) -> Pubkey {
        scout_solend_reserve(self.program_id)
    }

    fn scout_ensure_solend_deposit_fixture(&mut self) -> Pubkey {
        self.bank
    }

    fn scout_prepare_lending_pool_close_bank_success(&mut self) -> Pubkey {
        self.bank
    }

    fn scout_prepare_end_liquidation_marginfi_account(&mut self) -> Pubkey {
        self.marginfi_account
    }

    fn scout_prepare_migrate_curve_bank(&mut self) -> Pubkey {
        self.bank
    }

    fn scout_prepare_kamino_withdraw(&mut self) -> ScoutKaminoWithdrawAccounts {
        let bank = scout_kamino_withdraw_bank(self.program_id, self.marginfi_group, self.bank_mint);
        let liquidity_vault_authority =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        ScoutKaminoWithdrawAccounts {
            marginfi_account: self.marginfi_account,
            bank,
            liquidity_vault_authority,
            liquidity_vault: Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0,
            reserve: scout_kamino_reserve(self.program_id),
            obligation: scout_kamino_obligation(self.program_id, bank),
            oracle: scout_kamino_pyth_oracle(self.program_id),
            lending_market: scout_kamino_lending_market(self.program_id),
            lending_market_authority: scout_kamino_lending_market_authority(self.program_id),
            reserve_liquidity_supply: scout_kamino_reserve_liquidity_supply(self.program_id),
            reserve_collateral_mint: scout_kamino_reserve_collateral_mint(self.program_id),
            reserve_source_collateral: scout_kamino_reserve_destination_deposit_collateral(self.program_id),
            destination_token_account: self.signer_token_account,
        }
    }

    fn scout_prepare_kamino_harvest_bank(&mut self) -> Pubkey {
        self.bank
    }

    fn scout_ensure_kamino_harvest_destination_token_account(&mut self) -> Pubkey {
        self.fee_withdraw_dst_token_account
    }

    fn scout_ensure_kamino_harvest_system_account(&mut self, seed: &[u8]) -> Pubkey {
        Pubkey::find_program_address(&[seed], &self.program_id).0
    }

    fn scout_ensure_kamino_harvest_reward_mint(&mut self) -> Pubkey {
        self.emissions_mint
    }

    fn scout_ensure_kamino_harvest_user_reward_ata(&mut self, _bank: Pubkey) -> Pubkey {
        self.withdraw_emissions_destination_account
    }

    fn scout_ensure_kamino_harvest_rewards_vault(&mut self) -> Pubkey {
        Pubkey::find_program_address(&[b"scout_kh_rewards_vault"], &self.program_id).0
    }

    fn scout_ensure_kamino_harvest_rewards_treasury_vault(&mut self) -> Pubkey {
        Pubkey::find_program_address(&[b"scout_kh_rewards_treasury"], &self.program_id).0
    }

    fn scout_ensure_kamino_harvest_farm_vaults_authority(&mut self) -> Pubkey {
        Pubkey::find_program_address(&[b"scout_kh_farm_vaults_auth"], &self.program_id).0
    }

    fn scout_ensure_kamino_harvest_farms_program(&mut self) -> Pubkey {
        scout_kamino_farms_program_id()
    }

    fn scout_prepare_drift_harvest_reward_accounts(
        &mut self,
        with_admin_deposit: bool,
    ) -> Option<ScoutDriftHarvestAccounts> {
        let bank = self.bank;
        let liquidity_vault_authority =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        Some(ScoutDriftHarvestAccounts {
            bank,
            fee_state: Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0,
            liquidity_vault_authority,
            intermediary_token_account: self.signer_token_account,
            destination_token_account: self.fee_withdraw_dst_token_account,
            drift_state: scout_drift_state(),
            integration_acc_2: scout_drift_user(scout_drift_program_id(), liquidity_vault_authority),
            integration_acc_3: scout_drift_user_stats(scout_drift_program_id(), liquidity_vault_authority),
            harvest_drift_spot_market: scout_drift_spot_market(),
            harvest_drift_spot_market_vault: scout_drift_spot_market_vault(),
            drift_signer: scout_drift_signer(),
            reward_mint: if with_admin_deposit { self.emissions_mint } else { self.bank_mint },
        })
    }

    fn scout_prepare_solend_withdraw_bank(&mut self) -> Pubkey {
        scout_seeded_bank_pda(
            self.program_id,
            self.marginfi_group,
            self.bank_mint,
            SCOUT_SOLEND_WITHDRAW_BANK_SEED,
        )
    }

    fn scout_ensure_solend_withdraw_destination_token_account(&mut self) -> Pubkey {
        self.signer_token_account
    }

    pub fn action_lending_pool_add_bank_permissionless_staked(&mut self, bank_seed: u64) -> bool {
        let stake_pool_pubkey = Pubkey::new_unique();
        self.ctx.create_account()
            .pubkey(stake_pool_pubkey)
            .owner(spl_single_pool_id())
            .lamports(1_000_000)
            .create()
            .unwrap();
        let (lst_mint_pda, _) = Pubkey::find_program_address(
            &[b"mint", stake_pool_pubkey.as_ref()],
            &spl_single_pool_id(),
        );
        let (sol_pool_pda, _) = Pubkey::find_program_address(
            &[b"stake", stake_pool_pubkey.as_ref()],
            &spl_single_pool_id(),
        );
        self.ctx.create_mint()
            .pubkey(lst_mint_pda)
            .decimals(9)
            .mint_authority(self.payer.pubkey())
            .create()
            .unwrap();
        self.ctx.create_account()
            .pubkey(sol_pool_pda)
            .owner(native_stake_id())
            .lamports(1_000_000)
            .create()
            .unwrap();
        let staked_pool_onramp = self.scout_placeholder();
        let staked_validator_vote_account = self.scout_placeholder();

        let (bank_pubkey, _) = Pubkey::find_program_address(
            &[self.staked_group.as_ref(), lst_mint_pda.as_ref(), &bank_seed.to_le_bytes()],
            &self.program_id,
        );
        let (liquidity_vault_authority, _) = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank_pubkey.as_ref()], &self.program_id);
        let (liquidity_vault, _) = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank_pubkey.as_ref()], &self.program_id);
        let (insurance_vault_authority, _) = Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank_pubkey.as_ref()], &self.program_id);
        let (insurance_vault, _) = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank_pubkey.as_ref()], &self.program_id);
        let (fee_vault_authority, _) = Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank_pubkey.as_ref()], &self.program_id);
        let (fee_vault, _) = Pubkey::find_program_address(&[FEE_VAULT_SEED, bank_pubkey.as_ref()], &self.program_id);

        let __scout_success = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBankPermissionless { bank_seed })
            .accounts(accounts::LendingPoolAddBankPermissionless {
                marginfi_group: self.staked_group,
                staked_settings: self.staked_settings,
                fee_payer: self.payer.pubkey(),
                bank_mint: lst_mint_pda,
                sol_pool: sol_pool_pda,
                pool_onramp: staked_pool_onramp,
                validator_vote_account: staked_validator_vote_account,
                stake_pool: stake_pool_pubkey,
                bank: bank_pubkey,
                liquidity_vault_authority,
                liquidity_vault,
                insurance_vault_authority,
                insurance_vault,
                fee_vault_authority,
                fee_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![self.staked_oracle, lst_mint_pda, sol_pool_pda])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        __scout_success
    }

    fn scout_liquidate_add_bank(&mut self, config: marginfi::types::BankConfigCompact) -> Option<Pubkey> {
        let bank_keypair = Keypair::new();
        let bank = bank_keypair.pubkey();
        let (
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank);
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: config })
                .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, spl_token::id()))
                .signers(&[&*self.payer, &bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return None;
        }
        scout_run_property!("P-0006", {
            self.scout_p06_banks[self.scout_p06_bank_next % SCOUT_P06_BANK_CAP] = bank;
            self.scout_p06_bank_next = self.scout_p06_bank_next.saturating_add(1);
        });
        Some(bank)
    }

    fn scout_liquidate_raise_liab_bank_limits(&mut self, bank: Pubkey) -> bool {
        let mut bank_config_opt = scout_valid_bank_config_opt();
        bank_config_opt.deposit_limit = Some(u64::MAX);
        bank_config_opt.borrow_limit = Some(u64::MAX);
        bank_config_opt.total_asset_value_init_limit = Some(u64::MAX);
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolConfigureBank { bank_config_opt })
                .accounts(accounts::LendingPoolConfigureBank {
                    group: self.marginfi_group,
                    admin: self.payer.pubkey(),
                    bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return false;
        }
        true
    }

    fn scout_create_t22_fee_mint(&mut self, fee_bps: u16, max_fee: u64, decimals: u8) -> Option<Pubkey> {
        use spl_token_2022_interface::extension::transfer_fee::{TransferFee, TransferFeeConfig};
        use spl_token_2022_interface::extension::{
            BaseStateWithExtensionsMut, ExtensionType, StateWithExtensionsMut,
        };
        use spl_token_2022_interface::state::Mint as T22Mint;
        let mint_kp = Keypair::new();
        let mint = mint_kp.pubkey();
        let token_2022_id: Pubkey = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse().ok()?;
        let len =
            ExtensionType::try_calculate_account_len::<T22Mint>(&[ExtensionType::TransferFeeConfig])
                .ok()?;
        let mut data = vec![0u8; len];
        {
            let mut state =
                StateWithExtensionsMut::<T22Mint>::unpack_uninitialized(&mut data).ok()?;
            let ext = state.init_extension::<TransferFeeConfig>(true).ok()?;
            let fee = TransferFee {
                epoch: 0u64.into(),
                maximum_fee: max_fee.into(),
                transfer_fee_basis_points: fee_bps.into(),
            };
            ext.older_transfer_fee = fee;
            ext.newer_transfer_fee = fee;
            state.init_account_type().ok()?;
        }
        let base = spl_token::state::Mint {
            mint_authority: spl_token::solana_program::program_option::COption::Some(
                self.payer.pubkey(),
            ),
            supply: 0,
            decimals,
            is_initialized: true,
            freeze_authority: spl_token::solana_program::program_option::COption::None,
        };
        let base_len = <spl_token::state::Mint as spl_token::solana_program::program_pack::Pack>::LEN;
        <spl_token::state::Mint as spl_token::solana_program::program_pack::Pack>::pack(
            base,
            &mut data[..base_len],
        )
        .ok()?;
        let lamports = solana_program::rent::Rent::default().minimum_balance(data.len());
        self.ctx
            .write_account(
                &mint,
                solana_account::Account {
                    lamports,
                    data,
                    owner: token_2022_id,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .ok()?;
        Some(mint)
    }

    fn scout_create_t22_permdelegate_mint(&mut self, delegate: Pubkey, decimals: u8) -> Option<Pubkey> {
        use spl_token_2022_interface::extension::permanent_delegate::PermanentDelegate;
        use spl_token_2022_interface::extension::{
            BaseStateWithExtensionsMut, ExtensionType, StateWithExtensionsMut,
        };
        use spl_token_2022_interface::state::Mint as T22Mint;
        let mint = Keypair::new().pubkey();
        let token_2022_id: Pubkey = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse().ok()?;
        let len = ExtensionType::try_calculate_account_len::<T22Mint>(&[
            ExtensionType::PermanentDelegate,
        ])
        .ok()?;
        let mut data = vec![0u8; len];
        {
            let mut state =
                StateWithExtensionsMut::<T22Mint>::unpack_uninitialized(&mut data).ok()?;
            let ext = state.init_extension::<PermanentDelegate>(true).ok()?;
            bytemuck::bytes_of_mut(&mut ext.delegate).copy_from_slice(&delegate.to_bytes());
            state.init_account_type().ok()?;
        }
        let base = spl_token::state::Mint {
            mint_authority: spl_token::solana_program::program_option::COption::Some(self.payer.pubkey()),
            supply: 0,
            decimals,
            is_initialized: true,
            freeze_authority: spl_token::solana_program::program_option::COption::None,
        };
        let base_len = <spl_token::state::Mint as spl_token::solana_program::program_pack::Pack>::LEN;
        <spl_token::state::Mint as spl_token::solana_program::program_pack::Pack>::pack(base, &mut data[..base_len]).ok()?;
        let lamports = solana_program::rent::Rent::default().minimum_balance(data.len());
        self.ctx
            .write_account(&mint, solana_account::Account { lamports, data, owner: token_2022_id, executable: false, rent_epoch: 0 })
            .ok()?;
        Some(mint)
    }

    pub fn action_permanent_delegate_vault_drain_probe(&mut self) -> bool {
        let token_2022_id: Pubkey = match "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse() { Ok(v) => v, Err(_) => return false };
        let attacker = std::rc::Rc::new(Keypair::new());
        if self.ctx.create_account().pubkey(attacker.pubkey()).lamports(1_000_000_000).owner(system_program::ID).create().is_err() {
            return false;
        }
        let mint = match self.scout_create_t22_permdelegate_mint(attacker.pubkey(), 6) { Some(v) => v, None => return false };
        let mut cfg = scout_valid_bank_config(10);
        cfg.deposit_limit = u64::MAX;
        let bank = match self.scout_add_t22_bank(cfg, mint) { Some(v) => v, None => return false };
        if !self.scout_liquidate_set_fixed_price(bank, fixed::types::I80F48::ONE) {
            return false;
        }
        if !self.scout_liquidate_raise_liab_bank_limits(bank) {
            return false;
        }
        let depositor = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let payer_pk = self.payer.pubkey();
        let deposit_amount: u64 = 2_000_000;
        let depositor_ta = match self.scout_create_t22_token_account(mint, payer_pk, deposit_amount.saturating_mul(4)) { Some(v) => v, None => return false };
        if !self.scout_t22_deposit(depositor, bank, mint, deposit_amount, depositor_ta) {
            return false;
        }
        let liq_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let vault_before = self.scout_token_amount(liq_vault);
        if vault_before == 0 {
            return false;
        }
        let attacker_ta = match self.scout_create_t22_token_account(mint, attacker.pubkey(), 0) { Some(v) => v, None => return false };
        if let Ok(ix) = spl_token_2022_interface::instruction::transfer_checked(
            &token_2022_id,
            &liq_vault,
            &mint,
            &attacker_ta,
            &attacker.pubkey(),
            &[],
            vault_before,
            6,
        ) {
            let _ = self.ctx.raw_call(ix).signers(&[&*attacker]).send();
        }
        let vault_after = self.scout_token_amount(liq_vault);
        let asv = match self.scout_bank_i80f48(bank, 80) { Some(v) => v, None => return false };
        let tas = match self.scout_bank_i80f48(bank, 272) { Some(v) => v, None => return false };
        let scale = fixed::types::I80F48::from_num(1_000_000i64);
        let recorded = match tas.checked_mul(asv).and_then(|v| v.checked_div(scale)) { Some(v) => v, None => return false };
        let vault_after_val = match fixed::types::I80F48::from_num(vault_after).checked_div(scale) { Some(v) => v, None => return false };
        let dust = fixed::types::I80F48::from_num(0.01);
        scout_check!(
            "P-PERMDELEGATE",
            "liquidity-vault-balance-must-cover-recorded-deposits",
            vault_after_val + dust >= recorded,
            "P-PERMDELEGATE: PermanentDelegate drained liquidity vault {} -> {}, recorded deposits {} unchanged",
            vault_before,
            vault_after,
            recorded
        );
        let violated = vault_after_val + dust < recorded;
        if let Some(mut va) = self.ctx.svm.get_account(&liq_vault) {
            if va.data.len() >= 72 {
                va.data[64..72].copy_from_slice(&vault_before.to_le_bytes());
                let _ = self.ctx.svm.set_account(liq_vault, va);
            }
        }
        violated
    }

    fn scout_create_t22_token_account(
        &mut self,
        mint: Pubkey,
        owner: Pubkey,
        amount: u64,
    ) -> Option<Pubkey> {
        let acct = Keypair::new().pubkey();
        self.scout_write_t22_token_account_at(acct, mint, owner, amount)?;
        Some(acct)
    }

    fn scout_write_t22_token_account_at(
        &mut self,
        acct: Pubkey,
        mint: Pubkey,
        owner: Pubkey,
        amount: u64,
    ) -> Option<()> {
        use spl_token_2022_interface::extension::transfer_fee::TransferFeeAmount;
        use spl_token_2022_interface::extension::{
            BaseStateWithExtensionsMut, ExtensionType, StateWithExtensionsMut,
        };
        use spl_token_2022_interface::state::Account as T22Account;
        let token_2022_id: Pubkey = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse().ok()?;
        let len = ExtensionType::try_calculate_account_len::<T22Account>(&[
            ExtensionType::TransferFeeAmount,
        ])
        .ok()?;
        let mut data = vec![0u8; len];
        {
            let mut state =
                StateWithExtensionsMut::<T22Account>::unpack_uninitialized(&mut data).ok()?;
            let ext = state.init_extension::<TransferFeeAmount>(true).ok()?;
            ext.withheld_amount = 0u64.into();
            state.init_account_type().ok()?;
        }
        let base = spl_token::state::Account {
            mint,
            owner,
            amount,
            delegate: spl_token::solana_program::program_option::COption::None,
            state: spl_token::state::AccountState::Initialized,
            is_native: spl_token::solana_program::program_option::COption::None,
            delegated_amount: 0,
            close_authority: spl_token::solana_program::program_option::COption::None,
        };
        let base_len =
            <spl_token::state::Account as spl_token::solana_program::program_pack::Pack>::LEN;
        <spl_token::state::Account as spl_token::solana_program::program_pack::Pack>::pack(
            base,
            &mut data[..base_len],
        )
        .ok()?;
        let lamports = solana_program::rent::Rent::default().minimum_balance(data.len());
        self.ctx
            .write_account(
                &acct,
                solana_account::Account {
                    lamports,
                    data,
                    owner: token_2022_id,
                    executable: false,
                    rent_epoch: 0,
                },
            )
            .ok()?;
        Some(())
    }

    fn scout_bank_i80f48(&self, bank: Pubkey, offset: usize) -> Option<fixed::types::I80F48> {
        let data = self.ctx.read_account(&bank).ok()?.data;
        let b: [u8; 16] = data.get(offset..offset + 16)?.try_into().ok()?;
        Some(fixed::types::I80F48::from_le_bytes(b))
    }

    #[allow(clippy::type_complexity)]
    fn scout_build_bankruptcy_scenario(
        &mut self,
        permissionless: bool,
    ) -> Option<(
        Pubkey,
        Pubkey,
        Pubkey,
        Pubkey,
        [Pubkey; 2],
        fixed::types::I80F48,
        fixed::types::I80F48,
        fixed::types::I80F48,
        fixed::types::I80F48,
        fixed::types::I80F48,
    )> {
        let coll_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let liab_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if coll_bank == liab_bank || !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return None;
        }
        if permissionless {
            let mut bank_acct = self.ctx.svm.get_account(&liab_bank)?;
            if bank_acct.data.len() < 848 {
                return None;
            }
            let cur: [u8; 8] = bank_acct.data[840..848].try_into().ok()?;
            let flags = u64::from_le_bytes(cur) | (1u64 << 2);
            bank_acct.data[840..848].copy_from_slice(&flags.to_le_bytes());
            if self.ctx.svm.set_account(liab_bank, bank_acct).is_err() {
                return None;
            }
        }
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(10)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let provider = self.scout_create_initialized_marginfi_account()?;
        let bankrupt = self.scout_create_initialized_marginfi_account()?;
        if !self.scout_liquidate_deposit(provider, liab_bank, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(bankrupt, coll_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        let sorted_pair = if coll_bank.to_bytes() > liab_bank.to_bytes() { [coll_bank, liab_bank] } else { [liab_bank, coll_bank] };
        if !self.scout_liquidate_borrow(bankrupt, liab_bank, SCOUT_P33_BORROW_AMOUNT, sorted_pair.to_vec()) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(0.001)) {
            return None;
        }
        let lsv = self.scout_bank_i80f48(liab_bank, 96)?;
        let asv0 = self.scout_bank_i80f48(liab_bank, 80)?;
        let tas = self.scout_bank_i80f48(liab_bank, 272)?;
        let (_, bad_shares) = self.scout_p33_shares(bankrupt, liab_bank)?;
        let bad_debt = bad_shares.checked_mul(lsv)?;
        Some((coll_bank, liab_bank, provider, bankrupt, sorted_pair, lsv, asv0, tas, bad_shares, bad_debt))
    }

    pub fn action_bankruptcy_conservation_probe(&mut self) -> bool {
        let (coll_bank, liab_bank, _provider, bankrupt, sorted_pair, lsv, asv0, tas, bad_shares, bad_debt) = match self.scout_build_bankruptcy_scenario(false) { Some(v) => v, None => return false };
        let ins_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let ins0 = self.scout_token_amount(ins_vault);
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let insurance_vault_authority = Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, liab_bank.as_ref()], &self.program_id).0;
        let payer = self.payer.clone();
        if !self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolHandleBankruptcy {})
            .accounts(accounts::LendingPoolHandleBankruptcy {
                group: self.marginfi_group,
                signer: payer.pubkey(),
                bank: liab_bank,
                marginfi_account: bankrupt,
                liquidity_vault,
                insurance_vault: ins_vault,
                insurance_vault_authority,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![sorted_pair[0], sorted_pair[1]])
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }
        let asv1 = match self.scout_bank_i80f48(liab_bank, 80) { Some(v) => v, None => return false };
        let ins1 = self.scout_token_amount(ins_vault);
        let depositor_loss = match asv0.checked_sub(asv1).and_then(|d| d.checked_mul(tas)) { Some(v) => v, None => return false };
        let insurance_used = fixed::types::I80F48::from_num(ins0.saturating_sub(ins1));
        let residual = bad_debt - (depositor_loss + insurance_used);
        if bad_debt <= fixed::types::I80F48::from_num(1) {
            return false;
        }
        let dust = fixed::types::I80F48::from_num(4);
        scout_check!(
            "P-BANKRUPTCY-CONSV",
            "bad-debt-settlement-must-equal-insurance-used-plus-depositor-loss",
            residual.abs() <= dust,
            "P-BANKRUPTCY-CONSV: bad debt cleared {} but depositors bore {} and insurance {} (residual {})",
            bad_debt,
            depositor_loss,
            insurance_used,
            residual
        );
        residual.abs() > dust
    }

    pub fn action_over_utilization_conservation_probe(&mut self) -> bool {
        let coll_bank = match self.scout_liquidate_add_bank(scout_liquidation_bank_config()) { Some(v) => v, None => return false };
        let mut cfg = scout_valid_bank_config(10);
        cfg.deposit_limit = u64::MAX;
        cfg.borrow_limit = u64::MAX;
        cfg.total_asset_value_init_limit = u64::MAX;
        cfg.interest_rate_config.zero_util_rate = 85_899_346;
        cfg.interest_rate_config.hundred_util_rate = 429_496_729;
        cfg.interest_rate_config.insurance_ir_fee = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.1));
        cfg.interest_rate_config.protocol_ir_fee = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.05));
        let bank = match self.scout_liquidate_add_bank(cfg) { Some(v) => v, None => return false };
        if coll_bank == bank {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(10)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(bank, fixed::types::I80F48::ONE) {
            return false;
        }
        let provider = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let borrower = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if !self.scout_liquidate_deposit(provider, bank, 2_000_000) {
            return false;
        }
        if !self.scout_liquidate_deposit(borrower, coll_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return false;
        }
        let sorted_pair = if coll_bank.to_bytes() > bank.to_bytes() { [coll_bank, bank] } else { [bank, coll_bank] };
        if !self.scout_liquidate_borrow(borrower, bank, 1_980_000, sorted_pair.to_vec()) {
            return false;
        }
        use ::anchor_lang::prelude::Clock;
        let accrue = |me: &mut Self, secs: i64| -> bool {
            let clock = me.ctx.svm.get_sysvar::<Clock>();
            me.ctx.set_sysvar(&Clock {
                slot: clock.slot.saturating_add(1000),
                epoch_start_timestamp: clock.epoch_start_timestamp,
                epoch: clock.epoch,
                leader_schedule_epoch: clock.leader_schedule_epoch,
                unix_timestamp: clock.unix_timestamp.saturating_add(secs),
            });
            let payer = me.payer.clone();
            me.ctx.program(me.program_id)
                .call(instruction::LendingPoolAccrueBankInterest {})
                .accounts(accounts::LendingPoolAccrueBankInterest { group: me.marginfi_group, bank })
                .signers(&[&*payer]).send().map(|o| o.is_success()).unwrap_or(false)
        };
        if !accrue(self, 63_072_000) { return false; }
        let asv0 = match self.scout_bank_i80f48(bank, 80) { Some(v) => v, None => return false };
        let lsv0 = match self.scout_bank_i80f48(bank, 96) { Some(v) => v, None => return false };
        let tls = match self.scout_bank_i80f48(bank, 256) { Some(v) => v, None => return false };
        let tas = match self.scout_bank_i80f48(bank, 272) { Some(v) => v, None => return false };
        let ins0 = match self.scout_bank_i80f48(bank, 184) { Some(v) => v, None => return false };
        let grp0 = match self.scout_bank_i80f48(bank, 240) { Some(v) => v, None => return false };
        let prg0 = match self.scout_bank_i80f48(bank, 904) { Some(v) => v, None => return false };
        let asset0 = match tas.checked_mul(asv0) { Some(v) => v, None => return false };
        let liab0 = match tls.checked_mul(lsv0) { Some(v) => v, None => return false };
        let ur = if asset0 > fixed::types::I80F48::ZERO { liab0 / asset0 } else { fixed::types::I80F48::ZERO };
        if !accrue(self, 31_536_000) { return false; }
        let asv1 = match self.scout_bank_i80f48(bank, 80) { Some(v) => v, None => return false };
        let lsv1 = match self.scout_bank_i80f48(bank, 96) { Some(v) => v, None => return false };
        let ins1 = match self.scout_bank_i80f48(bank, 184) { Some(v) => v, None => return false };
        let grp1 = match self.scout_bank_i80f48(bank, 240) { Some(v) => v, None => return false };
        let prg1 = match self.scout_bank_i80f48(bank, 904) { Some(v) => v, None => return false };
        let asset1 = match tas.checked_mul(asv1) { Some(v) => v, None => return false };
        let liab1 = match tls.checked_mul(lsv1) { Some(v) => v, None => return false };
        let d_asset = asset1 - asset0;
        let d_liab = liab1 - liab0;
        let d_fees = (ins1 - ins0) + (grp1 - grp0) + (prg1 - prg0);
        let residual = d_liab - (d_asset + d_fees);
        if d_liab <= fixed::types::I80F48::from_num(1) {
            return false;
        }
        let dust = fixed::types::I80F48::from_num(4);
        scout_check!(
            "P-UR-OVERONE",
            "interest-accrual-conserves-value-even-above-100pct-utilization",
            residual.abs() <= dust,
            "P-UR-OVERONE: at utilization {} borrowers charged {} but credited {} (residual {})",
            ur,
            d_liab,
            d_asset + d_fees,
            residual
        );
        residual.abs() > dust
    }

    pub fn action_compound_permissionless_baddebt_probe(&mut self) -> bool {
        let (coll_bank, liab_bank, _provider, bankrupt, sorted_pair, lsv, asv0, tas, bad_shares, bad_debt) = match self.scout_build_bankruptcy_scenario(true) { Some(v) => v, None => return false };
        let attacker = std::rc::Rc::new(Keypair::new());
        if self.ctx.create_account().pubkey(attacker.pubkey()).lamports(1_000_000_000).owner(system_program::ID).create().is_err() {
            return false;
        }
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let ins_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let ins_vault_auth = Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, liab_bank.as_ref()], &self.program_id).0;
        let settled = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolHandleBankruptcy {})
            .accounts(accounts::LendingPoolHandleBankruptcy {
                group: self.marginfi_group,
                signer: attacker.pubkey(),
                bank: liab_bank,
                marginfi_account: bankrupt,
                liquidity_vault,
                insurance_vault: ins_vault,
                insurance_vault_authority: ins_vault_auth,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![sorted_pair[0], sorted_pair[1]])
            .signers(&[&attacker])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        let asv1 = match self.scout_bank_i80f48(liab_bank, 80) { Some(v) => v, None => return false };
        let depositor_loss = match asv0.checked_sub(asv1).and_then(|d| d.checked_mul(tas)) { Some(v) => v, None => return false };
        let dust = fixed::types::I80F48::from_num(4);
        scout_check!(
            "P-COMPOUND-BADDEBT",
            "an-unprivileged-signer-must-not-socialise-a-loss-onto-depositors",
            !(settled && depositor_loss > dust),
            "P-COMPOUND-BADDEBT: unprivileged signer socialised {} of loss for {} of bad debt",
            depositor_loss,
            bad_debt
        );
        settled && depositor_loss > dust
    }

    fn scout_token_amount(&self, token_account: Pubkey) -> u64 {
        self.ctx
            .read_account(&token_account)
            .ok()
            .and_then(|a| {
                a.data.get(64..72).map(|b| {
                    let mut x = [0u8; 8];
                    x.copy_from_slice(b);
                    u64::from_le_bytes(x)
                })
            })
            .unwrap_or(0)
    }

    fn scout_add_pyth_bank(&mut self, config: marginfi::types::BankConfigCompact, price_usd: i64, confidence: u64) -> Option<(Pubkey, Pubkey)> {
        use ::anchor_lang::prelude::Clock;
        let now_ts = self.ctx.svm.get_sysvar::<Clock>().unix_timestamp;
        let oracle = MockPythOracleBuilder::new(&mut self.ctx)
            .price(price_usd.saturating_mul(100_000_000))
            .exponent(-8)
            .confidence(confidence)
            .publish_time(now_ts)
            .build()
            .ok()?;
        let receiver: Pubkey = "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ".parse().ok()?;
        let mut oracle_acct = self.ctx.svm.get_account(&oracle)?;
        oracle_acct.owner = receiver;
        self.ctx.svm.set_account(oracle, oracle_acct).ok()?;
        let bank = self.scout_liquidate_add_bank(config)?;
        let ok = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankOracle { setup: 3, oracle })
            .accounts(accounts::LendingPoolConfigureBankOracle {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank,
            })
            .remaining_accounts(vec![oracle])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !ok {
            return None;
        }
        Some((bank, oracle))
    }

    #[allow(clippy::type_complexity)]
    fn scout_pyth_borrow_scenario(
        &mut self,
        confidence: u64,
    ) -> Option<(Pubkey, Pubkey, Pubkey, Pubkey, Rc<Keypair>, Pubkey, bool)> {
        let mut coll_cfg = scout_liquidation_bank_config();
        coll_cfg.asset_weight_init = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.5));
        coll_cfg.asset_weight_maint = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.6));
        let (pyth_bank, oracle) = self.scout_add_pyth_bank(coll_cfg, 10, confidence)?;
        let liab_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if pyth_bank == liab_bank || !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let provider = self.scout_create_initialized_marginfi_account()?;
        let borrower = self.scout_create_initialized_marginfi_account()?;
        if !self.scout_liquidate_deposit(provider, liab_bank, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT) {
            return None;
        }
        let payer = self.payer.clone();
        let coll_liq_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, pyth_bank.as_ref()], &self.program_id).0;
        let dep_ok = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit { amount: SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT, deposit_up_to_limit: None })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account: borrower,
                authority: payer.pubkey(),
                bank: pyth_bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault: coll_liq_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![oracle])
            .signers(&[&*payer])
            .send().map(|o| o.is_success()).unwrap_or(false);
        if !dep_ok {
            return None;
        }
        let liab_liq_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let liab_vault_auth = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, liab_bank.as_ref()], &self.program_id).0;
        let borrow_ok = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountBorrow { amount: SCOUT_P33_BORROW_AMOUNT })
            .accounts(accounts::LendingAccountBorrow {
                group: self.marginfi_group,
                marginfi_account: borrower,
                authority: payer.pubkey(),
                bank: liab_bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority: liab_vault_auth,
                liquidity_vault: liab_liq_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![pyth_bank, oracle, liab_bank])
            .signers(&[&*payer])
            .send().map(|o| o.is_success()).unwrap_or(false);
        Some((pyth_bank, oracle, liab_bank, borrower, payer, coll_liq_vault, borrow_ok))
    }

    pub fn action_pyth_staleness_probe(&mut self) -> bool {
        let (pyth_bank, oracle, liab_bank, borrower, payer, coll_liq_vault, borrow_ok) = match self.scout_pyth_borrow_scenario(1) { Some(v) => v, None => return false };
        if !borrow_ok {
            return false;
        }
        use ::anchor_lang::prelude::Clock;
        let clock = self.ctx.svm.get_sysvar::<Clock>();
        self.ctx.set_sysvar(&Clock {
            slot: clock.slot.saturating_add(500),
            epoch_start_timestamp: clock.epoch_start_timestamp,
            epoch: clock.epoch,
            leader_schedule_epoch: clock.leader_schedule_epoch,
            unix_timestamp: clock.unix_timestamp.saturating_add(600),
        });
        let coll_vault_auth = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, pyth_bank.as_ref()], &self.program_id).0;
        let withdraw_ok = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountWithdraw { amount: 1_000, withdraw_all: None })
            .accounts(accounts::LendingAccountWithdraw {
                group: self.marginfi_group,
                marginfi_account: borrower,
                authority: payer.pubkey(),
                bank: pyth_bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority: coll_vault_auth,
                liquidity_vault: coll_liq_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![pyth_bank, oracle, liab_bank])
            .signers(&[&*payer])
            .send().map(|o| o.is_success()).unwrap_or(false);
        scout_check!(
            "P-PYTH-STALE",
            "a-stale-pyth-oracle-must-not-value-collateral-for-a-health-checked-op",
            !withdraw_ok,
            "P-PYTH-STALE: withdraw valued collateral via a {}s-stale Pyth oracle",
            600
        );
        withdraw_ok
    }

    pub fn action_pyth_confidence_probe(&mut self) -> bool {
        let (_pyth_bank, _oracle, _liab_bank, _borrower, _payer, _coll_liq_vault, borrow_ok) = match self.scout_pyth_borrow_scenario(200_000_000) { Some(v) => v, None => return false };
        scout_check!(
            "P-PYTH-CONF",
            "a-borrow-against-collateral-with-excessive-oracle-confidence-must-be-rejected",
            !borrow_ok,
            "P-PYTH-CONF: borrow succeeded against collateral with 20% oracle confidence (> 10% max)"
        );
        borrow_ok
    }

    fn scout_add_t22_bank(
        &mut self,
        config: marginfi::types::BankConfigCompact,
        mint: Pubkey,
    ) -> Option<Pubkey> {
        let token_2022_id: Pubkey = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse().ok()?;
        let bank_keypair = Keypair::new();
        let bank = bank_keypair.pubkey();
        let (
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank);
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        if !(self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBank { bank_config: config })
            .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, token_2022_id))
            .signers(&[&*self.payer, &bank_keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false))
        {
            return None;
        }
        Some(bank)
    }

    fn scout_t22_deposit(&mut self, account: Pubkey, bank: Pubkey, mint: Pubkey, amount: u64, signer_ta: Pubkey) -> bool {
        let token_2022_id: Pubkey = match "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse() { Ok(v) => v, Err(_) => return false };
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let payer = self.payer.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit { amount, deposit_up_to_limit: None })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account: account,
                authority: payer.pubkey(),
                bank,
                signer_token_account: signer_ta,
                liquidity_vault,
                token_program: token_2022_id,
            })
            .remaining_accounts(vec![mint])
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_t22_borrow(&mut self, account: Pubkey, bank: Pubkey, mint: Pubkey, amount: u64, dest_ta: Pubkey, health_banks: Vec<Pubkey>) -> bool {
        let token_2022_id: Pubkey = match "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse() { Ok(v) => v, Err(_) => return false };
        let bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let mut ra = vec![mint];
        ra.extend(health_banks);
        let payer = self.payer.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountBorrow { amount })
            .accounts(accounts::LendingAccountBorrow {
                group: self.marginfi_group,
                marginfi_account: account,
                authority: payer.pubkey(),
                bank,
                destination_token_account: dest_ta,
                bank_liquidity_vault_authority,
                liquidity_vault,
                token_program: token_2022_id,
            })
            .remaining_accounts(ra)
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_t22_liquidate(&mut self, asset_bank: Pubkey, liab_bank: Pubkey, liab_mint: Pubkey, liquidator: Pubkey, liquidatee: Pubkey, sorted_pair: [Pubkey; 2], asset_amount: u64) -> bool {
        let token_2022_id: Pubkey = match "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse() { Ok(v) => v, Err(_) => return false };
        let bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, liab_bank.as_ref()], &self.program_id).0;
        let bank_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let bank_insurance_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let payer = self.payer.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountLiquidate {
                asset_amount,
                liquidatee_accounts: 2,
                liquidator_accounts: 2,
            })
            .accounts(accounts::LendingAccountLiquidate {
                group: self.marginfi_group,
                asset_bank,
                liab_bank,
                liquidator_marginfi_account: liquidator,
                authority: payer.pubkey(),
                liquidatee_marginfi_account: liquidatee,
                bank_liquidity_vault_authority,
                bank_liquidity_vault,
                bank_insurance_vault,
                token_program: token_2022_id,
            })
            .remaining_accounts(vec![liab_mint, sorted_pair[0], sorted_pair[1], sorted_pair[0], sorted_pair[1]])
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_liquidate_set_fixed_price(&mut self, bank: Pubkey, price: fixed::types::I80F48) -> bool {
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolSetFixedOraclePrice {
                    price: marginfi::types::WrappedI80F48::from_i80f48(price),
                })
                .accounts(accounts::LendingPoolSetFixedOraclePrice {
                    group: self.marginfi_group,
                    admin: self.payer.pubkey(),
                    bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return false;
        }
        true
    }

    fn scout_liquidate_deposit(&mut self, marginfi_account: Pubkey, bank: Pubkey, amount: u64) -> bool {
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountDeposit { amount, deposit_up_to_limit: None })
                .accounts(accounts::LendingAccountDeposit {
                    group: self.marginfi_group,
                    marginfi_account,
                    authority: self.payer.pubkey(),
                    bank,
                    signer_token_account: self.signer_token_account,
                    liquidity_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return false;
        }
        true
    }

    fn scout_liquidate_borrow(
        &mut self,
        marginfi_account: Pubkey,
        bank: Pubkey,
        amount: u64,
        remaining_accounts: Vec<Pubkey>,
    ) -> bool {
        let bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountBorrow { amount })
                .accounts(accounts::LendingAccountBorrow {
                    group: self.marginfi_group,
                    marginfi_account,
                    authority: self.payer.pubkey(),
                    bank,
                    destination_token_account: self.signer_token_account,
                    bank_liquidity_vault_authority,
                    liquidity_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(remaining_accounts)
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return false;
        }
        true
    }

    pub fn action_lending_account_liquidate_reachable(&mut self) -> bool {
        let asset_bank = match self.scout_liquidate_add_bank(scout_liquidation_bank_config()) { Some(v) => v, None => return false };
        let liab_bank = match self.scout_liquidate_add_bank(scout_valid_bank_config(10)) { Some(v) => v, None => return false };
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return false;
        }

        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return false;
        }

        let liquidity_provider = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let liquidator = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let liquidatee = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };

        if !self.scout_liquidate_deposit(liquidity_provider, liab_bank, SCOUT_LIQUIDATE_LIQUIDITY_DEPOSIT_AMOUNT) {
            return false;
        }
        if !self.scout_liquidate_deposit(liquidatee, asset_bank, SCOUT_LIQUIDATE_COLLATERAL_DEPOSIT_AMOUNT) {
            return false;
        }
        if !self.scout_liquidate_deposit(liquidator, asset_bank, SCOUT_LIQUIDATE_COLLATERAL_DEPOSIT_AMOUNT) {
            return false;
        }

        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(
            liquidatee,
            liab_bank,
            SCOUT_LIQUIDATE_BORROW_AMOUNT,
            sorted_pair.to_vec(),
        ) {
            return false;
        }

        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(0.1)) {
            return false;
        }

        let bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, liab_bank.as_ref()], &self.program_id).0;
        let bank_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let bank_insurance_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let remaining_accounts = vec![sorted_pair[0], sorted_pair[1], sorted_pair[0], sorted_pair[1]];
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountLiquidate {
                asset_amount: SCOUT_LIQUIDATE_ASSET_AMOUNT,
                liquidatee_accounts: 2,
                liquidator_accounts: 2,
            })
            .accounts(accounts::LendingAccountLiquidate {
                group: self.marginfi_group,
                asset_bank,
                liab_bank,
                liquidator_marginfi_account: liquidator,
                authority: self.payer.pubkey(),
                liquidatee_marginfi_account: liquidatee,
                bank_liquidity_vault_authority,
                bank_liquidity_vault,
                bank_insurance_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(remaining_accounts)
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_liquidate_scenario_refresh(&mut self) -> bool {
        let asset_bank = match self.scout_liquidate_add_bank(scout_liquidation_bank_config()) {
            Some(v) => v,
            None => return false,
        };
        let liab_bank = match self.scout_liquidate_add_bank(scout_valid_bank_config(10)) {
            Some(v) => v,
            None => return false,
        };
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return false;
        }

        let liquidity_provider = match self.scout_create_initialized_marginfi_account() {
            Some(v) => v,
            None => return false,
        };
        let liquidator = match self.scout_create_initialized_marginfi_account() {
            Some(v) => v,
            None => return false,
        };
        let liquidatee = match self.scout_create_initialized_marginfi_account() {
            Some(v) => v,
            None => return false,
        };

        if !self.scout_liquidate_deposit(
            liquidity_provider,
            liab_bank,
            SCOUT_LIQUIDATE_LIQUIDITY_DEPOSIT_AMOUNT,
        ) {
            return false;
        }
        if !self.scout_liquidate_deposit(
            liquidatee,
            asset_bank,
            SCOUT_LIQUIDATE_COLLATERAL_DEPOSIT_AMOUNT,
        ) {
            return false;
        }
        if !self.scout_liquidate_deposit(
            liquidator,
            asset_bank,
            SCOUT_LIQUIDATE_COLLATERAL_DEPOSIT_AMOUNT,
        ) {
            return false;
        }

        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(
            liquidatee,
            liab_bank,
            SCOUT_LIQUIDATE_BORROW_AMOUNT,
            sorted_pair.to_vec(),
        ) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(0.1)) {
            return false;
        }

        self.scout_liq_asset_bank = asset_bank;
        self.scout_liq_liab_bank = liab_bank;
        self.scout_liq_liquidator = liquidator;
        self.scout_liq_liquidatee = liquidatee;
        self.scout_liq_remaining =
            vec![sorted_pair[0], sorted_pair[1], sorted_pair[0], sorted_pair[1]];
        true
    }

    pub fn action_lending_account_start_flashloan_with_matching_end(&mut self) -> bool {
        let (marginfi_account, asset_bank, liab_bank) = match self.scout_flashloan_prepare() {
            Some(v) => v,
            None => return false,
        };
        let authority = self.payer.pubkey();
        let payer = self.payer.clone();
        let start_ix = scout_lending_account_start_flashloan_ix(
            self.program_id,
            marginfi_account,
            authority,
            1,
        );
        let mut end_ix = scout_lending_account_end_flashloan_ix(
            self.program_id,
            marginfi_account,
            self.marginfi_group,
            authority,
        );
        for bank in [asset_bank, liab_bank] {
            end_ix.accounts.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(bank, false),
            );
        }
        if self.ctx.raw_call(start_ix).signers(&[&*payer]).add_transaction().is_err() {
            return false;
        }
        if self.ctx.raw_call(end_ix).signers(&[&*payer]).add_transaction().is_err() {
            return false;
        }
        self.ctx
            .send_batch()
            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
            .unwrap_or(false)
    }

    fn scout_build_liquidation_bracket_scenario(&mut self) -> Option<(Pubkey, Pubkey, Pubkey, [Pubkey; 2])> {
        let asset_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let liab_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let liquidity_provider = self.scout_create_initialized_marginfi_account()?;
        let target_account = self.scout_create_initialized_marginfi_account()?;
        if !self.scout_liquidate_deposit(liquidity_provider, liab_bank, SCOUT_LIQUIDATE_LIQUIDITY_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(target_account, asset_bank, SCOUT_LIQUIDATE_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(
            target_account,
            liab_bank,
            SCOUT_LIQUIDATE_BORROW_AMOUNT,
            sorted_pair.to_vec(),
        ) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(0.1)) {
            return None;
        }
        Some((asset_bank, liab_bank, target_account, sorted_pair))
    }

    fn scout_build_liquidation_bracket_ixs(
        &mut self,
        target_account: Pubkey,
        liquidation_record: Pubkey,
        sorted_pair: [Pubkey; 2],
    ) -> (anchor_lang::solana_program::instruction::Instruction, anchor_lang::solana_program::instruction::Instruction) {
        let liquidation_receiver = self.payer.pubkey();
        let remaining_accounts = vec![sorted_pair[0], sorted_pair[1]];
        let mut start_ix = scout_start_liquidation_ix(
            self.program_id,
            target_account,
            liquidation_record,
            self.marginfi_group,
            liquidation_receiver,
        );
        start_ix.accounts.extend(remaining_accounts.iter().map(|k| {
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)
        }));
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let mut end_ix = scout_end_liquidation_ix(
            self.program_id,
            target_account,
            liquidation_record,
            self.marginfi_group,
            liquidation_receiver,
            fee_state,
            self.global_fee_wallet,
            self.payer.pubkey(),
        );
        end_ix.accounts.extend(remaining_accounts.iter().map(|k| {
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)
        }));
        (start_ix, end_ix)
    }

    pub fn action_start_liquidation_with_matching_end(&mut self) -> bool {
        let (asset_bank, _liab_bank, target_account, sorted_pair) = match self.scout_build_liquidation_bracket_scenario() { Some(v) => v, None => return false };

        let liquidation_record = match self.scout_init_liquidation_record(target_account) { Some(v) => v, None => return false };

        let (start_ix, end_ix) = self.scout_build_liquidation_bracket_ixs(target_account, liquidation_record, sorted_pair);

        self.ctx.raw_call(start_ix).signers(&[&*self.payer]).add_transaction().unwrap();
        self.ctx.raw_call(end_ix).signers(&[&*self.payer]).add_transaction().unwrap();
        self.ctx
            .send_batch()
            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
            .unwrap_or(false)
    }

    pub fn action_noop_liquidation_bracket_probe(&mut self) -> bool {
        let (asset_bank, _liab_bank, target_account, sorted_pair) = match self.scout_build_liquidation_bracket_scenario() { Some(v) => v, None => return false };
        let liquidation_record = match self.scout_init_liquidation_record(target_account) { Some(v) => v, None => return false };

        let lending_end = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + SCOUT_MARGINFI_ACCOUNT_LENDING_LEN;
        let snapshot_balances = |f: &MarginfiFixture| -> Option<Vec<u8>> {
            f.ctx.read_account(&target_account).ok().and_then(|a| {
                let d = a.data;
                if d.len() >= lending_end {
                    Some(d[SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET..lending_end].to_vec())
                } else {
                    None
                }
            })
        };
        let pre_balances = snapshot_balances(self);

        let (start_ix, end_ix) = self.scout_build_liquidation_bracket_ixs(target_account, liquidation_record, sorted_pair);

        if self.ctx.raw_call(start_ix).signers(&[&*self.payer]).add_transaction().is_err() {
            return false;
        }
        if self.ctx.raw_call(end_ix).signers(&[&*self.payer]).add_transaction().is_err() {
            return false;
        }
        let bracket_ok = self.ctx
            .send_batch()
            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
            .unwrap_or(false);

        if bracket_ok {
            let post_balances = snapshot_balances(self);
            if let (Some(pre), Some(post)) = (pre_balances, post_balances) {
                scout_check!(
                    "P-0041",
                    "no-op-liquidation-bracket-rejected-like-one-shot",
                    pre != post,
                    "P-0041: no-op liquidation bracket succeeded on {} with balances unchanged",
                    target_account
                );
            }
        }
        bracket_ok
    }

    #[cfg(feature = "admin_actions")]
    pub fn action_emode_weight_above_one_scenario(&mut self, weight_seed: u8) -> bool {
        let mut cfg = scout_valid_bank_config(10);
        let two = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(2));
        cfg.liability_weight_init = two;
        cfg.liability_weight_maint = two;
        let bank = match self.scout_liquidate_add_bank(cfg) { Some(v) => v, None => return false };

        let raw_weight = fixed::types::I80F48::from_num(weight_seed % 180)
            .checked_div(fixed::types::I80F48::from_num(100))
            .unwrap_or(fixed::types::I80F48::ZERO);
        let w = marginfi::types::WrappedI80F48::from_i80f48(raw_weight);
        let mut entries: [marginfi::types::EmodeEntry; MAX_EMODE_ENTRIES] = Default::default();
        entries[0] = marginfi::types::EmodeEntry {
            collateral_bank_emode_tag: 1,
            flags: 0,
            pad0: [0u8; 5],
            asset_weight_init: w,
            asset_weight_maint: w,
        };
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankEmode { emode_tag: 1, entries })
            .accounts(accounts::LendingPoolConfigureBankEmode {
                group: self.marginfi_group,
                emode_admin: self.payer.pubkey(),
                bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    #[cfg(not(feature = "admin_actions"))]
    pub fn action_emode_weight_above_one_scenario(&mut self, _weight_seed: u8) -> bool {
        false
    }

    pub fn action_frozen_account_bracket_probe(&mut self) -> bool {
        let (asset_bank, _liab_bank, target_account, sorted_pair) = match self.scout_build_liquidation_bracket_scenario() { Some(v) => v, None => return false };
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::MarginfiAccountSetFreeze { frozen: true })
                .accounts(accounts::MarginfiAccountSetFreeze {
                    group: self.marginfi_group,
                    marginfi_account: target_account,
                    admin: self.payer.pubkey(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return false;
        }

        let liquidation_record = match self.scout_init_liquidation_record(target_account) { Some(v) => v, None => return false };

        let (start_ix, end_ix) = self.scout_build_liquidation_bracket_ixs(target_account, liquidation_record, sorted_pair);

        if self.ctx.raw_call(start_ix).signers(&[&*self.payer]).add_transaction().is_err() {
            return false;
        }
        if self.ctx.raw_call(end_ix).signers(&[&*self.payer]).add_transaction().is_err() {
            return false;
        }
        let bracket_ok = self.ctx
            .send_batch()
            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
            .unwrap_or(false);

        scout_check!(
            "P-0041",
            "frozen-account-rejected-by-liquidation-bracket",
            !bracket_ok,
            "P-0041: liquidation bracket succeeded on FROZEN account {}",
            target_account
        );
        bracket_ok
    }

    pub fn action_interest_split_identity_probe(&mut self) -> bool {
        const OFF_ASV: usize = 80;
        const OFF_LSV: usize = 96;
        const OFF_INS: usize = 184;
        const OFF_GRP: usize = 240;
        const OFF_TLS: usize = 256;
        const OFF_TAS: usize = 272;
        const OFF_PRG: usize = 904;
        let read_i80 = |data: &[u8], off: usize| -> Option<fixed::types::I80F48> {
            data.get(off..off + 16)
                .and_then(|s| s.try_into().ok())
                .map(fixed::types::I80F48::from_le_bytes)
        };

        let collateral_bank =
            match self.scout_liquidate_add_bank(scout_liquidation_bank_config()) { Some(v) => v, None => return false };
        let rate_bank = match self.scout_liquidate_add_bank(scout_valid_bank_config(10)) { Some(v) => v, None => return false };
        if collateral_bank == rate_bank || !self.scout_liquidate_raise_liab_bank_limits(rate_bank) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(collateral_bank, fixed::types::I80F48::from_num(SCOUT_PIR_COLLATERAL_PRICE)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(rate_bank, fixed::types::I80F48::ONE) {
            return false;
        }
        let lender = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let borrower = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if lender == borrower {
            return false;
        }
        if !self.scout_liquidate_deposit(lender, rate_bank, SCOUT_PIR_LIQUIDITY_AMOUNT) {
            return false;
        }
        if !self.scout_liquidate_deposit(borrower, collateral_bank, SCOUT_PIR_COLLATERAL_AMOUNT) {
            return false;
        }
        let sorted_pair = if collateral_bank.to_bytes() > rate_bank.to_bytes() {
            [collateral_bank, rate_bank]
        } else {
            [rate_bank, collateral_bank]
        };
        if !self.scout_liquidate_borrow(borrower, rate_bank, SCOUT_PIR_BORROW_AMOUNT, sorted_pair.to_vec()) {
            return false;
        }
        if !self.scout_pir_install_curve(rate_bank) {
            return false;
        }
        if !self.scout_pir_accrue(rate_bank) {
            return false;
        }

        let pre = match self.ctx.read_account(&rate_bank) { Ok(a) => a.data, Err(_) => return false };
        if pre.len() < OFF_PRG + 16 {
            return false;
        }
        let asv_pre = read_i80(&pre, OFF_ASV);
        let lsv_pre = read_i80(&pre, OFF_LSV);
        let tas_pre = read_i80(&pre, OFF_TAS);
        let tls_pre = read_i80(&pre, OFF_TLS);
        let ins_pre = read_i80(&pre, OFF_INS);
        let grp_pre = read_i80(&pre, OFF_GRP);
        let prg_pre = read_i80(&pre, OFF_PRG);

        self.scout_pir_advance_clock(SCOUT_P7_WARP_SECONDS);
        if !self.scout_pir_accrue(rate_bank) {
            return false;
        }

        let post = match self.ctx.read_account(&rate_bank) { Ok(a) => a.data, Err(_) => return false };
        if post.len() < OFF_PRG + 16 {
            return false;
        }
        let (
            Some(asv_pre), Some(lsv_pre), Some(tas_pre), Some(tls_pre),
            Some(ins_pre), Some(grp_pre), Some(prg_pre),
            Some(asv_post), Some(lsv_post), Some(ins_post), Some(grp_post), Some(prg_post),
        ) = (
            asv_pre, lsv_pre, tas_pre, tls_pre, ins_pre, grp_pre, prg_pre,
            read_i80(&post, OFF_ASV), read_i80(&post, OFF_LSV),
            read_i80(&post, OFF_INS), read_i80(&post, OFF_GRP), read_i80(&post, OFF_PRG),
        ) else {
            return false;
        };

        let lender_credit = match asv_post.checked_sub(asv_pre).and_then(|d| d.checked_mul(tas_pre)) {
            Some(v) => v,
            None => return false,
        };
        let borrower_charge = match lsv_post.checked_sub(lsv_pre).and_then(|d| d.checked_mul(tls_pre)) {
            Some(v) => v,
            None => return false,
        };
        let fee_delta = (ins_post - ins_pre) + (grp_post - grp_pre) + (prg_post - prg_pre);

        if lender_credit == fixed::types::I80F48::ZERO && borrower_charge == fixed::types::I80F48::ZERO {
            return true;
        }

        let user_side = lender_credit + fee_delta;
        let tolerance = fixed::types::I80F48::from_num(16);
        scout_check!(
            "P-0044",
            "interest-charged-covers-credited-plus-fees",
            user_side <= borrower_charge + tolerance,
            "P-0044: bank {} accrual credited {} + fees {} = {} but charged borrowers {} (gap {})",
            rate_bank,
            lender_credit,
            fee_delta,
            user_side,
            borrower_charge,
            user_side - borrower_charge
        );
        true
    }

    fn scout_p18_scenario_liquidatable(
        &mut self,
        collateral_config: marginfi::types::BankConfigCompact,
    ) -> Option<bool> {
        let coll_bank = self.scout_liquidate_add_bank(collateral_config)?;
        let debt_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if coll_bank == debt_bank || !self.scout_liquidate_raise_liab_bank_limits(debt_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(10)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(debt_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let provider = self.scout_create_initialized_marginfi_account()?;
        let liquidator = self.scout_create_initialized_marginfi_account()?;
        let liquidatee = self.scout_create_initialized_marginfi_account()?;
        if liquidator == liquidatee {
            return None;
        }
        if !self.scout_liquidate_deposit(provider, debt_bank, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidatee, coll_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidator, coll_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidator, debt_bank, SCOUT_P33_LIQUIDATOR_LIAB_DEPOSIT_AMOUNT) {
            return None;
        }
        let sorted_pair = if coll_bank.to_bytes() > debt_bank.to_bytes() {
            [coll_bank, debt_bank]
        } else {
            [debt_bank, coll_bank]
        };
        if !self.scout_liquidate_borrow(liquidatee, debt_bank, SCOUT_P33_BORROW_AMOUNT, sorted_pair.to_vec()) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(SCOUT_P33_CRASHED_PRICE)) {
            return None;
        }
        let collateral = SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT;
        let candidates = [
            collateral / 16,
            collateral / 8,
            collateral / 4,
            collateral / 2,
            (collateral * 3) / 4,
            collateral - 1,
        ];
        for q in candidates {
            if q == 0 {
                continue;
            }
            if self.scout_p33_liquidate(coll_bank, debt_bank, liquidator, liquidatee, sorted_pair, q) {
                return Some(true);
            }
        }
        Some(false)
    }

    pub fn action_maintenance_boundary_unliquidatable_probe(&mut self) -> bool {
        let control_liquidatable = match self.scout_p18_scenario_liquidatable(scout_liquidation_bank_config()) {
            Some(v) => v,
            None => return false,
        };
        let mut boundary_cfg = scout_liquidation_bank_config();
        boundary_cfg.asset_weight_maint =
            marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.96));
        let boundary_liquidatable = match self.scout_p18_scenario_liquidatable(boundary_cfg) {
            Some(v) => v,
            None => return false,
        };

        if control_liquidatable {
            scout_check!(
                "P-0018-ESC",
                "below-maintenance-account-must-admit-a-successful-liquidation",
                boundary_liquidatable,
                "P-0018-ESC: liquidatee at collateral maint weight 0.96 has no successful liquidation while a 0.90 control does"
            );
        }
        control_liquidatable && !boundary_liquidatable
    }

    fn scout_liquidate_with_bystander(
        &mut self,
        coll_bank: Pubkey,
        debt_bank: Pubkey,
        bystander_bank: Pubkey,
        bystander_oracle: Pubkey,
        liquidator: Pubkey,
        liquidatee: Pubkey,
        asset_amount: u64,
    ) -> bool {
        let bank_liquidity_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, debt_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let bank_liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, debt_bank.as_ref()], &self.program_id).0;
        let bank_insurance_vault =
            Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, debt_bank.as_ref()], &self.program_id).0;
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountLiquidate {
                asset_amount,
                liquidatee_accounts: 4,
                liquidator_accounts: 2,
            })
            .accounts(accounts::LendingAccountLiquidate {
                group: self.marginfi_group,
                asset_bank: coll_bank,
                liab_bank: debt_bank,
                liquidator_marginfi_account: liquidator,
                authority: self.payer.pubkey(),
                liquidatee_marginfi_account: liquidatee,
                bank_liquidity_vault_authority,
                bank_liquidity_vault,
                bank_insurance_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![
                coll_bank,
                debt_bank,
                coll_bank,
                debt_bank,
                bystander_bank,
                bystander_oracle,
            ])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_stale_bystander_scenario(&mut self, advance: bool) -> Option<bool> {
        let coll_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let debt_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if coll_bank == debt_bank || !self.scout_liquidate_raise_liab_bank_limits(debt_bank) {
            return None;
        }
        let mut bystander_cfg = scout_liquidation_bank_config();
        bystander_cfg.asset_weight_init =
            marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.5));
        bystander_cfg.asset_weight_maint =
            marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.6));
        let (bystander_bank, bystander_oracle) = self.scout_add_pyth_bank(bystander_cfg, 10, 1)?;
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(10)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(debt_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let provider = self.scout_create_initialized_marginfi_account()?;
        let liquidator = self.scout_create_initialized_marginfi_account()?;
        let liquidatee = self.scout_create_initialized_marginfi_account()?;
        if liquidator == liquidatee {
            return None;
        }
        if !self.scout_liquidate_deposit(provider, debt_bank, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidatee, coll_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidator, coll_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidator, debt_bank, SCOUT_P33_LIQUIDATOR_LIAB_DEPOSIT_AMOUNT) {
            return None;
        }
        let sorted_pair = if coll_bank.to_bytes() > debt_bank.to_bytes() {
            [coll_bank, debt_bank]
        } else {
            [debt_bank, coll_bank]
        };
        if !self.scout_liquidate_borrow(liquidatee, debt_bank, SCOUT_P33_BORROW_AMOUNT, sorted_pair.to_vec()) {
            return None;
        }
        let payer = self.payer.clone();
        let bystander_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bystander_bank.as_ref()], &self.program_id).0;
        let bystander_dep_ok = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit {
                amount: SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT / 100,
                deposit_up_to_limit: None,
            })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: payer.pubkey(),
                bank: bystander_bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault: bystander_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![bystander_oracle])
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !bystander_dep_ok {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(SCOUT_P33_CRASHED_PRICE)) {
            return None;
        }
        if advance {
            use ::anchor_lang::prelude::Clock;
            let clock = self.ctx.svm.get_sysvar::<Clock>();
            self.ctx.set_sysvar(&Clock {
                slot: clock.slot.saturating_add(500),
                epoch_start_timestamp: clock.epoch_start_timestamp,
                epoch: clock.epoch,
                leader_schedule_epoch: clock.leader_schedule_epoch,
                unix_timestamp: clock.unix_timestamp.saturating_add(600),
            });
        }
        let collateral = SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT;
        let candidates = [collateral / 16, collateral / 8, collateral / 4, collateral / 2, (collateral * 3) / 4, collateral - 1];
        for q in candidates {
            if q == 0 {
                continue;
            }
            if self.scout_liquidate_with_bystander(coll_bank, debt_bank, bystander_bank, bystander_oracle, liquidator, liquidatee, q) {
                return Some(true);
            }
        }
        Some(false)
    }

    pub fn action_stale_bystander_unliquidatable_probe(&mut self) -> bool {
        let control = match self.scout_stale_bystander_scenario(false) {
            Some(v) => v,
            None => return false,
        };
        let stale = match self.scout_stale_bystander_scenario(true) {
            Some(v) => v,
            None => return false,
        };
        if control {
            scout_check!(
                "P-STALE-BYSTANDER",
                "below-maintenance-account-must-be-liquidatable-despite-a-stale-bystander-oracle",
                stale,
                "P-STALE-BYSTANDER: below-maintenance account with a stale bystander oracle has no successful liquidation"
            );
        }
        control && !stale
    }

    fn scout_solvency_terms(&mut self, bank: Pubkey) -> Option<(fixed::types::I80F48, fixed::types::I80F48)> {
        use fixed::types::I80F48;
        let data = self.ctx.read_account(&bank).ok()?.data;
        if data.len() < 920 {
            return None;
        }
        let rd = |o: usize| -> I80F48 {
            let b: [u8; 16] = data[o..o + 16].try_into().unwrap_or_default();
            I80F48::from_le_bytes(b)
        };
        let total_asset = rd(8 + 264).checked_mul(rd(8 + 72))?;
        let total_liab = rd(8 + 248).checked_mul(rd(8 + 88))?;
        let fees = rd(184).checked_add(rd(240))?.checked_add(rd(904))?;
        let liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let vault = I80F48::from_num(self.scout_token_amount(liquidity_vault));
        let lhs = vault.checked_add(total_liab)?;
        let rhs = total_asset.checked_add(fees)?;
        Some((lhs, rhs))
    }

    pub fn action_solvency_exact_probe(&mut self) -> bool {
        let coll = match self.scout_liquidate_add_bank(scout_liquidation_bank_config()) { Some(v) => v, None => return false };
        let debt = match self.scout_liquidate_add_bank(scout_valid_bank_config(10)) { Some(v) => v, None => return false };
        if coll == debt || !self.scout_liquidate_raise_liab_bank_limits(debt) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(coll, fixed::types::I80F48::from_num(10)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(debt, fixed::types::I80F48::ONE) {
            return false;
        }
        let provider = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let borrower = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if !self.scout_liquidate_deposit(provider, debt, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT) {
            return false;
        }
        if !self.scout_liquidate_deposit(borrower, coll, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return false;
        }
        let sorted = if coll.to_bytes() > debt.to_bytes() { [coll, debt] } else { [debt, coll] };
        if !self.scout_liquidate_borrow(borrower, debt, SCOUT_P33_BORROW_AMOUNT, sorted.to_vec()) {
            return false;
        }
        let (lhs, rhs) = match self.scout_solvency_terms(debt) { Some(v) => v, None => return false };
        let diff = if lhs >= rhs { lhs - rhs } else { rhs - lhs };
        let holds = diff <= fixed::types::I80F48::from_num(1);
        scout_check!(
            "P-SOLVENCY-EXACT",
            "vault-plus-liabilities-must-EQUAL-assets-plus-fees-both-directions",
            holds,
            "P-SOLVENCY-EXACT: vault+liabilities {} != assets+fees {}",
            lhs, rhs
        );
        !holds
    }

    pub fn action_token22_deposit_conservation_probe(&mut self) -> bool {
        let token_2022_id: Pubkey = match "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let mint = match self.scout_create_t22_fee_mint(200, u64::MAX, 6) { Some(v) => v, None => return false };
        let mut cfg = scout_valid_bank_config(10);
        cfg.deposit_limit = u64::MAX;
        let bank = match self.scout_add_t22_bank(cfg, mint) { Some(v) => v, None => return false };
        if !self.scout_liquidate_set_fixed_price(bank, fixed::types::I80F48::ONE) {
            return false;
        }
        if !self.scout_liquidate_raise_liab_bank_limits(bank) {
            return false;
        }
        let depositor = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let deposit_amount: u64 = 1_000_000;
        let signer_ta = match self.scout_create_t22_token_account(mint, self.payer.pubkey(), deposit_amount.saturating_mul(4)) {
            Some(v) => v,
            None => return false,
        };
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let vault_before = self.scout_token_amount(liquidity_vault);
        let payer = self.payer.clone();
        let ok = self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit { amount: deposit_amount, deposit_up_to_limit: None })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account: depositor,
                authority: payer.pubkey(),
                bank,
                signer_token_account: signer_ta,
                liquidity_vault,
                token_program: token_2022_id,
            })
            .remaining_accounts(vec![mint])
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !ok {
            return false;
        }
        let vault_after = self.scout_token_amount(liquidity_vault);
        let vault_received = vault_after.saturating_sub(vault_before);
        let (asset_shares, _) = match self.scout_p33_shares(depositor, bank) { Some(v) => v, None => return false };
        let (asv, _lsv, _price, scale) = match self.scout_p33_bank_mark(bank) { Some(v) => v, None => return false };
        let recorded = match asset_shares.checked_mul(asv).and_then(|v| v.checked_div(scale)) {
            Some(v) => v,
            None => return false,
        };
        let vault_received_f = match fixed::types::I80F48::from_num(vault_received).checked_div(scale) {
            Some(v) => v,
            None => return false,
        };
        let dust = fixed::types::I80F48::from_num(0.0001);
        scout_check!(
            "P-0001-T22",
            "t22-deposit-must-not-credit-more-value-than-the-vault-received",
            recorded <= vault_received_f + dust,
            "P-0001-T22: T22 deposit of {} credited {} redeemable but vault received only {}",
            deposit_amount,
            recorded,
            vault_received_f
        );
        recorded > vault_received_f + dust
    }

    pub fn action_token22_insurance_fee_leak_probe(&mut self) -> bool {
        let liab_mint = match self.scout_create_t22_fee_mint(200, u64::MAX, 6) { Some(v) => v, None => return false };
        let asset_bank = match self.scout_liquidate_add_bank(scout_liquidation_bank_config()) { Some(v) => v, None => return false };
        let liab_bank = match self.scout_add_t22_bank(scout_valid_bank_config(10), liab_mint) { Some(v) => v, None => return false };
        if asset_bank == liab_bank || !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return false;
        }
        let provider = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let liquidator = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let liquidatee = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if liquidator == liquidatee {
            return false;
        }
        let payer_pk = self.payer.pubkey();
        let provider_ta = match self.scout_create_t22_token_account(liab_mint, payer_pk, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT.saturating_mul(8)) { Some(v) => v, None => return false };
        if !self.scout_t22_deposit(provider, liab_bank, liab_mint, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT, provider_ta) {
            return false;
        }
        if !self.scout_liquidate_deposit(liquidatee, asset_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return false;
        }
        if !self.scout_liquidate_deposit(liquidator, asset_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return false;
        }
        let liquidator_ta = match self.scout_create_t22_token_account(liab_mint, payer_pk, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT.saturating_mul(8)) { Some(v) => v, None => return false };
        if !self.scout_t22_deposit(liquidator, liab_bank, liab_mint, SCOUT_P33_LIQUIDATOR_LIAB_DEPOSIT_AMOUNT, liquidator_ta) {
            return false;
        }
        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        let borrower_ta = match self.scout_create_t22_token_account(liab_mint, payer_pk, 0) { Some(v) => v, None => return false };
        if !self.scout_t22_borrow(liquidatee, liab_bank, liab_mint, SCOUT_P33_BORROW_AMOUNT, borrower_ta, sorted_pair.to_vec()) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(SCOUT_P33_CRASHED_PRICE)) {
            return false;
        }
        let liq_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let ins_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let liq_before = self.scout_token_amount(liq_vault);
        let ins_before = self.scout_token_amount(ins_vault);
        if !self.scout_t22_liquidate(asset_bank, liab_bank, liab_mint, liquidator, liquidatee, sorted_pair, SCOUT_P32_SEIZE_AMOUNT) {
            return false;
        }
        let liq_after = self.scout_token_amount(liq_vault);
        let ins_after = self.scout_token_amount(ins_vault);
        let liq_debit = liq_before.saturating_sub(liq_after);
        let ins_credit = ins_after.saturating_sub(ins_before);
        if liq_debit == 0 {
            return true;
        }
        scout_check!(
            "P-0019-T22",
            "t22-liquidation-insurance-fee-must-not-leak-from-the-liquidity-vault",
            liq_debit <= ins_credit.saturating_add(1),
            "P-0019-T22: T22 liquidation debited vault {} but insurance received only {} (fee {} leaked)",
            liq_debit,
            ins_credit,
            liq_debit.saturating_sub(ins_credit)
        );
        liq_debit > ins_credit.saturating_add(1)
    }

    pub fn action_token22_collect_fees_leak_probe(&mut self) -> bool {
        let token_2022_id: Pubkey = match "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse() { Ok(v) => v, Err(_) => return false };
        let mint = match self.scout_create_t22_fee_mint(200, u64::MAX, 6) { Some(v) => v, None => return false };
        let mut cfg = scout_valid_bank_config(10);
        cfg.deposit_limit = u64::MAX;
        let bank = match self.scout_add_t22_bank(cfg, mint) { Some(v) => v, None => return false };
        if !self.scout_liquidate_set_fixed_price(bank, fixed::types::I80F48::ONE) {
            return false;
        }
        if !self.scout_liquidate_raise_liab_bank_limits(bank) {
            return false;
        }
        let provider = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let payer_pk = self.payer.pubkey();
        let provider_ta = match self.scout_create_t22_token_account(mint, payer_pk, 8_000_000) { Some(v) => v, None => return false };
        if !self.scout_t22_deposit(provider, bank, mint, 2_000_000, provider_ta) {
            return false;
        }
        let inject = fixed::types::I80F48::from_num(100_000i64);
        let mut bank_acct = match self.ctx.svm.get_account(&bank) { Some(v) => v, None => return false };
        if bank_acct.data.len() < 200 {
            return false;
        }
        bank_acct.data[184..200].copy_from_slice(&inject.to_le_bytes());
        if self.ctx.svm.set_account(bank, bank_acct).is_err() {
            return false;
        }
        let liq_vault_pk = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let backed = self.scout_token_amount(liq_vault_pk).saturating_add(100_000);
        let mut vault_acct = match self.ctx.svm.get_account(&liq_vault_pk) { Some(v) => v, None => return false };
        if vault_acct.data.len() < 72 {
            return false;
        }
        vault_acct.data[64..72].copy_from_slice(&backed.to_le_bytes());
        if self.ctx.svm.set_account(liq_vault_pk, vault_acct).is_err() {
            return false;
        }
        let ata_program: Pubkey = match "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL".parse() { Ok(v) => v, Err(_) => return false };
        let fee_ata = Pubkey::find_program_address(&[self.global_fee_wallet.as_ref(), token_2022_id.as_ref(), mint.as_ref()], &ata_program).0;
        if self.scout_write_t22_token_account_at(fee_ata, mint, self.global_fee_wallet, 0).is_none() {
            return false;
        }
        let liq_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let ins_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &self.program_id).0;
        let fee_vault = Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let liq_before = self.scout_token_amount(liq_vault);
        let ins_before = self.scout_token_amount(ins_vault);
        let payer = self.payer.clone();
        let ok = self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolCollectBankFees {})
            .accounts(accounts::LendingPoolCollectBankFees {
                group: self.marginfi_group,
                bank,
                liquidity_vault_authority,
                liquidity_vault: liq_vault,
                insurance_vault: ins_vault,
                fee_vault,
                fee_state,
                fee_ata,
                token_program: token_2022_id,
            })
            .remaining_accounts(vec![mint])
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !ok {
            return false;
        }
        let liq_after = self.scout_token_amount(liq_vault);
        let ins_after = self.scout_token_amount(ins_vault);
        let liq_debit = liq_before.saturating_sub(liq_after);
        let ins_credit = ins_after.saturating_sub(ins_before);
        if liq_debit == 0 {
            return true;
        }
        scout_check!(
            "P-0019B-T22",
            "t22-collect-bank-fees-must-not-leak-from-the-liquidity-vault",
            liq_debit <= ins_credit.saturating_add(1),
            "P-0019B-T22: liquidity vault debited {} but insurance received {} (withheld fee {})",
            liq_debit,
            ins_credit,
            liq_debit.saturating_sub(ins_credit)
        );
        liq_debit > ins_credit.saturating_add(1)
    }

    // P-0002-STALE (REFUTED, kept as guard) -- shared scenario builder for the overseize probe family below.
    fn scout_build_overseize_scenario(&mut self) -> Option<(Pubkey, Pubkey, Pubkey, [Pubkey; 2])> {
        let mut coll_cfg = scout_liquidation_bank_config();
        coll_cfg.asset_weight_init = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.4));
        coll_cfg.asset_weight_maint = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.5));
        let asset_bank = self.scout_liquidate_add_bank(coll_cfg)?;
        let liab_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if asset_bank == liab_bank || !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let provider = self.scout_create_initialized_marginfi_account()?;
        let liquidatee = self.scout_create_initialized_marginfi_account()?;
        if !self.scout_liquidate_deposit(provider, liab_bank, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidatee, asset_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() { [asset_bank, liab_bank] } else { [liab_bank, asset_bank] };
        if !self.scout_liquidate_borrow(liquidatee, liab_bank, SCOUT_P33_BORROW_AMOUNT, sorted_pair.to_vec()) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(SCOUT_P33_CRASHED_PRICE)) {
            return None;
        }
        Some((asset_bank, liab_bank, liquidatee, sorted_pair))
    }

    fn scout_init_liquidation_record(&mut self, liquidatee: Pubkey) -> Option<Pubkey> {
        let liquidation_record = scout_liquidation_record_pda(self.program_id, liquidatee);
        let payer = self.payer.clone();
        if !self.ctx.program(self.program_id)
            .call(instruction::MarginfiAccountInitLiqRecord {})
            .accounts(accounts::MarginfiAccountInitLiqRecord { marginfi_account: liquidatee, fee_payer: payer.pubkey(), liquidation_record })
            .signers(&[&*payer]).send().map(|o| o.is_success()).unwrap_or(false)
        {
            return None;
        }
        self.scout_register_subject_record(liquidation_record);
        Some(liquidation_record)
    }

    #[allow(clippy::type_complexity)]
    fn scout_premeasure_overseize(
        &mut self,
        asset_bank: Pubkey,
        liab_bank: Pubkey,
        liquidatee: Pubkey,
    ) -> Option<(
        (fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48),
        (fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48),
        (fixed::types::I80F48, fixed::types::I80F48),
    )> {
        let pre_asset_mark = self.scout_p33_bank_mark(asset_bank)?;
        let pre_liab_mark = self.scout_p33_bank_mark(liab_bank)?;
        let pre_marks = [(asset_bank, pre_asset_mark), (liab_bank, pre_liab_mark)];
        let pre = self.scout_p33_account_value(liquidatee, &pre_marks)?;
        Some((pre_asset_mark, pre_liab_mark, pre))
    }

    #[allow(clippy::type_complexity)]
    fn scout_postmeasure_overseize(
        &mut self,
        asset_bank: Pubkey,
        liab_bank: Pubkey,
        liquidatee: Pubkey,
        pre_asset_mark: (fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48),
        pre_liab_mark: (fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48),
        pre: (fixed::types::I80F48, fixed::types::I80F48),
        max_fee: fixed::types::I80F48,
    ) -> Option<(fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48)> {
        let post_asset_mark = self.scout_p33_bank_mark(asset_bank)?;
        let post_liab_mark = self.scout_p33_bank_mark(liab_bank)?;
        if pre_asset_mark != post_asset_mark || pre_liab_mark != post_liab_mark {
            return None;
        }
        let post_marks = [(asset_bank, post_asset_mark), (liab_bank, post_liab_mark)];
        let post = self.scout_p33_account_value(liquidatee, &post_marks)?;
        let seized = pre.0.checked_sub(post.0)?;
        let repaid = pre.1.checked_sub(post.1)?;
        let premium_multiplier = (fixed::types::I80F48::ONE + max_fee).max(fixed::types::I80F48::from_num(1.05));
        let fair_ceiling = premium_multiplier.checked_mul(repaid)?;
        Some((seized, repaid, fair_ceiling))
    }

    pub fn action_stale_price_overseize_probe(&mut self) -> bool {
        let (asset_bank, liab_bank, liquidatee, sorted_pair) = match self.scout_build_overseize_scenario() { Some(v) => v, None => return false };
        let liquidation_record = match self.scout_init_liquidation_record(liquidatee) { Some(v) => v, None => return false };
        let payer = self.payer.clone();

        let (coll_pre, _) = match self.scout_p33_shares(liquidatee, asset_bank) { Some(v) => v, None => return false };
        let (_, liab_pre) = match self.scout_p33_shares(liquidatee, liab_bank) { Some(v) => v, None => return false };
        let max_fee = match self.scout_p32_liquidation_max_fee() { Some(v) => v, None => return false };

        let receiver = payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let asset_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, asset_bank.as_ref()], &self.program_id).0;
        let asset_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, asset_bank.as_ref()], &self.program_id).0;
        let liab_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;

        let mut start_ix = scout_start_liquidation_ix(self.program_id, liquidatee, liquidation_record, self.marginfi_group, receiver);
        start_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));
        let price_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingPoolSetFixedOraclePrice { price: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(10)) },
            accounts::LendingPoolSetFixedOraclePrice { group: self.marginfi_group, admin: payer.pubkey(), bank: asset_bank },
        );
        let mut withdraw_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingAccountWithdraw { amount: SCOUT_P32_SEIZE_AMOUNT, withdraw_all: None },
            accounts::LendingAccountWithdraw {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: receiver,
                bank: asset_bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority: asset_vault_authority,
                liquidity_vault: asset_liquidity_vault,
                token_program: spl_token::id(),
            },
        );
        withdraw_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));
        let repay_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingAccountRepay { amount: SCOUT_P20_DELEV_REPAY_AMOUNT, repay_all: None },
            accounts::LendingAccountRepay {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: receiver,
                bank: liab_bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault: liab_liquidity_vault,
                token_program: spl_token::id(),
            },
        );
        let mut end_ix = scout_end_liquidation_ix(self.program_id, liquidatee, liquidation_record, self.marginfi_group, receiver, fee_state, self.global_fee_wallet, self.payer.pubkey());
        end_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));

        for ix in [start_ix, price_ix, withdraw_ix, repay_ix, end_ix] {
            if self.ctx.raw_call(ix).signers(&[&*payer]).add_transaction().is_err() {
                return false;
            }
        }
        if !self.ctx.send_batch().map(|o| o.map(|tx| tx.is_success()).unwrap_or(false)).unwrap_or(false) {
            return false;
        }

        let (coll_post, _) = match self.scout_p33_shares(liquidatee, asset_bank) { Some(v) => v, None => return false };
        let (_, liab_post) = match self.scout_p33_shares(liquidatee, liab_bank) { Some(v) => v, None => return false };
        let asset_mark = match self.scout_p33_bank_mark(asset_bank) { Some(v) => v, None => return false };
        let liab_mark = match self.scout_p33_bank_mark(liab_bank) { Some(v) => v, None => return false };
        let (a_asv, _, a_price, a_scale) = asset_mark;
        let (_, l_lsv, l_price, l_scale) = liab_mark;
        let seized_units = coll_pre - coll_post;
        let repaid_units = liab_pre - liab_post;
        let seized_live = match seized_units.checked_mul(a_asv).and_then(|v| v.checked_mul(a_price)).and_then(|v| v.checked_div(a_scale)) { Some(v) => v, None => return false };
        let repaid_live = match repaid_units.checked_mul(l_lsv).and_then(|v| v.checked_mul(l_price)).and_then(|v| v.checked_div(l_scale)) { Some(v) => v, None => return false };
        if repaid_live <= fixed::types::I80F48::ZERO {
            return true;
        }
        let ceiling = (fixed::types::I80F48::ONE + max_fee).max(fixed::types::I80F48::from_num(1.05));
        let fair = match ceiling.checked_mul(repaid_live) { Some(v) => v, None => return false };
        scout_check!(
            "P-0002-STALE",
            "liquidation-seizure-must-be-capped-at-live-value-not-a-stale-cached-price",
            seized_live <= fair,
            "P-0002-STALE: seizure used stale cached price; seized live value {} for {} repaid (fair ceiling {})",
            seized_live,
            repaid_live,
            fair
        );
        seized_live > fair
    }

    // P-0018-NOREMEDY: an underwater account must admit either liquidation or bankruptcy.
    pub fn action_unliquidatable_no_remedy_probe(&mut self) -> bool {
        let mut boundary_cfg = scout_liquidation_bank_config();
        boundary_cfg.asset_weight_maint =
            marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(0.96));
        let coll_bank = match self.scout_liquidate_add_bank(boundary_cfg) { Some(v) => v, None => return false };
        let debt_bank = match self.scout_liquidate_add_bank(scout_valid_bank_config(10)) { Some(v) => v, None => return false };
        if coll_bank == debt_bank || !self.scout_liquidate_raise_liab_bank_limits(debt_bank) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(10)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(debt_bank, fixed::types::I80F48::ONE) {
            return false;
        }
        let provider = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let liquidator = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let liquidatee = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if liquidator == liquidatee {
            return false;
        }
        if !self.scout_liquidate_deposit(provider, debt_bank, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT) {
            return false;
        }
        if !self.scout_liquidate_deposit(liquidatee, coll_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return false;
        }
        if !self.scout_liquidate_deposit(liquidator, coll_bank, SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT) {
            return false;
        }
        if !self.scout_liquidate_deposit(liquidator, debt_bank, SCOUT_P33_LIQUIDATOR_LIAB_DEPOSIT_AMOUNT) {
            return false;
        }
        let sorted_pair = if coll_bank.to_bytes() > debt_bank.to_bytes() {
            [coll_bank, debt_bank]
        } else {
            [debt_bank, coll_bank]
        };
        if !self.scout_liquidate_borrow(liquidatee, debt_bank, SCOUT_P33_BORROW_AMOUNT, sorted_pair.to_vec()) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(coll_bank, fixed::types::I80F48::from_num(SCOUT_P33_CRASHED_PRICE)) {
            return false;
        }

        let collateral = SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT;
        let candidates = [
            collateral / 16,
            collateral / 8,
            collateral / 4,
            collateral / 2,
            (collateral * 3) / 4,
            collateral - 1,
        ];
        let mut liquidatable = false;
        for q in candidates {
            if q == 0 {
                continue;
            }
            if self.scout_p33_liquidate(coll_bank, debt_bank, liquidator, liquidatee, sorted_pair, q) {
                liquidatable = true;
                break;
            }
        }

        let coll_mark = match self.scout_p33_bank_mark(coll_bank) { Some(v) => v, None => return false };
        let debt_mark = match self.scout_p33_bank_mark(debt_bank) { Some(v) => v, None => return false };
        let (assets, liabilities) = match self.scout_p33_account_value(
            liquidatee,
            &[(coll_bank, coll_mark), (debt_bank, debt_mark)],
        ) {
            Some(v) => v,
            None => return false,
        };
        let bankrupt_threshold = fixed::types::I80F48::from_num(0.1); // BANKRUPT_THRESHOLD
        let zero_threshold = fixed::types::I80F48::from_num(0.0001); // ZERO_AMOUNT_THRESHOLD
        let underwater = assets < liabilities;
        let bankruptable = assets < bankrupt_threshold && liabilities > zero_threshold;

        if underwater {
            scout_check!(
                "P-0018-NOREMEDY",
                "underwater-account-must-admit-liquidation-or-bankruptcy",
                liquidatable || bankruptable,
                "P-0018-NOREMEDY: underwater account (assets {} < liabilities {}) admits neither liquidation nor bankruptcy (assets {} vs threshold; deficit ~{})",
                assets,
                liabilities,
                assets,
                liabilities.saturating_sub(assets)
            );
        }
        underwater && !liquidatable && !bankruptable
    }

    // P-0020-DELEV: a deleverage bracket must not seize more than the fair premium repaid.
    pub fn action_deleverage_premium_cap_bypass_probe(&mut self) -> bool {
        let (asset_bank, liab_bank, liquidatee, sorted_pair) = match self.scout_build_overseize_scenario() { Some(v) => v, None => return false };

        let liquidation_record = match self.scout_init_liquidation_record(liquidatee) { Some(v) => v, None => return false };
        let payer = self.payer.clone();

        let max_fee = match self.scout_p32_liquidation_max_fee() { Some(v) => v, None => return false };
        let (pre_asset_mark, pre_liab_mark, pre) = match self.scout_premeasure_overseize(asset_bank, liab_bank, liquidatee) { Some(v) => v, None => return false };

        let risk_admin = payer.pubkey();
        let asset_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, asset_bank.as_ref()], &self.program_id).0;
        let asset_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, asset_bank.as_ref()], &self.program_id).0;
        let liab_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;

        let mut start_ix = scout_start_deleverage_ix(self.program_id, liquidatee, liquidation_record, self.marginfi_group, risk_admin);
        start_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));
        let mut withdraw_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingAccountWithdraw { amount: SCOUT_P32_SEIZE_AMOUNT, withdraw_all: None },
            accounts::LendingAccountWithdraw {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: risk_admin,
                bank: asset_bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority: asset_vault_authority,
                liquidity_vault: asset_liquidity_vault,
                token_program: spl_token::id(),
            },
        );
        withdraw_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));
        let repay_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingAccountRepay { amount: SCOUT_P20_DELEV_REPAY_AMOUNT, repay_all: None },
            accounts::LendingAccountRepay {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: risk_admin,
                bank: liab_bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault: liab_liquidity_vault,
                token_program: spl_token::id(),
            },
        );
        let mut end_ix = scout_end_deleverage_ix(self.program_id, liquidatee, liquidation_record, self.marginfi_group, risk_admin);
        end_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));

        for ix in [start_ix, withdraw_ix, repay_ix, end_ix] {
            if self.ctx.raw_call(ix).signers(&[&*payer]).add_transaction().is_err() {
                return false;
            }
        }
        let bracket_ok = self.ctx.send_batch().map(|o| o.map(|tx| tx.is_success()).unwrap_or(false)).unwrap_or(false);
        if !bracket_ok {
            return false;
        }

        let (seized, repaid, fair_ceiling) = match self.scout_postmeasure_overseize(asset_bank, liab_bank, liquidatee, pre_asset_mark, pre_liab_mark, pre, max_fee) {
            Some(v) => v,
            None => return false,
        };
        if repaid <= fixed::types::I80F48::ZERO {
            return true;
        }
        scout_check!(
            "P-0020-DELEV",
            "deleverage-seizure-must-not-exceed-the-fair-premium",
            seized <= fair_ceiling,
            "P-0020-DELEV: deleverage bracket seized {} while clearing only {} of debt (fair ceiling {})",
            seized,
            repaid,
            fair_ceiling
        );
        true
    }

    // P-0014-ESC: a permissionless liquidation bracket must not seize more than the fair premium repaid.
    pub fn action_below_closeout_threshold_overseize_probe(&mut self) -> bool {
        let (asset_bank, liab_bank, liquidatee, sorted_pair) = match self.scout_build_overseize_scenario() { Some(v) => v, None => return false };

        let liquidation_record = match self.scout_init_liquidation_record(liquidatee) { Some(v) => v, None => return false };
        let payer = self.payer.clone();

        let max_fee = match self.scout_p32_liquidation_max_fee() { Some(v) => v, None => return false };
        let (pre_asset_mark, pre_liab_mark, pre) = match self.scout_premeasure_overseize(asset_bank, liab_bank, liquidatee) { Some(v) => v, None => return false };

        let receiver = payer.pubkey();
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let asset_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, asset_bank.as_ref()], &self.program_id).0;
        let asset_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, asset_bank.as_ref()], &self.program_id).0;
        let liab_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;

        let mut start_ix = scout_start_liquidation_ix(self.program_id, liquidatee, liquidation_record, self.marginfi_group, receiver);
        start_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));
        let mut withdraw_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingAccountWithdraw { amount: SCOUT_P32_SEIZE_AMOUNT, withdraw_all: None },
            accounts::LendingAccountWithdraw {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: receiver,
                bank: asset_bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority: asset_vault_authority,
                liquidity_vault: asset_liquidity_vault,
                token_program: spl_token::id(),
            },
        );
        withdraw_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));
        let repay_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingAccountRepay { amount: SCOUT_P20_DELEV_REPAY_AMOUNT, repay_all: None },
            accounts::LendingAccountRepay {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: receiver,
                bank: liab_bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault: liab_liquidity_vault,
                token_program: spl_token::id(),
            },
        );
        let mut end_ix = scout_end_liquidation_ix(self.program_id, liquidatee, liquidation_record, self.marginfi_group, receiver, fee_state, self.global_fee_wallet, self.payer.pubkey());
        end_ix.accounts.extend(sorted_pair.iter().map(|k| anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)));

        for ix in [start_ix, withdraw_ix, repay_ix, end_ix] {
            if self.ctx.raw_call(ix).signers(&[&*payer]).add_transaction().is_err() {
                return false;
            }
        }
        if !self.ctx.send_batch().map(|o| o.map(|tx| tx.is_success()).unwrap_or(false)).unwrap_or(false) {
            return false;
        }

        let (seized, repaid, fair_ceiling) = match self.scout_postmeasure_overseize(asset_bank, liab_bank, liquidatee, pre_asset_mark, pre_liab_mark, pre, max_fee) {
            Some(v) => v,
            None => return false,
        };
        if repaid <= fixed::types::I80F48::ZERO {
            return true;
        }
        scout_check!(
            "P-0014-ESC",
            "below-5-liquidation-seizure-must-not-exceed-the-fair-premium",
            seized <= fair_ceiling,
            "P-0014-ESC: permissionless bracket seized {} while clearing only {} of debt (fair ceiling {})",
            seized,
            repaid,
            fair_ceiling
        );
        true
    }

    pub fn action_lending_account_start_flashloan_live_pair(&mut self) -> bool {
        let marginfi_account = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let authority = self.payer.pubkey();
        let payer = self.payer.clone();
        let start_ix = scout_lending_account_start_flashloan_ix(
            self.program_id,
            marginfi_account,
            authority,
            1,
        );
        let end_ix = scout_lending_account_end_flashloan_ix(
            self.program_id,
            marginfi_account,
            self.marginfi_group,
            authority,
        );
        if self.ctx.raw_call(start_ix).signers(&[&*payer]).add_transaction().is_err() {
            return false;
        }
        if self.ctx.raw_call(end_ix).signers(&[&*payer]).add_transaction().is_err() {
            return false;
        }
        self.ctx
            .send_batch()
            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
            .unwrap_or(false)
    }

    // Fresh MarginfiAccount + LiquidationRecord with receivership fields patched by hand.
    fn scout_prepare_end_liquidation_marginfi_account_receivership(&mut self) -> Option<Pubkey> {
        let marginfi_account = self.scout_create_initialized_marginfi_account()?;
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::MarginfiAccountInitLiqRecord {})
                .accounts(accounts::MarginfiAccountInitLiqRecord {
                    marginfi_account,
                    fee_payer: self.payer.pubkey(),
                    liquidation_record,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return None;
        }
        self.scout_register_subject_record(liquidation_record);
        let account_len_needed = MARGINFI_ACCOUNT_FLAGS_OFFSET + 8;
        if !self
            .ctx
            .read_account(&marginfi_account)
            .map(|a| a.data.len() >= account_len_needed)
            .unwrap_or(false)
        {
            return None;
        }
        let record_len_needed = SCOUT_LIQUIDATION_RECORD_RECEIVER_OFFSET
            .max(SCOUT_LIQUIDATION_RECORD_ASSET_EQUITY_OFFSET)
            .max(SCOUT_LIQUIDATION_RECORD_LIABILITY_EQUITY_OFFSET)
            + 32;
        if !self
            .ctx
            .read_account(&liquidation_record)
            .map(|a| a.data.len() >= record_len_needed)
            .unwrap_or(false)
        {
            return None;
        }
        if self
            .ctx
            .update_account(&marginfi_account, |data| {
                let start = MARGINFI_ACCOUNT_FLAGS_OFFSET;
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data[start..start + 8]);
                let flags = u64::from_le_bytes(bytes) | SCOUT_ACCOUNT_IN_RECEIVERSHIP;
                data[start..start + 8].copy_from_slice(&flags.to_le_bytes());
            })
            .is_err()
        {
            return None;
        }
        if !self.scout_p17_harness_flagged.contains(&marginfi_account) {
            self.scout_p17_harness_flagged.push(marginfi_account);
        }
        let receiver = self.payer.pubkey();
        let asset_equity =
            marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(6));
        let liability_equity =
            marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::from_num(1000));
        if self
            .ctx
            .update_account(&liquidation_record, |data| {
                let recv_start = SCOUT_LIQUIDATION_RECORD_RECEIVER_OFFSET;
                data[recv_start..recv_start + 32].copy_from_slice(receiver.as_ref());
                let asset_start = SCOUT_LIQUIDATION_RECORD_ASSET_EQUITY_OFFSET;
                data[asset_start..asset_start + 16].copy_from_slice(&asset_equity.value);
                let liab_start = SCOUT_LIQUIDATION_RECORD_LIABILITY_EQUITY_OFFSET;
                data[liab_start..liab_start + 16].copy_from_slice(&liability_equity.value);
            })
            .is_err()
        {
            return None;
        }
        Some(marginfi_account)
    }

    fn scout_prepare_collect_bank_fees(&mut self) -> Pubkey {
        let fee_ata = scout_associated_token_address(
            &self.global_fee_wallet,
            &self.bank_mint,
            &spl_token::id(),
        );
        if self.ctx.svm.get_account(&fee_ata).is_none() {
            self.ctx
                .create_token_account()
                .pubkey(fee_ata)
                .mint(self.bank_mint)
                .token_owner(self.global_fee_wallet)
                .amount(0)
                .create()
                .unwrap();
        }

        let bank = self.bank;
        self.ctx
            .update_account(&bank, |data| {
                let insurance = fixed::types::I80F48::from_le_bytes(
                    data[SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET
                        ..SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET + 16]
                        .try_into()
                        .unwrap(),
                );
                let floor_insurance =
                    fixed::types::I80F48::from_num(SCOUT_COLLECT_BANK_FEES_INSURANCE_AMOUNT);
                if insurance < floor_insurance {
                    data[SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET
                        ..SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET + 16]
                        .copy_from_slice(&floor_insurance.to_le_bytes());
                }
                let group = fixed::types::I80F48::from_le_bytes(
                    data[SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET
                        ..SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET + 16]
                        .try_into()
                        .unwrap(),
                );
                let floor_group =
                    fixed::types::I80F48::from_num(SCOUT_COLLECT_BANK_FEES_GROUP_AMOUNT);
                if group < floor_group {
                    data[SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET
                        ..SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET + 16]
                        .copy_from_slice(&floor_group.to_le_bytes());
                }
                let program_fees = fixed::types::I80F48::from_le_bytes(
                    data[SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET
                        ..SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET + 16]
                        .try_into()
                        .unwrap(),
                );
                let floor_program =
                    fixed::types::I80F48::from_num(SCOUT_COLLECT_BANK_FEES_PROGRAM_AMOUNT);
                if program_fees < floor_program {
                    data[SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET
                        ..SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET + 16]
                        .copy_from_slice(&floor_program.to_le_bytes());
                }
            })
            .unwrap();

        let liquidity_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, bank.as_ref()],
            &self.program_id,
        )
        .0;
        self.ctx
            .update_account(&liquidity_vault, |data| {
                let amount = u64::from_le_bytes(
                    data[SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET
                        ..SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET + 8]
                        .try_into()
                        .unwrap(),
                );
                if amount < SCOUT_COLLECT_BANK_FEES_LIQUIDITY_AMOUNT {
                    data[SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET
                        ..SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET + 8]
                        .copy_from_slice(&SCOUT_COLLECT_BANK_FEES_LIQUIDITY_AMOUNT.to_le_bytes());
                }
            })
            .unwrap();

        fee_ata
    }

    pub fn scout_lending_account_liquidate_build_scenario(
        &mut self,
    ) -> Option<(Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Vec<Pubkey>)> {
        let asset_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let liab_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(11))?;
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return None;
        }

        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return None;
        }

        let liquidity_provider = self.scout_create_initialized_marginfi_account()?;
        let liquidator = self.scout_create_initialized_marginfi_account()?;
        let liquidatee = self.scout_create_initialized_marginfi_account()?;

        if !self.scout_liquidate_deposit(liquidity_provider, liab_bank, SCOUT_LIQUIDATE_LIQUIDITY_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidatee, asset_bank, SCOUT_LIQUIDATE_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(liquidator, asset_bank, SCOUT_LIQUIDATE_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }

        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(
            liquidatee,
            liab_bank,
            SCOUT_LIQUIDATE_BORROW_AMOUNT,
            sorted_pair.to_vec(),
        ) {
            return None;
        }

        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(0.1)) {
            return None;
        }

        let bank_liquidity_vault_authority = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, liab_bank.as_ref()], &self.program_id).0;
        let bank_liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let bank_insurance_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, liab_bank.as_ref()], &self.program_id).0;
        let remaining_accounts = vec![sorted_pair[0], sorted_pair[1], sorted_pair[0], sorted_pair[1]];
        Some((
            asset_bank,
            liab_bank,
            liquidator,
            liquidatee,
            bank_liquidity_vault_authority,
            bank_liquidity_vault,
            bank_insurance_vault,
            remaining_accounts,
        ))
    }

    pub fn action_advance_panic_pause_expiry(&mut self) -> bool {
        use ::anchor_lang::prelude::Clock;
        let clock = self.ctx.svm.get_sysvar::<Clock>();
        self.ctx.set_sysvar(&Clock {
            slot: clock.slot + 10,
            epoch_start_timestamp: clock.epoch_start_timestamp,
            epoch: clock.epoch,
            leader_schedule_epoch: clock.leader_schedule_epoch,
            unix_timestamp: clock.unix_timestamp + SCOUT_PANIC_PAUSE_EXPIRY_SECONDS,
        });
        true
    }

    pub fn action_advance_clock(&mut self, hours: u8) -> bool {
        use ::anchor_lang::prelude::Clock;
        let seconds = (((hours as i64) % 24) + 1) * 3600;
        let clock = self.ctx.svm.get_sysvar::<Clock>();
        self.ctx.set_sysvar(&Clock {
            slot: clock.slot + 1,
            epoch_start_timestamp: clock.epoch_start_timestamp,
            epoch: clock.epoch,
            leader_schedule_epoch: clock.leader_schedule_epoch,
            unix_timestamp: clock.unix_timestamp.saturating_add(seconds),
        });
        true
    }

    fn scout_prepare_migrate_curve_legacy_bank_append_only(&mut self) -> Pubkey {
        let bank = self.bank;
        let optimal = fixed::types::I80F48::from_num(0.5);
        let plateau = fixed::types::I80F48::from_num(0.1);
        let max = fixed::types::I80F48::from_num(0.5);
        self.ctx
            .update_account(&bank, |data| {
                let irc = SCOUT_BANK_INTEREST_RATE_CONFIG_OFFSET;
                data[irc + SCOUT_IRC_OPTIMAL_UTIL_OFFSET..irc + SCOUT_IRC_OPTIMAL_UTIL_OFFSET + 16]
                    .copy_from_slice(&optimal.to_le_bytes());
                data[irc + SCOUT_IRC_PLATEAU_RATE_OFFSET..irc + SCOUT_IRC_PLATEAU_RATE_OFFSET + 16]
                    .copy_from_slice(&plateau.to_le_bytes());
                data[irc + SCOUT_IRC_MAX_RATE_OFFSET..irc + SCOUT_IRC_MAX_RATE_OFFSET + 16]
                    .copy_from_slice(&max.to_le_bytes());
                data[irc + SCOUT_IRC_CURVE_TYPE_OFFSET] = SCOUT_INTEREST_CURVE_LEGACY;
            })
            .unwrap();
        bank
    }

    pub fn action_lending_pool_handle_bankruptcy_reachable(&mut self) -> bool {
        let Some((marginfi_account, bank, liquidity_vault, insurance_vault, insurance_vault_authority)) =
            self.scout_prepare_lending_pool_handle_bankruptcy_accounts()
        else {
            return false;
        };
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolHandleBankruptcy {})
            .accounts(accounts::LendingPoolHandleBankruptcy {
                group: self.marginfi_group,
                signer: self.payer.pubkey(),
                bank,
                marginfi_account,
                liquidity_vault,
                insurance_vault,
                insurance_vault_authority,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![bank])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_prepare_lending_pool_handle_bankruptcy_missing_balance_accounts(
        &mut self,
    ) -> Option<ScoutHandleBankruptcyVariantAccounts> {
        let (marginfi_account, bank_a, _liquidity_vault_a, _insurance_vault_a, _insurance_vault_authority_a) =
            self.scout_prepare_lending_pool_handle_bankruptcy_accounts()?;

        let bank_b = Pubkey::find_program_address(&[b"scout_hb_missing_balance_bank"], &self.program_id).0;
        let (liquidity_vault_authority_b, liquidity_vault_authority_bump_b) =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank_b.as_ref()], &self.program_id);
        let (liquidity_vault_b, liquidity_vault_bump_b) =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank_b.as_ref()], &self.program_id);
        let (insurance_vault_authority_b, insurance_vault_authority_bump_b) =
            Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank_b.as_ref()], &self.program_id);
        let (insurance_vault_b, insurance_vault_bump_b) =
            Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank_b.as_ref()], &self.program_id);
        let (fee_vault_authority_b, fee_vault_authority_bump_b) =
            Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank_b.as_ref()], &self.program_id);
        let (fee_vault_b, fee_vault_bump_b) =
            Pubkey::find_program_address(&[FEE_VAULT_SEED, bank_b.as_ref()], &self.program_id);

        self.ctx
            .create_account()
            .pubkey(bank_b)
            .owner(self.program_id)
            .data(&scout_handle_bankruptcy_bank_bytes(
                self.marginfi_group,
                self.bank_mint,
                liquidity_vault_b,
                liquidity_vault_bump_b,
                liquidity_vault_authority_bump_b,
                insurance_vault_b,
                insurance_vault_bump_b,
                insurance_vault_authority_bump_b,
                fee_vault_b,
                fee_vault_bump_b,
                fee_vault_authority_bump_b,
                fixed::types::I80F48::ZERO,
            ))
            .create()
            .ok()?;
        self.ctx
            .create_token_account()
            .pubkey(liquidity_vault_b)
            .mint(self.bank_mint)
            .token_owner(liquidity_vault_authority_b)
            .amount(0)
            .create()
            .ok()?;
        self.ctx
            .create_token_account()
            .pubkey(insurance_vault_b)
            .mint(self.bank_mint)
            .token_owner(insurance_vault_authority_b)
            .amount(0)
            .create()
            .ok()?;

        Some((
            marginfi_account,
            bank_b,
            liquidity_vault_b,
            insurance_vault_b,
            insurance_vault_authority_b,
            vec![bank_a],
        ))
    }

    fn scout_prepare_lending_pool_handle_bankruptcy_zero_target_debt_accounts(
        &mut self,
    ) -> Option<ScoutHandleBankruptcyVariantAccounts> {
        let liability = fixed::types::I80F48::from_num(SCOUT_HANDLE_BANKRUPTCY_LIABILITY_AMOUNT);

        let bank_debt = Pubkey::find_program_address(&[b"scout_hb_zero_debt_bank_debt"], &self.program_id).0;
        let (debt_liq_vault_auth, debt_liq_vault_auth_bump) =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank_debt.as_ref()], &self.program_id);
        let (debt_liq_vault, debt_liq_vault_bump) =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank_debt.as_ref()], &self.program_id);
        let (debt_ins_vault_auth, debt_ins_vault_auth_bump) =
            Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank_debt.as_ref()], &self.program_id);
        let (debt_ins_vault, debt_ins_vault_bump) =
            Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank_debt.as_ref()], &self.program_id);
        let (debt_fee_vault_auth, debt_fee_vault_auth_bump) =
            Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank_debt.as_ref()], &self.program_id);
        let (debt_fee_vault, debt_fee_vault_bump) =
            Pubkey::find_program_address(&[FEE_VAULT_SEED, bank_debt.as_ref()], &self.program_id);
        self.ctx
            .create_account()
            .pubkey(bank_debt)
            .owner(self.program_id)
            .data(&scout_handle_bankruptcy_bank_bytes(
                self.marginfi_group,
                self.bank_mint,
                debt_liq_vault,
                debt_liq_vault_bump,
                debt_liq_vault_auth_bump,
                debt_ins_vault,
                debt_ins_vault_bump,
                debt_ins_vault_auth_bump,
                debt_fee_vault,
                debt_fee_vault_bump,
                debt_fee_vault_auth_bump,
                liability,
            ))
            .create()
            .ok()?;
        self.ctx
            .create_token_account()
            .pubkey(debt_liq_vault)
            .mint(self.bank_mint)
            .token_owner(debt_liq_vault_auth)
            .amount(0)
            .create()
            .ok()?;
        self.ctx
            .create_token_account()
            .pubkey(debt_ins_vault)
            .mint(self.bank_mint)
            .token_owner(debt_ins_vault_auth)
            .amount(SCOUT_HANDLE_BANKRUPTCY_LIABILITY_AMOUNT as u64)
            .create()
            .ok()?;

        let bank_target = Pubkey::find_program_address(&[b"scout_hb_zero_debt_bank_target"], &self.program_id).0;
        let (tgt_liq_vault_auth, tgt_liq_vault_auth_bump) =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank_target.as_ref()], &self.program_id);
        let (tgt_liq_vault, tgt_liq_vault_bump) =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank_target.as_ref()], &self.program_id);
        let (tgt_ins_vault_auth, tgt_ins_vault_auth_bump) =
            Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank_target.as_ref()], &self.program_id);
        let (tgt_ins_vault, tgt_ins_vault_bump) =
            Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank_target.as_ref()], &self.program_id);
        let (tgt_fee_vault_auth, tgt_fee_vault_auth_bump) =
            Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank_target.as_ref()], &self.program_id);
        let (tgt_fee_vault, tgt_fee_vault_bump) =
            Pubkey::find_program_address(&[FEE_VAULT_SEED, bank_target.as_ref()], &self.program_id);
        self.ctx
            .create_account()
            .pubkey(bank_target)
            .owner(self.program_id)
            .data(&scout_handle_bankruptcy_bank_bytes(
                self.marginfi_group,
                self.bank_mint,
                tgt_liq_vault,
                tgt_liq_vault_bump,
                tgt_liq_vault_auth_bump,
                tgt_ins_vault,
                tgt_ins_vault_bump,
                tgt_ins_vault_auth_bump,
                tgt_fee_vault,
                tgt_fee_vault_bump,
                tgt_fee_vault_auth_bump,
                fixed::types::I80F48::ZERO,
            ))
            .create()
            .ok()?;
        self.ctx
            .create_token_account()
            .pubkey(tgt_liq_vault)
            .mint(self.bank_mint)
            .token_owner(tgt_liq_vault_auth)
            .amount(0)
            .create()
            .ok()?;
        self.ctx
            .create_token_account()
            .pubkey(tgt_ins_vault)
            .mint(self.bank_mint)
            .token_owner(tgt_ins_vault_auth)
            .amount(0)
            .create()
            .ok()?;

        let marginfi_account = Pubkey::find_program_address(&[b"scout_hb_zero_debt_account"], &self.program_id).0;
        self.ctx
            .create_account()
            .pubkey(marginfi_account)
            .owner(self.program_id)
            .data(&scout_handle_bankruptcy_account_bytes(
                self.marginfi_group,
                self.payer.pubkey(),
                bank_debt,
                liability,
                Some(bank_target),
                fixed::types::I80F48::ZERO,
            ))
            .create()
            .ok()?;

        Some((
            marginfi_account,
            bank_target,
            tgt_liq_vault,
            tgt_ins_vault,
            tgt_ins_vault_auth,
            vec![bank_debt, bank_target],
        ))
    }

    fn scout_prepare_purge_deleverage_balance_unmatched_complete_bank(&mut self) -> Pubkey {
        let bank_keypair = Keypair::new();
        let bank = bank_keypair.pubkey();
        let (
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank);
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: scout_valid_bank_config(10) })
                .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, spl_token::id()))
                .signers(&[&*self.payer, &bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout helper: lending_pool_add_bank failed for purge_deleverage_balance unmatched-bank test"
        );
        let mut bank_config_opt = scout_valid_bank_config_opt();
        bank_config_opt.tokenless_repayments_allowed = Some(true);
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolConfigureBank { bank_config_opt })
                .accounts(accounts::LendingPoolConfigureBank {
                    group: self.marginfi_group,
                    admin: self.payer.pubkey(),
                    bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout helper: lending_pool_configure_bank failed for purge_deleverage_balance unmatched-bank test"
        );
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolForceTokenlessRepayComplete {})
                .accounts(accounts::LendingPoolForceTokenlessRepayComplete {
                    group: self.marginfi_group,
                    risk_admin: self.payer.pubkey(),
                    bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout helper: lending_pool_force_tokenless_repay_complete failed for purge_deleverage_balance unmatched-bank test"
        );
        bank
    }

    fn scout_mint_fresh_withdraw_pair(&mut self, deposit_amount: u64) -> Option<(Pubkey, Pubkey, Pubkey)> {
        let marginfi_account = self.scout_create_initialized_marginfi_account()?;
        let bank_keypair = Keypair::new();
        let bank = bank_keypair.pubkey();
        let (
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank);
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let mut bank_config = scout_valid_bank_config(10);
        bank_config.deposit_limit = u64::MAX;
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAddBank { bank_config })
                .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, spl_token::id()))
                .signers(&[&*self.payer, &bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return None;
        }
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountDeposit {
                    amount: deposit_amount,
                    deposit_up_to_limit: None,
                })
                .accounts(accounts::LendingAccountDeposit {
                    group: self.marginfi_group,
                    marginfi_account,
                    authority: self.payer.pubkey(),
                    bank,
                    signer_token_account: self.signer_token_account,
                    liquidity_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(bank, fixed::types::I80F48::ONE) {
            return None;
        }
        Some((marginfi_account, bank, liquidity_vault))
    }

    fn scout_send_withdraw(
        &mut self,
        marginfi_account: Pubkey,
        bank: Pubkey,
        liquidity_vault: Pubkey,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> bool {
        let bank_liquidity_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()],
            &self.program_id,
        )
        .0;
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountWithdraw {
                amount,
                withdraw_all,
            })
            .accounts(accounts::LendingAccountWithdraw {
                group: self.marginfi_group,
                marginfi_account,
                authority: self.payer.pubkey(),
                bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![bank])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_lending_account_withdraw_partial_amount(&mut self, amount: u64) -> bool {
        let withdraw_amount = (amount % 900_000) + 1;
        let deposit_amount = withdraw_amount + SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT;
        let (marginfi_account, bank, liquidity_vault) =
            match self.scout_mint_fresh_withdraw_pair(deposit_amount) { Some(v) => v, None => return false };
        self.scout_send_withdraw(marginfi_account, bank, liquidity_vault, withdraw_amount, Some(false))
    }

    pub fn action_lending_account_withdraw_tokenless_complete(&mut self) -> bool {
        let deposit_amount = SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT;
        let (marginfi_account, bank, liquidity_vault) =
            match self.scout_mint_fresh_withdraw_pair(deposit_amount) { Some(v) => v, None => return false };
        let mut bank_flags_patched = false;
        if self
            .ctx
            .update_account(&bank, |data| {
                let start = SCOUT_BANK_FLAGS_OFFSET;
                if data.len() < start + 8 {
                    return;
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data[start..start + 8]);
                let flags = u64::from_le_bytes(bytes) | SCOUT_TOKENLESS_REPAYMENTS_COMPLETE;
                data[start..start + 8].copy_from_slice(&flags.to_le_bytes());
                bank_flags_patched = true;
            })
            .is_err()
        {
            return false;
        }
        if !bank_flags_patched {
            return false;
        }
        self.scout_send_withdraw(marginfi_account, bank, liquidity_vault, deposit_amount, None)
    }

    pub fn action_lending_account_withdraw_deleverage(&mut self) -> bool {
        let deposit_amount = SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT;
        let (marginfi_account, bank, liquidity_vault) =
            match self.scout_mint_fresh_withdraw_pair(deposit_amount) { Some(v) => v, None => return false };
        let mut account_flags_patched = false;
        if self
            .ctx
            .update_account(&marginfi_account, |data| {
                let start = MARGINFI_ACCOUNT_FLAGS_OFFSET;
                if data.len() < start + 8 {
                    return;
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data[start..start + 8]);
                let flags = u64::from_le_bytes(bytes) | SCOUT_ACCOUNT_IN_DELEVERAGE;
                data[start..start + 8].copy_from_slice(&flags.to_le_bytes());
                account_flags_patched = true;
            })
            .is_err()
        {
            return false;
        }
        if !account_flags_patched {
            return false;
        }
        if !self.scout_p17_harness_flagged.contains(&marginfi_account) {
            self.scout_p17_harness_flagged.push(marginfi_account);
        }
        self.scout_send_withdraw(marginfi_account, bank, liquidity_vault, deposit_amount, None)
    }

    pub fn action_lending_account_withdraw_receivership(&mut self) -> bool {
        let deposit_amount = SCOUT_WITHDRAW_SETUP_DEPOSIT_AMOUNT;
        let (marginfi_account, bank, liquidity_vault) =
            match self.scout_mint_fresh_withdraw_pair(deposit_amount) { Some(v) => v, None => return false };
        let mut account_flags_patched = false;
        if self
            .ctx
            .update_account(&marginfi_account, |data| {
                let start = MARGINFI_ACCOUNT_FLAGS_OFFSET;
                if data.len() < start + 8 {
                    return;
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&data[start..start + 8]);
                let flags = u64::from_le_bytes(bytes) | SCOUT_ACCOUNT_IN_RECEIVERSHIP;
                data[start..start + 8].copy_from_slice(&flags.to_le_bytes());
                account_flags_patched = true;
            })
            .is_err()
        {
            return false;
        }
        if !account_flags_patched {
            return false;
        }
        if !self.scout_p17_harness_flagged.contains(&marginfi_account) {
            self.scout_p17_harness_flagged.push(marginfi_account);
        }
        let mut bank_config_patched = false;
        if self
            .ctx
            .update_account(&bank, |data| {
                if data.len() < SCOUT_WITHDRAW_ACTION_BANK_ASSET_WEIGHT_INIT_OFFSET + 16
                    || data.len() < SCOUT_WITHDRAW_ACTION_BANK_ASSET_WEIGHT_MAINT_OFFSET + 16
                    || data.len() <= SCOUT_WITHDRAW_ACTION_BANK_RISK_TIER_OFFSET
                    || data.len() <= SCOUT_WITHDRAW_ACTION_BANK_ORACLE_SETUP_OFFSET
                    || data.len() < SCOUT_WITHDRAW_ACTION_BANK_FIXED_PRICE_OFFSET + 16
                {
                    return;
                }
                let one = fixed::types::I80F48::ONE.to_bits().to_le_bytes();
                data[SCOUT_WITHDRAW_ACTION_BANK_ASSET_WEIGHT_INIT_OFFSET
                    ..SCOUT_WITHDRAW_ACTION_BANK_ASSET_WEIGHT_INIT_OFFSET + 16]
                    .copy_from_slice(&one);
                data[SCOUT_WITHDRAW_ACTION_BANK_ASSET_WEIGHT_MAINT_OFFSET
                    ..SCOUT_WITHDRAW_ACTION_BANK_ASSET_WEIGHT_MAINT_OFFSET + 16]
                    .copy_from_slice(&one);
                data[SCOUT_WITHDRAW_ACTION_BANK_RISK_TIER_OFFSET] =
                    SCOUT_WITHDRAW_ACTION_RISK_TIER_COLLATERAL;
                data[SCOUT_WITHDRAW_ACTION_BANK_ORACLE_SETUP_OFFSET] =
                    SCOUT_WITHDRAW_ACTION_ORACLE_SETUP_FIXED;
                data[SCOUT_WITHDRAW_ACTION_BANK_FIXED_PRICE_OFFSET
                    ..SCOUT_WITHDRAW_ACTION_BANK_FIXED_PRICE_OFFSET + 16]
                    .copy_from_slice(&one);
                bank_config_patched = true;
            })
            .is_err()
        {
            return false;
        }
        if !bank_config_patched {
            return false;
        }
        self.scout_send_withdraw(marginfi_account, bank, liquidity_vault, deposit_amount, Some(true))
    }

    pub fn action_propagate_staked_settings_oracle_changed(&mut self) -> bool {
        let new_oracle_pubkey = Pubkey::new_unique();
        let new_oracle_bytes = scout_price_update_v2_bytes([11u8; 32], 150_000_000, 3);
        self.ctx.create_account()
            .pubkey(new_oracle_pubkey)
            .owner(pyth_receiver_program_id())
            .data(&new_oracle_bytes)
            .create()
            .unwrap();

        if !(self.ctx
                .program(self.program_id)
                .call(instruction::EditStakedSettings {
                    settings: marginfi::types::StakedSettingsEditConfig {
                        oracle: Some(new_oracle_pubkey),
                        ..Default::default()
                    },
                })
                .accounts(accounts::EditStakedSettings {
                    marginfi_group: self.staked_group,
                    admin: self.payer.pubkey(),
                    staked_settings: self.staked_settings,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return false;
        }

        self.ctx
            .program(self.program_id)
            .call(instruction::PropagateStakedSettings {})
            .accounts(accounts::PropagateStakedSettings {
                marginfi_group: self.staked_group,
                staked_settings: self.staked_settings,
                bank: self.staked_bank,
            })
            .remaining_accounts(vec![new_oracle_pubkey])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_mint_lending_pool_close_bank_guard_bank(
        &mut self,
        close_enabled: bool,
        lending_position_count: i32,
        total_shares: fixed::types::I80F48,
        emissions_remaining: fixed::types::I80F48,
    ) -> Option<Pubkey> {
        let bank_keypair = Keypair::new();
        let bank = bank_keypair.pubkey();
        let (
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank);
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        if !(self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAddBank {
                    bank_config: scout_valid_bank_config(10),
                })
                .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, spl_token::id()))
                .signers(&[&*self.payer, &bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)) {
            return None;
        }
        self.ctx
            .update_account(&bank, |data| {
                if !close_enabled {
                    data[SCOUT_HANDLE_BANKRUPTCY_BANK_FLAGS_VALUE_OFFSET
                        ..SCOUT_HANDLE_BANKRUPTCY_BANK_FLAGS_VALUE_OFFSET + 8]
                        .copy_from_slice(&0u64.to_le_bytes());
                }
                data[SCOUT_BANK_LENDING_POSITION_COUNT_OFFSET
                    ..SCOUT_BANK_LENDING_POSITION_COUNT_OFFSET + 4]
                    .copy_from_slice(&lending_position_count.to_le_bytes());
                data[SCOUT_HANDLE_BANKRUPTCY_BANK_TOTAL_ASSET_SHARES_OFFSET
                    ..SCOUT_HANDLE_BANKRUPTCY_BANK_TOTAL_ASSET_SHARES_OFFSET + 16]
                    .copy_from_slice(&total_shares.to_le_bytes());
                data[SCOUT_BANK_TOTAL_LIABILITY_SHARES_OFFSET
                    ..SCOUT_BANK_TOTAL_LIABILITY_SHARES_OFFSET + 16]
                    .copy_from_slice(&total_shares.to_le_bytes());
                data[SCOUT_BANK_EMISSIONS_REMAINING_OFFSET
                    ..SCOUT_BANK_EMISSIONS_REMAINING_OFFSET + 16]
                    .copy_from_slice(&emissions_remaining.to_le_bytes());
            })
            .unwrap();
        Some(bank)
    }

    pub fn scout_call_lending_pool_handle_bankruptcy_append(
        &mut self,
        marginfi_account: Pubkey,
        bank: Pubkey,
        liquidity_vault: Pubkey,
        insurance_vault: Pubkey,
        insurance_vault_authority: Pubkey,
        token_program: Pubkey,
        remaining_accounts: Vec<Pubkey>,
    ) -> bool {
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolHandleBankruptcy {})
            .accounts(accounts::LendingPoolHandleBankruptcy {
                group: self.marginfi_group,
                signer: self.payer.pubkey(),
                bank,
                marginfi_account,
                liquidity_vault,
                insurance_vault,
                insurance_vault_authority,
                token_program,
            })
            .remaining_accounts(remaining_accounts)
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_lending_pool_handle_bankruptcy_token2022_append(&mut self) -> bool {
        let Some((marginfi_account, bank, liquidity_vault, insurance_vault, insurance_vault_authority)) =
            self.scout_prepare_lending_pool_handle_bankruptcy_accounts()
        else {
            return false;
        };
        let token_2022_id: Pubkey = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
            .parse()
            .unwrap();
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolHandleBankruptcy {})
            .accounts(accounts::LendingPoolHandleBankruptcy {
                group: self.marginfi_group,
                signer: self.payer.pubkey(),
                bank,
                marginfi_account,
                liquidity_vault,
                insurance_vault,
                insurance_vault_authority,
                token_program: token_2022_id,
            })
            .remaining_accounts(vec![self.bank_mint, bank])
            .signers(&[&*self.payer])
            .send()
            .is_ok()
    }

    pub fn action_start_deleverage_with_matching_end(&mut self) -> bool {
        let marginfi_account = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        let group = self.marginfi_group;
        let risk_admin = self.payer.pubkey();
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        let record_ok = self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitLiqRecord {})
            .accounts(accounts::MarginfiAccountInitLiqRecord {
                marginfi_account,
                fee_payer: self.payer.pubkey(),
                liquidation_record,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !record_ok {
            return false;
        }
        self.scout_register_subject_record(liquidation_record);

        let start_ix = scout_start_deleverage_ix(
            self.program_id,
            marginfi_account,
            liquidation_record,
            group,
            risk_admin,
        );
        let end_ix = scout_end_deleverage_ix(
            self.program_id,
            marginfi_account,
            liquidation_record,
            group,
            risk_admin,
        );
        if self.ctx.raw_call(start_ix).signers(&[&*self.payer]).add_transaction().is_err() {
            return false;
        }
        if self.ctx.raw_call(end_ix).signers(&[&*self.payer]).add_transaction().is_err() {
            return false;
        }
        self.ctx
            .send_batch()
            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
            .unwrap_or(false)
    }

    pub fn action_lending_account_repay_tokenless_all(&mut self) -> bool {
        let marginfi_account = match self.scout_create_lending_account_repay_marginfi_account() { Some(v) => v, None => return false };
        let bank = match self.scout_create_lending_account_repay_bank_with_liability(
            marginfi_account,
            true,
            SCOUT_REPAY_SETUP_LIABILITY_AMOUNT,
        ) { Some(v) => v, None => return false };
        let group = self.marginfi_group;
        let authority = self.payer.pubkey();
        let signer_token_account = self.signer_token_account;
        let liquidity_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, bank.as_ref()],
            &self.program_id,
        )
        .0;
        let token_program = spl_token::id();
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountRepay {
                amount: 0,
                repay_all: Some(true),
            })
            .accounts(accounts::LendingAccountRepay {
                group,
                marginfi_account,
                authority,
                bank,
                signer_token_account,
                liquidity_vault,
                token_program,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_lending_account_borrow_with_origination_fee(&mut self) -> bool {
        let bank_liquidity_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, self.fee_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liquidity_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, self.fee_bank.as_ref()],
            &self.program_id,
        )
        .0;
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountBorrow { amount: 1_000_000 })
            .accounts(accounts::LendingAccountBorrow {
                group: self.marginfi_group,
                marginfi_account: self.fee_borrow_marginfi_account,
                authority: self.payer.pubkey(),
                bank: self.fee_bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_write_bank_metadata_immediate(&mut self) -> bool {
        let ticker: Option<Vec<u8>> = Some(b"SCOUT".to_vec());
        let description: Option<Vec<u8>> = Some(b"Scout bank metadata".to_vec());
        let group = self.marginfi_group;
        let bank = self.metadata_bank;
        let metadata_admin = self.payer.pubkey();
        let metadata = Pubkey::find_program_address(&[METADATA_SEED, bank.as_ref()], &self.program_id).0;
        self.ctx
            .program(self.program_id)
            .call(instruction::WriteBankMetadata { ticker, description })
            .accounts(accounts::WriteBankMetadata {
                group,
                bank,
                metadata_admin,
                metadata,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_lending_pool_add_bank_kamino_with_remaining_accounts(&mut self, bank_seed: u64) -> bool {
        let group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let bank_mint = self.bank_mint;
        let bank = scout_seeded_bank_pda(self.program_id, group, bank_mint, bank_seed);
        let lending_market = scout_kamino_lending_market(self.program_id);
        let reserve = Pubkey::new_unique();
        let reserve_bytes = scout_kamino_reserve_bytes(lending_market, bank_mint);
        self.ctx.create_account()
            .pubkey(reserve)
            .owner(scout_kamino_program_id())
            .data(&reserve_bytes)
            .create()
            .unwrap();
        let oracle = Pubkey::new_unique();
        let oracle_bytes = scout_price_update_v2_bytes([19u8; 32], 100_000_000, 1);
        self.ctx.create_account()
            .pubkey(oracle)
            .owner(pyth_receiver_program_id())
            .data(&oracle_bytes)
            .create()
            .unwrap();
        let bank_config = scout_valid_kamino_config(oracle);
        let (liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault) = scout_bank_vault_pdas(self.program_id, bank);
        let integration_acc_2 = scout_kamino_obligation_pda(liquidity_vault_authority, lending_market);
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBankKamino { bank_config, bank_seed })
            .accounts(accounts::LendingPoolAddBankKamino {
                group,
                admin,
                fee_payer,
                bank_mint,
                bank,
                integration_acc_1: reserve,
                integration_acc_2,
                liquidity_vault_authority,
                liquidity_vault,
                insurance_vault_authority,
                insurance_vault,
                fee_vault_authority,
                fee_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![oracle, reserve])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_create_drift_spot_market_with_oracle(&mut self, mint: Pubkey, oracle: Pubkey) -> Pubkey {
        let spot_market = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let bytes = scout_drift_spot_market_bytes(spot_market, oracle, mint, vault, 6, 0, 0, 0);
        self.ctx
            .create_account()
            .pubkey(spot_market)
            .owner(scout_drift_program_id())
            .data(&bytes)
            .create()
            .unwrap();
        spot_market
    }

    fn scout_send_lending_pool_add_bank_drift_for_mint(
        &mut self,
        bank_seed: u64,
        bank_config: marginfi::types::DriftConfigCompact,
        spot_market: Pubkey,
        oracle: Pubkey,
        bank_mint: Pubkey,
    ) -> bool {
        let group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let bank = Pubkey::find_program_address(&[group.as_ref(), bank_mint.as_ref(), &bank_seed.to_le_bytes()], &self.program_id).0;
        let (liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault) = scout_bank_vault_pdas(self.program_id, bank);
        let integration_acc_2 = self.scout_ensure_drift_user_account(liquidity_vault_authority);
        let integration_acc_3 = self.scout_ensure_drift_user_stats_account(liquidity_vault_authority);
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBankDrift { bank_config, bank_seed })
            .accounts(accounts::LendingPoolAddBankDrift {
                group,
                admin,
                fee_payer,
                bank_mint,
                bank,
                integration_acc_1: spot_market,
                integration_acc_2,
                integration_acc_3,
                liquidity_vault_authority,
                liquidity_vault,
                insurance_vault_authority,
                insurance_vault,
                fee_vault_authority,
                fee_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![oracle, spot_market])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_send_lending_pool_add_bank_drift(
        &mut self,
        bank_seed: u64,
        bank_config: marginfi::types::DriftConfigCompact,
        spot_market: Pubkey,
        oracle: Pubkey,
    ) -> bool {
        let bank_mint = self.bank_mint;
        self.scout_send_lending_pool_add_bank_drift_for_mint(bank_seed, bank_config, spot_market, oracle, bank_mint)
    }

    pub fn action_lending_pool_add_bank_drift_with_remaining(&mut self, bank_seed: u64) -> bool {
        let oracle = self.scout_create_drift_pyth_oracle();
        let oracle_bytes = scout_price_update_v2_bytes([9u8; 32], 100_000_000, 1);
        self.ctx
            .create_account()
            .pubkey(oracle)
            .owner(pyth_receiver_program_id())
            .data(&oracle_bytes)
            .create()
            .unwrap();
        let bank_mint = self.bank_mint;
        let spot_market = self.scout_create_drift_spot_market_with_oracle(bank_mint, oracle);
        let bank_config = scout_valid_drift_config(oracle);
        self.scout_send_lending_pool_add_bank_drift(bank_seed, bank_config, spot_market, oracle)
    }

    fn scout_ensure_solend_pyth_oracle_account(&mut self, oracle: Pubkey) -> Pubkey {
        let bytes = scout_price_update_v2_bytes([11u8; 32], 100_000_000, 1);
        self.ctx
            .create_account()
            .pubkey(oracle)
            .owner(pyth_receiver_program_id())
            .data(&bytes)
            .create()
            .unwrap();
        oracle
    }

    fn scout_ensure_solend_reserve_account(&mut self, reserve: Pubkey, mint: Pubkey) -> Pubkey {
        let lending_market = scout_solend_lending_market(self.program_id);
        let liquidity_supply = scout_solend_reserve_liquidity_supply(self.program_id);
        let pyth_oracle = scout_solend_pyth_price(self.program_id);
        let switchboard_oracle = scout_solend_switchboard_feed(self.program_id);
        let collateral_mint = scout_solend_reserve_collateral_mint(self.program_id);
        let collateral_supply = scout_solend_reserve_collateral_supply(self.program_id);
        let bytes = scout_solend_reserve_bytes(
            mint,
            lending_market,
            liquidity_supply,
            pyth_oracle,
            switchboard_oracle,
            collateral_mint,
            collateral_supply,
        );
        self.ctx
            .create_account()
            .pubkey(reserve)
            .owner(scout_solend_program_id())
            .data(&bytes)
            .create()
            .unwrap();
        reserve
    }

    fn scout_send_lending_pool_add_bank_solend(
        &mut self,
        bank_seed: u64,
        bank_config: marginfi::types::SolendConfigCompact,
        reserve: Pubkey,
        remaining_accounts: Vec<Pubkey>,
    ) -> bool {
        let group = self.marginfi_group;
        let admin = self.payer.pubkey();
        let fee_payer = self.payer.pubkey();
        let bank_mint = self.bank_mint;
        let bank = scout_seeded_bank_pda(self.program_id, group, bank_mint, bank_seed);
        let (liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault) = scout_bank_vault_pdas(self.program_id, bank);
        let integration_acc_2 = scout_solend_obligation_pda(self.program_id, bank);
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBankSolend { bank_config, bank_seed })
            .accounts(accounts::LendingPoolAddBankSolend {
                group,
                admin,
                fee_payer,
                bank_mint,
                bank,
                integration_acc_1: reserve,
                integration_acc_2,
                liquidity_vault_authority,
                liquidity_vault,
                insurance_vault_authority,
                insurance_vault,
                fee_vault_authority,
                fee_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(remaining_accounts)
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_lending_pool_add_bank_solend_with_remaining(&mut self, bank_seed: u64) -> bool {
        let oracle_pda = scout_solend_pyth_price(self.program_id);
        let oracle = self.scout_ensure_solend_pyth_oracle_account(oracle_pda);
        let reserve_pda = scout_solend_reserve(self.program_id);
        let bank_mint = self.bank_mint;
        let reserve = self.scout_ensure_solend_reserve_account(reserve_pda, bank_mint);
        let bank_config = scout_valid_solend_config(oracle);
        self.scout_send_lending_pool_add_bank_solend(bank_seed, bank_config, reserve, vec![oracle, reserve])
    }

    fn scout_solend_setup(&mut self, bank_seed: u64, mint: Pubkey, token_program: Pubkey) -> Option<ScoutSolendAccounts> {
        let group = self.marginfi_group;
        let pid = self.program_id;
        let bank = scout_seeded_bank_pda(pid, group, mint, bank_seed);
        let lva = Pubkey::find_program_address(&[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()], &pid).0;
        let liquidity_vault = Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &pid).0;
        let insurance_vault_authority = Pubkey::find_program_address(&[INSURANCE_VAULT_AUTHORITY_SEED, bank.as_ref()], &pid).0;
        let insurance_vault = Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &pid).0;
        let fee_vault_authority = Pubkey::find_program_address(&[FEE_VAULT_AUTHORITY_SEED, bank.as_ref()], &pid).0;
        let fee_vault = Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &pid).0;
        let obligation = scout_solend_obligation_pda(pid, bank);
        let oracle = Pubkey::find_program_address(&[b"scout_sol_or", bank.as_ref()], &pid).0;
        let reserve = Pubkey::find_program_address(&[b"scout_sol_rs", bank.as_ref()], &pid).0;
        let lending_market = Pubkey::find_program_address(&[b"scout_sol_lm", bank.as_ref()], &pid).0;
        let user_collateral = Pubkey::find_program_address(&[b"scout_sol_uc", bank.as_ref()], &pid).0;
        let switchboard = Pubkey::find_program_address(&[b"scout_sol_sb", bank.as_ref()], &pid).0;
        let collateral_mint = Pubkey::find_program_address(&[b"scout_sol_cm", bank.as_ref()], &pid).0;
        let lma = Pubkey::find_program_address(&[lending_market.as_ref()], &scout_solend_program_id()).0;

        self.scout_ensure_solend_pyth_oracle_account(oracle);
        let token_2022_id: Pubkey = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse().ok()?;
        let is_t22 = token_program == token_2022_id;
        let liquidity_supply = if is_t22 {
            self.scout_create_t22_token_account(mint, lma, 0)?
        } else {
            let acct = Keypair::new().pubkey();
            self.ctx.create_token_account().pubkey(acct).mint(mint).token_owner(lma).amount(0).create().ok()?;
            acct
        };
        self.ctx.create_mint().pubkey(collateral_mint).decimals(6).mint_authority(lma).create().ok()?;
        let collateral_supply = Keypair::new().pubkey();
        self.ctx.create_token_account().pubkey(collateral_supply).mint(collateral_mint).token_owner(lma).amount(0).create().ok()?;
        let reserve_bytes = scout_solend_reserve_bytes(mint, lending_market, liquidity_supply, oracle, switchboard, collateral_mint, collateral_supply);
        self.ctx.create_account().pubkey(reserve).owner(scout_solend_program_id()).data(&reserve_bytes).create().ok()?;
        let mut lm_bytes = vec![0u8; 290];
        lm_bytes[0] = 1;
        self.ctx.create_account().pubkey(lending_market).owner(scout_solend_program_id()).data(&lm_bytes).create().ok()?;
        let payer = self.payer.clone();
        let added = self.ctx
            .program(pid)
            .call(instruction::LendingPoolAddBankSolend { bank_config: scout_valid_solend_config(oracle), bank_seed })
            .accounts(accounts::LendingPoolAddBankSolend {
                group,
                admin: payer.pubkey(),
                fee_payer: payer.pubkey(),
                bank_mint: mint,
                bank,
                integration_acc_1: reserve,
                integration_acc_2: obligation,
                liquidity_vault_authority: lva,
                liquidity_vault,
                insurance_vault_authority,
                insurance_vault,
                fee_vault_authority,
                fee_vault,
                token_program,
            })
            .remaining_accounts(vec![oracle, reserve])
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !added {
            return None;
        }
        let obl_bytes = scout_solend_obligation_bytes(lending_market, lva, reserve);
        self.ctx.create_account().pubkey(obligation).owner(scout_solend_program_id()).data(&obl_bytes).create().ok()?;
        Some(ScoutSolendAccounts {
            bank, lva, liquidity_vault, obligation, oracle, reserve, lending_market, lma,
            liquidity_supply, collateral_mint, collateral_supply, user_collateral, switchboard, mint, token_program,
        })
    }

    fn scout_solend_deposit_a(&mut self, a: &ScoutSolendAccounts, signer_ta: Pubkey, depositor: Pubkey, amount: u64) -> bool {
        let payer = self.payer.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::SolendDeposit { amount })
            .accounts(accounts::SolendDeposit {
                group: self.marginfi_group,
                marginfi_account: depositor,
                authority: payer.pubkey(),
                bank: a.bank,
                signer_token_account: signer_ta,
                liquidity_vault_authority: a.lva,
                liquidity_vault: a.liquidity_vault,
                integration_acc_2: a.obligation,
                lending_market: a.lending_market,
                lending_market_authority: a.lma,
                integration_acc_1: a.reserve,
                mint: a.mint,
                reserve_liquidity_supply: a.liquidity_supply,
                reserve_collateral_mint: a.collateral_mint,
                reserve_collateral_supply: a.collateral_supply,
                user_collateral: a.user_collateral,
                pyth_price: a.oracle,
                switchboard_feed: a.switchboard,
                token_program: a.token_program,
            })
            .remaining_accounts(vec![a.oracle, a.reserve])
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_solend_withdraw_a(&mut self, a: &ScoutSolendAccounts, withdrawer: Pubkey, dest_ta: Pubkey, amount: u64, remaining: Vec<Pubkey>) -> bool {
        let payer = self.payer.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::SolendWithdraw { amount, withdraw_all: None })
            .accounts(accounts::SolendWithdraw {
                group: self.marginfi_group,
                marginfi_account: withdrawer,
                authority: payer.pubkey(),
                bank: a.bank,
                destination_token_account: dest_ta,
                liquidity_vault_authority: a.lva,
                liquidity_vault: a.liquidity_vault,
                integration_acc_2: a.obligation,
                lending_market: a.lending_market,
                lending_market_authority: a.lma,
                integration_acc_1: a.reserve,
                mint: a.mint,
                reserve_liquidity_supply: a.liquidity_supply,
                reserve_collateral_mint: a.collateral_mint,
                reserve_collateral_supply: a.collateral_supply,
                user_collateral: a.user_collateral,
                token_program: a.token_program,
            })
            .remaining_accounts(remaining)
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_solend_leg(&mut self, bank_seed: u64, mint: Pubkey, token_program: Pubkey, signer_ta: Pubkey, amount: u64) -> Option<bool> {
        let a = self.scout_solend_setup(bank_seed, mint, token_program)?;
        let depositor = self.scout_create_initialized_marginfi_account()?;
        Some(self.scout_solend_deposit_a(&a, signer_ta, depositor, amount))
    }

    // P-0035-SOLEND-T22: differential probe -- normal mint control vs Token-2022 fee mint deposit.
    pub fn action_solend_t22_deposit_leak_probe(&mut self) -> bool {
        let amount: u64 = 10_000;
        let control = match self.scout_solend_leg(6_050_240_010, self.bank_mint, spl_token::id(), self.signer_token_account, amount) {
            Some(v) => v,
            None => return false,
        };
        let payer_pk = self.payer.pubkey();
        let t22_mint = match self.scout_create_t22_fee_mint(200, u64::MAX, 6) { Some(v) => v, None => return false };
        let t22_ta = match self.scout_create_t22_token_account(t22_mint, payer_pk, amount.saturating_mul(4)) { Some(v) => v, None => return false };
        let token_2022_id: Pubkey = match "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb".parse() { Ok(v) => v, Err(_) => return false };
        let t22 = match self.scout_solend_leg(6_050_240_011, t22_mint, token_2022_id, t22_ta, amount) {
            Some(v) => v,
            None => return false,
        };
        if control {
            scout_check!(
                "P-0035-SOLEND-T22",
                "solend-integration-deposit-must-not-break-on-a-token-2022-fee-mint",
                t22,
                "P-0035-SOLEND-T22: Solend deposit succeeds on normal mint but fails on T22 fee mint"
            );
        }
        control && !t22
    }

    fn scout_solend_debt_remaining(a: &ScoutSolendAccounts, debt_bank: Pubkey) -> Vec<Pubkey> {
        if a.bank.to_bytes() > debt_bank.to_bytes() {
            vec![a.bank, a.oracle, a.reserve, debt_bank]
        } else {
            vec![debt_bank, a.bank, a.oracle, a.reserve]
        }
    }

    // P-0037-SOLEND: Solend-integration withdraw must respect the RiskEngine lockout.
    pub fn action_solend_withdraw_lockout_probe(&mut self) -> bool {
        let a = match self.scout_solend_setup(6_050_240_030, self.bank_mint, spl_token::id()) { Some(v) => v, None => return false };
        let debt_bank = match self.scout_liquidate_add_bank(scout_valid_bank_config(10)) { Some(v) => v, None => return false };
        if a.bank == debt_bank || !self.scout_liquidate_raise_liab_bank_limits(debt_bank) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(debt_bank, fixed::types::I80F48::ONE) {
            return false;
        }
        let provider = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if !self.scout_liquidate_deposit(provider, debt_bank, SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT) {
            return false;
        }
        let account = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if !self.scout_solend_deposit_a(&a, self.signer_token_account, account, 100_000) {
            return false;
        }
        let remaining = Self::scout_solend_debt_remaining(&a, debt_bank);
        if !self.scout_liquidate_borrow(account, debt_bank, 100, remaining.clone()) {
            return false;
        }
        let escaped = self.scout_solend_withdraw_a(&a, account, self.signer_token_account, 100_000, remaining);
        scout_check!(
            "P-0037-SOLEND",
            "solend-withdraw-must-be-locked-out-while-a-debt-would-be-left-unbacked",
            !escaped,
            "P-0037-SOLEND: Solend withdraw removed all collateral while a debt remained"
        );
        escaped
    }

    pub fn action_panic_pause_then_permissionless_unpause(&mut self) -> bool {
        if !self.action_panic_pause() {
            return false;
        }
        if !self.action_advance_panic_pause_expiry() {
            return false;
        }
        self.action_panic_unpause_permissionless()
    }

    pub fn action_pulse_health_after_healthy_collateral_deposit(&mut self) -> bool {
        let bank_keypair = Keypair::new();
        let bank = bank_keypair.pubkey();
        let (
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank);
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let one = marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ONE);
        let mut bank_config = scout_valid_bank_config(10);
        bank_config.asset_weight_init = one;
        bank_config.asset_weight_maint = one;
        bank_config.risk_tier = marginfi::types::RiskTier::Collateral;
        bank_config.deposit_limit = u64::MAX;
        bank_config.total_asset_value_init_limit = u64::MAX;
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBank { bank_config })
            .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, spl_token::id()))
            .signers(&[&*self.payer, &bank_keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolSetFixedOraclePrice { price: one })
            .accounts(accounts::LendingPoolSetFixedOraclePrice {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        let marginfi_account = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };

        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit {
                amount: 1_000_000,
                deposit_up_to_limit: None,
            })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account,
                authority: self.payer.pubkey(),
                bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountPulseHealth {})
            .accounts(accounts::LendingAccountPulseHealth { marginfi_account, group: self.marginfi_group })
            .remaining_accounts(vec![bank])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_pulse_health_after_real_price_crash(&mut self) -> bool {
        let one = fixed::types::I80F48::ONE;
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;

        let collateral_keypair = Keypair::new();
        let bank_collateral = collateral_keypair.pubkey();
        let (
            collateral_liquidity_vault_authority,
            collateral_liquidity_vault,
            collateral_insurance_vault_authority,
            collateral_insurance_vault,
            collateral_fee_vault_authority,
            collateral_fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank_collateral);
        let mut collateral_config = scout_valid_bank_config(10);
        collateral_config.asset_weight_init = marginfi::types::WrappedI80F48::from_i80f48(one);
        collateral_config.asset_weight_maint = marginfi::types::WrappedI80F48::from_i80f48(one);
        collateral_config.risk_tier = marginfi::types::RiskTier::Collateral;
        collateral_config.deposit_limit = u64::MAX;
        collateral_config.total_asset_value_init_limit = u64::MAX;
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBank { bank_config: collateral_config })
            .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank_collateral, collateral_liquidity_vault_authority, collateral_liquidity_vault, collateral_insurance_vault_authority, collateral_insurance_vault, collateral_fee_vault_authority, collateral_fee_vault, spl_token::id()))
            .signers(&[&*self.payer, &collateral_keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolSetFixedOraclePrice {
                price: marginfi::types::WrappedI80F48::from_i80f48(one),
            })
            .accounts(accounts::LendingPoolSetFixedOraclePrice {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank: bank_collateral,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        let liability_keypair = Keypair::new();
        let bank_liability = liability_keypair.pubkey();
        let (
            liability_liquidity_vault_authority,
            liability_liquidity_vault,
            liability_insurance_vault_authority,
            liability_insurance_vault,
            liability_fee_vault_authority,
            liability_fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank_liability);
        let mut liability_config = scout_valid_bank_config(10);
        liability_config.borrow_limit = u64::MAX;
        liability_config.deposit_limit = u64::MAX;
        liability_config.total_asset_value_init_limit = u64::MAX;
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBank { bank_config: liability_config })
            .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank_liability, liability_liquidity_vault_authority, liability_liquidity_vault, liability_insurance_vault_authority, liability_insurance_vault, liability_fee_vault_authority, liability_fee_vault, spl_token::id()))
            .signers(&[&*self.payer, &liability_keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolSetFixedOraclePrice {
                price: marginfi::types::WrappedI80F48::from_i80f48(one),
            })
            .accounts(accounts::LendingPoolSetFixedOraclePrice {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank: bank_liability,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        let seed_account = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit {
                amount: 5_000_000,
                deposit_up_to_limit: None,
            })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account: seed_account,
                authority: self.payer.pubkey(),
                bank: bank_liability,
                signer_token_account: self.signer_token_account,
                liquidity_vault: liability_liquidity_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        let marginfi_account = match self.scout_create_initialized_marginfi_account() { Some(v) => v, None => return false };
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit {
                amount: 1_000_000,
                deposit_up_to_limit: None,
            })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account,
                authority: self.payer.pubkey(),
                bank: bank_collateral,
                signer_token_account: self.signer_token_account,
                liquidity_vault: collateral_liquidity_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        let sorted_banks = if bank_collateral.to_bytes() > bank_liability.to_bytes() {
            [bank_collateral, bank_liability]
        } else {
            [bank_liability, bank_collateral]
        };
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountBorrow { amount: 500_000 })
            .accounts(accounts::LendingAccountBorrow {
                group: self.marginfi_group,
                marginfi_account,
                authority: self.payer.pubkey(),
                bank: bank_liability,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority: liability_liquidity_vault_authority,
                liquidity_vault: liability_liquidity_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(sorted_banks.to_vec())
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolSetFixedOraclePrice {
                price: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ZERO),
            })
            .accounts(accounts::LendingPoolSetFixedOraclePrice {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank: bank_collateral,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }

        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountPulseHealth {})
            .accounts(accounts::LendingAccountPulseHealth { marginfi_account, group: self.marginfi_group })
            .remaining_accounts(sorted_banks.to_vec())
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    fn scout_setup_dedicated_pulse_health_accounts(&mut self) {
        let one = fixed::types::I80F48::ONE;
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;

        let healthy_bank_keypair = Keypair::new();
        let healthy_bank = healthy_bank_keypair.pubkey();
        let (
            healthy_liquidity_vault_authority,
            healthy_liquidity_vault,
            healthy_insurance_vault_authority,
            healthy_insurance_vault,
            healthy_fee_vault_authority,
            healthy_fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, healthy_bank);
        let mut healthy_config = scout_valid_bank_config(10);
        healthy_config.asset_weight_init = marginfi::types::WrappedI80F48::from_i80f48(one);
        healthy_config.asset_weight_maint = marginfi::types::WrappedI80F48::from_i80f48(one);
        healthy_config.risk_tier = marginfi::types::RiskTier::Collateral;
        healthy_config.deposit_limit = u64::MAX;
        healthy_config.total_asset_value_init_limit = u64::MAX;
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: healthy_config })
                .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, healthy_bank, healthy_liquidity_vault_authority, healthy_liquidity_vault, healthy_insurance_vault_authority, healthy_insurance_vault, healthy_fee_vault_authority, healthy_fee_vault, spl_token::id()))
                .signers(&[&*self.payer, &healthy_bank_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: healthy bank add failed"
        );
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolSetFixedOraclePrice {
                    price: marginfi::types::WrappedI80F48::from_i80f48(one),
                })
                .accounts(accounts::LendingPoolSetFixedOraclePrice {
                    group: self.marginfi_group,
                    admin: self.payer.pubkey(),
                    bank: healthy_bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: healthy bank fixed price failed"
        );
        let healthy_account = self.scout_create_initialized_marginfi_account().expect("scout helper: scout_create_initialized_marginfi_account prerequisite failed");
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountDeposit {
                    amount: 1_000_000,
                    deposit_up_to_limit: None,
                })
                .accounts(accounts::LendingAccountDeposit {
                    group: self.marginfi_group,
                    marginfi_account: healthy_account,
                    authority: self.payer.pubkey(),
                    bank: healthy_bank,
                    signer_token_account: self.signer_token_account,
                    liquidity_vault: healthy_liquidity_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: healthy deposit failed"
        );
        self.pulse_health_healthy_account = healthy_account;
        self.pulse_health_healthy_bank = healthy_bank;

        let collateral_keypair = Keypair::new();
        let bank_collateral = collateral_keypair.pubkey();
        let (
            collateral_liquidity_vault_authority,
            collateral_liquidity_vault,
            collateral_insurance_vault_authority,
            collateral_insurance_vault,
            collateral_fee_vault_authority,
            collateral_fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank_collateral);
        let mut collateral_config = scout_valid_bank_config(10);
        collateral_config.asset_weight_init = marginfi::types::WrappedI80F48::from_i80f48(one);
        collateral_config.asset_weight_maint = marginfi::types::WrappedI80F48::from_i80f48(one);
        collateral_config.risk_tier = marginfi::types::RiskTier::Collateral;
        collateral_config.deposit_limit = u64::MAX;
        collateral_config.total_asset_value_init_limit = u64::MAX;
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: collateral_config })
                .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank_collateral, collateral_liquidity_vault_authority, collateral_liquidity_vault, collateral_insurance_vault_authority, collateral_insurance_vault, collateral_fee_vault_authority, collateral_fee_vault, spl_token::id()))
                .signers(&[&*self.payer, &collateral_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: collateral bank add failed"
        );
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolSetFixedOraclePrice {
                    price: marginfi::types::WrappedI80F48::from_i80f48(one),
                })
                .accounts(accounts::LendingPoolSetFixedOraclePrice {
                    group: self.marginfi_group,
                    admin: self.payer.pubkey(),
                    bank: bank_collateral,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: collateral bank fixed price failed"
        );

        let liability_keypair = Keypair::new();
        let bank_liability = liability_keypair.pubkey();
        let (
            liability_liquidity_vault_authority,
            liability_liquidity_vault,
            liability_insurance_vault_authority,
            liability_insurance_vault,
            liability_fee_vault_authority,
            liability_fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank_liability);
        let mut liability_config = scout_valid_bank_config(10);
        liability_config.borrow_limit = u64::MAX;
        liability_config.deposit_limit = u64::MAX;
        liability_config.total_asset_value_init_limit = u64::MAX;
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAddBank { bank_config: liability_config })
                .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank_liability, liability_liquidity_vault_authority, liability_liquidity_vault, liability_insurance_vault_authority, liability_insurance_vault, liability_fee_vault_authority, liability_fee_vault, spl_token::id()))
                .signers(&[&*self.payer, &liability_keypair])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: liability bank add failed"
        );
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolSetFixedOraclePrice {
                    price: marginfi::types::WrappedI80F48::from_i80f48(one),
                })
                .accounts(accounts::LendingPoolSetFixedOraclePrice {
                    group: self.marginfi_group,
                    admin: self.payer.pubkey(),
                    bank: bank_liability,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: liability bank fixed price failed"
        );

        let seed_account = self.scout_create_initialized_marginfi_account().expect("scout helper: scout_create_initialized_marginfi_account prerequisite failed");
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountDeposit {
                    amount: 5_000_000,
                    deposit_up_to_limit: None,
                })
                .accounts(accounts::LendingAccountDeposit {
                    group: self.marginfi_group,
                    marginfi_account: seed_account,
                    authority: self.payer.pubkey(),
                    bank: bank_liability,
                    signer_token_account: self.signer_token_account,
                    liquidity_vault: liability_liquidity_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: liability seed deposit failed"
        );

        let risk_rejected_account = self.scout_create_initialized_marginfi_account().expect("scout helper: scout_create_initialized_marginfi_account prerequisite failed");
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountDeposit {
                    amount: 1_000_000,
                    deposit_up_to_limit: None,
                })
                .accounts(accounts::LendingAccountDeposit {
                    group: self.marginfi_group,
                    marginfi_account: risk_rejected_account,
                    authority: self.payer.pubkey(),
                    bank: bank_collateral,
                    signer_token_account: self.signer_token_account,
                    liquidity_vault: collateral_liquidity_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: risk-rejected collateral deposit failed"
        );

        let sorted_banks = if bank_collateral.to_bytes() > bank_liability.to_bytes() {
            [bank_collateral, bank_liability]
        } else {
            [bank_liability, bank_collateral]
        };
        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountBorrow { amount: 500_000 })
                .accounts(accounts::LendingAccountBorrow {
                    group: self.marginfi_group,
                    marginfi_account: risk_rejected_account,
                    authority: self.payer.pubkey(),
                    bank: bank_liability,
                    destination_token_account: self.signer_token_account,
                    bank_liquidity_vault_authority: liability_liquidity_vault_authority,
                    liquidity_vault: liability_liquidity_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(sorted_banks.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: risk-rejected borrow failed"
        );

        assert!(
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolSetFixedOraclePrice {
                    price: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ZERO),
                })
                .accounts(accounts::LendingPoolSetFixedOraclePrice {
                    group: self.marginfi_group,
                    admin: self.payer.pubkey(),
                    bank: bank_collateral,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false),
            "scout_setup_dedicated_pulse_health_accounts: collateral price crash failed"
        );

        self.pulse_health_risk_rejected_account = risk_rejected_account;
        self.pulse_health_risk_rejected_remaining = sorted_banks.to_vec();
    }

    pub fn action_pulse_health_healthy_dedicated(&mut self) -> bool {
        let marginfi_account = self.pulse_health_healthy_account;
        let bank = self.pulse_health_healthy_bank;
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountPulseHealth {})
            .accounts(accounts::LendingAccountPulseHealth { marginfi_account, group: self.marginfi_group })
            .remaining_accounts(vec![bank])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_pulse_health_risk_rejected_dedicated(&mut self) -> bool {
        let marginfi_account = self.pulse_health_risk_rejected_account;
        let remaining = self.pulse_health_risk_rejected_remaining.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountPulseHealth {})
            .accounts(accounts::LendingAccountPulseHealth { marginfi_account, group: self.marginfi_group })
            .remaining_accounts(remaining)
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_write_bank_metadata_varying_lengths(
        &mut self,
        ticker_len: u8,
        description_len: u8,
    ) -> bool {
        let ticker: Option<Vec<u8>> = Some(vec![b'T'; ticker_len as usize]);
        let description: Option<Vec<u8>> = Some(vec![b'D'; description_len as usize]);
        let group = self.marginfi_group;
        let bank = self.metadata_bank;
        let metadata_admin = self.payer.pubkey();
        let metadata =
            Pubkey::find_program_address(&[METADATA_SEED, bank.as_ref()], &self.program_id).0;
        self.ctx
            .program(self.program_id)
            .call(instruction::WriteBankMetadata { ticker, description })
            .accounts(accounts::WriteBankMetadata {
                group,
                bank,
                metadata_admin,
                metadata,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    pub fn action_write_bank_metadata_description_only(
        &mut self,
        description_len: u8,
    ) -> bool {
        let ticker: Option<Vec<u8>> = None;
        let description: Option<Vec<u8>> = Some(vec![b'D'; description_len as usize]);
        let group = self.marginfi_group;
        let bank = self.metadata_bank;
        let metadata_admin = self.payer.pubkey();
        let metadata =
            Pubkey::find_program_address(&[METADATA_SEED, bank.as_ref()], &self.program_id).0;
        self.ctx
            .program(self.program_id)
            .call(instruction::WriteBankMetadata { ticker, description })
            .accounts(accounts::WriteBankMetadata {
                group,
                bank,
                metadata_admin,
                metadata,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    // === Flashloan bracket: start_flashloan -> fuzzer-chosen middle -> end_flashloan ==============
    // (flashloan.rs:147-152), which needs one remaining account per ACTIVE balance -- the raw
    // builder passes none, so bank metas are appended by hand below; (2) those banks need a Fixed

    /// Mint two Fixed-priced banks plus a funded, initialized marginfi account for a flashloan bracket.
    /// Returns `(bracket_account, asset_bank, liab_bank)`.
    fn scout_flashloan_prepare(&mut self) -> Option<(Pubkey, Pubkey, Pubkey)> {
        let first = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let second = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let (asset_bank, liab_bank) = if first.to_bytes() > second.to_bytes() {
            (first, second)
        } else {
            (second, first)
        };
        for bank in [asset_bank, liab_bank] {
            if !self.scout_liquidate_set_fixed_price(bank, fixed::types::I80F48::ONE) {
                return None;
            }
        }

        let provider = self.scout_create_initialized_marginfi_account()?;
        for bank in [asset_bank, liab_bank] {
            if !self.scout_liquidate_deposit(provider, bank, SCOUT_FLASHLOAN_VAULT_LIQUIDITY) {
                return None;
            }
        }

        let account = self.scout_create_initialized_marginfi_account()?;
        if !self.scout_liquidate_deposit(account, asset_bank, SCOUT_FLASHLOAN_SEED_DEPOSIT) {
            return None;
        }
        if !self.scout_liquidate_borrow(
            account,
            liab_bank,
            SCOUT_FLASHLOAN_SEED_BORROW,
            vec![asset_bank, liab_bank],
        ) {
            return None;
        }
        Some((account, asset_bank, liab_bank))
    }

    /// QUEUE (never send) one deposit inside the bracket.
    fn scout_queue_flashloan_deposit(&mut self, account: Pubkey, bank: Pubkey, amount: u64) {
        let liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let _ = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit {
                amount,
                deposit_up_to_limit: None,
            })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account: account,
                authority: self.payer.pubkey(),
                bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .add_transaction();
    }

    /// QUEUE (never send) one borrow inside the bracket.
    fn scout_queue_flashloan_borrow(&mut self, account: Pubkey, bank: Pubkey, amount: u64) {
        let bank_liquidity_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let _ = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountBorrow { amount })
            .accounts(accounts::LendingAccountBorrow {
                group: self.marginfi_group,
                marginfi_account: account,
                authority: self.payer.pubkey(),
                bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![bank])
            .signers(&[&*self.payer])
            .add_transaction();
    }

    /// QUEUE (never send) one withdraw inside the bracket.
    fn scout_queue_flashloan_withdraw(&mut self, account: Pubkey, bank: Pubkey, amount: u64) {
        let bank_liquidity_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let _ = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountWithdraw {
                amount,
                withdraw_all: Some(false),
            })
            .accounts(accounts::LendingAccountWithdraw {
                group: self.marginfi_group,
                marginfi_account: account,
                authority: self.payer.pubkey(),
                bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![bank])
            .signers(&[&*self.payer])
            .add_transaction();
    }

    /// QUEUE (never send) one repay inside the bracket.
    fn scout_queue_flashloan_repay(
        &mut self,
        account: Pubkey,
        bank: Pubkey,
        amount: u64,
        repay_all: bool,
    ) {
        let liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let _ = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountRepay {
                amount,
                repay_all: Some(repay_all),
            })
            .accounts(accounts::LendingAccountRepay {
                group: self.marginfi_group,
                marginfi_account: account,
                authority: self.payer.pubkey(),
                bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .add_transaction();
    }

    /// One queued instruction, selected by a fuzzer byte.
    fn scout_queue_flashloan_middle(
        &mut self,
        choice: u8,
        account: Pubkey,
        asset_bank: Pubkey,
        liab_bank: Pubkey,
        amount: u64,
    ) {
        match choice % 5 {
            0 => self.scout_queue_flashloan_deposit(account, asset_bank, amount),
            1 => self.scout_queue_flashloan_borrow(account, liab_bank, amount),
            2 => self.scout_queue_flashloan_withdraw(account, asset_bank, amount),
            3 => self.scout_queue_flashloan_repay(account, liab_bank, amount, amount % 2 == 0),
            _ => {}
        }
    }

    /// Compound action: `start_flashloan` -> fuzzer-selected middle(s) -> `end_flashloan`, in one atomic `send_batch()`.
    pub fn action_flashloan_bracket(&mut self, choice: u8, amount: u64, second: u8) -> bool {
        let (account, asset_bank, liab_bank) = match self.scout_flashloan_prepare() {
            Some(v) => v,
            None => return false,
        };
        let authority = self.payer.pubkey();
        let payer = self.payer.clone();
        let middle_amount = amount % SCOUT_FLASHLOAN_MIDDLE_AMOUNT_MODULUS + 1;
        let use_second = second % 3 != 0;

        let base = self.ctx.pending_instructions.len();
        self.scout_queue_flashloan_middle(choice, account, asset_bank, liab_bank, middle_amount);
        if use_second {
            self.scout_queue_flashloan_middle(second, account, asset_bank, liab_bank, middle_amount);
        }
        let end_index = (self.ctx.pending_instructions.len() + 1) as u64;

        let start_ix = scout_lending_account_start_flashloan_ix(
            self.program_id,
            account,
            authority,
            end_index,
        );
        if self
            .ctx
            .raw_call(start_ix)
            .signers(&[&*payer])
            .add_transaction()
            .is_err()
        {
            return false;
        }
        self.ctx.pending_instructions[base..].rotate_right(1);

        let mut end_ix =
            scout_lending_account_end_flashloan_ix(self.program_id, account, self.marginfi_group, authority);
        for bank in [asset_bank, liab_bank] {
            end_ix.accounts.push(
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(bank, false),
            );
        }
        if self
            .ctx
            .raw_call(end_ix)
            .signers(&[&*payer])
            .add_transaction()
            .is_err()
        {
            return false;
        }

        self.ctx
            .send_batch()
            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
            .unwrap_or(false)
    }
    // Builds the shared borrow scenario bound by the generated borrow/repay actions.
    fn scout_setup_borrow_scenario(&mut self) -> bool {
        let asset_bank = match self.scout_liquidate_add_bank(scout_liquidation_bank_config()) {
            Some(v) => v,
            None => return false,
        };
        let liab_bank = match self.scout_liquidate_add_bank(scout_valid_bank_config(10)) {
            Some(v) => v,
            None => return false,
        };
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return false;
        }

        let liquidity_provider = match self.scout_create_initialized_marginfi_account() {
            Some(v) => v,
            None => return false,
        };
        let borrower = match self.scout_create_initialized_marginfi_account() {
            Some(v) => v,
            None => return false,
        };
        if !self.scout_liquidate_deposit(
            liquidity_provider,
            liab_bank,
            SCOUT_BORROW_LIQUIDITY_DEPOSIT_AMOUNT,
        ) {
            return false;
        }
        if !self.scout_liquidate_deposit(
            borrower,
            asset_bank,
            SCOUT_BORROW_COLLATERAL_DEPOSIT_AMOUNT,
        ) {
            return false;
        }

        let remaining = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            vec![asset_bank, liab_bank]
        } else {
            vec![liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(borrower, liab_bank, SCOUT_BORROW_AMOUNT, remaining.clone())
        {
            return false;
        }

        self.borrow_asset_bank = asset_bank;
        self.borrow_liab_bank = liab_bank;
        self.borrow_marginfi_account = borrower;
        self.borrow_remaining_accounts = remaining;
        for account in [liquidity_provider, borrower] {
            self.scout_known_accounts[self.scout_known_next % SCOUT_KNOWN_CAP] = account;
            self.scout_known_next = self.scout_known_next.saturating_add(1);
        }
        true
    }

    // The `bank` binding for the generated `action_lending_account_repay`.
    fn scout_borrow_scenario_ensure_liability(&mut self) -> Option<Pubkey> {
        let account = self.borrow_marginfi_account;
        let bank = self.borrow_liab_bank;
        if account == Pubkey::default() || bank == Pubkey::default() {
            return None;
        }
        let remaining = self.borrow_remaining_accounts.clone();
        if !self.scout_liquidate_borrow(account, bank, SCOUT_BORROW_AMOUNT, remaining) {
            return None;
        }
        Some(bank)
    }

    // Keeps `BankAccountWrapper::repay`'s PARTIAL branch covered.
    pub fn action_lending_account_repay_partial_borrow_scenario(&mut self) -> bool {
        let bank = match self.scout_borrow_scenario_ensure_liability() {
            Some(v) => v,
            None => return false,
        };
        let group = self.marginfi_group;
        let marginfi_account = self.borrow_marginfi_account;
        let authority = self.payer.pubkey();
        let signer_token_account = self.signer_token_account;
        let liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountRepay {
                amount: SCOUT_BORROW_REPAY_PARTIAL_AMOUNT,
                repay_all: Some(false),
            })
            .accounts(accounts::LendingAccountRepay {
                group,
                marginfi_account,
                authority,
                bank,
                signer_token_account,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    // The `bank` binding for the generated `action_lending_pool_pulse_bank_price_cache`.
    fn scout_pulse_bank_price_cache_target(&self) -> Option<Pubkey> {
        if self.borrow_liab_bank == Pubkey::default() {
            return None;
        }
        if !self.ctx.account_exists(&self.borrow_liab_bank) {
            return None;
        }
        Some(self.borrow_liab_bank)
    }
    /// Mint a fresh `stake_pool -> lst_mint -> sol_pool` triple for `action_lending_pool_add_bank_permissionless`; returns `lst_mint`.
    fn scout_prepare_add_bank_permissionless(&mut self) -> Option<Pubkey> {
        let stake_pool = Pubkey::new_unique();
        let (lst_mint, _) =
            Pubkey::find_program_address(&[b"mint", stake_pool.as_ref()], &spl_single_pool_id());
        let (sol_pool, _) =
            Pubkey::find_program_address(&[b"stake", stake_pool.as_ref()], &spl_single_pool_id());
        self.ctx
            .create_account()
            .pubkey(stake_pool)
            .owner(spl_single_pool_id())
            .lamports(1_000_000)
            .create()
            .ok()?;
        self.ctx
            .create_mint()
            .pubkey(lst_mint)
            .decimals(9)
            .mint_authority(self.payer.pubkey())
            .create()
            .ok()?;
        self.ctx
            .create_account()
            .pubkey(sol_pool)
            .owner(native_stake_id())
            .lamports(1_000_000)
            .create()
            .ok()?;
        self.perm_stake_pool = stake_pool;
        Some(lst_mint)
    }
    // ---- P-0037 / P-0038 probe machinery ---------------------------------------------------
    fn scout_hp_maintenance_health(&self, account: Pubkey) -> Option<fixed::types::I80F48> {
        let data = self.ctx.read_account(&account).ok()?.data;
        if data.len() != SCOUT_HP_ACCOUNT_LEN || data[..8] != SCOUT_HP_ACCOUNT_DISCRIMINATOR {
            return None;
        }
        let flag_bytes: [u8; 8] = data
            [SCOUT_HP_ACCOUNT_FLAGS_OFFSET..SCOUT_HP_ACCOUNT_FLAGS_OFFSET + 8]
            .try_into()
            .ok()?;
        if u64::from_le_bytes(flag_bytes) & SCOUT_HP_ACCOUNT_EXEMPT_FLAGS != 0 {
            return None;
        }
        let group_bytes: [u8; 32] = data
            [SCOUT_HP_ACCOUNT_GROUP_OFFSET..SCOUT_HP_ACCOUNT_GROUP_OFFSET + 32]
            .try_into()
            .ok()?;
        let group = Pubkey::new_from_array(group_bytes);

        let one = fixed::types::I80F48::from_num(1);
        let mut assets = fixed::types::I80F48::from_num(0);
        let mut liabilities = fixed::types::I80F48::from_num(0);
        for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
            let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
            if data.len() < base + SCOUT_BALANCE_STRIDE {
                break;
            }
            if data[base] == 0 {
                continue;
            }
            if data[base + 33] != SCOUT_HP_ASSET_TAG_DEFAULT {
                return None;
            }
            let bank_bytes: [u8; 32] = data[base + 1..base + 33].try_into().ok()?;
            let bank_pk = Pubkey::new_from_array(bank_bytes);
            let bank_data = self.ctx.read_account(&bank_pk).ok()?.data;
            if bank_data.len() != SCOUT_HP_BANK_LEN || bank_data[..8] != SCOUT_HP_BANK_DISCRIMINATOR
            {
                return None;
            }
            let bank_group: [u8; 32] = bank_data
                [SCOUT_HP_BANK_GROUP_OFFSET..SCOUT_HP_BANK_GROUP_OFFSET + 32]
                .try_into()
                .ok()?;
            if Pubkey::new_from_array(bank_group) != group {
                return None;
            }
            if bank_data[SCOUT_HP_BANK_ORACLE_SETUP_OFFSET] != SCOUT_HP_ORACLE_SETUP_FIXED
                || bank_data[SCOUT_HP_BANK_ASSET_TAG_OFFSET] != SCOUT_HP_ASSET_TAG_DEFAULT
            {
                return None;
            }
            let emode_tag: [u8; 2] = bank_data
                [SCOUT_HP_BANK_EMODE_TAG_OFFSET..SCOUT_HP_BANK_EMODE_TAG_OFFSET + 2]
                .try_into()
                .ok()?;
            let emode_flags: [u8; 8] = bank_data
                [SCOUT_HP_BANK_EMODE_FLAGS_OFFSET..SCOUT_HP_BANK_EMODE_FLAGS_OFFSET + 8]
                .try_into()
                .ok()?;
            if u16::from_le_bytes(emode_tag) != 0 || u64::from_le_bytes(emode_flags) != 0 {
                return None;
            }
            let decimals = bank_data[SCOUT_HP_BANK_MINT_DECIMALS_OFFSET];
            if decimals > 24 {
                return None;
            }
            let scale =
                fixed::types::I80F48::checked_from_num(SCOUT_HP_EXP_10[decimals as usize])?;
            let price_bytes: [u8; 16] = bank_data
                [SCOUT_HP_BANK_FIXED_PRICE_OFFSET..SCOUT_HP_BANK_FIXED_PRICE_OFFSET + 16]
                .try_into()
                .ok()?;
            let price = fixed::types::I80F48::from_le_bytes(price_bytes);
            let asv_bytes: [u8; 16] = bank_data
                [SCOUT_HP_BANK_ASSET_SHARE_VALUE_OFFSET..SCOUT_HP_BANK_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?;
            let asset_share_value = fixed::types::I80F48::from_le_bytes(asv_bytes);
            let lsv_bytes: [u8; 16] = bank_data[SCOUT_HP_BANK_LIABILITY_SHARE_VALUE_OFFSET
                ..SCOUT_HP_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?;
            let liability_share_value = fixed::types::I80F48::from_le_bytes(lsv_bytes);
            let awm_bytes: [u8; 16] = bank_data[SCOUT_HP_BANK_ASSET_WEIGHT_MAINT_OFFSET
                ..SCOUT_HP_BANK_ASSET_WEIGHT_MAINT_OFFSET + 16]
                .try_into()
                .ok()?;
            let asset_weight_maint = fixed::types::I80F48::from_le_bytes(awm_bytes);
            let lwm_bytes: [u8; 16] = bank_data[SCOUT_HP_BANK_LIABILITY_WEIGHT_MAINT_OFFSET
                ..SCOUT_HP_BANK_LIABILITY_WEIGHT_MAINT_OFFSET + 16]
                .try_into()
                .ok()?;
            let liability_weight_maint = fixed::types::I80F48::from_le_bytes(lwm_bytes);
            let effective_asset_weight =
                if bank_data[SCOUT_HP_BANK_RISK_TIER_OFFSET] == SCOUT_HP_RISK_TIER_ISOLATED {
                    fixed::types::I80F48::from_num(0)
                } else {
                    asset_weight_maint
                };

            let asset_share_bytes: [u8; 16] = data[base + 40..base + 56].try_into().ok()?;
            let asset_shares = fixed::types::I80F48::from_le_bytes(asset_share_bytes);
            let liability_share_bytes: [u8; 16] = data[base + 56..base + 72].try_into().ok()?;
            let liability_shares = fixed::types::I80F48::from_le_bytes(liability_share_bytes);

            if liability_shares >= one {
                let amount = liability_shares.checked_mul(liability_share_value)?;
                let value = amount
                    .checked_mul(liability_weight_maint)?
                    .checked_mul(price)?
                    .checked_div(scale)?;
                liabilities = liabilities.checked_add(value)?;
            } else if asset_shares >= one {
                let amount = asset_shares.checked_mul(asset_share_value)?;
                let value = amount
                    .checked_mul(effective_asset_weight)?
                    .checked_mul(price)?
                    .checked_div(scale)?;
                assets = assets.checked_add(value)?;
            }
        }
        assets.checked_sub(liabilities)
    }

    /// Build a fresh, maintenance-liquidatable MarginfiAccount from real instructions.
    fn scout_hp_build_liquidatable(&mut self) -> Option<(Pubkey, Pubkey, Pubkey, [Pubkey; 2])> {
        let asset_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let liab_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let liquidity_provider = self.scout_create_initialized_marginfi_account()?;
        let borrower = self.scout_create_initialized_marginfi_account()?;
        if !self.scout_liquidate_deposit(
            liquidity_provider,
            liab_bank,
            SCOUT_HP_LIQUIDITY_DEPOSIT_AMOUNT,
        ) {
            return None;
        }
        if !self.scout_liquidate_deposit(borrower, asset_bank, SCOUT_HP_COLLATERAL_DEPOSIT_AMOUNT) {
            return None;
        }
        let sorted = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(
            borrower,
            liab_bank,
            SCOUT_HP_BORROW_AMOUNT,
            sorted.to_vec(),
        ) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(
            asset_bank,
            fixed::types::I80F48::from_num(SCOUT_HP_CRASHED_PRICE),
        ) {
            return None;
        }
        Some((borrower, asset_bank, liab_bank, sorted))
    }

    /// P-0037 / P-0038's driver: build a liquidatable account, snapshot health, then send one fuzzer-selected instruction.
    pub fn action_health_probe_while_liquidatable(&mut self, choice: u8) -> bool {
        let (account, asset_bank, liab_bank, sorted) = match self.scout_hp_build_liquidatable() {
            Some(v) => v,
            None => return false,
        };
        let pre = self.scout_hp_maintenance_health(account);
        let kind = choice % SCOUT_HP_KIND_COUNT + 1;
        self.scout_hp_subject = account;
        self.scout_hp_kind = kind;
        self.scout_hp_pre_valid = pre.is_some();
        self.scout_hp_pre_health = pre.unwrap_or(fixed::types::I80F48::ZERO).to_bits();
        self.scout_hp_succeeded = false;

        let group = self.marginfi_group;
        let authority = self.payer.pubkey();
        let signer_token_account = self.signer_token_account;
        let asset_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, asset_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let asset_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, asset_bank.as_ref()], &self.program_id)
                .0;
        let liab_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, liab_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liab_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id)
                .0;

        let succeeded = if kind == SCOUT_HP_KIND_WITHDRAW || kind == SCOUT_HP_KIND_WITHDRAW_ALL {
            let withdraw_all = if kind == SCOUT_HP_KIND_WITHDRAW_ALL {
                Some(true)
            } else {
                None
            };
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountWithdraw {
                    amount: SCOUT_HP_PROBE_WITHDRAW_AMOUNT,
                    withdraw_all,
                })
                .accounts(accounts::LendingAccountWithdraw {
                    group,
                    marginfi_account: account,
                    authority,
                    bank: asset_bank,
                    destination_token_account: signer_token_account,
                    bank_liquidity_vault_authority: asset_vault_authority,
                    liquidity_vault: asset_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if kind == SCOUT_HP_KIND_BORROW {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountBorrow {
                    amount: SCOUT_HP_PROBE_BORROW_AMOUNT,
                })
                .accounts(accounts::LendingAccountBorrow {
                    group,
                    marginfi_account: account,
                    authority,
                    bank: liab_bank,
                    destination_token_account: signer_token_account,
                    bank_liquidity_vault_authority: liab_vault_authority,
                    liquidity_vault: liab_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if kind == SCOUT_HP_KIND_DEPOSIT {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountDeposit {
                    amount: SCOUT_HP_PROBE_DEPOSIT_AMOUNT,
                    deposit_up_to_limit: None,
                })
                .accounts(accounts::LendingAccountDeposit {
                    group,
                    marginfi_account: account,
                    authority,
                    bank: asset_bank,
                    signer_token_account,
                    liquidity_vault: asset_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if kind == SCOUT_HP_KIND_REPAY {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountRepay {
                    amount: SCOUT_HP_PROBE_REPAY_AMOUNT,
                    repay_all: Some(false),
                })
                .accounts(accounts::LendingAccountRepay {
                    group,
                    marginfi_account: account,
                    authority,
                    bank: liab_bank,
                    signer_token_account,
                    liquidity_vault: liab_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if kind == SCOUT_HP_KIND_PULSE_HEALTH {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountPulseHealth {})
                .accounts(accounts::LendingAccountPulseHealth {
                    marginfi_account: account,
                    group: self.marginfi_group,
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else {
            false
        };
        self.scout_hp_succeeded = succeeded;
        succeeded
    }

    // ---- CLOSE / RE-OCCUPY, DECOMPOSED INTO ONE-INSTRUCTION PRIMITIVES ----------------------

    // PRIMITIVE 1/4 -- mint a fresh MarginfiAccount and RETAIN its keypair.
    pub fn action_retain_new_marginfi_account(&mut self) -> bool {
        if self.scout_reoccupy_keypair.is_some() {
            return false;
        }
        let account_keypair = Rc::new(Keypair::new());
        let marginfi_account = account_keypair.pubkey();
        let payer = self.payer.clone();
        let sent = self
            .ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitialize {})
            .accounts(accounts::MarginfiAccountInitialize {
                marginfi_group: self.marginfi_group,
                marginfi_account,
                authority: payer.pubkey(),
                fee_payer: payer.pubkey(),
            })
            .signers(&[&*payer, &*account_keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !sent {
            return false;
        }
        self.scout_reoccupy_keypair = Some(account_keypair);
        true
    }

    // PRIMITIVE 2/4 -- give the retained account a LiquidationRecord via the real instruction.
    pub fn action_init_liq_record_on_retained(&mut self) -> bool {
        let account_keypair = match self.scout_reoccupy_keypair.clone() {
            Some(keypair) => keypair,
            None => return false,
        };
        let marginfi_account = account_keypair.pubkey();
        let liquidation_record = scout_liquidation_record_pda(self.program_id, marginfi_account);
        let payer = self.payer.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitLiqRecord {})
            .accounts(accounts::MarginfiAccountInitLiqRecord {
                marginfi_account,
                fee_payer: payer.pubkey(),
                liquidation_record,
            })
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    // PRIMITIVE 3/4 -- close the retained account.
    pub fn action_close_retained(&mut self) -> bool {
        let account_keypair = match self.scout_reoccupy_keypair.clone() {
            Some(keypair) => keypair,
            None => return false,
        };
        let marginfi_account = account_keypair.pubkey();
        let payer = self.payer.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountClose {})
            .accounts(accounts::MarginfiAccountClose {
                marginfi_account,
                authority: payer.pubkey(),
                fee_payer: payer.pubkey(),
            })
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    // PRIMITIVE 4/4 -- re-init a MarginfiAccount at the SAME address/keypair.
    pub fn action_reinitialize_retained(&mut self) -> bool {
        let account_keypair = match self.scout_reoccupy_keypair.clone() {
            Some(keypair) => keypair,
            None => return false,
        };
        let marginfi_account = account_keypair.pubkey();
        let payer = self.payer.clone();
        self.ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitialize {})
            .accounts(accounts::MarginfiAccountInitialize {
                marginfi_group: self.marginfi_group,
                marginfi_account,
                authority: payer.pubkey(),
                fee_payer: payer.pubkey(),
            })
            .signers(&[&*payer, &*account_keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }
    // ---- P-0033 cumulative-extraction probe machinery ---------------------------------------

    /// `(asset_share_value, liability_share_value, fixed price, 10^mint_decimals)` for `bank`, or `None` if unmodelable.
    fn scout_p33_bank_mark(
        &self,
        bank: Pubkey,
    ) -> Option<(
        fixed::types::I80F48,
        fixed::types::I80F48,
        fixed::types::I80F48,
        fixed::types::I80F48,
    )> {
        let data = self.ctx.read_account(&bank).ok()?.data;
        if data.len() != SCOUT_HP_BANK_LEN || data[..8] != SCOUT_HP_BANK_DISCRIMINATOR {
            return None;
        }
        if data[SCOUT_HP_BANK_ORACLE_SETUP_OFFSET] != SCOUT_HP_ORACLE_SETUP_FIXED
            || data[SCOUT_HP_BANK_ASSET_TAG_OFFSET] != SCOUT_HP_ASSET_TAG_DEFAULT
        {
            return None;
        }
        let decimals = data[SCOUT_HP_BANK_MINT_DECIMALS_OFFSET] as usize;
        if decimals >= SCOUT_HP_EXP_10.len() {
            return None;
        }
        let scale = fixed::types::I80F48::checked_from_num(SCOUT_HP_EXP_10[decimals])?;
        let asv_bytes: [u8; 16] = data[SCOUT_HP_BANK_ASSET_SHARE_VALUE_OFFSET
            ..SCOUT_HP_BANK_ASSET_SHARE_VALUE_OFFSET + 16]
            .try_into()
            .ok()?;
        let lsv_bytes: [u8; 16] = data[SCOUT_HP_BANK_LIABILITY_SHARE_VALUE_OFFSET
            ..SCOUT_HP_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16]
            .try_into()
            .ok()?;
        let price_bytes: [u8; 16] = data
            [SCOUT_HP_BANK_FIXED_PRICE_OFFSET..SCOUT_HP_BANK_FIXED_PRICE_OFFSET + 16]
            .try_into()
            .ok()?;
        Some((
            fixed::types::I80F48::from_le_bytes(asv_bytes),
            fixed::types::I80F48::from_le_bytes(lsv_bytes),
            fixed::types::I80F48::from_le_bytes(price_bytes),
            scale,
        ))
    }

    /// Raw `(asset_shares, liability_shares)` held by `account` in `bank`, summed over live balances; `None` if unmodelable.
    fn scout_p33_shares(
        &self,
        account: Pubkey,
        bank: Pubkey,
    ) -> Option<(fixed::types::I80F48, fixed::types::I80F48)> {
        let data = self.ctx.read_account(&account).ok()?.data;
        if data.len() != SCOUT_HP_ACCOUNT_LEN || data[..8] != SCOUT_HP_ACCOUNT_DISCRIMINATOR {
            return None;
        }
        let flag_bytes: [u8; 8] = data
            [SCOUT_HP_ACCOUNT_FLAGS_OFFSET..SCOUT_HP_ACCOUNT_FLAGS_OFFSET + 8]
            .try_into()
            .ok()?;
        if u64::from_le_bytes(flag_bytes) & SCOUT_HP_ACCOUNT_EXEMPT_FLAGS != 0 {
            return None;
        }
        let mut asset_shares = fixed::types::I80F48::ZERO;
        let mut liability_shares = fixed::types::I80F48::ZERO;
        for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
            let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
            if data.len() < base + SCOUT_BALANCE_STRIDE {
                break;
            }
            if data[base] == 0 {
                continue;
            }
            if data[base + 33] != SCOUT_HP_ASSET_TAG_DEFAULT {
                return None;
            }
            let bank_bytes: [u8; 32] = data[base + 1..base + 33].try_into().ok()?;
            if Pubkey::new_from_array(bank_bytes) != bank {
                continue;
            }
            let asset_bytes: [u8; 16] = data[base + 40..base + 56].try_into().ok()?;
            let liability_bytes: [u8; 16] = data[base + 56..base + 72].try_into().ok()?;
            asset_shares =
                asset_shares.checked_add(fixed::types::I80F48::from_le_bytes(asset_bytes))?;
            liability_shares = liability_shares
                .checked_add(fixed::types::I80F48::from_le_bytes(liability_bytes))?;
        }
        Some((asset_shares, liability_shares))
    }

    /// `(total asset value, total liability value)` of `account` across the two probe banks, at the given marks.
    fn scout_p33_account_value(
        &self,
        account: Pubkey,
        banks: &[(
            Pubkey,
            (
                fixed::types::I80F48,
                fixed::types::I80F48,
                fixed::types::I80F48,
                fixed::types::I80F48,
            ),
        )],
    ) -> Option<(fixed::types::I80F48, fixed::types::I80F48)> {
        let mut assets = fixed::types::I80F48::ZERO;
        let mut liabilities = fixed::types::I80F48::ZERO;
        for (bank, (asv, lsv, price, scale)) in banks.iter() {
            let (asset_shares, liability_shares) = self.scout_p33_shares(account, *bank)?;
            let asset_value = asset_shares
                .checked_mul(*asv)?
                .checked_mul(*price)?
                .checked_div(*scale)?;
            let liability_value = liability_shares
                .checked_mul(*lsv)?
                .checked_mul(*price)?
                .checked_div(*scale)?;
            assets = assets.checked_add(asset_value)?;
            liabilities = liabilities.checked_add(liability_value)?;
        }
        Some((assets, liabilities))
    }

    /// Asset-side value only, for one bank.
    fn scout_p33_asset_value(
        &self,
        account: Pubkey,
        bank: Pubkey,
        mark: (
            fixed::types::I80F48,
            fixed::types::I80F48,
            fixed::types::I80F48,
            fixed::types::I80F48,
        ),
    ) -> Option<fixed::types::I80F48> {
        let (asset_shares, _) = self.scout_p33_shares(account, bank)?;
        asset_shares
            .checked_mul(mark.0)?
            .checked_mul(mark.2)?
            .checked_div(mark.3)
    }

    /// One real `lending_account_liquidate` call.
    fn scout_p33_liquidate(
        &mut self,
        asset_bank: Pubkey,
        liab_bank: Pubkey,
        liquidator: Pubkey,
        liquidatee: Pubkey,
        sorted_pair: [Pubkey; 2],
        asset_amount: u64,
    ) -> bool {
        let bank_liquidity_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, liab_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let bank_liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()], &self.program_id)
                .0;
        let bank_insurance_vault =
            Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, liab_bank.as_ref()], &self.program_id)
                .0;
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountLiquidate {
                asset_amount,
                liquidatee_accounts: 2,
                liquidator_accounts: 2,
            })
            .accounts(accounts::LendingAccountLiquidate {
                group: self.marginfi_group,
                asset_bank,
                liab_bank,
                liquidator_marginfi_account: liquidator,
                authority: self.payer.pubkey(),
                liquidatee_marginfi_account: liquidatee,
                bank_liquidity_vault_authority,
                bank_liquidity_vault,
                bank_insurance_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(vec![
                sorted_pair[0],
                sorted_pair[1],
                sorted_pair[0],
                sorted_pair[1],
            ])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    /// P-0021/P-0023's per-round recorder: fold one measured liquidation into the two single-liquidation memos.
    fn scout_p2123_record_round(
        &mut self,
        liquidator: Pubkey,
        liquidatee: Pubkey,
        arm: u8,
        gross: fixed::types::I80F48,
        gain: fixed::types::I80F48,
        loss: fixed::types::I80F48,
    ) {
        let gain_allowance = match gross.checked_mul(SCOUT_P33_LIQUIDATOR_FEE) {
            Some(value) => value,
            None => return,
        };
        let loss_allowance = match gross.checked_mul(SCOUT_P33_LIQUIDATOR_PLUS_INSURANCE_FEE) {
            Some(value) => value,
            None => return,
        };
        let gain_excess = match gain.checked_sub(gain_allowance) {
            Some(value) => value,
            None => return,
        };
        let loss_excess = match loss.checked_sub(loss_allowance) {
            Some(value) => value,
            None => return,
        };

        // P-0023 -- liquidator side.
        let stored_gain_gross = fixed::types::I80F48::from_bits(self.scout_p23_gross_bits);
        let stored_gain_excess = match stored_gain_gross
            .checked_mul(SCOUT_P33_LIQUIDATOR_FEE)
            .and_then(|allowance| {
                fixed::types::I80F48::from_bits(self.scout_p23_gain_bits).checked_sub(allowance)
            }) {
            Some(value) => value,
            None => fixed::types::I80F48::MIN,
        };
        if !self.scout_p23_valid || gain_excess > stored_gain_excess {
            self.scout_p23_gain_bits = gain.to_bits();
            self.scout_p23_gross_bits = gross.to_bits();
            self.scout_p23_arm = arm;
            self.scout_p23_liquidator = liquidator;
            self.scout_p23_liquidatee = liquidatee;
        }
        self.scout_p23_rounds = self.scout_p23_rounds.saturating_add(1);
        self.scout_p23_valid = true;

        // P-0021 -- liquidatee side.
        let stored_loss_gross = fixed::types::I80F48::from_bits(self.scout_p21_gross_bits);
        let stored_loss_excess = match stored_loss_gross
            .checked_mul(SCOUT_P33_LIQUIDATOR_PLUS_INSURANCE_FEE)
            .and_then(|allowance| {
                fixed::types::I80F48::from_bits(self.scout_p21_loss_bits).checked_sub(allowance)
            }) {
            Some(value) => value,
            None => fixed::types::I80F48::MIN,
        };
        if !self.scout_p21_valid || loss_excess > stored_loss_excess {
            self.scout_p21_loss_bits = loss.to_bits();
            self.scout_p21_gross_bits = gross.to_bits();
            self.scout_p21_arm = arm;
            self.scout_p21_liquidator = liquidator;
            self.scout_p21_liquidatee = liquidatee;
        }
        self.scout_p21_rounds = self.scout_p21_rounds.saturating_add(1);
        self.scout_p21_valid = true;
    }

    /// P-0033's driver: build a fresh liquidatable pair, run 1..=3 real liquidations, folding each into the running accumulator.
    pub fn action_liquidation_extraction_probe(&mut self, choice: u8) -> bool {
        let rounds = choice % SCOUT_P33_MAX_ROUNDS + 1;
        let arm = (choice / SCOUT_P33_MAX_ROUNDS) % 3;

        let asset_bank = match self.scout_liquidate_add_bank(scout_liquidation_bank_config()) {
            Some(v) => v,
            None => return false,
        };
        let liab_bank = match self.scout_liquidate_add_bank(scout_valid_bank_config(10)) {
            Some(v) => v,
            None => return false,
        };
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return false;
        }
        let liquidity_provider = match self.scout_create_initialized_marginfi_account() {
            Some(v) => v,
            None => return false,
        };
        let liquidator = match self.scout_create_initialized_marginfi_account() {
            Some(v) => v,
            None => return false,
        };
        let liquidatee = match self.scout_create_initialized_marginfi_account() {
            Some(v) => v,
            None => return false,
        };
        if liquidator == liquidatee {
            return false;
        }
        let forged = scout_forged_bank_and_account_pdas(self.program_id);
        for excluded in forged.iter() {
            if *excluded == asset_bank
                || *excluded == liab_bank
                || *excluded == liquidator
                || *excluded == liquidatee
            {
                return false;
            }
        }

        if !self.scout_liquidate_deposit(
            liquidity_provider,
            liab_bank,
            SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT,
        ) {
            return false;
        }
        if !self.scout_liquidate_deposit(
            liquidatee,
            asset_bank,
            SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT,
        ) {
            return false;
        }
        if !self.scout_liquidate_deposit(
            liquidator,
            asset_bank,
            SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT,
        ) {
            return false;
        }
        let liquidator_liab_seed = match arm {
            1 => SCOUT_P33_LIQUIDATOR_LIAB_DEPOSIT_AMOUNT,
            2 => SCOUT_P2123_LIQUIDATOR_LIAB_PARTIAL_AMOUNT,
            _ => 0,
        };
        if liquidator_liab_seed > 0
            && !self.scout_liquidate_deposit(liquidator, liab_bank, liquidator_liab_seed)
        {
            return false;
        }
        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(
            liquidatee,
            liab_bank,
            SCOUT_P33_BORROW_AMOUNT,
            sorted_pair.to_vec(),
        ) {
            return false;
        }
        if !self.scout_liquidate_set_fixed_price(
            asset_bank,
            fixed::types::I80F48::from_num(SCOUT_P33_CRASHED_PRICE),
        ) {
            return false;
        }

        let mut measured_any = false;
        for _round in 0..rounds {
            let pre_asset_mark = match self.scout_p33_bank_mark(asset_bank) {
                Some(v) => v,
                None => break,
            };
            let pre_liab_mark = match self.scout_p33_bank_mark(liab_bank) {
                Some(v) => v,
                None => break,
            };
            let pre_marks = [(asset_bank, pre_asset_mark), (liab_bank, pre_liab_mark)];
            let pre_liquidator = match self.scout_p33_account_value(liquidator, &pre_marks) {
                Some(v) => v,
                None => break,
            };
            let pre_liquidatee = match self.scout_p33_account_value(liquidatee, &pre_marks) {
                Some(v) => v,
                None => break,
            };
            let pre_collateral =
                match self.scout_p33_asset_value(liquidatee, asset_bank, pre_asset_mark) {
                    Some(v) => v,
                    None => break,
                };

            if !self.scout_p33_liquidate(
                asset_bank,
                liab_bank,
                liquidator,
                liquidatee,
                sorted_pair,
                SCOUT_P33_ASSET_AMOUNT,
            ) {
                break;
            }

            let post_asset_mark = match self.scout_p33_bank_mark(asset_bank) {
                Some(v) => v,
                None => break,
            };
            let post_liab_mark = match self.scout_p33_bank_mark(liab_bank) {
                Some(v) => v,
                None => break,
            };
            if pre_asset_mark != post_asset_mark || pre_liab_mark != post_liab_mark {
                continue;
            }
            let post_marks = [(asset_bank, post_asset_mark), (liab_bank, post_liab_mark)];
            let post_liquidator = match self.scout_p33_account_value(liquidator, &post_marks) {
                Some(v) => v,
                None => break,
            };
            let post_liquidatee = match self.scout_p33_account_value(liquidatee, &post_marks) {
                Some(v) => v,
                None => break,
            };
            let post_collateral =
                match self.scout_p33_asset_value(liquidatee, asset_bank, post_asset_mark) {
                    Some(v) => v,
                    None => break,
                };

            let pre_liquidator_net = match pre_liquidator.0.checked_sub(pre_liquidator.1) {
                Some(v) => v,
                None => break,
            };
            let post_liquidator_net = match post_liquidator.0.checked_sub(post_liquidator.1) {
                Some(v) => v,
                None => break,
            };
            let pre_liquidatee_net = match pre_liquidatee.0.checked_sub(pre_liquidatee.1) {
                Some(v) => v,
                None => break,
            };
            let post_liquidatee_net = match post_liquidatee.0.checked_sub(post_liquidatee.1) {
                Some(v) => v,
                None => break,
            };
            let gross = match pre_collateral.checked_sub(post_collateral) {
                Some(v) => v,
                None => break,
            };
            let gain = match post_liquidator_net.checked_sub(pre_liquidator_net) {
                Some(v) => v,
                None => break,
            };
            let loss = match pre_liquidatee_net.checked_sub(post_liquidatee_net) {
                Some(v) => v,
                None => break,
            };
            let zero = fixed::types::I80F48::ZERO;
            let gross_term = if gross > zero { gross } else { zero };
            let gain_term = if gain > zero { gain } else { zero };
            let loss_term = if loss > zero { loss } else { zero };

            let next_gross = match fixed::types::I80F48::from_bits(self.scout_p33_gross_bits)
                .checked_add(gross_term)
            {
                Some(v) => v,
                None => break,
            };
            let next_gain = match fixed::types::I80F48::from_bits(self.scout_p33_gain_bits)
                .checked_add(gain_term)
            {
                Some(v) => v,
                None => break,
            };
            let next_loss = match fixed::types::I80F48::from_bits(self.scout_p33_loss_bits)
                .checked_add(loss_term)
            {
                Some(v) => v,
                None => break,
            };
            self.scout_p33_gross_bits = next_gross.to_bits();
            self.scout_p33_gain_bits = next_gain.to_bits();
            self.scout_p33_loss_bits = next_loss.to_bits();
            self.scout_p33_rounds = self.scout_p33_rounds.saturating_add(1);
            if gain_term > fixed::types::I80F48::from_bits(self.scout_p33_worst_gain_bits) {
                self.scout_p33_worst_gain_bits = gain_term.to_bits();
                self.scout_p33_worst_gross_bits = gross_term.to_bits();
            }
            self.scout_p33_liquidator = liquidator;
            self.scout_p33_liquidatee = liquidatee;
            self.scout_p33_valid = true;
            self.scout_p2123_record_round(
                liquidator,
                liquidatee,
                arm,
                gross_term,
                gain_term,
                loss_term,
            );
            measured_any = true;
        }
        measured_any
    }
    // ---------------------------------------------------------------------------------------
    // P-0029 (fee conservation) instrumentation.
    fn scout_p29_read_bank(&self, bank: &Pubkey) -> Option<(i128, i128, i128, i64, [u8; 16])> {
        const P29_BANK_LEN: usize = 8 + 1856;
        const P29_INSURANCE: usize = 8 + 176;
        const P29_GROUP: usize = 8 + 232;
        const P29_PROGRAM: usize = 8 + 896;
        const P29_LAST_UPDATE: usize = 8 + 280;
        const P29_ORIGINATION_FEE: usize = 8 + 288 + 72 + 7 * 16;
        let data = self.ctx.account_data(bank).ok()?;
        if data.len() != P29_BANK_LEN {
            return None;
        }
        let insurance = i128::from_le_bytes(
            data[P29_INSURANCE..P29_INSURANCE + 16].try_into().ok()?,
        );
        let group = i128::from_le_bytes(data[P29_GROUP..P29_GROUP + 16].try_into().ok()?);
        let program = i128::from_le_bytes(data[P29_PROGRAM..P29_PROGRAM + 16].try_into().ok()?);
        let last_update =
            i64::from_le_bytes(data[P29_LAST_UPDATE..P29_LAST_UPDATE + 8].try_into().ok()?);
        let origination: [u8; 16] = data
            [P29_ORIGINATION_FEE..P29_ORIGINATION_FEE + 16]
            .try_into()
            .ok()?;
        Some((insurance, group, program, last_update, origination))
    }

    // SPL token balance at the canonical `amount` offset.
    fn scout_p29_read_token_amount(&self, account: &Pubkey) -> Option<u64> {
        let data = self.ctx.account_data(account).ok()?;
        if data.len() < SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET + 8 {
            return None;
        }
        Some(u64::from_le_bytes(
            data[SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET
                ..SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET + 8]
                .try_into()
                .ok()?,
        ))
    }

    // P-0029's probe: run the two real instructions that move the three fee counters in opposite directions, against `self.fee_bank`.
    pub fn action_fee_conservation_probe(&mut self) -> bool {
        const P29_GROUP_LEN: usize = 8 + 1056;
        const P29_GROUP_PROGRAM_FEE_RATE: usize = 8 + 40 + 48;
        const P29_BORROW_AMOUNT: u64 = 1_000_000;

        self.scout_p29_gen_valid = false;
        self.scout_p29_gen_exact = false;
        self.scout_p29_pay_valid = false;

        let bank = self.fee_bank;
        let group = self.marginfi_group;
        if bank == Pubkey::default() {
            return false;
        }

        let program_fee_rate = match self.ctx.account_data(&group) {
            Ok(data) if data.len() == P29_GROUP_LEN => {
                match data[P29_GROUP_PROGRAM_FEE_RATE..P29_GROUP_PROGRAM_FEE_RATE + 16].try_into()
                {
                    Ok(buf) => Some(fixed::types::I80F48::from_le_bytes(buf)),
                    Err(_) => None,
                }
            }
            _ => None,
        };
        let gen_pre = self.scout_p29_read_bank(&bank);
        let borrowed = self.action_lending_account_borrow_with_origination_fee();
        let gen_post = self.scout_p29_read_bank(&bank);
        if borrowed {
            if let (Some(pre), Some(post), Some(rate)) = (gen_pre, gen_post, program_fee_rate) {
                let origination_rate = fixed::types::I80F48::from_le_bytes(pre.4);
                let origination_fee = if origination_rate == fixed::types::I80F48::ZERO {
                    Some(fixed::types::I80F48::ZERO)
                } else {
                    fixed::types::I80F48::from_num(P29_BORROW_AMOUNT)
                        .checked_mul(origination_rate)
                };
                if let Some(fee) = origination_fee {
                    let program_part = if fee == fixed::types::I80F48::ZERO {
                        Some(fixed::types::I80F48::ZERO)
                    } else if rate == fixed::types::I80F48::ZERO {
                        Some(fixed::types::I80F48::ZERO)
                    } else {
                        fee.checked_mul(rate)
                    };
                    if let Some(program_part) = program_part {
                        let group_part = fee.saturating_sub(program_part);
                        self.scout_p29_gen_expect_group = group_part.to_bits();
                        self.scout_p29_gen_expect_program = program_part.to_bits();
                        self.scout_p29_gen_delta_group = post.1.saturating_sub(pre.1);
                        self.scout_p29_gen_delta_program = post.2.saturating_sub(pre.2);
                        self.scout_p29_gen_delta_insurance = post.0.saturating_sub(pre.0);
                        self.scout_p29_gen_exact = pre.3 == post.3;
                        self.scout_p29_gen_valid = true;
                    }
                }
            }
        }

        let fee_ata = scout_associated_token_address(
            &self.global_fee_wallet,
            &self.bank_mint,
            &spl_token::id(),
        );
        if self.ctx.svm.get_account(&fee_ata).is_none() {
            if self
                .ctx
                .create_token_account()
                .pubkey(fee_ata)
                .mint(self.bank_mint)
                .token_owner(self.global_fee_wallet)
                .amount(0)
                .create()
                .is_err()
            {
                return borrowed;
            }
        }
        let liquidity_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liquidity_vault =
            Pubkey::find_program_address(&[LIQUIDITY_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let insurance_vault =
            Pubkey::find_program_address(&[INSURANCE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let fee_vault =
            Pubkey::find_program_address(&[FEE_VAULT_SEED, bank.as_ref()], &self.program_id).0;
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;

        let pay_pre_bank = self.scout_p29_read_bank(&bank);
        let pay_pre_insurance = self.scout_p29_read_token_amount(&insurance_vault);
        let pay_pre_fee = self.scout_p29_read_token_amount(&fee_vault);
        let pay_pre_ata = self.scout_p29_read_token_amount(&fee_ata);
        let pay_pre_liquidity = self.scout_p29_read_token_amount(&liquidity_vault);

        let collected = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolCollectBankFees {})
            .accounts(accounts::LendingPoolCollectBankFees {
                group: group,
                bank: bank,
                liquidity_vault_authority: liquidity_vault_authority,
                liquidity_vault: liquidity_vault,
                insurance_vault: insurance_vault,
                fee_vault: fee_vault,
                fee_state: fee_state,
                fee_ata: fee_ata,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);

        let pay_post_bank = self.scout_p29_read_bank(&bank);
        let pay_post_insurance = self.scout_p29_read_token_amount(&insurance_vault);
        let pay_post_fee = self.scout_p29_read_token_amount(&fee_vault);
        let pay_post_ata = self.scout_p29_read_token_amount(&fee_ata);
        let pay_post_liquidity = self.scout_p29_read_token_amount(&liquidity_vault);

        if let (
            Some(pre),
            Some(post),
            Some(pre_insurance),
            Some(post_insurance),
            Some(pre_fee),
            Some(post_fee),
            Some(pre_ata),
            Some(post_ata),
            Some(pre_liquidity),
            Some(post_liquidity),
        ) = (
            pay_pre_bank,
            pay_post_bank,
            pay_pre_insurance,
            pay_post_insurance,
            pay_pre_fee,
            pay_post_fee,
            pay_pre_ata,
            pay_post_ata,
            pay_pre_liquidity,
            pay_post_liquidity,
        ) {
            self.scout_p29_pay_succeeded = collected;
            self.scout_p29_pay_dec_insurance = pre.0.saturating_sub(post.0);
            self.scout_p29_pay_dec_group = pre.1.saturating_sub(post.1);
            self.scout_p29_pay_dec_program = pre.2.saturating_sub(post.2);
            self.scout_p29_pay_out_insurance = post_insurance.saturating_sub(pre_insurance);
            self.scout_p29_pay_out_group = post_fee.saturating_sub(pre_fee);
            self.scout_p29_pay_out_program = post_ata.saturating_sub(pre_ata);
            self.scout_p29_pay_liquidity_out = pre_liquidity.saturating_sub(post_liquidity);
            self.scout_p29_pay_valid = true;
        }

        borrowed || collected
    }

    /// P-0013's registry seeder, called from setup(). Records banks whose CLOSE_ENABLED_FLAG is set at that moment.
    fn scout_p13_seed_close_enabled_banks(&mut self) {
        const P13_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
        const P13_BANK_LEN: usize = 8 + 1856;

        let forged_banks = scout_forged_bank_pdas(self.program_id);

        let dirty: Vec<Pubkey> = self
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .iter()
            .copied()
            .collect();
        let mut seeded: usize = 0;
        for key in dirty {
            if seeded >= SCOUT_P13_BANK_CAP {
                break;
            }
            if forged_banks.contains(&key) {
                continue;
            }
            let flags = match self.ctx.account_data(&key) {
                Ok(data)
                    if data.len() == P13_BANK_LEN && data[..8] == P13_BANK_DISCRIMINATOR =>
                {
                    let buf: [u8; 8] = data
                        [SCOUT_BANK_FLAGS_OFFSET..SCOUT_BANK_FLAGS_OFFSET + 8]
                        .try_into()
                        .unwrap_or_default();
                    u64::from_le_bytes(buf)
                }
                _ => continue,
            };
            if flags & SCOUT_CLOSE_ENABLED_FLAG == 0 {
                continue;
            }
            self.scout_p13_banks[seeded] = key;
            seeded = seeded + 1;
        }
        self.scout_p13_bank_next = seeded.min(SCOUT_P13_BANK_CAP - 1);
    }
    /// P-0011's authorised-baseline seeder, called from setup(). Records each bank's flag word and the group's authority slots/flags.
    fn scout_p11_seed_authority_baseline(&mut self) {
        let forged_banks = scout_forged_bank_pdas(self.program_id);
        let dirty: Vec<Pubkey> = self
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .iter()
            .copied()
            .collect();
        let mut seeded: usize = 0;
        for key in dirty {
            if seeded >= SCOUT_P11_SEED_CAP {
                break;
            }
            if forged_banks.contains(&key) {
                continue;
            }
            let flags = match self.ctx.account_data(&key) {
                Ok(data)
                    if data.len() == SCOUT_P11_BANK_ACCOUNT_LEN
                        && data[..8] == SCOUT_P11_BANK_DISCRIMINATOR =>
                {
                    let buf: [u8; 8] = data
                        [SCOUT_BANK_FLAGS_OFFSET..SCOUT_BANK_FLAGS_OFFSET + 8]
                        .try_into()
                        .unwrap_or_default();
                    u64::from_le_bytes(buf)
                }
                _ => continue,
            };
            self.scout_p11_seed_bank[seeded] = key;
            self.scout_p11_seed_flags[seeded] = flags;
            seeded = seeded + 1;
        }
        self.scout_p11_seed_next = seeded.min(SCOUT_P11_SEED_CAP - 1);

        self.scout_p11_group = self.marginfi_group;
        self.scout_p11_refresh_group_authority_expectation(self.marginfi_group);
        self.scout_p11_refresh_group_flags_expectation(self.marginfi_group);
    }

    /// Move P-0011's shadow expectation for the seven MarginfiGroup authority slots to the account's current value.
    fn scout_p11_refresh_group_authority_expectation(&mut self, group: Pubkey) {
        if self.scout_p11_group == Pubkey::default() || group != self.scout_p11_group {
            return;
        }
        let data = match self.ctx.account_data(&group) {
            Ok(d) if d.len() == SCOUT_P11_GROUP_ACCOUNT_LEN => d,
            _ => return,
        };
        let mut authorities = [Pubkey::default(); 7];
        for slot in 0..7 {
            let offset = SCOUT_P11_GROUP_AUTH_OFFSETS[slot];
            let bytes: [u8; 32] = match data[offset..offset + 32].try_into() {
                Ok(b) => b,
                Err(_) => return,
            };
            authorities[slot] = Pubkey::new_from_array(bytes);
        }
        self.scout_p11_group_auth = authorities;
    }

    /// Same, for `group_flags`.
    fn scout_p11_refresh_group_flags_expectation(&mut self, group: Pubkey) {
        if self.scout_p11_group == Pubkey::default() || group != self.scout_p11_group {
            return;
        }
        let data = match self.ctx.account_data(&group) {
            Ok(d) if d.len() == SCOUT_P11_GROUP_ACCOUNT_LEN => d,
            _ => return,
        };
        let bytes: [u8; 8] = match data
            [SCOUT_P11_GROUP_FLAGS_OFFSET..SCOUT_P11_GROUP_FLAGS_OFFSET + 8]
            .try_into()
        {
            Ok(b) => b,
            Err(_) => return,
        };
        self.scout_p11_group_flags = u64::from_le_bytes(bytes);
    }
    // ---------------------------------------------------------------------------------------
    // P-0030 (fee monotonicity) instrumentation.
    fn scout_p30_read_fee_counters(&self, bank: &Pubkey) -> Option<(i128, i128, i128)> {
        let data = self.ctx.account_data(bank).ok()?;
        if data.len() != SCOUT_P30_BANK_LEN || data[..8] != SCOUT_P30_BANK_DISCRIMINATOR {
            return None;
        }
        let insurance = i128::from_le_bytes(
            data[SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET
                ..SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let group = i128::from_le_bytes(
            data[SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET..SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let program = i128::from_le_bytes(
            data[SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET
                ..SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        Some((insurance, group, program))
    }

    /// P-0030's subject registry + baseline writer. Called from setup() and from `action_fee_monotonicity_probe`.
    fn scout_p30_record_fee_baselines(&mut self) -> usize {
        let fee_bank = self.fee_bank;
        let forged_banks = scout_forged_bank_pdas(self.program_id);

        // 1. Release slots whose bank was closed.
        for slot in 0..SCOUT_P30_BANK_CAP {
            let bank = self.scout_p30_banks[slot];
            if bank == Pubkey::default() {
                continue;
            }
            let alive = match self.ctx.account_data(&bank) {
                Ok(data) => {
                    data.len() == SCOUT_P30_BANK_LEN && data[..8] == SCOUT_P30_BANK_DISCRIMINATOR
                }
                Err(_) => false,
            };
            if !alive {
                self.scout_p30_banks[slot] = Pubkey::default();
                self.scout_p30_insurance_bits[slot] = 0;
                self.scout_p30_group_bits[slot] = 0;
                self.scout_p30_program_bits[slot] = 0;
            }
        }

        // 2. Register newly-seen banks.
        let dirty: Vec<Pubkey> = self
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .iter()
            .copied()
            .collect();
        for key in dirty {
            if key == fee_bank || forged_banks.contains(&key) {
                continue;
            }
            if self.scout_p30_read_fee_counters(&key).is_none() {
                continue;
            }
            let mut known = false;
            let mut free_slot = SCOUT_P30_BANK_CAP;
            for slot in 0..SCOUT_P30_BANK_CAP {
                if self.scout_p30_banks[slot] == key {
                    known = true;
                    break;
                }
                if self.scout_p30_banks[slot] == Pubkey::default() && free_slot == SCOUT_P30_BANK_CAP
                {
                    free_slot = slot;
                }
            }
            if known || free_slot == SCOUT_P30_BANK_CAP {
                continue;
            }
            self.scout_p30_banks[free_slot] = key;
        }

        // 3. Refresh baselines.
        let mut recorded: usize = 0;
        for slot in 0..SCOUT_P30_BANK_CAP {
            let bank = self.scout_p30_banks[slot];
            if bank == Pubkey::default() {
                continue;
            }
            let counters = match self.scout_p30_read_fee_counters(&bank) {
                Some(values) => values,
                None => continue,
            };
            self.scout_p30_insurance_bits[slot] = counters.0;
            self.scout_p30_group_bits[slot] = counters.1;
            self.scout_p30_program_bits[slot] = counters.2;
            recorded = recorded + 1;
        }
        self.scout_p30_collect_seq_at_baseline = self.scout_p30_collect_seq;
        recorded
    }

    // ---------------------------------------------------------------------------------------
    // P-0014 (share-value monotonicity) / P-0016 (positive asset share value) instrumentation.
    fn scout_sv_read_share_values(&self, bank: &Pubkey) -> Option<(i128, i128)> {
        let data = self.ctx.account_data(bank).ok()?;
        if data.len() != SCOUT_SV_BANK_LEN || data[..8] != SCOUT_SV_BANK_DISCRIMINATOR {
            return None;
        }
        let asset = i128::from_le_bytes(
            data[SCOUT_SV_ASSET_SHARE_VALUE_OFFSET..SCOUT_SV_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let liability = i128::from_le_bytes(
            data[SCOUT_SV_LIABILITY_SHARE_VALUE_OFFSET
                ..SCOUT_SV_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        Some((asset, liability))
    }

    /// P-0014's subject registry + baseline writer, and P-0016's subject registry. Called from setup() and from `action_share_value_monotonicity_probe`.
    fn scout_sv_record_share_baselines(&mut self) -> usize {
        let forged_banks = scout_forged_bank_pdas(self.program_id);

        // 1. Release slots whose bank was closed.
        for slot in 0..SCOUT_SV_BANK_CAP {
            let bank = self.scout_sv_banks[slot];
            if bank == Pubkey::default() {
                continue;
            }
            let alive = match self.ctx.account_data(&bank) {
                Ok(data) => {
                    data.len() == SCOUT_SV_BANK_LEN && data[..8] == SCOUT_SV_BANK_DISCRIMINATOR
                }
                Err(_) => false,
            };
            if !alive {
                self.scout_sv_banks[slot] = Pubkey::default();
                self.scout_sv_asset_bits[slot] = 0;
                self.scout_sv_liability_bits[slot] = 0;
                self.scout_sv_forged[slot] = false;
            }
        }

        // 2. Register newly-seen banks.
        let dirty: Vec<Pubkey> = self
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .iter()
            .copied()
            .collect();
        for key in dirty {
            if self.scout_sv_read_share_values(&key).is_none() {
                continue;
            }
            let mut known = false;
            let mut free_slot = SCOUT_SV_BANK_CAP;
            for slot in 0..SCOUT_SV_BANK_CAP {
                if self.scout_sv_banks[slot] == key {
                    known = true;
                    break;
                }
                if self.scout_sv_banks[slot] == Pubkey::default() && free_slot == SCOUT_SV_BANK_CAP
                {
                    free_slot = slot;
                }
            }
            if known || free_slot == SCOUT_SV_BANK_CAP {
                continue;
            }
            self.scout_sv_banks[free_slot] = key;
            self.scout_sv_forged[free_slot] = forged_banks.contains(&key);
        }

        // 3. Refresh baselines.
        let mut recorded: usize = 0;
        for slot in 0..SCOUT_SV_BANK_CAP {
            let bank = self.scout_sv_banks[slot];
            if bank == Pubkey::default() {
                continue;
            }
            let values = match self.scout_sv_read_share_values(&bank) {
                Some(values) => values,
                None => continue,
            };
            self.scout_sv_asset_bits[slot] = values.0;
            self.scout_sv_liability_bits[slot] = values.1;
            recorded = recorded + 1;
        }
        for entry in 0..SCOUT_SV_SOCIALIZED_CAP {
            self.scout_sv_socialized[entry] = Pubkey::default();
        }
        recorded
    }

    // ---- P-0024 probe machinery ------------------------------------------------------------
    fn scout_p24_fixed_price_bits(&self, bank: Pubkey) -> Option<i128> {
        let data = self.ctx.read_account(&bank).ok()?.data;
        if data.len() != SCOUT_HP_BANK_LEN || data[..8] != SCOUT_HP_BANK_DISCRIMINATOR {
            return None;
        }
        if data[SCOUT_HP_BANK_ORACLE_SETUP_OFFSET] != SCOUT_HP_ORACLE_SETUP_FIXED {
            return None;
        }
        let bytes: [u8; 16] = data
            [SCOUT_HP_BANK_FIXED_PRICE_OFFSET..SCOUT_HP_BANK_FIXED_PRICE_OFFSET + 16]
            .try_into()
            .ok()?;
        Some(fixed::types::I80F48::from_le_bytes(bytes).to_bits())
    }

    /// Mint a real MarginfiAccount under an arbitrary authority, via `marginfi_account_initialize`.
    fn scout_p24_init_account(&mut self, authority: &Keypair) -> Option<Pubkey> {
        let account_keypair = Keypair::new();
        let account = account_keypair.pubkey();
        if !(self
            .ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitialize {})
            .accounts(accounts::MarginfiAccountInitialize {
                marginfi_group: self.marginfi_group,
                marginfi_account: account,
                authority: authority.pubkey(),
                fee_payer: authority.pubkey(),
            })
            .signers(&[authority, &account_keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false))
        {
            return None;
        }
        Some(account)
    }

    /// Build P-0024's scenario; returns `(victim, victim_authority, third_party_account, collateral_bank, liability_bank, sorted)`.
    fn scout_p24_build_scenario(
        &mut self,
    ) -> Option<(Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, [Pubkey; 2])> {
        let collateral_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let liability_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if !self.scout_liquidate_raise_liab_bank_limits(liability_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(
            collateral_bank,
            fixed::types::I80F48::from_num(SCOUT_P24_START_PRICE),
        ) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liability_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let sorted = if collateral_bank.to_bytes() > liability_bank.to_bytes() {
            [collateral_bank, liability_bank]
        } else {
            [liability_bank, collateral_bank]
        };

        let actor_authority = self.payer.clone();
        let liquidity_provider = self.scout_p24_init_account(&actor_authority)?;
        if !self.scout_liquidate_deposit(
            liquidity_provider,
            liability_bank,
            SCOUT_P24_LIQUIDITY_DEPOSIT_AMOUNT,
        ) {
            return None;
        }

        let third_party_account = self.scout_p24_init_account(&actor_authority)?;
        if !self.scout_liquidate_deposit(
            third_party_account,
            collateral_bank,
            SCOUT_P24_THIRD_PARTY_COLLATERAL_AMOUNT,
        ) {
            return None;
        }
        if !self.scout_liquidate_borrow(
            third_party_account,
            liability_bank,
            SCOUT_P24_THIRD_PARTY_BORROW_AMOUNT,
            sorted.to_vec(),
        ) {
            return None;
        }

        let victim_authority = Keypair::new();
        self.ctx
            .create_account()
            .pubkey(victim_authority.pubkey())
            .owner(system_program::ID)
            .lamports(SCOUT_P24_VICTIM_LAMPORTS)
            .create()
            .ok()?;
        let victim_token_account = self
            .ctx
            .create_token_account()
            .pubkey(Pubkey::new_unique())
            .mint(self.bank_mint)
            .token_owner(victim_authority.pubkey())
            .amount(SCOUT_P24_VICTIM_TOKENS)
            .create()
            .ok()?;
        let victim = self.scout_p24_init_account(&victim_authority)?;
        let collateral_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, collateral_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liability_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, liability_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liability_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, liability_bank.as_ref()],
            &self.program_id,
        )
        .0;
        if !(self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit {
                amount: SCOUT_P24_VICTIM_COLLATERAL_AMOUNT,
                deposit_up_to_limit: None,
            })
            .accounts(accounts::LendingAccountDeposit {
                group: self.marginfi_group,
                marginfi_account: victim,
                authority: victim_authority.pubkey(),
                bank: collateral_bank,
                signer_token_account: victim_token_account,
                liquidity_vault: collateral_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&victim_authority])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false))
        {
            return None;
        }
        if !(self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountBorrow {
                amount: SCOUT_P24_VICTIM_BORROW_AMOUNT,
            })
            .accounts(accounts::LendingAccountBorrow {
                group: self.marginfi_group,
                marginfi_account: victim,
                authority: victim_authority.pubkey(),
                bank: liability_bank,
                destination_token_account: victim_token_account,
                bank_liquidity_vault_authority: liability_vault_authority,
                liquidity_vault: liability_vault,
                token_program: spl_token::id(),
            })
            .remaining_accounts(sorted.to_vec())
            .signers(&[&victim_authority])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false))
        {
            return None;
        }

        if !self.scout_liquidate_set_fixed_price(
            collateral_bank,
            fixed::types::I80F48::from_num(SCOUT_P24_SETTLED_PRICE),
        ) {
            return None;
        }

        Some((
            victim,
            victim_authority.pubkey(),
            third_party_account,
            collateral_bank,
            liability_bank,
            sorted,
        ))
    }

    /// P-0024's driver: stand up a non-liquidatable account, send one fuzzer-selected transaction signed by a different identity, and record maintenance health before/after.
    pub fn action_third_party_health_probe(&mut self, choice: u8) -> bool {
        let arm = choice % SCOUT_P24_ARM_COUNT + 1;
        let (victim, victim_authority, third_party_account, collateral_bank, liability_bank, sorted) =
            match self.scout_p24_build_scenario() {
                Some(scenario) => scenario,
                None => return false,
            };

        let pre = self.scout_hp_maintenance_health(victim);
        let pre_collateral_price = self.scout_p24_fixed_price_bits(collateral_bank);
        let pre_liability_price = self.scout_p24_fixed_price_bits(liability_bank);

        self.scout_p24_arm = arm;
        self.scout_p24_victim = victim;
        self.scout_p24_victim_authority = victim_authority;
        self.scout_p24_actor = self.payer.pubkey();
        self.scout_p24_valid = false;
        self.scout_p24_pinned = false;
        self.scout_p24_succeeded = false;
        self.scout_p24_pre_health = pre.unwrap_or(fixed::types::I80F48::ZERO).to_bits();
        self.scout_p24_post_health = 0;

        let group = self.marginfi_group;
        let actor = self.payer.pubkey();
        let actor_token_account = self.signer_token_account;
        let collateral_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, collateral_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let collateral_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, collateral_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liability_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, liability_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liability_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, liability_bank.as_ref()],
            &self.program_id,
        )
        .0;

        let succeeded = if arm == SCOUT_P24_ARM_THIRD_PARTY_DEPOSIT_OWN {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountDeposit {
                    amount: SCOUT_P24_PROBE_DEPOSIT_AMOUNT,
                    deposit_up_to_limit: None,
                })
                .accounts(accounts::LendingAccountDeposit {
                    group,
                    marginfi_account: third_party_account,
                    authority: actor,
                    bank: collateral_bank,
                    signer_token_account: actor_token_account,
                    liquidity_vault: collateral_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P24_ARM_THIRD_PARTY_WITHDRAW_OWN {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountWithdraw {
                    amount: SCOUT_P24_PROBE_WITHDRAW_AMOUNT,
                    withdraw_all: None,
                })
                .accounts(accounts::LendingAccountWithdraw {
                    group,
                    marginfi_account: third_party_account,
                    authority: actor,
                    bank: collateral_bank,
                    destination_token_account: actor_token_account,
                    bank_liquidity_vault_authority: collateral_vault_authority,
                    liquidity_vault: collateral_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P24_ARM_THIRD_PARTY_BORROW_OWN {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountBorrow {
                    amount: SCOUT_P24_PROBE_BORROW_AMOUNT,
                })
                .accounts(accounts::LendingAccountBorrow {
                    group,
                    marginfi_account: third_party_account,
                    authority: actor,
                    bank: liability_bank,
                    destination_token_account: actor_token_account,
                    bank_liquidity_vault_authority: liability_vault_authority,
                    liquidity_vault: liability_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P24_ARM_THIRD_PARTY_REPAY_OWN {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountRepay {
                    amount: SCOUT_P24_PROBE_REPAY_AMOUNT,
                    repay_all: Some(false),
                })
                .accounts(accounts::LendingAccountRepay {
                    group,
                    marginfi_account: third_party_account,
                    authority: actor,
                    bank: liability_bank,
                    signer_token_account: actor_token_account,
                    liquidity_vault: liability_vault,
                    token_program: spl_token::id(),
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P24_ARM_WITHDRAW_VICTIM {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountWithdraw {
                    amount: SCOUT_P24_PROBE_WITHDRAW_AMOUNT,
                    withdraw_all: None,
                })
                .accounts(accounts::LendingAccountWithdraw {
                    group,
                    marginfi_account: victim,
                    authority: actor,
                    bank: collateral_bank,
                    destination_token_account: actor_token_account,
                    bank_liquidity_vault_authority: collateral_vault_authority,
                    liquidity_vault: collateral_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P24_ARM_BORROW_VICTIM {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountBorrow {
                    amount: SCOUT_P24_PROBE_BORROW_AMOUNT,
                })
                .accounts(accounts::LendingAccountBorrow {
                    group,
                    marginfi_account: victim,
                    authority: actor,
                    bank: liability_bank,
                    destination_token_account: actor_token_account,
                    bank_liquidity_vault_authority: liability_vault_authority,
                    liquidity_vault: liability_vault,
                    token_program: spl_token::id(),
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P24_ARM_PULSE_VICTIM {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountPulseHealth {})
                .accounts(accounts::LendingAccountPulseHealth {
                    marginfi_account: victim,
                    group: self.marginfi_group,
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P24_ARM_ACCRUE_COLLATERAL_BANK {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAccrueBankInterest {})
                .accounts(accounts::LendingPoolAccrueBankInterest {
                    group,
                    bank: collateral_bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P24_ARM_ACCRUE_LIABILITY_BANK {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAccrueBankInterest {})
                .accounts(accounts::LendingPoolAccrueBankInterest {
                    group,
                    bank: liability_bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else {
            self.scout_p24_arm = SCOUT_P24_ARM_NONE;
            return false;
        };

        let post = self.scout_hp_maintenance_health(victim);
        let post_collateral_price = self.scout_p24_fixed_price_bits(collateral_bank);
        let post_liability_price = self.scout_p24_fixed_price_bits(liability_bank);

        self.scout_p24_succeeded = succeeded;
        self.scout_p24_post_health = post.unwrap_or(fixed::types::I80F48::ZERO).to_bits();
        self.scout_p24_valid = pre.is_some() && post.is_some();
        self.scout_p24_pinned = pre_collateral_price.is_some()
            && pre_liability_price.is_some()
            && pre_collateral_price == post_collateral_price
            && pre_liability_price == post_liability_price;
        true
    }

    // ---- P-0025 repay-liveness probe machinery -------------------------------------------------

    /// A `BankConfigOpt` that changes ONLY `operational_state`.
    fn scout_rl_operational_state_opt(
        &self,
        state: marginfi::types::BankOperationalState,
    ) -> marginfi::types::BankConfigOpt {
        marginfi::types::BankConfigOpt {
            asset_weight_init: None,
            asset_weight_maint: None,
            liability_weight_init: None,
            liability_weight_maint: None,
            deposit_limit: None,
            borrow_limit: None,
            operational_state: Some(state),
            interest_rate_config: None,
            risk_tier: None,
            asset_tag: None,
            total_asset_value_init_limit: None,
            oracle_max_confidence: None,
            oracle_max_age: None,
            permissionless_bad_debt_settlement: None,
            freeze_settings: None,
            liquidation_liquidator_fee: None,
            liquidation_insurance_fee: None,
            circuit_breaker_enabled: None,
            cb_deviation_bps_tiers: None,
            cb_tier_durations_seconds: None,
            cb_escalation_window_mult: None,
            cb_ema_alpha_bps: None,
            cb_window_seconds: None,
            cb_window_max_up_bps: None,
            cb_window_max_down_bps: None,
            tokenless_repayments_allowed: None,
        }
    }

    fn scout_rl_configure_operational_state(
        &mut self,
        bank: Pubkey,
        state: marginfi::types::BankOperationalState,
    ) -> bool {
        let bank_config_opt = self.scout_rl_operational_state_opt(state);
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBank { bank_config_opt })
            .accounts(accounts::LendingPoolConfigureBank {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    /// Mint a fresh, real MarginfiAccount WITHOUT adding it to the shared subject registries.
    fn scout_rl_create_account(&mut self) -> Option<Pubkey> {
        let keypair = Keypair::new();
        let account = keypair.pubkey();
        if !(self
            .ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitialize {})
            .accounts(accounts::MarginfiAccountInitialize {
                marginfi_group: self.marginfi_group,
                marginfi_account: account,
                authority: self.payer.pubkey(),
                fee_payer: self.payer.pubkey(),
            })
            .signers(&[&*self.payer, &keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false))
        {
            return None;
        }
        Some(account)
    }

    /// The active banks of `account`, sorted descending by pubkey (the order `RiskEngine` expects).
    fn scout_rl_sorted(&self, banks: &[Pubkey]) -> Vec<Pubkey> {
        let mut sorted = banks.to_vec();
        sorted.sort();
        sorted.reverse();
        sorted
    }

    /// Raw `liability_shares` bits of `account`'s balance in `bank`, plus the account's flags.
    fn scout_rl_read_position(&self, account: Pubkey, bank: Pubkey) -> Option<(i128, u64)> {
        let data = self.ctx.read_account(&account).ok()?.data;
        if data.len() != SCOUT_HP_ACCOUNT_LEN || data[..8] != SCOUT_HP_ACCOUNT_DISCRIMINATOR {
            return None;
        }
        let flag_bytes: [u8; 8] = data
            [MARGINFI_ACCOUNT_FLAGS_OFFSET..MARGINFI_ACCOUNT_FLAGS_OFFSET + 8]
            .try_into()
            .ok()?;
        let flags = u64::from_le_bytes(flag_bytes);
        for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
            let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
            if data.len() < base + SCOUT_BALANCE_STRIDE {
                break;
            }
            if data[base] == 0 {
                continue;
            }
            let bank_bytes: [u8; 32] = data[base + 1..base + 33].try_into().ok()?;
            if Pubkey::new_from_array(bank_bytes) != bank {
                continue;
            }
            let liability_bytes: [u8; 16] = data[base + 56..base + 72].try_into().ok()?;
            return Some((
                fixed::types::I80F48::from_le_bytes(liability_bytes).to_bits(),
                flags,
            ));
        }
        None
    }

    /// `(operational_state, asset_tag)` of a bank.
    fn scout_rl_read_bank_gates(&self, bank: Pubkey) -> Option<(u8, u8)> {
        let data = self.ctx.read_account(&bank).ok()?.data;
        if data.len() != SCOUT_HP_BANK_LEN || data[..8] != SCOUT_HP_BANK_DISCRIMINATOR {
            return None;
        }
        Some((
            data[SCOUT_RL_BANK_OPERATIONAL_STATE_OFFSET],
            data[SCOUT_HP_BANK_ASSET_TAG_OFFSET],
        ))
    }

    /// The SPL `amount` of a token account.
    fn scout_rl_token_amount(&self, token_account: Pubkey) -> Option<u64> {
        let data = self.ctx.read_account(&token_account).ok()?.data;
        if data.len() < SCOUT_RL_TOKEN_ACCOUNT_LEN {
            return None;
        }
        let bytes: [u8; 8] = data[SCOUT_RL_TOKEN_ACCOUNT_AMOUNT_OFFSET
            ..SCOUT_RL_TOKEN_ACCOUNT_AMOUNT_OFFSET + 8]
            .try_into()
            .ok()?;
        Some(u64::from_le_bytes(bytes))
    }

    /// Build the shared scenario: a borrower with collateral in bank C and liabilities in banks X and Y. Returns `(borrower, collateral_bank, repay_bank, other_bank)`.
    fn scout_rl_build_scenario(&mut self) -> Option<(Pubkey, Pubkey, Pubkey, Pubkey)> {
        let collateral_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let repay_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let other_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        if !self.scout_liquidate_set_fixed_price(
            collateral_bank,
            fixed::types::I80F48::from_num(SCOUT_RL_HEALTHY_COLLATERAL_PRICE),
        ) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(
            repay_bank,
            fixed::types::I80F48::from_num(SCOUT_RL_LIABILITY_PRICE),
        ) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(
            other_bank,
            fixed::types::I80F48::from_num(SCOUT_RL_LIABILITY_PRICE),
        ) {
            return None;
        }
        let liquidity_provider = self.scout_rl_create_account()?;
        let borrower = self.scout_rl_create_account()?;
        if !self.scout_liquidate_deposit(
            liquidity_provider,
            repay_bank,
            SCOUT_RL_LIQUIDITY_DEPOSIT_AMOUNT,
        ) {
            return None;
        }
        if !self.scout_liquidate_deposit(
            liquidity_provider,
            other_bank,
            SCOUT_RL_LIQUIDITY_DEPOSIT_AMOUNT,
        ) {
            return None;
        }
        if !self.scout_liquidate_deposit(
            borrower,
            collateral_bank,
            SCOUT_RL_COLLATERAL_DEPOSIT_AMOUNT,
        ) {
            return None;
        }
        if !self.scout_liquidate_borrow(
            borrower,
            repay_bank,
            SCOUT_RL_BORROW_AMOUNT,
            self.scout_rl_sorted(&[collateral_bank, repay_bank]),
        ) {
            return None;
        }
        if !self.scout_liquidate_borrow(
            borrower,
            other_bank,
            SCOUT_RL_BORROW_AMOUNT,
            self.scout_rl_sorted(&[collateral_bank, repay_bank, other_bank]),
        ) {
            return None;
        }
        Some((borrower, collateral_bank, repay_bank, other_bank))
    }

    /// Apply the arm's degradation.
    fn scout_rl_degrade(
        &mut self,
        arm: u8,
        borrower: Pubkey,
        collateral_bank: Pubkey,
        repay_bank: Pubkey,
        other_bank: Pubkey,
    ) -> bool {
        if arm == SCOUT_RL_ARM_NONE {
            return true;
        }
        if arm == SCOUT_RL_ARM_CRASH_COLLATERAL_ORACLE {
            return self.scout_liquidate_set_fixed_price(
                collateral_bank,
                fixed::types::I80F48::from_num(SCOUT_RL_CRASHED_PRICE),
            );
        }
        if arm == SCOUT_RL_ARM_PAUSE_OTHER_BANK {
            return self.scout_rl_configure_operational_state(
                other_bank,
                marginfi::types::BankOperationalState::Paused,
            );
        }
        if arm == SCOUT_RL_ARM_REDUCE_ONLY_OTHER_BANK {
            return self.scout_rl_configure_operational_state(
                other_bank,
                marginfi::types::BankOperationalState::ReduceOnly,
            );
        }
        if arm == SCOUT_RL_ARM_ZERO_OTHER_BANK_ORACLE {
            return self.scout_liquidate_set_fixed_price(
                other_bank,
                fixed::types::I80F48::from_num(SCOUT_RL_ZERO_PRICE),
            );
        }
        if arm == SCOUT_RL_ARM_BANKRUPT_OTHER_BANK {
            // 1. Crash the collateral oracle to zero.
            if !self.scout_liquidate_set_fixed_price(
                collateral_bank,
                fixed::types::I80F48::from_num(SCOUT_RL_ZERO_PRICE),
            ) {
                return false;
            }
            // 2. Handle bankruptcy on the other bank.
            let (_, liquidity_vault, insurance_vault_authority, insurance_vault, _, _) =
                scout_bank_vault_pdas(self.program_id, other_bank);
            let remaining =
                self.scout_rl_sorted(&[collateral_bank, repay_bank, other_bank]);
            if !self.scout_call_lending_pool_handle_bankruptcy_append(
                borrower,
                other_bank,
                liquidity_vault,
                insurance_vault,
                insurance_vault_authority,
                spl_token::id(),
                remaining,
            ) {
                return false;
            }
            // 3. Restore the collateral price.
            return self.scout_liquidate_set_fixed_price(
                collateral_bank,
                fixed::types::I80F48::from_num(SCOUT_RL_HEALTHY_COLLATERAL_PRICE),
            );
        }
        false
    }

    /// P-0025's driver. Build a borrower with a real liability in bank X, degrade some unrelated
    /// part of the account, attempt ONE well-formed partial repay of X, and record the raw
    pub fn action_repay_liveness_probe(&mut self, choice: u8) -> bool {
        let (borrower, collateral_bank, repay_bank, other_bank) =
            match self.scout_rl_build_scenario() {
                Some(v) => v,
                None => return false,
            };
        let arm = choice % SCOUT_RL_ARM_COUNT;
        if !self.scout_rl_degrade(arm, borrower, collateral_bank, repay_bank, other_bank) {
            return false;
        }
        let (liab_bits, flags) = match self.scout_rl_read_position(borrower, repay_bank) {
            Some(v) => v,
            None => return false,
        };
        let wallet = match self.scout_rl_token_amount(self.signer_token_account) {
            Some(v) => v,
            None => return false,
        };
        let (bank_state, bank_tag) = match self.scout_rl_read_bank_gates(repay_bank) {
            Some(v) => v,
            None => return false,
        };
        self.scout_rl_armed = true;
        self.scout_rl_arm = arm;
        self.scout_rl_subject = borrower;
        self.scout_rl_bank = repay_bank;
        self.scout_rl_other_bank = other_bank;
        self.scout_rl_liab_bits = liab_bits;
        self.scout_rl_wallet = wallet;
        self.scout_rl_amount = SCOUT_RL_REPAY_AMOUNT;
        self.scout_rl_bank_state = bank_state;
        self.scout_rl_bank_tag = bank_tag;
        self.scout_rl_flags = flags;
        self.scout_rl_succeeded = false;

        let liquidity_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, repay_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let succeeded = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountRepay {
                amount: SCOUT_RL_REPAY_AMOUNT,
                repay_all: Some(false),
            })
            .accounts(accounts::LendingAccountRepay {
                group: self.marginfi_group,
                marginfi_account: borrower,
                authority: self.payer.pubkey(),
                bank: repay_bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        self.scout_rl_succeeded = succeeded;
        succeeded
    }

    // ---- P-0001 / P-0022 PER-ACTION MEASUREMENT ---------------------------------------------

    /// P-0001's measurement.
    fn scout_p1_measure(
        &self,
    ) -> Option<(
        fixed::types::I80F48,
        fixed::types::I80F48,
        fixed::types::I80F48,
        usize,
        Vec<(Pubkey, [u8; 16], [u8; 16])>,
    )> {
        const P1_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
        const P1_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
        const P1_BANK_LEN: usize = 8 + 1856;
        const P1_ACCOUNT_LEN: usize = 8 + 2304;
        const P1_BANK_MINT_OFFSET: usize = 8;
        const P1_BANK_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
        const P1_BANK_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
        const P1_ACCOUNT_AUTHORITY_OFFSET: usize = 8 + 32;
        const P1_TOKEN_AMOUNT_OFFSET: usize = 64;

        if self.bank == Pubkey::default()
            || self.marginfi_group == Pubkey::default()
            || self.signer_token_account == Pubkey::default()
        {
            return None;
        }
        let attacker = self.payer.pubkey();
        let mint = self.bank_mint;

        // 1. the subject ring
        if self.scout_p1_accounts_next > SCOUT_SUBJECT_CAP {
            return None;
        }
        let registry_len = self.scout_p1_accounts_next.min(SCOUT_SUBJECT_CAP);

        // 2. realised side
        let tokens = match self.ctx.account_data(&self.signer_token_account) {
            Ok(data) if data.len() >= P1_TOKEN_AMOUNT_OFFSET + 8 => {
                let buf: [u8; 8] = data[P1_TOKEN_AMOUNT_OFFSET..P1_TOKEN_AMOUNT_OFFSET + 8]
                    .try_into()
                    .unwrap_or_default();
                fixed::types::I80F48::from_num(u64::from_le_bytes(buf))
            }
            _ => return None,
        };

        let forged_banks = scout_forged_bank_pdas(self.program_id);

        // 3. claimable side, over the registry only
        let mut banks: Vec<(Pubkey, fixed::types::I80F48, fixed::types::I80F48, [u8; 16], [u8; 16])> =
            Vec::new();
        let mut claim = fixed::types::I80F48::ZERO;
        let registry = self.scout_p1_accounts;
        let mut actor_accounts: usize = 0;
        for slot in 0..registry_len {
            let account = registry[slot];
            if account == Pubkey::default() {
                continue;
            }
            let mut already_counted = false;
            for earlier in 0..slot {
                if registry[earlier] == account {
                    already_counted = true;
                }
            }
            if already_counted {
                continue;
            }
            let data = match self.ctx.account_data(&account) {
                Ok(d) if d.len() == P1_ACCOUNT_LEN && d[..8] == P1_ACCOUNT_DISCRIMINATOR => d,
                _ => continue,
            };
            if data[P1_ACCOUNT_AUTHORITY_OFFSET..P1_ACCOUNT_AUTHORITY_OFFSET + 32]
                != *attacker.as_ref()
            {
                continue;
            }
            actor_accounts += 1;
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                if data[base] == 0 {
                    continue;
                }
                if data[base + 33] != 0 {
                    continue;
                }
                let bank_bytes: [u8; 32] =
                    data[base + 1..base + 33].try_into().unwrap_or_default();
                let bank_pk = Pubkey::new_from_array(bank_bytes);
                let mut memo_hit: Option<(fixed::types::I80F48, fixed::types::I80F48)> = None;
                for entry in banks.iter() {
                    if entry.0 == bank_pk && memo_hit.is_none() {
                        memo_hit = Some((entry.1, entry.2));
                    }
                }
                let resolved = match memo_hit {
                    Some(entry) => Some(entry),
                    None => {
                        if forged_banks.contains(&bank_pk) {
                            None
                        } else {
                            match self.ctx.account_data(&bank_pk) {
                                Ok(bank_data)
                                    if bank_data.len() == P1_BANK_LEN
                                        && bank_data[..8] == P1_BANK_DISCRIMINATOR
                                        && bank_data[P1_BANK_MINT_OFFSET..P1_BANK_MINT_OFFSET + 32]
                                            == *mint.as_ref() =>
                                {
                                    let asv: [u8; 16] = bank_data[P1_BANK_ASSET_SHARE_VALUE_OFFSET
                                        ..P1_BANK_ASSET_SHARE_VALUE_OFFSET + 16]
                                        .try_into()
                                        .unwrap_or_default();
                                    let lsv: [u8; 16] = bank_data
                                        [P1_BANK_LIABILITY_SHARE_VALUE_OFFSET
                                            ..P1_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16]
                                        .try_into()
                                        .unwrap_or_default();
                                    let asv_v = fixed::types::I80F48::from_le_bytes(asv);
                                    let lsv_v = fixed::types::I80F48::from_le_bytes(lsv);
                                    banks.push((bank_pk, asv_v, lsv_v, asv, lsv));
                                    Some((asv_v, lsv_v))
                                }
                                _ => None,
                            }
                        }
                    }
                };
                let (asv, lsv) = match resolved {
                    Some(v) => v,
                    None => continue,
                };
                let asset_shares: [u8; 16] =
                    data[base + 40..base + 56].try_into().unwrap_or_default();
                let liability_shares: [u8; 16] =
                    data[base + 56..base + 72].try_into().unwrap_or_default();
                claim = claim
                    .saturating_add(
                        fixed::types::I80F48::from_le_bytes(asset_shares).saturating_mul(asv),
                    )
                    .saturating_sub(
                        fixed::types::I80F48::from_le_bytes(liability_shares).saturating_mul(lsv),
                    );
            }
        }
        let value = tokens.saturating_add(claim);
        let marks: Vec<(Pubkey, [u8; 16], [u8; 16])> = banks
            .iter()
            .map(|(pk, _, _, asv, lsv)| (*pk, *asv, *lsv))
            .collect();
        Some((value, tokens, claim, actor_accounts, marks))
    }

    /// P-0022's measurement.
    fn scout_p22_measure(
        &self,
    ) -> (
        Vec<(Pubkey, i128, i128, bool)>,
        Vec<(Pubkey, [u8; 16], [u8; 16], [u8; 16])>,
    ) {
        const P22_ACCOUNT_DISCRIMINATOR: [u8; 8] = SCOUT_HANDLE_BANKRUPTCY_ACCOUNT_DISCRIMINATOR;
        const P22_ACCOUNT_LEN: usize = SCOUT_HANDLE_BANKRUPTCY_ACCOUNT_LEN;
        const P22_BANK_DISCRIMINATOR: [u8; 8] = SCOUT_HANDLE_BANKRUPTCY_BANK_DISCRIMINATOR;
        const P22_BANK_LEN: usize = SCOUT_HANDLE_BANKRUPTCY_BANK_LEN;
        const P22_BANK_MINT_DECIMALS_OFFSET: usize =
            SCOUT_HANDLE_BANKRUPTCY_BANK_MINT_DECIMALS_OFFSET;
        const P22_BANK_ORACLE_SETUP_OFFSET: usize = SCOUT_HANDLE_BANKRUPTCY_BANK_ORACLE_SETUP_OFFSET;
        const P22_ORACLE_SETUP_FIXED: u8 = SCOUT_HANDLE_BANKRUPTCY_ORACLE_SETUP_FIXED;
        const P22_BANK_FIXED_PRICE_OFFSET: usize = SCOUT_HANDLE_BANKRUPTCY_BANK_FIXED_PRICE_OFFSET;
        const P22_BANK_OPERATIONAL_STATE_OFFSET: usize =
            SCOUT_HANDLE_BANKRUPTCY_BANK_OPERATIONAL_STATE_OFFSET;
        const P22_KILLED_BY_BANKRUPTCY: u8 = 3;
        const P22_BANK_ASSET_TAG_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 489;
        const P22_ASSET_TAG_DEFAULT: u8 = 0;
        const P22_TOLERANCE: fixed::types::I80F48 = fixed::types::I80F48::lit("0.000000001");
        let excluded_flags = SCOUT_ACCOUNT_DISABLED
            | SCOUT_ACCOUNT_IN_FLASHLOAN
            | SCOUT_ACCOUNT_IN_RECEIVERSHIP
            | SCOUT_ACCOUNT_IN_DELEVERAGE;

        let registry = self.scout_p22_accounts;
        let registry_len = self.scout_p22_accounts_next.min(SCOUT_SUBJECT_CAP);

        // Exclusion 4
        let forged_banks = scout_forged_bank_pdas(self.program_id);
        let forged_accounts = [
            Pubkey::find_program_address(&[b"scout_handle_bankruptcy_account"], &self.program_id).0,
            Pubkey::find_program_address(&[b"scout_hb_zero_debt_account"], &self.program_id).0,
        ];

        let mut valued: Vec<(Pubkey, i128, i128, bool)> = Vec::new();
        let mut marks: Vec<(Pubkey, [u8; 16], [u8; 16], [u8; 16])> = Vec::new();

        for slot in 0..registry_len {
            let account = registry[slot];
            if account == Pubkey::default() {
                continue;
            }
            // Exclusion 5
            if forged_accounts.contains(&account)
                || self.scout_p22_forged_accounts.contains(&account)
            {
                continue;
            }
            let data = match self.ctx.account_data(&account) {
                Ok(d) if d.len() == P22_ACCOUNT_LEN && d[..8] == P22_ACCOUNT_DISCRIMINATOR => d,
                _ => continue,
            };
            if data.len() < MARGINFI_ACCOUNT_FLAGS_OFFSET + 8 {
                continue;
            }
            let flag_bytes: [u8; 8] = data
                [MARGINFI_ACCOUNT_FLAGS_OFFSET..MARGINFI_ACCOUNT_FLAGS_OFFSET + 8]
                .try_into()
                .unwrap_or_default();
            let flags = u64::from_le_bytes(flag_bytes);
            if flags & excluded_flags != 0 {
                continue; // exclusions 1, 2 and 3
            }

            let mut asset_value = fixed::types::I80F48::ZERO;
            let mut liability_value = fixed::types::I80F48::ZERO;
            let mut valuable = true;
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                if data[base] != 1 {
                    continue;
                }
                let bank_bytes: [u8; 32] =
                    data[base + 1..base + 33].try_into().unwrap_or_default();
                let bank_pk = Pubkey::new_from_array(bank_bytes);
                if forged_banks.contains(&bank_pk) {
                    valuable = false;
                    break;
                }
                let bank_data = match self.ctx.account_data(&bank_pk) {
                    Ok(d) if d.len() == P22_BANK_LEN && d[..8] == P22_BANK_DISCRIMINATOR => d,
                    _ => {
                        valuable = false;
                        break;
                    }
                };
                if bank_data[P22_BANK_ASSET_TAG_OFFSET] != P22_ASSET_TAG_DEFAULT
                    || bank_data[P22_BANK_ORACLE_SETUP_OFFSET] != P22_ORACLE_SETUP_FIXED
                    || bank_data[P22_BANK_OPERATIONAL_STATE_OFFSET] == P22_KILLED_BY_BANKRUPTCY
                {
                    valuable = false; // exclusion 6
                    break;
                }
                if bank_data.len() < SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16 {
                    valuable = false;
                    break;
                }
                let asv_bytes: [u8; 16] = bank_data[SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET
                    ..SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET + 16]
                    .try_into()
                    .unwrap_or_default();
                let lsv_bytes: [u8; 16] = bank_data[SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET
                    ..SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16]
                    .try_into()
                    .unwrap_or_default();
                let price_bytes: [u8; 16] = bank_data
                    [P22_BANK_FIXED_PRICE_OFFSET..P22_BANK_FIXED_PRICE_OFFSET + 16]
                    .try_into()
                    .unwrap_or_default();
                let mut already_marked = false;
                for mark in marks.iter() {
                    if mark.0 == bank_pk {
                        already_marked = true;
                    }
                }
                if !already_marked {
                    marks.push((bank_pk, asv_bytes, lsv_bytes, price_bytes));
                }

                let asv = fixed::types::I80F48::from_le_bytes(asv_bytes);
                let lsv = fixed::types::I80F48::from_le_bytes(lsv_bytes);
                let price = fixed::types::I80F48::from_le_bytes(price_bytes);
                let decimals = bank_data[P22_BANK_MINT_DECIMALS_OFFSET];
                let scale_base = match 10u128.checked_pow(decimals as u32) {
                    Some(v) => v,
                    None => {
                        valuable = false;
                        break;
                    }
                };
                let scale = match fixed::types::I80F48::checked_from_num(scale_base) {
                    Some(v) if v > fixed::types::I80F48::ZERO => v,
                    _ => {
                        valuable = false;
                        break;
                    }
                };

                let asset_shares: [u8; 16] =
                    data[base + 40..base + 56].try_into().unwrap_or_default();
                let liability_shares: [u8; 16] =
                    data[base + 56..base + 72].try_into().unwrap_or_default();

                let asset_leg = fixed::types::I80F48::from_le_bytes(asset_shares)
                    .checked_mul(asv)
                    .and_then(|amount| amount.checked_mul(price))
                    .and_then(|value| value.checked_div(scale))
                    .and_then(|value| asset_value.checked_add(value));
                let liability_leg = fixed::types::I80F48::from_le_bytes(liability_shares)
                    .checked_mul(lsv)
                    .and_then(|amount| amount.checked_mul(price))
                    .and_then(|value| value.checked_div(scale))
                    .and_then(|value| liability_value.checked_add(value));
                match (asset_leg, liability_leg) {
                    (Some(a), Some(l)) => {
                        asset_value = a;
                        liability_value = l;
                    }
                    _ => {
                        valuable = false;
                        break;
                    }
                }
            }
            if !valuable {
                continue;
            }
            let underwater = match asset_value.checked_add(P22_TOLERANCE) {
                Some(allowed) => liability_value > allowed,
                None => continue,
            };
            valued.push((
                account,
                asset_value.to_bits(),
                liability_value.to_bits(),
                underwater,
            ));
        }

        (valued, marks)
    }

    /// `#[fuzz_fixture]`'s per-action callback.
    fn after_action(&mut self) {
        // P-0001
        if let Some((value, tokens, claim, actor_accounts, marks)) = self.scout_p1_measure() {
            self.scout_p1_prev_ok = self.scout_p1_cur_ok;
            self.scout_p1_prev_value = self.scout_p1_cur_value;
            self.scout_p1_prev_actor_count = self.scout_p1_cur_actor_count;
            self.scout_p1_prev_share_values =
                std::mem::take(&mut self.scout_p1_cur_share_values);
            self.scout_p1_cur_ok = true;
            self.scout_p1_cur_value = value.to_bits();
            self.scout_p1_cur_tokens = tokens.to_bits();
            self.scout_p1_cur_claim = claim.to_bits();
            self.scout_p1_cur_actor_count = actor_accounts;
            self.scout_p1_cur_share_values = marks;
        }

        // P-0022
        let (valued, marks) = self.scout_p22_measure();
        self.scout_p22_solvency = self
            .scout_p22_cur_valued
            .iter()
            .map(|(account, _, _, underwater)| (*account, *underwater))
            .collect();
        self.scout_p22_bank_marks = std::mem::take(&mut self.scout_p22_cur_marks);
        self.scout_p22_cur_valued = valued;
        self.scout_p22_cur_marks = marks;

        // P-0007
        let (p7_valid, p7_bank, p7_last_update, p7_both_sided, p7_asset_sv, p7_liab_sv) =
            self.scout_p7_measure();
        self.scout_p7_prev_valid = self.scout_p7_cur_valid;
        self.scout_p7_prev_bank = self.scout_p7_cur_bank;
        self.scout_p7_prev_last_update = self.scout_p7_cur_last_update;
        self.scout_p7_prev_both_sided = self.scout_p7_cur_both_sided;
        self.scout_p7_prev_asset_sv = self.scout_p7_cur_asset_sv;
        self.scout_p7_prev_liab_sv = self.scout_p7_cur_liab_sv;
        self.scout_p7_cur_valid = p7_valid;
        self.scout_p7_cur_bank = p7_bank;
        self.scout_p7_cur_last_update = p7_last_update;
        self.scout_p7_cur_both_sided = p7_both_sided;
        self.scout_p7_cur_asset_sv = p7_asset_sv;
        self.scout_p7_cur_liab_sv = p7_liab_sv;

        // P-0009
        let (p9_valid, p9_account, p9_digest) = self.scout_p9_measure();
        self.scout_p9_prev_valid = self.scout_p9_cur_valid;
        self.scout_p9_prev_account = self.scout_p9_cur_account;
        self.scout_p9_prev_digest = self.scout_p9_cur_digest;
        self.scout_p9_cur_valid = p9_valid;
        self.scout_p9_cur_account = p9_account;
        self.scout_p9_cur_digest = p9_digest;
        self.scout_p9_clock = {
            use ::anchor_lang::prelude::Clock;
            self.ctx.svm.get_sysvar::<Clock>().unix_timestamp
        };

        // P-0030/P-0014/P-0016
        self.scout_p30_record_fee_baselines();
        self.scout_sv_record_share_baselines();

        const SCOUT_PER_ACTION_CLOCK_ADVANCE_SECONDS: i64 = 7;
        {
            use ::anchor_lang::prelude::Clock;
            let clock = self.ctx.svm.get_sysvar::<Clock>();
            self.ctx.set_sysvar(&Clock {
                slot: clock.slot + 1,
                epoch_start_timestamp: clock.epoch_start_timestamp,
                epoch: clock.epoch,
                leader_schedule_epoch: clock.leader_schedule_epoch,
                unix_timestamp: clock
                    .unix_timestamp
                    .saturating_add(SCOUT_PER_ACTION_CLOCK_ADVANCE_SECONDS),
            });
        }
    }
    // ---------------------------------------------------------------------------------------
    // P-0026 / P-0027 probes.

    /// Prices cycled through by the recompute probe.
    fn scout_p27_price_for(choice: u8) -> fixed::types::I80F48 {
        match choice % SCOUT_P27_PRICE_CHOICES {
            0 => fixed::types::I80F48::ZERO,
            1 => fixed::types::I80F48::ONE,
            2 => fixed::types::I80F48::lit("3.25"),
            3 => fixed::types::I80F48::lit("0.000000000123456789"),
            4 => fixed::types::I80F48::lit("123456.789012345678901"),
            _ => fixed::types::I80F48::lit("999999999999.5"),
        }
    }

    /// Mint (once per lineage) the Bank that P-0027 owns outright.
    fn scout_p27_ensure_bank(&mut self) -> Option<Pubkey> {
        if self.scout_p27_bank != Pubkey::default() {
            return Some(self.scout_p27_bank);
        }
        let bank_keypair = Keypair::new();
        let bank = bank_keypair.pubkey();
        let (
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
        ) = scout_bank_vault_pdas(self.program_id, bank);
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let config = scout_valid_bank_config(10);
        let added = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAddBank { bank_config: config })
            .accounts(scout_lending_pool_add_bank_accounts(self.marginfi_group, self.payer.pubkey(), self.payer.pubkey(), fee_state, self.global_fee_wallet, self.bank_mint, bank, liquidity_vault_authority, liquidity_vault, insurance_vault_authority, insurance_vault, fee_vault_authority, fee_vault, spl_token::id()))
            .signers(&[&*self.payer, &bank_keypair])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !added {
            return None;
        }
        let priced = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolSetFixedOraclePrice {
                price: marginfi::types::WrappedI80F48::from_i80f48(fixed::types::I80F48::ONE),
            })
            .accounts(accounts::LendingPoolSetFixedOraclePrice {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !priced {
            return None;
        }
        self.scout_p27_bank = bank;
        Some(bank)
    }

    /// P-0027's probe.
    pub fn action_pulse_bank_price_cache_recompute_probe(&mut self, choice: u8) -> bool {
        let bank = match self.scout_p27_ensure_bank() {
            Some(v) => v,
            None => return false,
        };
        let price = Self::scout_p27_price_for(choice);
        let set = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolSetFixedOraclePrice {
                price: marginfi::types::WrappedI80F48::from_i80f48(price),
            })
            .accounts(accounts::LendingPoolSetFixedOraclePrice {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !set {
            return false;
        }
        let pre_ts = self.scout_pc_clock_timestamp();
        let pulsed = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingPoolPulseBankPriceCache {})
            .accounts(accounts::LendingPoolPulseBankPriceCache {
                group: self.marginfi_group,
                bank,
            })
            .remaining_accounts(vec![])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !pulsed {
            return false;
        }
        let post_ts = self.scout_pc_clock_timestamp();
        self.scout_p27_asked_bits = price.to_bits();
        self.scout_p27_pre_ts = pre_ts;
        self.scout_p27_post_ts = post_ts;
        self.scout_p27_valid = true;
        true
    }

    /// Read the SVM Clock's unix_timestamp.
    fn scout_pc_clock_timestamp(&self) -> i64 {
        use ::anchor_lang::prelude::Clock;
        self.ctx.svm.get_sysvar::<Clock>().unix_timestamp
    }

    /// P-0026's memo refresh.
    fn scout_p26_refresh_high_water(&mut self) {
        let accounts = [
            self.marginfi_account,
            self.borrow_marginfi_account,
            self.pulse_health_healthy_account,
            self.pulse_health_risk_rejected_account,
            self.scout_p26_probe_account,
        ];
        for index in 0..SCOUT_P26_ACCOUNT_COUNT {
            let subject = accounts[index];
            if subject == Pubkey::default() {
                continue;
            }
            let data = match self.ctx.read_account(&subject) {
                Ok(account) => account.data,
                Err(_) => continue,
            };
            if data.len() != SCOUT_PC_ACCOUNT_LEN || data[..8] != SCOUT_PC_ACCOUNT_DISCRIMINATOR {
                continue;
            }
            let bytes: [u8; 8] = match data[SCOUT_PC_ACCOUNT_HEALTH_TIMESTAMP_OFFSET
                ..SCOUT_PC_ACCOUNT_HEALTH_TIMESTAMP_OFFSET + 8]
                .try_into()
            {
                Ok(v) => v,
                Err(_) => continue,
            };
            let observed = i64::from_le_bytes(bytes);
            if observed > self.scout_p26_account_hw[index] {
                self.scout_p26_account_hw[index] = observed;
            }
        }
        let banks = [
            self.bank,
            self.borrow_liab_bank,
            self.borrow_asset_bank,
            self.pulse_health_healthy_bank,
        ];
        for index in 0..SCOUT_P26_BANK_COUNT {
            let subject = banks[index];
            if subject == Pubkey::default() {
                continue;
            }
            let data = match self.ctx.read_account(&subject) {
                Ok(account) => account.data,
                Err(_) => continue,
            };
            if data.len() != SCOUT_PC_BANK_LEN || data[..8] != SCOUT_PC_BANK_DISCRIMINATOR {
                continue;
            }
            let bytes: [u8; 8] = match data[SCOUT_PC_BANK_CACHE_TIMESTAMP_OFFSET
                ..SCOUT_PC_BANK_CACHE_TIMESTAMP_OFFSET + 8]
                .try_into()
            {
                Ok(v) => v,
                Err(_) => continue,
            };
            let observed = i64::from_le_bytes(bytes);
            if observed > self.scout_p26_bank_hw[index] {
                self.scout_p26_bank_hw[index] = observed;
            }
        }
    }

    /// Mint (once per lineage) the MarginfiAccount P-0026 owns outright and pulse its health.
    fn scout_p26_ensure_probe_account_pulsed(&mut self) -> bool {
        if self.scout_p26_probe_account == Pubkey::default() {
            if let Some(account) = self.scout_create_initialized_marginfi_account() {
                let liquidation_record = scout_liquidation_record_pda(self.program_id, account);
                let recorded = self
                    .ctx
                    .program(self.program_id)
                    .call(instruction::MarginfiAccountInitLiqRecord {})
                    .accounts(accounts::MarginfiAccountInitLiqRecord {
                        marginfi_account: account,
                        fee_payer: self.payer.pubkey(),
                        liquidation_record,
                    })
                    .signers(&[&*self.payer])
                    .send()
                    .map(|o| o.is_success())
                    .unwrap_or(false);
                if recorded {
                    self.scout_p26_probe_account = account;
                    self.scout_register_subject_record(liquidation_record);
                }
            }
        }
        let account = self.scout_p26_probe_account;
        if account == Pubkey::default() {
            return false;
        }
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingAccountPulseHealth {})
            .accounts(accounts::LendingAccountPulseHealth {
                marginfi_account: account,
                group: self.marginfi_group,
            })
            .remaining_accounts(vec![])
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    /// P-0026's probe. Four arms, all built from REAL instructions:
    ///   0 -- pulse the two dedicated health accounts (`lending_account_pulse_health`);
    ///   1 -- pulse the price cache of the three Fixed-oracle fixture banks;
    ///   2 -- mint (once) the probe-owned MarginfiAccount + its LiquidationRecord and pulse it;
    ///   3 -- pulse that same account, record the high-water it produced, and THEN run the real
    ///        `start_deleverage` + `end_deleverage` bracket on it.
    pub fn action_pulse_freshness_probe(&mut self, arm: u8) -> bool {
        self.action_advance_panic_pause_expiry();
        let mut acted = false;
        match arm % 4 {
            0 => {
                acted = self.action_pulse_health_healthy_dedicated() || acted;
                acted = self.action_pulse_health_risk_rejected_dedicated() || acted;
            }
            1 => {
                let banks = [
                    self.pulse_health_healthy_bank,
                    self.borrow_liab_bank,
                    self.borrow_asset_bank,
                ];
                for bank in banks {
                    if bank == Pubkey::default() {
                        continue;
                    }
                    let ok = self
                        .ctx
                        .program(self.program_id)
                        .call(instruction::LendingPoolPulseBankPriceCache {})
                        .accounts(accounts::LendingPoolPulseBankPriceCache {
                            group: self.marginfi_group,
                            bank,
                        })
                        .remaining_accounts(vec![])
                        .signers(&[&*self.payer])
                        .send()
                        .map(|o| o.is_success())
                        .unwrap_or(false);
                    acted = ok || acted;
                }
            }
            2 => {
                acted = self.scout_p26_ensure_probe_account_pulsed();
            }
            _ => {
                let pulsed = self.scout_p26_ensure_probe_account_pulsed();
                self.scout_p26_refresh_high_water();
                acted = pulsed;
                let account = self.scout_p26_probe_account;
                if account != Pubkey::default() {
                    let liquidation_record =
                        scout_liquidation_record_pda(self.program_id, account);
                    let group = self.marginfi_group;
                    let risk_admin = self.payer.pubkey();
                    let start_ix = scout_start_deleverage_ix(
                        self.program_id,
                        account,
                        liquidation_record,
                        group,
                        risk_admin,
                    );
                    let end_ix = scout_end_deleverage_ix(
                        self.program_id,
                        account,
                        liquidation_record,
                        group,
                        risk_admin,
                    );
                    if self
                        .ctx
                        .raw_call(start_ix)
                        .signers(&[&*self.payer])
                        .add_transaction()
                        .is_ok()
                        && self
                            .ctx
                            .raw_call(end_ix)
                            .signers(&[&*self.payer])
                            .add_transaction()
                            .is_ok()
                    {
                        let bracketed = self
                            .ctx
                            .send_batch()
                            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
                            .unwrap_or(false);
                        acted = bracketed || acted;
                    }
                }
            }
        }
        self.scout_p26_refresh_high_water();
        acted
    }

    /// Move one bank's fixed oracle price by at most 1%, through the real instruction.
    pub fn action_oracle_drift(&mut self, bank_choice: u8, up: bool, bps: u8) -> bool {
        let candidates = [self.bank, self.withdraw_bank, self.fee_bank];
        let start = (bank_choice as usize) % candidates.len();
        let mut bank = Pubkey::default();
        let mut data: Vec<u8> = Vec::new();
        for offset in 0..candidates.len() {
            let candidate = candidates[(start + offset) % candidates.len()];
            let bytes = match self.ctx.read_account(&candidate) {
                Ok(a) => a.data,
                Err(_) => continue,
            };
            if bytes.len() != SCOUT_HP_BANK_LEN || bytes[..8] != SCOUT_HP_BANK_DISCRIMINATOR {
                continue;
            }
            if bytes[SCOUT_HP_BANK_ORACLE_SETUP_OFFSET] != SCOUT_HP_ORACLE_SETUP_FIXED {
                continue;
            }
            bank = candidate;
            data = bytes;
            break;
        }
        if bank == Pubkey::default() {
            return false;
        }
        let price_bytes: [u8; 16] = match data
            [SCOUT_HP_BANK_FIXED_PRICE_OFFSET..SCOUT_HP_BANK_FIXED_PRICE_OFFSET + 16]
            .try_into()
        {
            Ok(b) => b,
            Err(_) => return false,
        };
        let current = fixed::types::I80F48::from_le_bytes(price_bytes);
        if current <= fixed::types::I80F48::ZERO {
            return false;
        }

        let step_bps = ((bps as u64) % SCOUT_DRIFT_MAX_BPS) + 1;
        let delta = match current
            .checked_mul(fixed::types::I80F48::from_num(step_bps))
            .and_then(|v| v.checked_div(fixed::types::I80F48::from_num(SCOUT_DRIFT_BPS_DENOMINATOR)))
        {
            Some(v) => v,
            None => return false,
        };
        let next = if up {
            match current.checked_add(delta) {
                Some(v) => v,
                None => return false,
            }
        } else {
            match current.checked_sub(delta) {
                Some(v) if v >= SCOUT_DRIFT_MIN_PRICE => v,
                _ => return false,
            }
        };

        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolSetFixedOraclePrice {
                price: marginfi::types::WrappedI80F48::from_i80f48(next),
            })
            .accounts(accounts::LendingPoolSetFixedOraclePrice {
                group: self.marginfi_group,
                admin: self.payer.pubkey(),
                bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    // ---- P-0028 CRANK NON-INTERFERENCE PROBE ---------------------------------------------------

    /// First differing byte in `[start, end)`, or `None` when the range is identical.
    fn scout_p28_first_difference(
        pre: &[u8],
        post: &[u8],
        start: usize,
        end: usize,
    ) -> Option<usize> {
        let mut index = start;
        while index < end {
            if index >= pre.len() || index >= post.len() {
                return Some(index);
            }
            if pre[index] != post[index] {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    /// Fold one region into `(mask, first_offset)`. `permitted` regions are skipped entirely.
    fn scout_p28_fold_region(
        pre: &[u8],
        post: &[u8],
        start: usize,
        end: usize,
        bit: u32,
        permitted: bool,
        mask: u32,
        first: u32,
    ) -> (u32, u32) {
        if permitted {
            return (mask, first);
        }
        match Self::scout_p28_first_difference(pre, post, start, end) {
            Some(offset) => {
                let next_first = if mask == 0 { offset as u32 } else { first };
                (mask | bit, next_first)
            }
            None => (mask, first),
        }
    }

    /// Every region of the victim's MarginfiAccount the crank may NOT write.
    fn scout_p28_account_diff_mask(
        pre: &[u8],
        post: &[u8],
        arm: u8,
        settle_balance_index: usize,
    ) -> (u32, u32) {
        if pre.len() != SCOUT_P28_ACC_LEN
            || post.len() != SCOUT_P28_ACC_LEN
            || pre[..8] != SCOUT_P28_ACC_DISCRIMINATOR
            || post[..8] != SCOUT_P28_ACC_DISCRIMINATOR
        {
            return (SCOUT_P28_ACC_BIT_SHAPE, 0);
        }
        let allow_health_cache = arm == SCOUT_P28_ARM_PULSE_HEALTH;
        let allow_emissions = false;
        let mut mask: u32 = 0;
        let mut first: u32 = 0;

        let fixed_regions: [(usize, usize, u32, bool); 10] = [
            (0, SCOUT_P28_ACC_BALANCES_OFFSET, SCOUT_P28_ACC_BIT_IDENTITY, false),
            (
                SCOUT_P28_ACC_LENDING_PAD_OFFSET,
                SCOUT_P28_ACC_FLAGS_OFFSET,
                SCOUT_P28_ACC_BIT_LENDING_PAD,
                false,
            ),
            (
                SCOUT_P28_ACC_FLAGS_OFFSET,
                SCOUT_P28_ACC_EMISSIONS_DEST_OFFSET,
                SCOUT_P28_ACC_BIT_FLAGS,
                false,
            ),
            (
                SCOUT_P28_ACC_EMISSIONS_DEST_OFFSET,
                SCOUT_P28_ACC_HEALTH_CACHE_OFFSET,
                SCOUT_P28_ACC_BIT_EMISSIONS_DEST,
                false,
            ),
            (
                SCOUT_P28_ACC_HEALTH_CACHE_OFFSET,
                SCOUT_P28_ACC_MIGRATED_OFFSET,
                SCOUT_P28_ACC_BIT_HEALTH_CACHE,
                allow_health_cache,
            ),
            (
                SCOUT_P28_ACC_MIGRATED_OFFSET,
                SCOUT_P28_ACC_LAST_UPDATE_OFFSET,
                SCOUT_P28_ACC_BIT_MIGRATED,
                false,
            ),
            (
                SCOUT_P28_ACC_LAST_UPDATE_OFFSET,
                SCOUT_P28_ACC_INDEX_OFFSET,
                SCOUT_P28_ACC_BIT_LAST_UPDATE,
                allow_emissions,
            ),
            (
                SCOUT_P28_ACC_INDEX_OFFSET,
                SCOUT_P28_ACC_LIQ_RECORD_OFFSET,
                SCOUT_P28_ACC_BIT_INDEX,
                false,
            ),
            (
                SCOUT_P28_ACC_LIQ_RECORD_OFFSET,
                SCOUT_P28_ACC_TAIL_PAD_OFFSET,
                SCOUT_P28_ACC_BIT_LIQ_RECORD,
                false,
            ),
            (
                SCOUT_P28_ACC_TAIL_PAD_OFFSET,
                SCOUT_P28_ACC_LEN,
                SCOUT_P28_ACC_BIT_TAIL_PAD,
                false,
            ),
        ];
        for entry in fixed_regions.iter() {
            let folded = Self::scout_p28_fold_region(
                pre, post, entry.0, entry.1, entry.2, entry.3, mask, first,
            );
            mask = folded.0;
            first = folded.1;
        }

        for index in 0..SCOUT_P28_BALANCE_COUNT {
            let base = SCOUT_P28_ACC_BALANCES_OFFSET + index * SCOUT_P28_BALANCE_LEN;
            let permitted_here = allow_emissions && index == settle_balance_index;
            let balance_regions: [(usize, usize, u32, bool); 5] = [
                (
                    base,
                    base + SCOUT_P28_BALANCE_SHARES_OFFSET,
                    SCOUT_P28_ACC_BIT_BALANCE_SLOT,
                    false,
                ),
                (
                    base + SCOUT_P28_BALANCE_SHARES_OFFSET,
                    base + SCOUT_P28_BALANCE_EMISSIONS_OFFSET,
                    SCOUT_P28_ACC_BIT_BALANCE_SHARES,
                    false,
                ),
                (
                    base + SCOUT_P28_BALANCE_EMISSIONS_OFFSET,
                    base + SCOUT_P28_BALANCE_LAST_UPDATE_OFFSET,
                    SCOUT_P28_ACC_BIT_BALANCE_EMISSIONS,
                    permitted_here,
                ),
                (
                    base + SCOUT_P28_BALANCE_LAST_UPDATE_OFFSET,
                    base + SCOUT_P28_BALANCE_PAD_OFFSET,
                    SCOUT_P28_ACC_BIT_BALANCE_LAST_UPDATE,
                    permitted_here,
                ),
                (
                    base + SCOUT_P28_BALANCE_PAD_OFFSET,
                    base + SCOUT_P28_BALANCE_LEN,
                    SCOUT_P28_ACC_BIT_BALANCE_PAD,
                    false,
                ),
            ];
            for entry in balance_regions.iter() {
                let folded = Self::scout_p28_fold_region(
                    pre, post, entry.0, entry.1, entry.2, entry.3, mask, first,
                );
                mask = folded.0;
                first = folded.1;
            }
        }
        (mask, first)
    }

    /// Every region of one Bank the crank is NOT entitled to write.
    fn scout_p28_bank_diff_mask(pre: &[u8], post: &[u8], arm: u8, is_target: bool) -> (u32, u32) {
        if pre.len() != SCOUT_P28_BANK_LEN
            || post.len() != SCOUT_P28_BANK_LEN
            || pre[..8] != SCOUT_P28_BANK_DISCRIMINATOR
            || post[..8] != SCOUT_P28_BANK_DISCRIMINATOR
        {
            return (SCOUT_P28_BANK_BIT_SHAPE, 0);
        }
        // pulse_bank_price_cache ACCRUES INTEREST as well as refreshing the price
        // (pulse_bank_price_cache.rs:22-28 -- `bank.accrue_interest` then
        // `update_bank_cache`), so it legitimately stamps last_update / share values
        // / fee counters exactly like the dedicated accrue arms. Treat it as accruing,
        // or the probe reports every pulse as an out-of-set write.
        let accruing = is_target
            && (arm == SCOUT_P28_ARM_ACCRUE_COLLATERAL
                || arm == SCOUT_P28_ARM_ACCRUE_LIABILITY
                || arm == SCOUT_P28_ARM_PULSE_PRICE_CACHE);
        let settling = false;
        let caching = accruing || (is_target && arm == SCOUT_P28_ARM_PULSE_PRICE_CACHE);
        let mut mask: u32 = 0;
        let mut first: u32 = 0;
        let regions: [(usize, usize, u32, bool); 21] = [
            (0, SCOUT_P28_BANK_ASSET_SHARE_VALUE_OFFSET, SCOUT_P28_BANK_BIT_IDENTITY, false),
            (
                SCOUT_P28_BANK_ASSET_SHARE_VALUE_OFFSET,
                SCOUT_P28_BANK_LIABILITY_SHARE_VALUE_OFFSET,
                SCOUT_P28_BANK_BIT_ASSET_SHARE_VALUE,
                accruing,
            ),
            (
                SCOUT_P28_BANK_LIABILITY_SHARE_VALUE_OFFSET,
                SCOUT_P28_BANK_VAULTS_OFFSET,
                SCOUT_P28_BANK_BIT_LIABILITY_SHARE_VALUE,
                accruing,
            ),
            (
                SCOUT_P28_BANK_VAULTS_OFFSET,
                SCOUT_P28_BANK_INSURANCE_FEES_OFFSET,
                SCOUT_P28_BANK_BIT_VAULTS,
                false,
            ),
            (
                SCOUT_P28_BANK_INSURANCE_FEES_OFFSET,
                SCOUT_P28_BANK_FEE_VAULT_OFFSET,
                SCOUT_P28_BANK_BIT_INSURANCE_FEES,
                accruing,
            ),
            (
                SCOUT_P28_BANK_FEE_VAULT_OFFSET,
                SCOUT_P28_BANK_GROUP_FEES_OFFSET,
                SCOUT_P28_BANK_BIT_FEE_VAULT,
                false,
            ),
            (
                SCOUT_P28_BANK_GROUP_FEES_OFFSET,
                SCOUT_P28_BANK_TOTAL_SHARES_OFFSET,
                SCOUT_P28_BANK_BIT_GROUP_FEES,
                accruing,
            ),
            (
                SCOUT_P28_BANK_TOTAL_SHARES_OFFSET,
                SCOUT_P28_BANK_LAST_UPDATE_OFFSET,
                SCOUT_P28_BANK_BIT_TOTAL_SHARES,
                false,
            ),
            (
                SCOUT_P28_BANK_LAST_UPDATE_OFFSET,
                SCOUT_P28_BANK_CONFIG_OFFSET,
                SCOUT_P28_BANK_BIT_LAST_UPDATE,
                accruing,
            ),
            (
                SCOUT_P28_BANK_CONFIG_OFFSET,
                SCOUT_P28_BANK_FLAGS_OFFSET,
                SCOUT_P28_BANK_BIT_CONFIG,
                false,
            ),
            (
                SCOUT_P28_BANK_FLAGS_OFFSET,
                SCOUT_P28_BANK_EMISSIONS_RATE_OFFSET,
                SCOUT_P28_BANK_BIT_FLAGS,
                false,
            ),
            (
                SCOUT_P28_BANK_EMISSIONS_RATE_OFFSET,
                SCOUT_P28_BANK_EMISSIONS_REMAINING_OFFSET,
                SCOUT_P28_BANK_BIT_EMISSIONS_RATE,
                false,
            ),
            (
                SCOUT_P28_BANK_EMISSIONS_REMAINING_OFFSET,
                SCOUT_P28_BANK_EMISSIONS_MINT_OFFSET,
                SCOUT_P28_BANK_BIT_EMISSIONS_REMAINING,
                settling,
            ),
            (
                SCOUT_P28_BANK_EMISSIONS_MINT_OFFSET,
                SCOUT_P28_BANK_PROGRAM_FEES_OFFSET,
                SCOUT_P28_BANK_BIT_EMISSIONS_MINT,
                false,
            ),
            (
                SCOUT_P28_BANK_PROGRAM_FEES_OFFSET,
                SCOUT_P28_BANK_EMODE_OFFSET,
                SCOUT_P28_BANK_BIT_PROGRAM_FEES,
                accruing,
            ),
            (
                SCOUT_P28_BANK_EMODE_OFFSET,
                SCOUT_P28_BANK_FEES_DEST_OFFSET,
                SCOUT_P28_BANK_BIT_EMODE,
                false,
            ),
            (
                SCOUT_P28_BANK_FEES_DEST_OFFSET,
                SCOUT_P28_BANK_CACHE_OFFSET,
                SCOUT_P28_BANK_BIT_FEES_DEST,
                false,
            ),
            (
                SCOUT_P28_BANK_CACHE_OFFSET,
                SCOUT_P28_BANK_COUNTS_OFFSET,
                SCOUT_P28_BANK_BIT_CACHE,
                caching,
            ),
            (
                SCOUT_P28_BANK_COUNTS_OFFSET,
                SCOUT_P28_BANK_INTEGRATION_OFFSET,
                SCOUT_P28_BANK_BIT_COUNTS,
                false,
            ),
            (
                SCOUT_P28_BANK_INTEGRATION_OFFSET,
                SCOUT_P28_BANK_TAIL_PAD_OFFSET,
                SCOUT_P28_BANK_BIT_INTEGRATION,
                false,
            ),
            (
                SCOUT_P28_BANK_TAIL_PAD_OFFSET,
                SCOUT_P28_BANK_LEN,
                SCOUT_P28_BANK_BIT_TAIL_PAD,
                false,
            ),
        ];
        for entry in regions.iter() {
            let folded = Self::scout_p28_fold_region(
                pre, post, entry.0, entry.1, entry.2, entry.3, mask, first,
            );
            mask = folded.0;
            first = folded.1;
        }
        (mask, first)
    }

    /// Index of the active balance whose `bank_pk` is `bank`, or `usize::MAX`.
    fn scout_p28_balance_index(data: &[u8], bank: Pubkey) -> usize {
        if data.len() != SCOUT_P28_ACC_LEN {
            return usize::MAX;
        }
        let wanted = bank.to_bytes();
        for index in 0..SCOUT_P28_BALANCE_COUNT {
            let base = SCOUT_P28_ACC_BALANCES_OFFSET + index * SCOUT_P28_BALANCE_LEN;
            if data[base] == 0 {
                continue;
            }
            let start = base + SCOUT_P28_BALANCE_BANK_PK_OFFSET;
            if data[start..start + 32] == wanted[..] {
                return index;
            }
        }
        usize::MAX
    }

    /// P-0028's driver.
    pub fn action_crank_non_interference_probe(&mut self, choice: u8) -> bool {
        let arm = choice % SCOUT_P28_ARM_COUNT + 1;
        let (victim, _victim_authority, third_party_account, collateral_bank, liability_bank, sorted) =
            match self.scout_p24_build_scenario() {
                Some(scenario) => scenario,
                None => return false,
            };

        {
            use ::anchor_lang::prelude::Clock;
            let clock = self.ctx.svm.get_sysvar::<Clock>();
            self.ctx.set_sysvar(&Clock {
                slot: clock.slot + SCOUT_P28_WARP_SLOTS,
                epoch_start_timestamp: clock.epoch_start_timestamp,
                epoch: clock.epoch,
                leader_schedule_epoch: clock.leader_schedule_epoch,
                unix_timestamp: clock.unix_timestamp + SCOUT_P28_WARP_SECONDS,
            });
        }

        let pre_account = match self.ctx.read_account(&victim) {
            Ok(account) => account.data,
            Err(_) => return false,
        };
        let pre_collateral = match self.ctx.read_account(&collateral_bank) {
            Ok(account) => account.data,
            Err(_) => return false,
        };
        let pre_liability = match self.ctx.read_account(&liability_bank) {
            Ok(account) => account.data,
            Err(_) => return false,
        };
        let pre_health = self.scout_hp_maintenance_health(victim);
        let pre_collateral_price = self.scout_p24_fixed_price_bits(collateral_bank);
        let pre_liability_price = self.scout_p24_fixed_price_bits(liability_bank);

        self.scout_p28_arm = arm;
        self.scout_p28_victim = victim;
        self.scout_p28_actor = self.payer.pubkey();
        self.scout_p28_measured = false;
        self.scout_p28_health_valid = false;
        self.scout_p28_pinned = false;
        self.scout_p28_succeeded = false;
        self.scout_p28_account_mask = 0;
        self.scout_p28_bank_mask = 0;
        self.scout_p28_first_offset = 0;
        self.scout_p28_pre_health = pre_health.unwrap_or(fixed::types::I80F48::ZERO).to_bits();
        self.scout_p28_post_health = 0;
        self.scout_p28_followup_measured = false;
        self.scout_p28_followup_ok = false;

        let group = self.marginfi_group;
        let succeeded = if arm == SCOUT_P28_ARM_PULSE_HEALTH {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingAccountPulseHealth {})
                .accounts(accounts::LendingAccountPulseHealth {
                    marginfi_account: victim,
                    group: self.marginfi_group,
                })
                .remaining_accounts(sorted.to_vec())
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P28_ARM_ACCRUE_COLLATERAL {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAccrueBankInterest {})
                .accounts(accounts::LendingPoolAccrueBankInterest {
                    group,
                    bank: collateral_bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P28_ARM_ACCRUE_LIABILITY {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolAccrueBankInterest {})
                .accounts(accounts::LendingPoolAccrueBankInterest {
                    group,
                    bank: liability_bank,
                })
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else if arm == SCOUT_P28_ARM_PULSE_PRICE_CACHE {
            self.ctx
                .program(self.program_id)
                .call(instruction::LendingPoolPulseBankPriceCache {})
                .accounts(accounts::LendingPoolPulseBankPriceCache {
                    group,
                    bank: collateral_bank,
                })
                .remaining_accounts(vec![])
                .signers(&[&*self.payer])
                .send()
                .map(|o| o.is_success())
                .unwrap_or(false)
        } else {
            self.scout_p28_arm = SCOUT_P28_ARM_NONE;
            return false;
        };

        let post_account = match self.ctx.read_account(&victim) {
            Ok(account) => account.data,
            Err(_) => return false,
        };
        let post_collateral = match self.ctx.read_account(&collateral_bank) {
            Ok(account) => account.data,
            Err(_) => return false,
        };
        let post_liability = match self.ctx.read_account(&liability_bank) {
            Ok(account) => account.data,
            Err(_) => return false,
        };
        let post_health = self.scout_hp_maintenance_health(victim);
        let post_collateral_price = self.scout_p24_fixed_price_bits(collateral_bank);
        let post_liability_price = self.scout_p24_fixed_price_bits(liability_bank);

        let settle_bank = collateral_bank;
        let settle_index = Self::scout_p28_balance_index(&pre_account, settle_bank);
        let account_diff =
            Self::scout_p28_account_diff_mask(&pre_account, &post_account, arm, settle_index);
        let collateral_is_target = arm == SCOUT_P28_ARM_ACCRUE_COLLATERAL
            || arm == SCOUT_P28_ARM_PULSE_PRICE_CACHE;
        let liability_is_target = arm == SCOUT_P28_ARM_ACCRUE_LIABILITY;
        let collateral_diff = Self::scout_p28_bank_diff_mask(
            &pre_collateral,
            &post_collateral,
            arm,
            collateral_is_target,
        );
        let liability_diff = Self::scout_p28_bank_diff_mask(
            &pre_liability,
            &post_liability,
            arm,
            liability_is_target,
        );

        self.scout_p28_succeeded = succeeded;
        self.scout_p28_measured = true;
        self.scout_p28_account_mask = account_diff.0;
        self.scout_p28_bank_mask = collateral_diff.0 | liability_diff.0;
        self.scout_p28_first_offset = if account_diff.0 != 0 {
            account_diff.1
        } else if collateral_diff.0 != 0 {
            collateral_diff.1
        } else {
            liability_diff.1
        };
        self.scout_p28_post_health = post_health.unwrap_or(fixed::types::I80F48::ZERO).to_bits();
        self.scout_p28_health_valid = pre_health.is_some() && post_health.is_some();
        self.scout_p28_pinned = pre_collateral_price.is_some()
            && pre_liability_price.is_some()
            && pre_collateral_price == post_collateral_price
            && pre_liability_price == post_liability_price;

        // LEG B
        let (_collateral_vault_authority, collateral_vault) = (
            Pubkey::find_program_address(
                &[LIQUIDITY_VAULT_AUTHORITY_SEED, collateral_bank.as_ref()],
                &self.program_id,
            )
            .0,
            Pubkey::find_program_address(
                &[LIQUIDITY_VAULT_SEED, collateral_bank.as_ref()],
                &self.program_id,
            )
            .0,
        );
        let actor = self.payer.pubkey();
        let actor_token_account = self.signer_token_account;
        let followup = self
            .ctx
            .program(self.program_id)
            .call(instruction::LendingAccountDeposit {
                amount: SCOUT_P28_FOLLOWUP_DEPOSIT_AMOUNT,
                deposit_up_to_limit: None,
            })
            .accounts(accounts::LendingAccountDeposit {
                group,
                marginfi_account: third_party_account,
                authority: actor,
                bank: collateral_bank,
                signer_token_account: actor_token_account,
                liquidity_vault: collateral_vault,
                token_program: spl_token::id(),
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        self.scout_p28_followup_measured = true;
        self.scout_p28_followup_ok = followup;
        true
    }

    // P-0012 reachability.
    pub fn action_transfer_carrying_emissions_destination(&mut self) -> bool {
        let __p12_source_signer = Keypair::new();
        let source_account = __p12_source_signer.pubkey();
        let authority_a = self.payer.pubkey();
        let initialized = self
            .ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitialize {})
            .accounts(accounts::MarginfiAccountInitialize {
                marginfi_group: self.marginfi_group,
                marginfi_account: source_account,
                authority: authority_a,
                fee_payer: authority_a,
            })
            .signers(&[&*self.payer, &__p12_source_signer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !initialized {
            return false;
        }
        let registered = self
            .ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountUpdateEmissionsDestinationAccount {})
            .accounts(accounts::MarginfiAccountUpdateEmissionsDestinationAccount {
                marginfi_account: source_account,
                authority: authority_a,
                destination_account: authority_a,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if !registered {
            return false;
        }
        let __p12_new_signer = Keypair::new();
        let new_marginfi_account = __p12_new_signer.pubkey();
        let new_authority = Pubkey::new_unique();
        let __scout_success = self
            .ctx
            .program(self.program_id)
            .call(instruction::TransferToNewAccount {})
            .accounts(accounts::TransferToNewAccount {
                group: self.marginfi_group,
                fee_state: Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0,
                old_marginfi_account: source_account,
                new_marginfi_account,
                authority: authority_a,
                fee_payer: authority_a,
                new_authority,
                global_fee_wallet: self.global_fee_wallet,
            })
            .signers(&[&*self.payer, &__p12_new_signer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);
        if __scout_success {
            self.scout_p12_transferred_accounts
                [self.scout_p12_transferred_next % SCOUT_SUBJECT_CAP] = new_marginfi_account;
            self.scout_p12_transferred_next = self.scout_p12_transferred_next.saturating_add(1);
        }
        __scout_success
    }

    // ---- P-0031 / P-0032 liquidation-parity probe machinery ---------------------------------

    /// The shared liquidatable pair scenario builder.
    fn scout_liq_parity_scenario(
        &mut self,
        liquidator_liab_seed: u64,
    ) -> Option<(Pubkey, Pubkey, Pubkey, Pubkey, [Pubkey; 2])> {
        let asset_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let liab_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::from_num(10)) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let liquidity_provider = self.scout_create_initialized_marginfi_account()?;
        let liquidator = self.scout_create_initialized_marginfi_account()?;
        let liquidatee = self.scout_create_initialized_marginfi_account()?;
        if liquidator == liquidatee {
            return None;
        }
        let forged = scout_forged_bank_and_account_pdas(self.program_id);
        for excluded in forged.iter() {
            if *excluded == asset_bank
                || *excluded == liab_bank
                || *excluded == liquidator
                || *excluded == liquidatee
            {
                return None;
            }
        }
        if !self.scout_liquidate_deposit(
            liquidity_provider,
            liab_bank,
            SCOUT_P33_LIQUIDITY_DEPOSIT_AMOUNT,
        ) {
            return None;
        }
        if !self.scout_liquidate_deposit(
            liquidatee,
            asset_bank,
            SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT,
        ) {
            return None;
        }
        if !self.scout_liquidate_deposit(
            liquidator,
            asset_bank,
            SCOUT_P33_COLLATERAL_DEPOSIT_AMOUNT,
        ) {
            return None;
        }
        if liquidator_liab_seed > 0
            && !self.scout_liquidate_deposit(liquidator, liab_bank, liquidator_liab_seed)
        {
            return None;
        }
        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(
            liquidatee,
            liab_bank,
            SCOUT_P33_BORROW_AMOUNT,
            sorted_pair.to_vec(),
        ) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(
            asset_bank,
            fixed::types::I80F48::from_num(SCOUT_P33_CRASHED_PRICE),
        ) {
            return None;
        }
        Some((asset_bank, liab_bank, liquidator, liquidatee, sorted_pair))
    }

    /// `asset_shares * asset_share_value - liability_shares * liability_share_value` for one
    /// (account, bank) pair, in native token units.
    fn scout_p31_net_amount(
        &self,
        account: Pubkey,
        bank: Pubkey,
        mark: (
            fixed::types::I80F48,
            fixed::types::I80F48,
            fixed::types::I80F48,
            fixed::types::I80F48,
        ),
    ) -> Option<fixed::types::I80F48> {
        let (asset_shares, liability_shares) = self.scout_p33_shares(account, bank)?;
        let assets = asset_shares.checked_mul(mark.0)?;
        let liabilities = liability_shares.checked_mul(mark.1)?;
        assets.checked_sub(liabilities)
    }

    /// P-0031's driver.
    pub fn action_liquidation_two_leg_conservation_probe(&mut self, choice: u8) -> bool {
        let rounds = choice % SCOUT_P33_MAX_ROUNDS + 1;
        let arm = (choice / SCOUT_P33_MAX_ROUNDS) % 3;
        let liquidator_liab_seed = match arm {
            1 => SCOUT_P33_LIQUIDATOR_LIAB_DEPOSIT_AMOUNT,
            2 => SCOUT_P2123_LIQUIDATOR_LIAB_PARTIAL_AMOUNT,
            _ => 0,
        };
        let (asset_bank, liab_bank, liquidator, liquidatee, sorted_pair) =
            match self.scout_liq_parity_scenario(liquidator_liab_seed) {
                Some(v) => v,
                None => return false,
            };

        let mut measured_any = false;
        for _round in 0..rounds {
            // pre
            let pre_asset_mark = match self.scout_p33_bank_mark(asset_bank) {
                Some(v) => v,
                None => break,
            };
            let pre_liab_mark = match self.scout_p33_bank_mark(liab_bank) {
                Some(v) => v,
                None => break,
            };
            let pre_or_a = match self.scout_p31_net_amount(liquidator, asset_bank, pre_asset_mark) {
                Some(v) => v,
                None => break,
            };
            let pre_ee_a = match self.scout_p31_net_amount(liquidatee, asset_bank, pre_asset_mark) {
                Some(v) => v,
                None => break,
            };
            let pre_or_l = match self.scout_p31_net_amount(liquidator, liab_bank, pre_liab_mark) {
                Some(v) => v,
                None => break,
            };
            let pre_ee_l = match self.scout_p31_net_amount(liquidatee, liab_bank, pre_liab_mark) {
                Some(v) => v,
                None => break,
            };

            if !self.scout_p33_liquidate(
                asset_bank,
                liab_bank,
                liquidator,
                liquidatee,
                sorted_pair,
                SCOUT_P33_ASSET_AMOUNT,
            ) {
                break;
            }

            // post
            let post_asset_mark = match self.scout_p33_bank_mark(asset_bank) {
                Some(v) => v,
                None => break,
            };
            let post_liab_mark = match self.scout_p33_bank_mark(liab_bank) {
                Some(v) => v,
                None => break,
            };
            if pre_asset_mark != post_asset_mark || pre_liab_mark != post_liab_mark {
                continue;
            }
            let post_or_a = match self.scout_p31_net_amount(liquidator, asset_bank, post_asset_mark)
            {
                Some(v) => v,
                None => break,
            };
            let post_ee_a = match self.scout_p31_net_amount(liquidatee, asset_bank, post_asset_mark)
            {
                Some(v) => v,
                None => break,
            };
            let post_or_l = match self.scout_p31_net_amount(liquidator, liab_bank, post_liab_mark) {
                Some(v) => v,
                None => break,
            };
            let post_ee_l = match self.scout_p31_net_amount(liquidatee, liab_bank, post_liab_mark) {
                Some(v) => v,
                None => break,
            };

            // the four legs
            let d_or_a = match post_or_a.checked_sub(pre_or_a) {
                Some(v) => v,
                None => break,
            };
            let d_ee_a = match post_ee_a.checked_sub(pre_ee_a) {
                Some(v) => v,
                None => break,
            };
            let d_or_l = match post_or_l.checked_sub(pre_or_l) {
                Some(v) => v,
                None => break,
            };
            let d_ee_l = match post_ee_l.checked_sub(pre_ee_l) {
                Some(v) => v,
                None => break,
            };
            // COLLATERAL leg
            let collateral_residual = match d_or_a.checked_add(d_ee_a) {
                Some(v) => v.abs(),
                None => break,
            };
            // LIABILITY leg
            let liquidator_leg = match fixed::types::I80F48::ZERO.checked_sub(d_or_l) {
                Some(v) => v,
                None => break,
            };
            let scaled = match liquidator_leg.checked_mul(SCOUT_P31_FINAL_DISCOUNT) {
                Some(v) => v,
                None => break,
            };
            let expected_ee_l = match scaled.checked_div(SCOUT_P31_LIQUIDATOR_DISCOUNT) {
                Some(v) => v,
                None => break,
            };
            let liability_residual = match d_ee_l.checked_sub(expected_ee_l) {
                Some(v) => v.abs(),
                None => break,
            };

            let worst = collateral_residual.max(liability_residual);
            let stored_worst = fixed::types::I80F48::from_bits(self.scout_p31_worst_bits);
            if !self.scout_p31_valid || worst > stored_worst {
                self.scout_p31_worst_bits = worst.to_bits();
                self.scout_p31_collateral_residual_bits = collateral_residual.to_bits();
                self.scout_p31_liability_residual_bits = liability_residual.to_bits();
                self.scout_p31_collateral_leg_bits = d_or_a.to_bits();
                self.scout_p31_liquidator_liab_leg_bits = liquidator_leg.to_bits();
                self.scout_p31_liquidatee_liab_leg_bits = d_ee_l.to_bits();
                self.scout_p31_arm = arm;
                self.scout_p31_liquidator = liquidator;
                self.scout_p31_liquidatee = liquidatee;
            }
            self.scout_p31_rounds = self.scout_p31_rounds.saturating_add(1);
            self.scout_p31_valid = true;
            measured_any = true;
        }
        measured_any
    }

    /// The live `fee_state.liquidation_max_fee`.
    fn scout_p32_liquidation_max_fee(&self) -> Option<fixed::types::I80F48> {
        const P32_FEE_STATE_MAX_FEE_OFFSET: usize = 8 + 112;
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;
        let data = self.ctx.read_account(&fee_state).ok()?.data;
        if data.len() < P32_FEE_STATE_MAX_FEE_OFFSET + 16 {
            return None;
        }
        let bytes: [u8; 16] = data
            [P32_FEE_STATE_MAX_FEE_OFFSET..P32_FEE_STATE_MAX_FEE_OFFSET + 16]
            .try_into()
            .ok()?;
        Some(fixed::types::I80F48::from_le_bytes(bytes))
    }

    /// P-0032's driver.
    pub fn action_liquidation_bracket_parity_probe(&mut self, choice: u8) -> bool {
        let arm = choice % SCOUT_P32_ARMS;
        let repay_amount = if arm == 0 {
            SCOUT_P32_FAITHFUL_REPAY_AMOUNT
        } else {
            SCOUT_P32_HEALTH_NEUTRAL_REPAY_AMOUNT
        };
        let (asset_bank, liab_bank, _liquidator, liquidatee, sorted_pair) =
            match self.scout_liq_parity_scenario(0) {
                Some(v) => v,
                None => return false,
            };

        let liquidation_record = scout_liquidation_record_pda(self.program_id, liquidatee);
        let payer = self.payer.clone();
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitLiqRecord {})
            .accounts(accounts::MarginfiAccountInitLiqRecord {
                marginfi_account: liquidatee,
                fee_payer: payer.pubkey(),
                liquidation_record,
            })
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return false;
        }
        self.scout_register_subject_record(liquidation_record);

        let max_fee = match self.scout_p32_liquidation_max_fee() {
            Some(v) => v,
            None => return false,
        };

        // pre
        let pre_asset_mark = match self.scout_p33_bank_mark(asset_bank) {
            Some(v) => v,
            None => return false,
        };
        let pre_liab_mark = match self.scout_p33_bank_mark(liab_bank) {
            Some(v) => v,
            None => return false,
        };
        let pre_marks = [(asset_bank, pre_asset_mark), (liab_bank, pre_liab_mark)];
        let pre = match self.scout_p33_account_value(liquidatee, &pre_marks) {
            Some(v) => v,
            None => return false,
        };

        // the bracket
        let receiver = payer.pubkey();
        let asset_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, asset_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let asset_liquidity_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, asset_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liab_liquidity_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let fee_state = Pubkey::find_program_address(&[FEE_STATE_SEED], &self.program_id).0;

        let mut start_ix = scout_start_liquidation_ix(
            self.program_id,
            liquidatee,
            liquidation_record,
            self.marginfi_group,
            receiver,
        );
        start_ix.accounts.extend(sorted_pair.iter().map(|k| {
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)
        }));
        let mut withdraw_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingAccountWithdraw {
                amount: SCOUT_P32_SEIZE_AMOUNT,
                withdraw_all: None,
            },
            accounts::LendingAccountWithdraw {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: receiver,
                bank: asset_bank,
                destination_token_account: self.signer_token_account,
                bank_liquidity_vault_authority: asset_vault_authority,
                liquidity_vault: asset_liquidity_vault,
                token_program: spl_token::id(),
            },
        );
        withdraw_ix.accounts.extend(sorted_pair.iter().map(|k| {
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)
        }));
        let repay_ix = scout_anchor_instruction(
            self.program_id,
            instruction::LendingAccountRepay {
                amount: repay_amount,
                repay_all: None,
            },
            accounts::LendingAccountRepay {
                group: self.marginfi_group,
                marginfi_account: liquidatee,
                authority: receiver,
                bank: liab_bank,
                signer_token_account: self.signer_token_account,
                liquidity_vault: liab_liquidity_vault,
                token_program: spl_token::id(),
            },
        );
        let mut end_ix = scout_end_liquidation_ix(
            self.program_id,
            liquidatee,
            liquidation_record,
            self.marginfi_group,
            receiver,
            fee_state,
            self.global_fee_wallet,
            self.payer.pubkey(),
        );
        end_ix.accounts.extend(sorted_pair.iter().map(|k| {
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)
        }));

        for ix in [start_ix, withdraw_ix, repay_ix, end_ix] {
            if self
                .ctx
                .raw_call(ix)
                .signers(&[&*payer])
                .add_transaction()
                .is_err()
            {
                return false;
            }
        }
        if !self
            .ctx
            .send_batch()
            .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
            .unwrap_or(false)
        {
            return false;
        }

        // post
        let post_asset_mark = match self.scout_p33_bank_mark(asset_bank) {
            Some(v) => v,
            None => return false,
        };
        let post_liab_mark = match self.scout_p33_bank_mark(liab_bank) {
            Some(v) => v,
            None => return false,
        };
        if pre_asset_mark != post_asset_mark || pre_liab_mark != post_liab_mark {
            return false;
        }
        let post_marks = [(asset_bank, post_asset_mark), (liab_bank, post_liab_mark)];
        let post = match self.scout_p33_account_value(liquidatee, &post_marks) {
            Some(v) => v,
            None => return false,
        };

        let seized = match pre.0.checked_sub(post.0) {
            Some(v) => v,
            None => return false,
        };
        let repaid = match pre.1.checked_sub(post.1) {
            Some(v) => v,
            None => return false,
        };

        let scaled_seized = match seized.checked_mul(SCOUT_P31_FINAL_DISCOUNT) {
            Some(v) => v,
            None => return false,
        };
        let excess = match scaled_seized.checked_sub(repaid) {
            Some(v) => v,
            None => return false,
        };
        let stored_scaled =
            match fixed::types::I80F48::from_bits(self.scout_p32_seized_bits)
                .checked_mul(SCOUT_P31_FINAL_DISCOUNT)
            {
                Some(v) => v,
                None => fixed::types::I80F48::MIN,
            };
        let stored_excess =
            match stored_scaled.checked_sub(fixed::types::I80F48::from_bits(
                self.scout_p32_repaid_bits,
            )) {
                Some(v) => v,
                None => fixed::types::I80F48::MIN,
            };
        if !self.scout_p32_valid || excess > stored_excess {
            self.scout_p32_seized_bits = seized.to_bits();
            self.scout_p32_repaid_bits = repaid.to_bits();
            self.scout_p32_max_fee_bits = max_fee.to_bits();
            self.scout_p32_pre_assets_bits = pre.0.to_bits();
            self.scout_p32_arm = arm;
            self.scout_p32_account = liquidatee;
        }
        self.scout_p32_brackets = self.scout_p32_brackets.saturating_add(1);
        self.scout_p32_valid = true;
        true
    }

    // ---------------------------------------------------------------------------------------
    // P-0010 / P-0019 instrumentation -- the interest-rate model.
    fn scout_pir_build_rate_scenario(&mut self) -> Option<Pubkey> {
        let collateral_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let rate_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if collateral_bank == rate_bank {
            return None;
        }
        if !self.scout_liquidate_raise_liab_bank_limits(rate_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(
            collateral_bank,
            fixed::types::I80F48::from_num(SCOUT_PIR_COLLATERAL_PRICE),
        ) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(rate_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let lender = self.scout_create_initialized_marginfi_account()?;
        let borrower = self.scout_create_initialized_marginfi_account()?;
        if lender == borrower {
            return None;
        }
        let forged = scout_forged_bank_and_account_pdas(self.program_id);
        for excluded in forged.iter() {
            if *excluded == collateral_bank
                || *excluded == rate_bank
                || *excluded == lender
                || *excluded == borrower
            {
                return None;
            }
        }
        if !self.scout_liquidate_deposit(lender, rate_bank, SCOUT_PIR_LIQUIDITY_AMOUNT) {
            return None;
        }
        if !self.scout_liquidate_deposit(borrower, collateral_bank, SCOUT_PIR_COLLATERAL_AMOUNT) {
            return None;
        }
        let sorted_pair = if collateral_bank.to_bytes() > rate_bank.to_bytes() {
            [collateral_bank, rate_bank]
        } else {
            [rate_bank, collateral_bank]
        };
        if !self.scout_liquidate_borrow(
            borrower,
            rate_bank,
            SCOUT_PIR_BORROW_AMOUNT,
            sorted_pair.to_vec(),
        ) {
            return None;
        }
        if !self.scout_pir_install_curve(rate_bank) {
            return None;
        }
        let totals = self.scout_pir_read_totals(rate_bank)?;
        if totals.0 == fixed::types::I80F48::ZERO || totals.1 == fixed::types::I80F48::ZERO {
            return None;
        }
        Some(rate_bank)
    }

    pub fn action_interest_rate_model_probe(&mut self, choice: u8) -> bool {
        let arm = choice % 3;

        self.scout_pir_ready = false;
        self.scout_pir_acc_valid = false;

        let rate_bank = match self.scout_pir_build_rate_scenario() { Some(v) => v, None => return false };
        self.scout_pir_bank = rate_bank;
        self.scout_pir_ready = true;

        if arm == 0 {
            return true;
        }

        // arm 1/2
        if !self.scout_pir_accrue(rate_bank) {
            return false;
        }
        if arm == 1 {
            return true;
        }

        // arm 2: the P-0019 accrual window
        let pre = match self.scout_pir_read_accrual_inputs(rate_bank) {
            Some(v) => v,
            None => return false,
        };
        let group_fees = match self.scout_pir_read_group_fees() {
            Some(v) => v,
            None => return false,
        };
        self.scout_pir_advance_clock(SCOUT_PIR_ACCRUAL_SECONDS);
        if !self.scout_pir_accrue(rate_bank) {
            return false;
        }
        let post = match self.scout_pir_read_share_values(rate_bank) {
            Some(v) => v,
            None => return false,
        };
        let delta_signed = post.2.saturating_sub(pre.5);
        if delta_signed <= 0 {
            return false;
        }
        let delta: u64 = match u64::try_from(delta_signed) {
            Ok(v) => v,
            Err(_) => return false,
        };
        self.scout_pir_acc_bank = rate_bank;
        self.scout_pir_acc_delta = delta;
        self.scout_pir_acc_pre_asv = pre.0.to_bits();
        self.scout_pir_acc_pre_lsv = pre.1.to_bits();
        self.scout_pir_acc_post_asv = post.0.to_bits();
        self.scout_pir_acc_post_lsv = post.1.to_bits();
        self.scout_pir_acc_pre_asset_shares = pre.2.to_bits();
        self.scout_pir_acc_pre_liability_shares = pre.3.to_bits();
        self.scout_pir_acc_irc = pre.4;
        self.scout_pir_acc_program_fee_fixed = group_fees.0;
        self.scout_pir_acc_program_fee_rate = group_fees.1;
        self.scout_pir_acc_program_fees = group_fees.2;
        self.scout_pir_acc_valid = true;
        true
    }

    /// Installs the curve via the real lending_pool_configure_bank_interest_only.
    fn scout_pir_install_curve(&mut self, bank: Pubkey) -> bool {
        let interest_rate_config = marginfi::types::InterestRateConfigOpt {
            insurance_fee_fixed_apr: Some(marginfi::types::WrappedI80F48::from_i80f48(
                SCOUT_PIR_CURVE_INSURANCE_FIXED_APR,
            )),
            insurance_ir_fee: Some(marginfi::types::WrappedI80F48::from_i80f48(
                SCOUT_PIR_CURVE_INSURANCE_IR_FEE,
            )),
            protocol_fixed_fee_apr: Some(marginfi::types::WrappedI80F48::from_i80f48(
                SCOUT_PIR_CURVE_PROTOCOL_FIXED_APR,
            )),
            protocol_ir_fee: Some(marginfi::types::WrappedI80F48::from_i80f48(
                SCOUT_PIR_CURVE_PROTOCOL_IR_FEE,
            )),
            protocol_origination_fee: Some(marginfi::types::WrappedI80F48::from_i80f48(
                fixed::types::I80F48::ZERO,
            )),
            zero_util_rate: Some(SCOUT_PIR_CURVE_ZERO_UTIL_RATE),
            hundred_util_rate: Some(SCOUT_PIR_CURVE_HUNDRED_UTIL_RATE),
            points: Some([
                marginfi::types::RatePoint {
                    util: SCOUT_PIR_CURVE_P0_UTIL,
                    rate: SCOUT_PIR_CURVE_P0_RATE,
                },
                marginfi::types::RatePoint {
                    util: SCOUT_PIR_CURVE_P1_UTIL,
                    rate: SCOUT_PIR_CURVE_P1_RATE,
                },
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
                marginfi::types::RatePoint { util: 0, rate: 0 },
            ]),
        };
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolConfigureBankInterestOnly { interest_rate_config })
            .accounts(accounts::LendingPoolConfigureBankInterestOnly {
                group: self.marginfi_group,
                delegate_curve_admin: self.payer.pubkey(),
                bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    /// Real permissionless crank: lending_pool_accrue_bank_interest.
    fn scout_pir_accrue(&mut self, bank: Pubkey) -> bool {
        self.ctx
            .program(self.program_id)
            .call(instruction::LendingPoolAccrueBankInterest {})
            .accounts(accounts::LendingPoolAccrueBankInterest {
                group: self.marginfi_group,
                bank,
            })
            .signers(&[&*self.payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
    }

    /// Pushes the SVM clock forward via a direct litesvm sysvar write.
    fn scout_pir_advance_clock(&mut self, seconds: i64) {
        use ::anchor_lang::prelude::Clock;
        let clock = self.ctx.svm.get_sysvar::<Clock>();
        self.ctx.set_sysvar(&Clock {
            slot: clock.slot + 10,
            epoch_start_timestamp: clock.epoch_start_timestamp,
            epoch: clock.epoch,
            leader_schedule_epoch: clock.leader_schedule_epoch,
            unix_timestamp: clock.unix_timestamp + seconds,
        });
    }

    /// (total_assets_amount, total_liabilities_amount) as update_bank_cache derives them.
    fn scout_pir_read_totals(
        &self,
        bank: Pubkey,
    ) -> Option<(fixed::types::I80F48, fixed::types::I80F48)> {
        let data = self.ctx.account_data(&bank).ok()?;
        if data.len() != SCOUT_PIR_BANK_LEN {
            return None;
        }
        let asv = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_ASSET_SHARE_VALUE_OFFSET..SCOUT_PIR_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let lsv = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_LIABILITY_SHARE_VALUE_OFFSET
                ..SCOUT_PIR_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let asset_shares = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_TOTAL_ASSET_SHARES_OFFSET..SCOUT_PIR_TOTAL_ASSET_SHARES_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let liability_shares = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_TOTAL_LIABILITY_SHARES_OFFSET
                ..SCOUT_PIR_TOTAL_LIABILITY_SHARES_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        Some((
            asset_shares.checked_mul(asv)?,
            liability_shares.checked_mul(lsv)?,
        ))
    }

    /// (asset_share_value, liability_share_value, last_update).
    fn scout_pir_read_share_values(
        &self,
        bank: Pubkey,
    ) -> Option<(fixed::types::I80F48, fixed::types::I80F48, i64)> {
        let data = self.ctx.account_data(&bank).ok()?;
        if data.len() != SCOUT_PIR_BANK_LEN {
            return None;
        }
        let asv = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_ASSET_SHARE_VALUE_OFFSET..SCOUT_PIR_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let lsv = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_LIABILITY_SHARE_VALUE_OFFSET
                ..SCOUT_PIR_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let last_update = i64::from_le_bytes(
            data[SCOUT_PIR_LAST_UPDATE_OFFSET..SCOUT_PIR_LAST_UPDATE_OFFSET + 8]
                .try_into()
                .ok()?,
        );
        Some((asv, lsv, last_update))
    }

    /// Everything P-0019's recomputation needs, read in ONE pass: (asset_share_value, liability_share_value, total_asset_shares, total_liability_shares, interest_rate_config bytes, last_update).
    fn scout_pir_read_accrual_inputs(
        &self,
        bank: Pubkey,
    ) -> Option<(
        fixed::types::I80F48,
        fixed::types::I80F48,
        fixed::types::I80F48,
        fixed::types::I80F48,
        [u8; SCOUT_PIR_IRC_LEN],
        i64,
    )> {
        let data = self.ctx.account_data(&bank).ok()?;
        if data.len() != SCOUT_PIR_BANK_LEN {
            return None;
        }
        let asv = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_ASSET_SHARE_VALUE_OFFSET..SCOUT_PIR_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let lsv = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_LIABILITY_SHARE_VALUE_OFFSET
                ..SCOUT_PIR_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let asset_shares = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_TOTAL_ASSET_SHARES_OFFSET..SCOUT_PIR_TOTAL_ASSET_SHARES_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let liability_shares = fixed::types::I80F48::from_le_bytes(
            data[SCOUT_PIR_TOTAL_LIABILITY_SHARES_OFFSET
                ..SCOUT_PIR_TOTAL_LIABILITY_SHARES_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let irc: [u8; SCOUT_PIR_IRC_LEN] = data
            [SCOUT_PIR_IRC_OFFSET..SCOUT_PIR_IRC_OFFSET + SCOUT_PIR_IRC_LEN]
            .try_into()
            .ok()?;
        let last_update = i64::from_le_bytes(
            data[SCOUT_PIR_LAST_UPDATE_OFFSET..SCOUT_PIR_LAST_UPDATE_OFFSET + 8]
                .try_into()
                .ok()?,
        );
        Some((asv, lsv, asset_shares, liability_shares, irc, last_update))
    }

    /// (program_fee_fixed bits, program_fee_rate bits, program_fees_enabled).
    fn scout_pir_read_group_fees(&self) -> Option<(i128, i128, bool)> {
        let data = self.ctx.account_data(&self.marginfi_group).ok()?;
        if data.len() != SCOUT_PIR_GROUP_LEN {
            return None;
        }
        let fixed_bits = i128::from_le_bytes(
            data[SCOUT_PIR_GROUP_PROGRAM_FEE_FIXED_OFFSET
                ..SCOUT_PIR_GROUP_PROGRAM_FEE_FIXED_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let rate_bits = i128::from_le_bytes(
            data[SCOUT_PIR_GROUP_PROGRAM_FEE_RATE_OFFSET
                ..SCOUT_PIR_GROUP_PROGRAM_FEE_RATE_OFFSET + 16]
                .try_into()
                .ok()?,
        );
        let flags = u64::from_le_bytes(
            data[SCOUT_PIR_GROUP_FLAGS_OFFSET..SCOUT_PIR_GROUP_FLAGS_OFFSET + 8]
                .try_into()
                .ok()?,
        );
        Some((
            fixed_bits,
            rate_bits,
            flags & SCOUT_PIR_PROGRAM_FEES_ENABLED != 0,
        ))
    }

    // ---------------------------------------------------------------------------------------
    // P-0007 / P-0009 instrumentation.

    /// P-0007's per-boundary reading of every subject bank.
    fn scout_p7_measure(
        &self,
    ) -> (
        [bool; SCOUT_P7_BANK_COUNT],
        [Pubkey; SCOUT_P7_BANK_COUNT],
        [i64; SCOUT_P7_BANK_COUNT],
        [bool; SCOUT_P7_BANK_COUNT],
        [[u8; 16]; SCOUT_P7_BANK_COUNT],
        [[u8; 16]; SCOUT_P7_BANK_COUNT],
    ) {
        const P7_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
        const P7_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
        let banks = [
            self.scout_p7_bank,
            self.borrow_liab_bank,
            self.borrow_asset_bank,
            self.fee_bank,
            self.bank,
        ];
        let mut valid = [false; SCOUT_P7_BANK_COUNT];
        let mut subject = [Pubkey::default(); SCOUT_P7_BANK_COUNT];
        let mut last_update = [0i64; SCOUT_P7_BANK_COUNT];
        let mut both_sided = [false; SCOUT_P7_BANK_COUNT];
        let mut asset_sv = [[0u8; 16]; SCOUT_P7_BANK_COUNT];
        let mut liab_sv = [[0u8; 16]; SCOUT_P7_BANK_COUNT];
        for index in 0..SCOUT_P7_BANK_COUNT {
            let bank = banks[index];
            if bank == Pubkey::default() {
                continue;
            }
            let data = match self.ctx.account_data(&bank) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            if data.len() != SCOUT_P7_BANK_LEN {
                continue;
            }
            if data[..8] != SCOUT_P7_BANK_DISCRIMINATOR {
                continue;
            }
            let stamp_bytes: [u8; 8] = match data[SCOUT_P7_BANK_LAST_UPDATE_OFFSET
                ..SCOUT_P7_BANK_LAST_UPDATE_OFFSET + 8]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let asset_bytes: [u8; 16] = match data[SCOUT_P7_BANK_TOTAL_ASSET_SHARES_OFFSET
                ..SCOUT_P7_BANK_TOTAL_ASSET_SHARES_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let liability_bytes: [u8; 16] = match data[SCOUT_P7_BANK_TOTAL_LIABILITY_SHARES_OFFSET
                ..SCOUT_P7_BANK_TOTAL_LIABILITY_SHARES_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            valid[index] = true;
            subject[index] = bank;
            last_update[index] = i64::from_le_bytes(stamp_bytes);
            both_sided[index] =
                i128::from_le_bytes(asset_bytes) > 0 && i128::from_le_bytes(liability_bytes) > 0;
            if let Ok(bytes) = data
                [P7_ASSET_SHARE_VALUE_OFFSET..P7_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
            {
                asset_sv[index] = bytes;
            }
            if let Ok(bytes) = data
                [P7_LIABILITY_SHARE_VALUE_OFFSET..P7_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
            {
                liab_sv[index] = bytes;
            }
        }
        (valid, subject, last_update, both_sided, asset_sv, liab_sv)
    }

    /// FNV-1a over the 304-byte health_cache region.
    fn scout_p9_digest(data: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for index in 0..SCOUT_P9_HEALTH_CACHE_LEN {
            hash ^= data[SCOUT_P9_HEALTH_CACHE_OFFSET + index] as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    /// P-0009's per-boundary reading of every subject MarginfiAccount.
    fn scout_p9_measure(
        &self,
    ) -> (
        [bool; SCOUT_P9_ACCOUNT_COUNT],
        [Pubkey; SCOUT_P9_ACCOUNT_COUNT],
        [u64; SCOUT_P9_ACCOUNT_COUNT],
    ) {
        let accounts = [
            self.borrow_marginfi_account,
            self.withdraw_marginfi_account,
            self.pulse_health_healthy_account,
            self.pulse_health_risk_rejected_account,
        ];
        let mut valid = [false; SCOUT_P9_ACCOUNT_COUNT];
        let mut subject = [Pubkey::default(); SCOUT_P9_ACCOUNT_COUNT];
        let mut digest = [0u64; SCOUT_P9_ACCOUNT_COUNT];
        for index in 0..SCOUT_P9_ACCOUNT_COUNT {
            let account = accounts[index];
            if account == Pubkey::default() {
                continue;
            }
            let data = match self.ctx.account_data(&account) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            if data.len() != SCOUT_P9_ACCOUNT_LEN {
                continue;
            }
            if data[..8] != SCOUT_P9_ACCOUNT_DISCRIMINATOR {
                continue;
            }
            valid[index] = true;
            subject[index] = account;
            digest[index] = Self::scout_p9_digest(&data);
        }
        (valid, subject, digest)
    }

    /// P-0007's probe.
    pub fn action_interest_accrual_stamp_probe(&mut self, arm: u8) -> bool {
        if !self.scout_p7_ready {
            let rate_bank = match self.scout_pir_build_rate_scenario() { Some(v) => v, None => return false };
            self.scout_p7_bank = rate_bank;
            self.scout_p7_ready = true;
            return true;
        }

        let bank = self.scout_p7_bank;
        if bank == Pubkey::default() {
            return false;
        }
        match arm % 3 {
            0 => {
                self.scout_pir_advance_clock(SCOUT_P7_WARP_SECONDS);
                self.scout_pir_accrue(bank)
            }
            1 => {
                self.scout_pir_advance_clock(SCOUT_P7_WARP_SECONDS);
                true
            }
            _ => self.scout_pir_accrue(bank),
        }
    }

    // ---- P-0015's deleverage withdraw-window machinery ----------------------------------------

    /// One u32 field of the live MarginfiGroup, or None if it doesn't read at the modelled shape. Used for daily_limit and withdrawn_today.
    fn scout_p15_group_u32(&self, offset: usize) -> Option<u32> {
        let data = self.ctx.read_account(&self.marginfi_group).ok()?.data;
        if data.len() != SCOUT_P15_GROUP_ACCOUNT_LEN || data[..8] != SCOUT_P15_GROUP_DISCRIMINATOR {
            return None;
        }
        let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
        Some(u32::from_le_bytes(bytes))
    }

    /// The exact dollar value update_withdrawn_equity is handed for a withdrawal of amount native units out of bank.
    fn scout_p15_withdrawn_equity(
        &self,
        bank: Pubkey,
        amount: u64,
    ) -> Option<fixed::types::I80F48> {
        let data = self.ctx.read_account(&bank).ok()?.data;
        if data.len() != SCOUT_HP_BANK_LEN || data[..8] != SCOUT_HP_BANK_DISCRIMINATOR {
            return None;
        }
        if data[SCOUT_HP_BANK_ORACLE_SETUP_OFFSET] != SCOUT_HP_ORACLE_SETUP_FIXED {
            return None;
        }
        let price_bytes: [u8; 16] = data
            [SCOUT_HP_BANK_FIXED_PRICE_OFFSET..SCOUT_HP_BANK_FIXED_PRICE_OFFSET + 16]
            .try_into()
            .ok()?;
        let price = fixed::types::I80F48::from_le_bytes(price_bytes);
        if price <= fixed::types::I80F48::ZERO {
            return None;
        }
        let decimals = data[SCOUT_HP_BANK_MINT_DECIMALS_OFFSET] as usize;
        if decimals >= SCOUT_HP_EXP_10.len() {
            return None;
        }
        let scale = fixed::types::I80F48::checked_from_num(SCOUT_HP_EXP_10[decimals])?;
        fixed::types::I80F48::from_num(amount)
            .checked_mul(price)?
            .checked_div(scale)
    }

    /// A fresh, genuinely deleverageable pair for P-0015's probe.
    fn scout_p15_scenario(&mut self) -> Option<(Pubkey, Pubkey, Pubkey, [Pubkey; 2])> {
        let asset_bank = self.scout_liquidate_add_bank(scout_liquidation_bank_config())?;
        let liab_bank = self.scout_liquidate_add_bank(scout_valid_bank_config(10))?;
        if !self.scout_liquidate_raise_liab_bank_limits(liab_bank) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(asset_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        if !self.scout_liquidate_set_fixed_price(liab_bank, fixed::types::I80F48::ONE) {
            return None;
        }
        let provider = self.scout_create_initialized_marginfi_account()?;
        let deleveragee = self.scout_create_initialized_marginfi_account()?;
        if provider == deleveragee {
            return None;
        }
        let forged = scout_forged_bank_and_account_pdas(self.program_id);
        for excluded in forged.iter() {
            if *excluded == asset_bank
                || *excluded == liab_bank
                || *excluded == provider
                || *excluded == deleveragee
            {
                return None;
            }
        }
        if !self.scout_liquidate_deposit(provider, liab_bank, SCOUT_P15_LIQUIDITY_DEPOSIT) {
            return None;
        }
        if !self.scout_liquidate_deposit(deleveragee, asset_bank, SCOUT_P15_ASSET_DEPOSIT) {
            return None;
        }
        let sorted_pair = if asset_bank.to_bytes() > liab_bank.to_bytes() {
            [asset_bank, liab_bank]
        } else {
            [liab_bank, asset_bank]
        };
        if !self.scout_liquidate_borrow(
            deleveragee,
            liab_bank,
            SCOUT_P15_BORROW,
            sorted_pair.to_vec(),
        ) {
            return None;
        }
        let liquidation_record = scout_liquidation_record_pda(self.program_id, deleveragee);
        let payer = self.payer.clone();
        if !self
            .ctx
            .program(self.program_id)
            .call(instruction::MarginfiAccountInitLiqRecord {})
            .accounts(accounts::MarginfiAccountInitLiqRecord {
                marginfi_account: deleveragee,
                fee_payer: payer.pubkey(),
                liquidation_record,
            })
            .signers(&[&*payer])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false)
        {
            return None;
        }
        self.scout_register_subject_record(liquidation_record);
        Some((asset_bank, liab_bank, deleveragee, sorted_pair))
    }

    /// P-0015's driver.
    pub fn action_deleverage_withdraw_window_probe(&mut self, choice: u8) -> bool {
        let rounds = (choice % SCOUT_P15_MAX_ROUNDS) + 1;
        if !self.action_configure_deleverage_withdrawal_limit(SCOUT_P15_DAILY_LIMIT) {
            return false;
        }
        let limit = match self.scout_p15_group_u32(SCOUT_P15_GROUP_DAILY_LIMIT_OFFSET) {
            Some(v) => v,
            None => return false,
        };
        if limit == 0 {
            return false;
        }
        let (asset_bank, liab_bank, deleveragee, sorted_pair) = match self.scout_p15_scenario() {
            Some(v) => v,
            None => return false,
        };
        let liquidation_record = scout_liquidation_record_pda(self.program_id, deleveragee);
        let payer = self.payer.clone();
        let risk_admin = payer.pubkey();
        let group = self.marginfi_group;
        let asset_vault_authority = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_AUTHORITY_SEED, asset_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let asset_liquidity_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, asset_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let liab_liquidity_vault = Pubkey::find_program_address(
            &[LIQUIDITY_VAULT_SEED, liab_bank.as_ref()],
            &self.program_id,
        )
        .0;
        let mut recorded_any = false;
        for _round in 0..rounds {
            let value = match self.scout_p15_withdrawn_equity(asset_bank, SCOUT_P15_SEIZE_AMOUNT) {
                Some(v) => v,
                None => break,
            };
            let mut start_ix = scout_start_deleverage_ix(
                self.program_id,
                deleveragee,
                liquidation_record,
                group,
                risk_admin,
            );
            start_ix.accounts.extend(sorted_pair.iter().map(|k| {
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)
            }));
            let mut withdraw_ix = scout_anchor_instruction(
                self.program_id,
                instruction::LendingAccountWithdraw {
                    amount: SCOUT_P15_SEIZE_AMOUNT,
                    withdraw_all: None,
                },
                accounts::LendingAccountWithdraw {
                    group,
                    marginfi_account: deleveragee,
                    authority: risk_admin,
                    bank: asset_bank,
                    destination_token_account: self.signer_token_account,
                    bank_liquidity_vault_authority: asset_vault_authority,
                    liquidity_vault: asset_liquidity_vault,
                    token_program: spl_token::id(),
                },
            );
            withdraw_ix.accounts.extend(sorted_pair.iter().map(|k| {
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)
            }));
            let repay_ix = scout_anchor_instruction(
                self.program_id,
                instruction::LendingAccountRepay {
                    amount: SCOUT_P15_REPAY_AMOUNT,
                    repay_all: None,
                },
                accounts::LendingAccountRepay {
                    group,
                    marginfi_account: deleveragee,
                    authority: risk_admin,
                    bank: liab_bank,
                    signer_token_account: self.signer_token_account,
                    liquidity_vault: liab_liquidity_vault,
                    token_program: spl_token::id(),
                },
            );
            let mut end_ix = scout_end_deleverage_ix(
                self.program_id,
                deleveragee,
                liquidation_record,
                group,
                risk_admin,
            );
            end_ix.accounts.extend(sorted_pair.iter().map(|k| {
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(*k, false)
            }));

            let mut queued = true;
            for ix in [start_ix, withdraw_ix, repay_ix, end_ix] {
                if self
                    .ctx
                    .raw_call(ix)
                    .signers(&[&*payer])
                    .add_transaction()
                    .is_err()
                {
                    queued = false;
                    break;
                }
            }
            if !queued {
                break;
            }
            if !self
                .ctx
                .send_batch()
                .map(|o| o.map(|tx| tx.is_success()).unwrap_or(false))
                .unwrap_or(false)
            {
                break;
            }
            let now = {
                use ::anchor_lang::prelude::Clock;
                self.ctx.svm.get_sysvar::<Clock>().unix_timestamp
            };
            if self.scout_p15_next < SCOUT_P15_CAP {
                let slot = self.scout_p15_next;
                self.scout_p15_ts[slot] = now;
                self.scout_p15_value_bits[slot] = value.to_bits();
                self.scout_p15_limit[slot] = limit;
                self.scout_p15_next = self.scout_p15_next.saturating_add(1);
            }
            recorded_any = true;
        }
        recorded_any
    }

    // SCOUT:EXTRA-ACTIONS:END
}

#[invariant_test]
fn invariant_test(_f: &mut MarginfiFixture) {
    scout_check_session!();
    // SCOUT:INVARIANTS:BEGIN
    // SCOUT:INVARIANT:P-0002:BEGIN
    // P-0002 -- share conservation: bank.total_asset_shares equals the sum of asset_shares across live balances referencing it.
    fn invariant_p_0002(f: &mut MarginfiFixture) {
        let bank_pk = f.bank;
        if bank_pk == Pubkey::default() {
            return;
        }
        let bank_data = match f.ctx.read_account(&bank_pk) {
            Ok(a) => a.data,
            Err(_) => return,
        };
        if bank_data.len() < SCOUT_BANK_TOTAL_ASSET_SHARES_OFFSET + 16 {
            return;
        }
        let buf: [u8; 16] =
            bank_data[SCOUT_BANK_TOTAL_ASSET_SHARES_OFFSET
                ..SCOUT_BANK_TOTAL_ASSET_SHARES_OFFSET + 16].try_into().unwrap_or_default();
        let bank_total = fixed::types::I80F48::from_le_bytes(buf);

        let mut summed = fixed::types::I80F48::ZERO;
        if f.scout_known_next > SCOUT_KNOWN_CAP {
            return;
        }
        let accounts = f.scout_known_accounts;
        for acct in accounts {
            let data = match f.ctx.read_account(&acct) {
                Ok(a) => a.data,
                Err(_) => continue,
            };
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                if data[base] != 1 {
                    continue;
                }
                if &data[base + 1..base + 33] != bank_pk.as_ref() {
                    continue;
                }
                let sb: [u8; 16] =
                    data[base + 40..base + 56].try_into().unwrap_or_default();
                summed = summed.saturating_add(fixed::types::I80F48::from_le_bytes(sb));
            }
        }

        let diff = (bank_total - summed).abs();
        scout_check!(
            "P-0002",
            "bank-total-asset-shares-equals-sum-of-balances",
            diff <= SCOUT_SHARE_SUM_TOLERANCE,
            "P-0002: bank {} total_asset_shares={} but balances over {} known account(s) sum to {} (diff={})",
            bank_pk,
            bank_total,
            f.scout_known_accounts.len(),
            summed,
            diff
        );
    }
    scout_run_property!("P-0002", invariant_p_0002(fixture));
    // SCOUT:INVARIANT:P-0002:END
    // SCOUT:INVARIANT:P-0003:BEGIN
    // P-0003 -- per-bank solvency: liquidity_vault + total_liability_amount >= total_asset_amount + collected fees.
    fn invariant_p_0003(f: &mut MarginfiFixture) {
        const SCOUT_P3_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
        const SCOUT_P3_BANK_ACCOUNT_LEN: usize = 8 + 1856;
        const SCOUT_P3_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
        const SCOUT_P3_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
        const SCOUT_P3_LIQUIDITY_VAULT_OFFSET: usize = 8 + 104;
        const SCOUT_P3_ASSET_TAG_OFFSET: usize = SCOUT_BANK_CONFIG_OFFSET + 489;
        const SCOUT_P3_TOKENLESS_REPAYMENTS_ALLOWED: u64 = 1 << 5;
        const SCOUT_P3_FIRST_EXTERNAL_CUSTODY_ASSET_TAG: u8 = 3;
        const SCOUT_P3_TOLERANCE: fixed::types::I80F48 = fixed::types::I80F48::lit("1");

        let harness_fee_fabrication_bank = f.bank;

        for bank_pk in f.ctx.dirty_tracker.dirty_accounts() {
            if bank_pk == &harness_fee_fabrication_bank {
                continue;
            }
            let data = match f.ctx.read_account(bank_pk) {
                Ok(bank_account) => bank_account.data,
                _ => continue,
            };
            if data.len() != SCOUT_P3_BANK_ACCOUNT_LEN {
                continue;
            }
            if data[..8] != SCOUT_P3_BANK_DISCRIMINATOR[..] {
                continue;
            }
            let flag_bytes: [u8; 8] =
                data[SCOUT_BANK_FLAGS_OFFSET..SCOUT_BANK_FLAGS_OFFSET + 8].try_into().unwrap_or_default();
            let flags = u64::from_le_bytes(flag_bytes);
            if flags
                & (SCOUT_P3_TOKENLESS_REPAYMENTS_ALLOWED | SCOUT_TOKENLESS_REPAYMENTS_COMPLETE)
                != 0
            {
                continue;
            }
            if data[SCOUT_P3_ASSET_TAG_OFFSET] >= SCOUT_P3_FIRST_EXTERNAL_CUSTODY_ASSET_TAG {
                continue;
            }

            let vault_bytes: [u8; 32] =
                data[SCOUT_P3_LIQUIDITY_VAULT_OFFSET..SCOUT_P3_LIQUIDITY_VAULT_OFFSET + 32].try_into().unwrap_or_default();
            let liquidity_vault = Pubkey::new_from_array(vault_bytes);
            let vault_amount = match f.ctx.read_account(&liquidity_vault) {
                Ok(vault_account)
                    if vault_account.data.len() >= SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET + 8 =>
                {
                    u64::from_le_bytes(
                        vault_account.data[SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET
                            ..SCOUT_SPL_TOKEN_ACCOUNT_AMOUNT_OFFSET + 8]
                            .try_into()
                            .unwrap_or_default(),
                    )
                }
                _ => continue,
            };

            let asset_share_value_bytes: [u8; 16] = data[SCOUT_P3_ASSET_SHARE_VALUE_OFFSET
                ..SCOUT_P3_ASSET_SHARE_VALUE_OFFSET + 16].try_into().unwrap_or_default();
            let asset_share_value = fixed::types::I80F48::from_le_bytes(asset_share_value_bytes);
            let liability_share_value_bytes: [u8; 16] = data[SCOUT_P3_LIABILITY_SHARE_VALUE_OFFSET
                ..SCOUT_P3_LIABILITY_SHARE_VALUE_OFFSET + 16].try_into().unwrap_or_default();
            let liability_share_value =
                fixed::types::I80F48::from_le_bytes(liability_share_value_bytes);
            let total_asset_shares_bytes: [u8; 16] = data[SCOUT_BANK_TOTAL_ASSET_SHARES_OFFSET
                ..SCOUT_BANK_TOTAL_ASSET_SHARES_OFFSET + 16].try_into().unwrap_or_default();
            let total_asset_shares = fixed::types::I80F48::from_le_bytes(total_asset_shares_bytes);
            let total_liability_shares_bytes: [u8; 16] = data
                [SCOUT_BANK_TOTAL_LIABILITY_SHARES_OFFSET
                    ..SCOUT_BANK_TOTAL_LIABILITY_SHARES_OFFSET + 16]
                .try_into()
                .unwrap_or_default();
            let total_liability_shares =
                fixed::types::I80F48::from_le_bytes(total_liability_shares_bytes);
            let collected_insurance_bytes: [u8; 16] = data[SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET
                ..SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET + 16].try_into().unwrap_or_default();
            let collected_insurance =
                fixed::types::I80F48::from_le_bytes(collected_insurance_bytes);
            let collected_group_bytes: [u8; 16] = data[SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET
                ..SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET + 16].try_into().unwrap_or_default();
            let collected_group = fixed::types::I80F48::from_le_bytes(collected_group_bytes);
            let collected_program_bytes: [u8; 16] = data[SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET
                ..SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET + 16].try_into().unwrap_or_default();
            let collected_program = fixed::types::I80F48::from_le_bytes(collected_program_bytes);

            let total_asset_amount = match total_asset_shares.checked_mul(asset_share_value) {
                Some(value) => value,
                None => continue,
            };
            let total_liability_amount =
                match total_liability_shares.checked_mul(liability_share_value) {
                    Some(value) => value,
                    None => continue,
                };

            let custodied = match fixed::types::I80F48::checked_from_num(vault_amount) {
                Some(value) => value,
                None => continue,
            };
            let covering = match custodied.checked_add(total_liability_amount) {
                Some(value) => value,
                None => continue,
            };
            let booked_fees_partial = match collected_insurance.checked_add(collected_group) {
                Some(value) => value,
                None => continue,
            };
            let booked_fees = match booked_fees_partial.checked_add(collected_program) {
                Some(value) => value,
                None => continue,
            };
            let owed = match total_asset_amount.checked_add(booked_fees) {
                Some(value) => value,
                None => continue,
            };

            let shortfall = owed - covering;
            scout_check!(
                "P-0003",
                "bank-vault-plus-liabilities-covers-assets-plus-booked-fees",
                shortfall <= SCOUT_P3_TOLERANCE,
                "P-0003: bank {} insolvent by {} -- liquidity_vault({})={} + total_liability_amount={} \
                 (shares={} x liab_share_value={}) < total_asset_amount={} (shares={} x \
                 asset_share_value={}) + fees(insurance={} group={} program={}); flags={:#x} \
                 asset_tag={}",
                bank_pk,
                shortfall,
                liquidity_vault,
                vault_amount,
                total_liability_amount,
                total_liability_shares,
                liability_share_value,
                total_asset_amount,
                total_asset_shares,
                asset_share_value,
                collected_insurance,
                collected_group,
                collected_program,
                flags,
                data[SCOUT_P3_ASSET_TAG_OFFSET]
            );
        }
    }
    scout_run_property!("P-0003", invariant_p_0003(fixture));
    // SCOUT:INVARIANT:P-0003:END
    // SCOUT:INVARIANT:P-0001:BEGIN
    // P-0001 -- adversary value conservation: actor's total value must not increase between action boundaries at unchanged share values.
    fn invariant_p_0001(f: &mut MarginfiFixture) {
        const P1_TOLERANCE_NATIVE_UNITS: i64 = 16;

        if !f.scout_p1_cur_ok || !f.scout_p1_prev_ok {
            return;
        }

        let mut share_values_unchanged = true;
        for current in f.scout_p1_cur_share_values.iter() {
            let mut prev_hit = None;
            for prev in f.scout_p1_prev_share_values.iter() {
                if prev.0 == current.0 && prev_hit.is_none() {
                    prev_hit = Some((prev.1, prev.2));
                }
            }
            if let Some((prev_asv, prev_lsv)) = prev_hit {
                if prev_asv != current.1 || prev_lsv != current.2 {
                    share_values_unchanged = false;
                    break;
                }
            }
        }
        if !share_values_unchanged {
            return;
        }
        if f.scout_p1_cur_actor_count > f.scout_p1_prev_actor_count {
            return;
        }

        let value = fixed::types::I80F48::from_bits(f.scout_p1_cur_value);
        let previous = fixed::types::I80F48::from_bits(f.scout_p1_prev_value);
        let tokens = fixed::types::I80F48::from_bits(f.scout_p1_cur_tokens);
        let claim = fixed::types::I80F48::from_bits(f.scout_p1_cur_claim);
        let gain = value.saturating_sub(previous);
        let tolerance = fixed::types::I80F48::from_num(P1_TOLERANCE_NATIVE_UNITS);
        scout_check!(
            "P-0001",
            "actor-total-value-never-increases",
            gain <= tolerance,
            "P-0001: actor {} gained {} native units of {} at unchanged share values \
             (value {} -> {}; realised token balance {}, claimable {} over {} tracked account(s) \
             and {} bank(s); tolerance {})",
            f.payer.pubkey(),
            gain,
            f.bank_mint,
            previous,
            value,
            tokens,
            claim,
            f.scout_p1_cur_actor_count,
            f.scout_p1_cur_share_values.len(),
            tolerance
        );
    }
    scout_run_property!("P-0001", invariant_p_0001(fixture));
    // SCOUT:INVARIANT:P-0001:END
    // SCOUT:INVARIANT:P-0004:BEGIN
    // P-0004 -- round-trip conservation: a deposit/withdraw cannot return more than was paid in; a borrow must book at least as much liability value as it pays out.
    fn invariant_p_0004(f: &mut MarginfiFixture) {
        let deposits = [(
            f.scout_p4_dep_account,
            f.scout_p4_dep_bank,
            f.scout_p4_dep_tokens,
            f.scout_p4_dep_asv,
        )];
        for (account, bank, tokens_in, asv_at_first_deposit) in deposits {
            if account == Pubkey::default() || bank == Pubkey::default() {
                continue;
            }
            if tokens_in == 0 {
                continue;
            }
            let bank_data = match f.ctx.account_data(&bank) {
                Ok(d) => d,
                Err(_) => continue,
            };
            if bank_data.len() < SCOUT_P4_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16 {
                continue;
            }
            let asv_now: [u8; 16] = match bank_data[SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET
                ..SCOUT_P4_BANK_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            if asv_now != asv_at_first_deposit {
                continue;
            }
            let account_data = match f.ctx.account_data(&account) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let mut asset_shares = fixed::types::I80F48::ZERO;
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if account_data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                if account_data[base] != 1 {
                    continue;
                }
                let slot_bank: [u8; 32] = match account_data[base + 1..base + 33].try_into() {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                if Pubkey::new_from_array(slot_bank) != bank {
                    continue;
                }
                let asset_bytes: [u8; 16] = match account_data[base + 40..base + 56].try_into() {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                asset_shares = fixed::types::I80F48::from_le_bytes(asset_bytes);
            }
            let asv = fixed::types::I80F48::from_le_bytes(asv_now);
            let claim = match asset_shares.checked_mul(asv) {
                Some(v) => v,
                None => continue,
            };
            let paid_in = match fixed::types::I80F48::checked_from_num(tokens_in) {
                Some(v) => v,
                None => continue,
            };
            let allowed = paid_in.saturating_add(SCOUT_P4_UNIT_TOLERANCE);
            scout_check!(
                "P-0004",
                "deposit-withdraw-round-trip-cannot-return-more-than-was-paid-in",
                claim <= allowed,
                "P-0004: marginfi_account {} paid {} native unit(s) into bank {} at asset_share_value {} (unchanged since the first deposit) but now holds {} asset_shares, a claim of {} -- an excess of {} that a withdraw_all would hand back",
                account,
                tokens_in,
                bank,
                asv,
                asset_shares,
                claim,
                claim.saturating_sub(paid_in)
            );
        }

        let borrows = [(
            f.scout_p4_bor_account,
            f.scout_p4_bor_bank,
            f.scout_p4_bor_amount,
        )];
        for (account, bank, amount) in borrows {
            if account == Pubkey::default() || bank == Pubkey::default() {
                continue;
            }
            if !f.scout_p4_bor_prev_ok || !f.scout_p4_bor_cur_ok {
                continue;
            }
            let mut prev_shares = fixed::types::I80F48::ZERO;
            let mut cur_shares = fixed::types::I80F48::ZERO;
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = i * SCOUT_BALANCE_STRIDE;
                if f.scout_p4_bor_prev_slots[base] == 1 {
                    let slot_bank: [u8; 32] =
                        match f.scout_p4_bor_prev_slots[base + 1..base + 33].try_into() {
                            Ok(bytes) => bytes,
                            Err(_) => continue,
                        };
                    if Pubkey::new_from_array(slot_bank) == bank {
                        let liab_bytes: [u8; 16] =
                            match f.scout_p4_bor_prev_slots[base + 56..base + 72].try_into() {
                                Ok(bytes) => bytes,
                                Err(_) => continue,
                            };
                        prev_shares = fixed::types::I80F48::from_le_bytes(liab_bytes);
                    }
                }
                if f.scout_p4_bor_cur_slots[base] == 1 {
                    let slot_bank: [u8; 32] =
                        match f.scout_p4_bor_cur_slots[base + 1..base + 33].try_into() {
                            Ok(bytes) => bytes,
                            Err(_) => continue,
                        };
                    if Pubkey::new_from_array(slot_bank) == bank {
                        let liab_bytes: [u8; 16] =
                            match f.scout_p4_bor_cur_slots[base + 56..base + 72].try_into() {
                                Ok(bytes) => bytes,
                                Err(_) => continue,
                            };
                        cur_shares = fixed::types::I80F48::from_le_bytes(liab_bytes);
                    }
                }
            }
            let prev_value = prev_shares
                .saturating_mul(fixed::types::I80F48::from_le_bytes(f.scout_p4_bor_prev_lsv));
            let cur_value = cur_shares
                .saturating_mul(fixed::types::I80F48::from_le_bytes(f.scout_p4_bor_cur_lsv));
            if cur_value < prev_value {
                continue;
            }
            let delta = cur_value - prev_value;
            if amount == 0 {
                continue;
            }
            let borrowed = match fixed::types::I80F48::checked_from_num(amount) {
                Some(v) => v,
                None => continue,
            };
            scout_check!(
                "P-0004",
                "borrow-books-at-least-the-tokens-it-pays-out",
                delta.saturating_add(SCOUT_P4_DUST_TOLERANCE) >= borrowed,
                "P-0004: marginfi_account {} borrowed {} native unit(s) from bank {} but its liability value only grew by {} -- a shortfall of {}, so repaying in full would cost less than the borrow paid out",
                account,
                amount,
                bank,
                delta,
                borrowed.saturating_sub(delta)
            );
        }
    }
    scout_run_property!("P-0004", invariant_p_0004(fixture));
    // SCOUT:INVARIANT:P-0004:END
    // SCOUT:INVARIANT:P-0017:BEGIN
    // P-0017 -- pairing/liveness on ACCOUNT_IN_FLASHLOAN / ACCOUNT_IN_RECEIVERSHIP / ACCOUNT_IN_DELEVERAGE: no flag survives its transaction, DELEVERAGE implies RECEIVERSHIP.
    fn invariant_p_0017(f: &mut MarginfiFixture) {
        const P17_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
        const P17_ACCOUNT_LEN: usize = 8 + 2304;

        let registry = f.scout_p17_accounts;
        let registry_len = f.scout_p17_accounts_next.min(SCOUT_SUBJECT_CAP);
        let harness_flagged = f.scout_p17_harness_flagged.clone();
        let setup_flashloan_account = f.fee_borrow_marginfi_account;

        for slot in 0..registry_len {
            let account = registry[slot];
            if account == Pubkey::default() {
                continue;
            }
            if account == setup_flashloan_account || harness_flagged.contains(&account) {
                continue;
            }
            let data = match f.ctx.account_data(&account) {
                Ok(d) if d.len() == P17_ACCOUNT_LEN && d[..8] == P17_ACCOUNT_DISCRIMINATOR => d,
                _ => continue,
            };
            let buf: [u8; 8] =
                data[MARGINFI_ACCOUNT_FLAGS_OFFSET..MARGINFI_ACCOUNT_FLAGS_OFFSET + 8].try_into().unwrap_or_default();
            let flags = u64::from_le_bytes(buf);
            let in_flashloan = flags & SCOUT_ACCOUNT_IN_FLASHLOAN != 0;
            let in_receivership = flags & SCOUT_ACCOUNT_IN_RECEIVERSHIP != 0;
            let in_deleverage = flags & SCOUT_ACCOUNT_IN_DELEVERAGE != 0;

            scout_check!(
                "P-0017",
                "no-marginfi-account-carries-ACCOUNT_IN_FLASHLOAN-at-a-transaction-boundary",
                !in_flashloan,
                "P-0017: marginfi_account {} still has ACCOUNT_IN_FLASHLOAN set at an action \
                 boundary (flags=0x{:016x}). start_flashloan \
                 only commits when an end_flashloan for THIS account exists later in the SAME \
                 transaction (flashloan.rs:78-99), so the bracket leaked. While this bit is set \
                 RiskEngine::check_account_init_health returns Ok(()) unconditionally \
                 (state/marginfi_account.rs:546-551) -- every health check on this account is \
                 disabled.",
                account,
                flags
            );

            scout_check!(
                "P-0017",
                "ACCOUNT_IN_DELEVERAGE-implies-ACCOUNT_IN_RECEIVERSHIP",
                !in_deleverage || in_receivership,
                "P-0017: marginfi_account {} has ACCOUNT_IN_DELEVERAGE set without \
                 ACCOUNT_IN_RECEIVERSHIP (flags=0x{:016x}). start_deleverage sets both (liquidate_start.rs:65 then :103 via \
                 start_receivership) and end_deleverage clears both (liquidate_end.rs:93 then :151 \
                 via end_receivership), so the two bits can only diverge if some path cleared \
                 RECEIVERSHIP alone -- end_liquidation does exactly that, and does not touch \
                 DELEVERAGE. The orphaned bit still drives the deleverage withdraw-window \
                 accounting at withdraw.rs:143-151.",
                account,
                flags
            );

            scout_check!(
                "P-0017",
                "no-receivership-or-deleverage-bracket-is-still-open-at-a-transaction-boundary",
                !in_receivership && !in_deleverage,
                "P-0017: marginfi_account {} is still inside a receivership/deleverage bracket at \
                 an action boundary (flags=0x{:016x}, in_receivership={}, in_deleverage={}). \
                 Both start_liquidation and start_deleverage run \
                 validate_instructions (liquidate_start.rs:113-179), which requires the matching \
                 end_* to be the LAST instruction of the same atomic transaction, so no bracket \
                 may still be open here. The account is now stranded for its own authority: \
                 StartLiquidation/StartDeleverage both refuse an account already in receivership \
                 (liquidate_start.rs:186-199, :226-239), so the state cannot be re-entered to be \
                 exited, and deposit/borrow/transfer/flashloan/handle_bankruptcy all refuse to run \
                 on it.",
                account,
                flags,
                in_receivership,
                in_deleverage
            );
        }
    }
    scout_run_property!("P-0017", invariant_p_0017(fixture));
    // SCOUT:INVARIANT:P-0017:END
    // SCOUT:INVARIANT:P-0022:BEGIN
    // P-0022 -- no bad debt: no account ends an instruction with liability value greater than asset value, except transiently while flagged for bankruptcy.
    fn invariant_p_0022(f: &mut MarginfiFixture) {

        let mut marks_unchanged = true;
        for current in f.scout_p22_cur_marks.iter() {
            let mut prev_hit = None;
            for prev in f.scout_p22_bank_marks.iter() {
                if prev.0 == current.0 && prev_hit.is_none() {
                    prev_hit = Some((prev.1, prev.2, prev.3));
                }
            }
            if let Some((prev_asv, prev_lsv, prev_price)) = prev_hit {
                if prev_asv != current.1 || prev_lsv != current.2 || prev_price != current.3 {
                    marks_unchanged = false;
                    break;
                }
            }
        }
        if !marks_unchanged {
            return;
        }

        for current in f.scout_p22_cur_valued.iter() {
            let mut prev_hit = None;
            for prev in f.scout_p22_solvency.iter() {
                if prev.0 == current.0 && prev_hit.is_none() {
                    prev_hit = Some(prev.1);
                }
            }
            let was_solvent = match prev_hit {
                Some(prev_underwater) => !prev_underwater,
                None => false,
            };
            if !was_solvent {
                continue;
            }
            let asset_value = fixed::types::I80F48::from_bits(current.1);
            let liability_value = fixed::types::I80F48::from_bits(current.2);
            let shortfall = liability_value.saturating_sub(asset_value);
            scout_check!(
                "P-0022",
                "no-instruction-turns-a-solvent-marginfi-account-into-bad-debt",
                !current.3,
                "P-0022: marginfi_account {} now holds liability value {} against asset value \
                 {} -- bad debt of {} created by a single instruction, at UNCHANGED oracle \
                 prices and UNCHANGED asset/liability share values (the account was solvent at \
                 the previous action boundary and neither interest, nor a socialised loss, nor \
                 any oracle move happened in between; {} bank(s) re-checked). The account is \
                 not ACCOUNT_DISABLED, is not inside a flashloan/receivership/deleverage \
                 bracket, is not one of the harness's forged bankruptcy fixtures, and holds no \
                 balance against a forged or KilledByBankruptcy bank. No liquidator can clear \
                 this: liquidate.rs pays the liquidator out of collateral this account no \
                 longer has, so the shortfall is socialised onto the bank's depositors by \
                 handle_bankruptcy's `socialize_loss`.",
                current.0,
                liability_value,
                asset_value,
                shortfall,
                f.scout_p22_cur_marks.len()
            );
        }
    }
    scout_run_property!("P-0022", invariant_p_0022(fixture));
    // SCOUT:INVARIANT:P-0022:END
    // SCOUT:INVARIANT:P-0036:BEGIN
    // P-0036 -- remaining-accounts integrity: active balances form a contiguous, strictly descending, duplicate-free prefix naming real banks; inactive slots are fully zeroed.
    fn invariant_p_0036(f: &mut MarginfiFixture) {
        const P36_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
        const P36_ACCOUNT_LEN: usize = 8 + 2304;

        let account = f.scout_p36_sorted_account;
        if account == Pubkey::default() {
            return;
        }
        let data = match f.ctx.read_account(&account) {
            Ok(read) => read.data,
            Err(_) => return,
        };
        if data.len() != P36_ACCOUNT_LEN || data[..8] != P36_ACCOUNT_DISCRIMINATOR {
            return;
        }

        let zero_pubkey = [0u8; 32];
        let zero_shares = [0u8; 16];
        let mut prev_active = true;
        let mut have_prev = false;
        let mut prev_bank = [0u8; 32];
        let mut prev_index = 0usize;
        for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
            let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
            if data.len() < base + SCOUT_BALANCE_STRIDE {
                break;
            }
            let bank: [u8; 32] = match data[base + 1..base + 33].try_into() {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let asset_shares: [u8; 16] = match data[base + 40..base + 56].try_into() {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let liability_shares: [u8; 16] = match data[base + 56..base + 72].try_into() {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let active = data[base] != 0;

            if active {
                scout_check!(
                    "P-0036",
                    "active-balances-occupy-a-prefix",
                    prev_active,
                    "P-0036: marginfi_account {} has an ACTIVE balance at slot {} (bank {}) \
                     following an INACTIVE slot {}. sort_balances() \
                     (state/marginfi_account.rs:877-880) sorts descending by bank_pk and a closed \
                     slot's bank_pk is Pubkey::default() (the minimum key), so every inactive slot \
                     must sort last. A hole means the risk engine's positional walk over \
                     balances.iter().filter(is_active) (state/marginfi_account.rs:189-193) no \
                     longer corresponds to a contiguous caller-supplied list",
                    account,
                    i,
                    Pubkey::new_from_array(bank),
                    prev_index
                );

                scout_check!(
                    "P-0036",
                    "active-balance-names-a-bank",
                    bank != zero_pubkey,
                    "P-0036: marginfi_account {} has an ACTIVE balance at slot {} whose bank_pk is \
                     Pubkey::default(). find_or_create (state/marginfi_account.rs:944-999) only \
                     ever writes a real bank key, and check_eq!(balance.bank_pk, *bank_ai.key, \
                     InvalidBankAccount) (state/marginfi_account.rs:195-199) can then only be \
                     satisfied by an account AccountLoader::<Bank> must reject",
                    account,
                    i
                );

                if have_prev {
                    let mut kind = "out of order";
                    if prev_bank == bank {
                        kind = "DUPLICATE bank";
                    }
                    scout_check!(
                        "P-0036",
                        "active-balances-strictly-descending-by-bank",
                        prev_bank > bank,
                        "P-0036: marginfi_account {} active balances are not strictly descending \
                         by bank_pk: slot {} = {} then slot {} = {} ({}). sort_balances() \
                         (state/marginfi_account.rs:877-880) is the last structural step of every \
                         handler that touches this array and in withdraw.rs:188 / borrow.rs:189 it \
                         runs IMMEDIATELY BEFORE RiskEngine::check_account_init_health(.., \
                         ctx.remaining_accounts, ..), so this is the exact order the positional \
                         bank/oracle pairing at state/marginfi_account.rs:189-218 just walked",
                        account,
                        prev_index,
                        Pubkey::new_from_array(prev_bank),
                        i,
                        Pubkey::new_from_array(bank),
                        kind
                    );
                }
                have_prev = true;
                prev_bank = bank;
                prev_index = i;
            } else {
                scout_check!(
                    "P-0036",
                    "closed-balance-is-fully-zeroed",
                    bank == zero_pubkey
                        && asset_shares == zero_shares
                        && liability_shares == zero_shares,
                    "P-0036: marginfi_account {} slot {} is INACTIVE but not empty: bank_pk {}, \
                     asset_shares {}, liability_shares {}. Balance::close \
                     (state/marginfi_account.rs:908-917) is the only path that clears `active` and \
                     it assigns Balance::empty_deactivated() \
                     (type-crate/src/types/user_account.rs:202-214), an all-zero Balance. Residue \
                     here is a position balances.iter().filter(is_active) \
                     (state/marginfi_account.rs:189-193) skips, i.e. collateral or DEBT that \
                     health, liquidation and bankruptcy all iterate straight past",
                    account,
                    i,
                    Pubkey::new_from_array(bank),
                    fixed::types::I80F48::from_le_bytes(asset_shares),
                    fixed::types::I80F48::from_le_bytes(liability_shares)
                );
                prev_index = i;
            }
            prev_active = active;
        }
    }
    scout_run_property!("P-0036", invariant_p_0036(fixture));
    // SCOUT:INVARIANT:P-0036:END
    // SCOUT:INVARIANT:P-0020:BEGIN
    // P-0020 -- liquidation liveness: any account with negative maintenance health admits at least one feasible (asset, liability) liquidation pair.
    fn invariant_p_0020(f: &mut MarginfiFixture) {
        const P20_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
        const P20_ACCOUNT_LEN: usize = 8 + 2304;
        const P20_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
        const P20_BANK_LEN: usize = 8 + 1856;
        const P20_GROUP_DISCRIMINATOR: [u8; 8] = [182, 23, 173, 240, 151, 206, 182, 67];
        const P20_GROUP_LEN: usize = 8 + 1056;
        const P20_ACCOUNT_GROUP_OFFSET: usize = 8;
        const P20_ACCOUNT_FLAGS_OFFSET: usize = 8 + 32 + 32 + 1728;
        const P20_ACCOUNT_BLOCKING_FLAGS: u64 = (1 << 0) | (1 << 1) | (1 << 4);
        const P20_GROUP_PANIC_PAUSE_FLAGS_OFFSET: usize = 8 + 248;
        const P20_PANIC_FLAG_PAUSED: u8 = 1 << 0;
        const P20_BANK_MINT_DECIMALS_OFFSET: usize = 8 + 32;
        const P20_BANK_GROUP_OFFSET: usize = 8 + 33;
        const P20_BANK_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
        const P20_BANK_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
        const P20_BANK_CONFIG_OFFSET: usize = 8 + 288;
        const P20_BANK_ASSET_WEIGHT_MAINT_OFFSET: usize = P20_BANK_CONFIG_OFFSET + 16;
        const P20_BANK_LIABILITY_WEIGHT_MAINT_OFFSET: usize = P20_BANK_CONFIG_OFFSET + 48;
        const P20_BANK_OPERATIONAL_STATE_OFFSET: usize = P20_BANK_CONFIG_OFFSET + 312;
        const P20_BANK_ORACLE_SETUP_OFFSET: usize = P20_BANK_CONFIG_OFFSET + 313;
        const P20_BANK_RISK_TIER_OFFSET: usize = P20_BANK_CONFIG_OFFSET + 488;
        const P20_BANK_ASSET_TAG_OFFSET: usize = P20_BANK_CONFIG_OFFSET + 489;
        const P20_BANK_FIXED_PRICE_OFFSET: usize = P20_BANK_CONFIG_OFFSET + 512;
        const P20_BANK_EMODE_TAG_OFFSET: usize = 8 + 912;
        const P20_BANK_EMODE_FLAGS_OFFSET: usize = 8 + 912 + 16;
        const P20_OPERATIONAL_PAUSED: u8 = 0;
        const P20_OPERATIONAL_KILLED: u8 = 3;
        const P20_ORACLE_SETUP_FIXED: u8 = 8;
        const P20_RISK_TIER_ISOLATED: u8 = 1;
        const P20_ASSET_TAG_DEFAULT: u8 = 0;
        let empty_balance_threshold = fixed::types::I80F48::ONE;
        let final_discount = fixed::types::I80F48::lit("0.95");
        let one_native_unit = fixed::types::I80F48::ONE;

        let forged_accounts = [
            Pubkey::find_program_address(&[b"scout_handle_bankruptcy_account"], &f.program_id).0,
            Pubkey::find_program_address(&[b"scout_hb_zero_debt_account"], &f.program_id).0,
            Pubkey::find_program_address(&[b"scout_drift_wd_mfi_acct"], &f.program_id).0,
        ];

        let registry = f.scout_p20_accounts;
        let registry_len = f.scout_p20_accounts_next.min(SCOUT_SUBJECT_CAP);

        for slot in 0..registry_len {
            let account = registry[slot];
            if account == Pubkey::default() || forged_accounts.contains(&account) {
                continue;
            }
            let data = match f.ctx.account_data(&account) {
                Ok(d) if d.len() == P20_ACCOUNT_LEN && d[..8] == P20_ACCOUNT_DISCRIMINATOR => d,
                _ => continue,
            };
            let flag_bytes: [u8; 8] =
                data[P20_ACCOUNT_FLAGS_OFFSET..P20_ACCOUNT_FLAGS_OFFSET + 8].try_into().unwrap_or_default();
            let account_flags = u64::from_le_bytes(flag_bytes);
            if account_flags & P20_ACCOUNT_BLOCKING_FLAGS != 0 {
                continue;
            }
            let group_bytes: [u8; 32] =
                data[P20_ACCOUNT_GROUP_OFFSET..P20_ACCOUNT_GROUP_OFFSET + 32].try_into().unwrap_or_default();
            let group = Pubkey::new_from_array(group_bytes);
            let group_ok = match f.ctx.account_data(&group) {
                Ok(g) if g.len() == P20_GROUP_LEN && g[..8] == P20_GROUP_DISCRIMINATOR => {
                    g[P20_GROUP_PANIC_PAUSE_FLAGS_OFFSET] & P20_PANIC_FLAG_PAUSED == 0
                }
                _ => false,
            };
            if !group_ok {
                continue;
            }

            let mut usable = true;
            let mut weighted_assets = fixed::types::I80F48::ZERO;
            let mut weighted_liabs = fixed::types::I80F48::ZERO;
            let mut equity_assets = fixed::types::I80F48::ZERO;
            let mut equity_liabs = fixed::types::I80F48::ZERO;
            let mut seizable: Vec<(Pubkey, fixed::types::I80F48, fixed::types::I80F48, fixed::types::I80F48, u8)> =
                Vec::new();
            let mut liabilities: Vec<(Pubkey, fixed::types::I80F48, fixed::types::I80F48, u8)> =
                Vec::new();
            let mut bank_cache: Vec<(Pubkey, &[u8])> = Vec::new();

            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                if data[base] == 0 {
                    continue;
                }
                if data[base + 33] != P20_ASSET_TAG_DEFAULT {
                    usable = false;
                    break;
                }
                let bank_bytes: [u8; 32] =
                    data[base + 1..base + 33].try_into().unwrap_or_default();
                let bank_pk = Pubkey::new_from_array(bank_bytes);
                let mut cache_hit: Option<&[u8]> = None;
                for entry in bank_cache.iter() {
                    if entry.0 == bank_pk && cache_hit.is_none() {
                        cache_hit = Some(entry.1);
                    }
                }
                let bank_data: &[u8] = match cache_hit {
                    Some(cached) => cached,
                    None => match f.ctx.account_data(&bank_pk) {
                        Ok(b) if b.len() == P20_BANK_LEN && b[..8] == P20_BANK_DISCRIMINATOR => {
                            bank_cache.push((bank_pk, b));
                            b
                        }
                        _ => {
                            usable = false;
                            break;
                        }
                    },
                };
                let bank_group: [u8; 32] =
                    bank_data[P20_BANK_GROUP_OFFSET..P20_BANK_GROUP_OFFSET + 32].try_into().unwrap_or_default();
                if Pubkey::new_from_array(bank_group) != group {
                    usable = false;
                    break;
                }
                if bank_data[P20_BANK_ORACLE_SETUP_OFFSET] != P20_ORACLE_SETUP_FIXED
                    || bank_data[P20_BANK_ASSET_TAG_OFFSET] != P20_ASSET_TAG_DEFAULT
                {
                    usable = false;
                    break;
                }
                let emode_tag: [u8; 2] =
                    bank_data[P20_BANK_EMODE_TAG_OFFSET..P20_BANK_EMODE_TAG_OFFSET + 2].try_into().unwrap_or_default();
                let emode_flags: [u8; 8] =
                    bank_data[P20_BANK_EMODE_FLAGS_OFFSET..P20_BANK_EMODE_FLAGS_OFFSET + 8].try_into().unwrap_or_default();
                if u16::from_le_bytes(emode_tag) != 0 || u64::from_le_bytes(emode_flags) != 0 {
                    usable = false;
                    break;
                }
                let decimals = bank_data[P20_BANK_MINT_DECIMALS_OFFSET];
                if decimals > 24 {
                    usable = false;
                    break;
                }
                let scale_base = match 10u128.checked_pow(decimals as u32) {
                    Some(v) => v,
                    None => {
                        usable = false;
                        break;
                    }
                };
                let scale = match fixed::types::I80F48::checked_from_num(scale_base)
                {
                    Some(v) => v,
                    None => {
                        usable = false;
                        break;
                    }
                };
                let price_bytes: [u8; 16] = bank_data
                    [P20_BANK_FIXED_PRICE_OFFSET..P20_BANK_FIXED_PRICE_OFFSET + 16]
                    .try_into()
                    .unwrap_or_default();
                let price = fixed::types::I80F48::from_le_bytes(price_bytes);
                let asset_share_value_bytes: [u8; 16] = bank_data
                    [P20_BANK_ASSET_SHARE_VALUE_OFFSET..P20_BANK_ASSET_SHARE_VALUE_OFFSET + 16]
                    .try_into()
                    .unwrap_or_default();
                let asset_share_value =
                    fixed::types::I80F48::from_le_bytes(asset_share_value_bytes);
                let liability_share_value_bytes: [u8; 16] = bank_data
                    [P20_BANK_LIABILITY_SHARE_VALUE_OFFSET
                        ..P20_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16]
                    .try_into()
                    .unwrap_or_default();
                let liability_share_value =
                    fixed::types::I80F48::from_le_bytes(liability_share_value_bytes);
                let asset_weight_maint_bytes: [u8; 16] = bank_data
                    [P20_BANK_ASSET_WEIGHT_MAINT_OFFSET..P20_BANK_ASSET_WEIGHT_MAINT_OFFSET + 16]
                    .try_into()
                    .unwrap_or_default();
                let asset_weight_maint =
                    fixed::types::I80F48::from_le_bytes(asset_weight_maint_bytes);
                let liability_weight_maint_bytes: [u8; 16] = bank_data
                    [P20_BANK_LIABILITY_WEIGHT_MAINT_OFFSET
                        ..P20_BANK_LIABILITY_WEIGHT_MAINT_OFFSET + 16]
                    .try_into()
                    .unwrap_or_default();
                let liability_weight_maint =
                    fixed::types::I80F48::from_le_bytes(liability_weight_maint_bytes);
                let operational_state = bank_data[P20_BANK_OPERATIONAL_STATE_OFFSET];
                let effective_asset_weight = if bank_data[P20_BANK_RISK_TIER_OFFSET]
                    == P20_RISK_TIER_ISOLATED
                {
                    fixed::types::I80F48::ZERO
                } else {
                    asset_weight_maint
                };

                let share_bytes: [u8; 16] =
                    data[base + 40..base + 56].try_into().unwrap_or_default();
                let asset_shares = fixed::types::I80F48::from_le_bytes(share_bytes);
                let liability_share_bytes: [u8; 16] =
                    data[base + 56..base + 72].try_into().unwrap_or_default();
                let liability_shares = fixed::types::I80F48::from_le_bytes(liability_share_bytes);

                let asset_amount = match asset_shares.checked_mul(asset_share_value) {
                    Some(v) => v,
                    None => {
                        usable = false;
                        break;
                    }
                };
                let liability_amount = match liability_shares.checked_mul(liability_share_value) {
                    Some(v) => v,
                    None => {
                        usable = false;
                        break;
                    }
                };

                if liability_shares >= empty_balance_threshold {
                    let weighted = match liability_amount.checked_mul(liability_weight_maint) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let priced = match weighted.checked_mul(price) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let value = match priced.checked_div(scale) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    weighted_liabs = match weighted_liabs.checked_add(value) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let equity_priced = match liability_amount.checked_mul(price) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let equity_scaled = match equity_priced.checked_div(scale) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let equity_value = match equity_liabs.checked_add(equity_scaled) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    equity_liabs = equity_value;
                    if asset_shares < empty_balance_threshold {
                        liabilities.push((
                            bank_pk,
                            liability_weight_maint,
                            price,
                            operational_state,
                        ));
                    }
                } else if asset_shares >= empty_balance_threshold {
                    let weighted = match asset_amount.checked_mul(effective_asset_weight) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let priced = match weighted.checked_mul(price) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let value = match priced.checked_div(scale) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    weighted_assets = match weighted_assets.checked_add(value) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let equity_weight =
                        if bank_data[P20_BANK_RISK_TIER_OFFSET] == P20_RISK_TIER_ISOLATED {
                            fixed::types::I80F48::ZERO
                        } else {
                            fixed::types::I80F48::ONE
                        };
                    let equity_weighted = match asset_amount.checked_mul(equity_weight) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let equity_priced = match equity_weighted.checked_mul(price) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let equity_scaled = match equity_priced.checked_div(scale) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    let equity_value = match equity_assets.checked_add(equity_scaled) {
                        Some(v) => v,
                        None => {
                            usable = false;
                            break;
                        }
                    };
                    equity_assets = equity_value;
                }

                if asset_amount >= one_native_unit {
                    seizable.push((
                        bank_pk,
                        asset_amount,
                        effective_asset_weight,
                        price,
                        operational_state,
                    ));
                }
            }

            if !usable {
                continue;
            }
            let health = match weighted_assets.checked_sub(weighted_liabs) {
                Some(v) => v,
                None => continue,
            };
            if health >= fixed::types::I80F48::ZERO {
                continue;
            }
            if seizable.is_empty() {
                continue;
            }
            if equity_assets < equity_liabs
                && equity_assets < fixed::types::I80F48::lit("0.1")
                && equity_liabs > fixed::types::I80F48::lit("0.0001")
            {
                continue;
            }

            let zero = fixed::types::I80F48::ZERO;
            let mut feasible = false;
            for (asset_bank, _amount, asset_weight, asset_price, asset_state) in &seizable {
                if *asset_state == P20_OPERATIONAL_PAUSED
                    || *asset_state == P20_OPERATIONAL_KILLED
                    || *asset_price <= zero
                {
                    continue;
                }
                for (liab_bank, liability_weight, liab_price, liab_state) in &liabilities {
                    if liab_bank == asset_bank
                        || *liab_state == P20_OPERATIONAL_PAUSED
                        || *liab_state == P20_OPERATIONAL_KILLED
                        || *liab_price <= zero
                    {
                        continue;
                    }
                    let improvement = match final_discount.checked_mul(*liability_weight) {
                        Some(v) => v,
                        None => continue,
                    };
                    if improvement > *asset_weight {
                        feasible = true;
                        break;
                    }
                }
                if feasible {
                    break;
                }
            }

            scout_check!(
                "P-0020",
                "liquidatable-account-admits-a-feasible-liquidation",
                feasible,
                "P-0020: MarginfiAccount {} (group {}) has maintenance health {} \
                 (weighted assets {} - weighted liabilities {}; equity assets {} vs equity \
                 liabilities {}, so `check_account_bankrupt` does NOT admit handle_bankruptcy \
                 either) so it is liquidatable, but NO \
                 (asset, liability) pair it holds admits a liquidation that could succeed: \
                 {} seizable collateral balance(s) {:?} and {} liability balance(s) {:?}, where \
                 a seizable tuple is (bank, seizable amount, effective maintenance asset weight, \
                 fixed price, operational_state), a liability tuple is (bank, maintenance \
                 liability weight, fixed price, operational_state), and a \
                 pair is feasible only when the banks differ, both are live (state 1 Operational \
                 or 2 ReduceOnly), both prices are > 0, and 0.95 * liability_weight_maint > \
                 asset_weight_maint -- otherwise liquidate.rs:430-434 \
                 (`WorseHealthPostLiquidation`, state/marginfi_account.rs:797) rejects EVERY \
                 asset_amount and the bad debt can never be cleared. Account flags {:#x}",
                account,
                group,
                health,
                weighted_assets,
                weighted_liabs,
                equity_assets,
                equity_liabs,
                seizable.len(),
                &seizable,
                liabilities.len(),
                &liabilities,
                account_flags
            );
        }
    }
    scout_run_property!("P-0020", invariant_p_0020(fixture));
    // SCOUT:INVARIANT:P-0020:END
    // SCOUT:INVARIANT:P-0005:BEGIN
    // P-0005 -- bank.lending_position_count / borrowing_position_count must each be >= the live lending/borrowing balances this check can see.
    fn invariant_p_0005(f: &mut MarginfiFixture) {
        const P5_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
        const P5_ACCOUNT_LEN: usize = 8 + 2304;
        const P5_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
        const P5_BANK_LEN: usize = 8 + 1856;
        const P5_BANK_LENDING_COUNT_OFFSET: usize = 8 + 1528;
        const P5_BANK_BORROWING_COUNT_OFFSET: usize = 8 + 1532;
        const P5_ASSET_TAG_DEFAULT: u8 = 0;
        let live_threshold = fixed::types::I80F48::lit("0.0002");

        let forged_accounts = [
            Pubkey::find_program_address(&[b"scout_handle_bankruptcy_account"], &f.program_id).0,
            Pubkey::find_program_address(&[b"scout_hb_zero_debt_account"], &f.program_id).0,
            Pubkey::find_program_address(&[b"scout_drift_wd_mfi_acct"], &f.program_id).0,
            f.kamino_withdraw_marginfi_account,
        ];
        let forged_banks = scout_forged_bank_pdas(f.program_id);
        let harness_forged_accounts = f.scout_p22_forged_accounts.clone();

        let dirty = f.ctx.dirty_tracker.dirty_accounts().clone();
        let mut slots: Vec<(Pubkey, i64, i64)> = Vec::new();
        let mut bank_keys: Vec<Pubkey> = Vec::new();
        for key in &dirty {
            if forged_accounts.contains(key) || harness_forged_accounts.contains(key) {
                continue;
            }
            let data = match f.ctx.account_data(key) {
                Ok(d) if d.len() == P5_ACCOUNT_LEN && d[..8] == P5_ACCOUNT_DISCRIMINATOR => d,
                _ => continue,
            };
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                if data[base] == 0 {
                    continue;
                }
                if data[base + 33] != P5_ASSET_TAG_DEFAULT {
                    continue;
                }
                let bank_bytes: [u8; 32] = match data[base + 1..base + 33].try_into() {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                let bank_pk = Pubkey::new_from_array(bank_bytes);
                if forged_banks.contains(&bank_pk) {
                    continue;
                }
                let asset_bytes: [u8; 16] = match data[base + 40..base + 56].try_into() {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                let liability_bytes: [u8; 16] = match data[base + 56..base + 72].try_into() {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                let lending = match fixed::types::I80F48::from_le_bytes(asset_bytes)
                    > live_threshold
                {
                    true => 1i64,
                    false => 0i64,
                };
                let borrowing = match fixed::types::I80F48::from_le_bytes(liability_bytes)
                    > live_threshold
                {
                    true => 1i64,
                    false => 0i64,
                };
                if lending == 0 && borrowing == 0 {
                    continue;
                }
                slots.push((bank_pk, lending, borrowing));
                if !bank_keys.contains(&bank_pk) {
                    bank_keys.push(bank_pk);
                }
            }
        }

        for bank_pk in bank_keys {
            let mut lending_seen = 0i64;
            let mut borrowing_seen = 0i64;
            for slot in &slots {
                if slot.0 == bank_pk {
                    lending_seen = lending_seen + slot.1;
                    borrowing_seen = borrowing_seen + slot.2;
                }
            }
            let bank_data = match f.ctx.account_data(&bank_pk) {
                Ok(d) if d.len() == P5_BANK_LEN && d[..8] == P5_BANK_DISCRIMINATOR => d,
                _ => continue,
            };
            let lending_bytes: [u8; 4] = match bank_data
                [P5_BANK_LENDING_COUNT_OFFSET..P5_BANK_LENDING_COUNT_OFFSET + 4]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let borrowing_bytes: [u8; 4] = match bank_data
                [P5_BANK_BORROWING_COUNT_OFFSET..P5_BANK_BORROWING_COUNT_OFFSET + 4]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let lending_count = i32::from_le_bytes(lending_bytes) as i64;
            let borrowing_count = i32::from_le_bytes(borrowing_bytes) as i64;
            scout_check!(
                "P-0005",
                "bank-lending-position-count-covers-live-lending-balances",
                lending_count >= lending_seen,
                "P-0005: bank {} lending_position_count={} but {} live lending balance(s) \
                 (asset_shares > 2e-4, ASSET_TAG_DEFAULT) are visible right now across the \
                 {} account(s) this iteration touched -- a bank whose count under-reports its \
                 open positions can be closed by lending_pool_close_bank (close_bank.rs:28) \
                 while depositors still hold claims against it",
                bank_pk,
                lending_count,
                lending_seen,
                dirty.len()
            );
            scout_check!(
                "P-0005",
                "bank-borrowing-position-count-covers-live-borrowing-balances",
                borrowing_count >= borrowing_seen,
                "P-0005: bank {} borrowing_position_count={} but {} live borrowing balance(s) \
                 (liability_shares > 2e-4, ASSET_TAG_DEFAULT) are visible right now across the \
                 {} account(s) this iteration touched -- a bank whose count under-reports its \
                 open positions can be closed by lending_pool_close_bank (close_bank.rs:28) \
                 while borrowers still owe it",
                bank_pk,
                borrowing_count,
                borrowing_seen,
                dirty.len()
            );
        }
    }
    scout_run_property!("P-0005", invariant_p_0005(fixture));
    // SCOUT:INVARIANT:P-0005:END
    // SCOUT:INVARIANT:P-0006:BEGIN
    // P-0006 -- group.banks must be >= the number of live banks under that group this check can prove exist.
    fn invariant_p_0006(f: &mut MarginfiFixture) {
        const P6_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
        const P6_BANK_LEN: usize = 8 + 1856;
        const P6_GROUP_DISCRIMINATOR: [u8; 8] = [182, 23, 173, 240, 151, 206, 182, 67];
        const P6_GROUP_LEN: usize = 8 + 1056;
        const P6_BANK_GROUP_OFFSET: usize = 8 + 33;
        const P6_GROUP_BANKS_OFFSET: usize = 8 + 112;

        if f.scout_p06_bank_next == 0 {
            return;
        }

        let registry = f.scout_p06_banks;
        let mut counted: Vec<Pubkey> = Vec::new();
        let mut bank_groups: Vec<Pubkey> = Vec::new();
        let mut group_keys: Vec<Pubkey> = Vec::new();
        for bank_pk in registry {
            if bank_pk == Pubkey::default() || counted.contains(&bank_pk) {
                continue;
            }
            counted.push(bank_pk);
            let bank_data = match f.ctx.account_data(&bank_pk) {
                Ok(d) if d.len() == P6_BANK_LEN && d[..8] == P6_BANK_DISCRIMINATOR => d,
                _ => continue,
            };
            let group_bytes: [u8; 32] = match bank_data
                [P6_BANK_GROUP_OFFSET..P6_BANK_GROUP_OFFSET + 32]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let group_pk = Pubkey::new_from_array(group_bytes);
            bank_groups.push(group_pk);
            if !group_keys.contains(&group_pk) {
                group_keys.push(group_pk);
            }
        }

        for group_pk in group_keys {
            let mut proven_live = 0i64;
            for entry in &bank_groups {
                if *entry == group_pk {
                    proven_live = proven_live + 1;
                }
            }
            let group_data = match f.ctx.account_data(&group_pk) {
                Ok(d) if d.len() == P6_GROUP_LEN && d[..8] == P6_GROUP_DISCRIMINATOR => d,
                _ => continue,
            };
            let banks_bytes: [u8; 2] = match group_data
                [P6_GROUP_BANKS_OFFSET..P6_GROUP_BANKS_OFFSET + 2]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let banks = u16::from_le_bytes(banks_bytes) as i64;
            scout_check!(
                "P-0006",
                "group-bank-count-covers-banks-created-under-it",
                banks >= proven_live,
                "P-0006: group {} reports banks={} but {} Bank account(s) created under it by a \
                 real add-bank instruction on this lineage are still live and still name it as \
                 their group (of {} registered, {} distinct) -- group.banks is only ever moved by \
                 MarginfiGroup::add_bank (+1) and lending_pool_close_bank (-1), so a count below \
                 the banks that demonstrably exist means one of those two legs was skipped or ran \
                 twice",
                group_pk,
                banks,
                proven_live,
                f.scout_p06_bank_next,
                counted.len()
            );
        }
    }
    scout_run_property!("P-0006", invariant_p_0006(fixture));
    // SCOUT:INVARIANT:P-0006:END
    // SCOUT:INVARIANT:P-0039:BEGIN
    // P-0039 -- registry biconditional parity: account_flags/migrated_to/migrated_from/liquidation_record must agree with each other and with LiquidationRecord's own fields.
    fn invariant_p_0039(f: &mut MarginfiFixture) {
        const P39_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
        const P39_ACCOUNT_LEN: usize = 8 + 2304;
        const P39_RECORD_DISCRIMINATOR: [u8; 8] = [95, 116, 23, 132, 89, 210, 245, 162];
        const P39_RECORD_LEN: usize = 8 + 512;

        const P39_HEALTH_CACHE_LEN: usize = 304;
        const P39_MIGRATED_FROM_OFFSET: usize =
            MARGINFI_ACCOUNT_FLAGS_OFFSET + 8 + 32 + P39_HEALTH_CACHE_LEN;
        const P39_MIGRATED_TO_OFFSET: usize = P39_MIGRATED_FROM_OFFSET + 32;
        const P39_ACCOUNT_RECORD_PTR_OFFSET: usize =
            P39_MIGRATED_TO_OFFSET + 32 + 8 + 2 + 2 + 1 + 3;

        const P39_RECORD_SELF_KEY_OFFSET: usize = 8;
        const P39_RECORD_BACKLINK_OFFSET: usize = 8 + 32;
        const P39_RECORD_RECEIVER_OFFSET: usize = SCOUT_LIQUIDATION_RECORD_RECEIVER_OFFSET;

        let registry = f.scout_p39_subjects;
        let registry_len = f.scout_p39_subjects_next.min(SCOUT_SUBJECT_CAP);

        let harness_flagged = f.scout_p17_harness_flagged.clone();
        let mut harness_records: Vec<Pubkey> = Vec::new();
        for account in &harness_flagged {
            harness_records.push(
                Pubkey::find_program_address(
                    &[LIQUIDATION_RECORD_SEED, account.as_ref()],
                    &f.program_id,
                )
                .0,
            );
        }

        for slot in 0..registry_len {
            let record = registry[slot];
            if record == Pubkey::default() || harness_records.contains(&record) {
                continue;
            }
            let data = match f.ctx.account_data(&record) {
                Ok(d) if d.len() == P39_RECORD_LEN && d[..8] == P39_RECORD_DISCRIMINATOR => d,
                _ => continue,
            };
            let self_key = match data
                [P39_RECORD_SELF_KEY_OFFSET..P39_RECORD_SELF_KEY_OFFSET + 32]
                .try_into()
            {
                Ok(bytes) => Pubkey::new_from_array(bytes),
                Err(_) => continue,
            };
            let backlink = match data
                [P39_RECORD_BACKLINK_OFFSET..P39_RECORD_BACKLINK_OFFSET + 32]
                .try_into()
            {
                Ok(bytes) => Pubkey::new_from_array(bytes),
                Err(_) => continue,
            };
            let receiver = match data
                [P39_RECORD_RECEIVER_OFFSET..P39_RECORD_RECEIVER_OFFSET + 32]
                .try_into()
            {
                Ok(bytes) => Pubkey::new_from_array(bytes),
                Err(_) => continue,
            };

            scout_check!(
                "P-0039",
                "liquidation-record-receiver-is-cleared-at-a-transaction-boundary",
                receiver == Pubkey::default(),
                "P-0039: liquidation_record {} still names liquidation_receiver {} at an action \
                 boundary (backlink marginfi_account {}). The receiver is written only by \
                 start_liquidation (liquidate_start.rs:36) / start_deleverage (:64) and cleared \
                 only by end_receivership (liquidate_end.rs:152), and both start_* handlers run \
                 validate_instructions (liquidate_start.rs:115-185) which pins the matching end_* \
                 as the LAST instruction of the same atomic transaction -- so the record half of \
                 the bracket leaked. This is not inert: EndLiquidation requires the named receiver \
                 to SIGN (has_one = liquidation_receiver, liquidate_end.rs:192-200) and \
                 EndDeleverage requires it to equal the risk_admin (:254-261), so a stale name \
                 here is a standing claim on this account's liquidation exit.",
                record,
                receiver,
                backlink
            );

            scout_check!(
                "P-0039",
                "liquidation-record-self-key-matches-its-own-address",
                self_key == record,
                "P-0039: liquidation_record at {} carries key={} in its own `key` field. That \
                 field is written exactly once, at init_liquid_record.rs:15, to \
                 ctx.accounts.liquidation_record.key(), and never again -- so it can only differ \
                 if the record was written by something other than initialize_liquidation_record.",
                record,
                self_key
            );

            let pointer_agrees = if backlink == Pubkey::default() {
                None
            } else {
                match f.ctx.account_data(&backlink) {
                    Ok(a)
                        if a.len() == P39_ACCOUNT_LEN
                            && a[..8] == P39_ACCOUNT_DISCRIMINATOR =>
                    {
                        match a[P39_ACCOUNT_RECORD_PTR_OFFSET..P39_ACCOUNT_RECORD_PTR_OFFSET + 32]
                            .try_into()
                        {
                            Ok(bytes) => Some(Pubkey::new_from_array(bytes) == record),
                            Err(_) => None,
                        }
                    }
                    _ => None,
                }
            };
            scout_check!(
                "P-0039",
                "liquidation-record-backlink-account-points-back-at-this-record",
                pointer_agrees != Some(false),
                "P-0039: liquidation_record {} claims marginfi_account {}, but that account's own \
                 `liquidation_record` field does NOT point back at it. The pair is bound after \
                 init only by `has_one = liquidation_record` on the ACCOUNT (liquidate_start.rs:192 \
                 and :231, liquidate_end.rs:181 and :243) -- the forward pointer alone. No handler \
                 re-derives the [\"liq_record\", marginfi_account] PDA seeds and none checks \
                 record.marginfi_account, so a broken backlink means start_receivership can write \
                 one account's health snapshot (liquidate_start.rs:106-109) into another's record \
                 and end_receivership will run the health-must-not-get-worse and \
                 seized <= repaid * max_fee checks (liquidate_end.rs:138-145, :56-61) against that \
                 foreign snapshot. If the account was CLOSED and its PDA address re-occupied by a \
                 new account, this also means the new account can never obtain a record \
                 (InitLiquidationRecord is `init`, not `init_if_needed`) and therefore can never \
                 enter receivership -- i.e. can never be liquidated.",
                record,
                backlink
            );
        }

        for slot in 0..registry_len {
            let account = registry[slot];
            if account == Pubkey::default() {
                continue;
            }
            let data = match f.ctx.account_data(&account) {
                Ok(d) if d.len() == P39_ACCOUNT_LEN && d[..8] == P39_ACCOUNT_DISCRIMINATOR => d,
                _ => continue,
            };
            let flags = match data
                [MARGINFI_ACCOUNT_FLAGS_OFFSET..MARGINFI_ACCOUNT_FLAGS_OFFSET + 8]
                .try_into()
            {
                Ok(bytes) => u64::from_le_bytes(bytes),
                Err(_) => continue,
            };
            let record_ptr = match data
                [P39_ACCOUNT_RECORD_PTR_OFFSET..P39_ACCOUNT_RECORD_PTR_OFFSET + 32]
                .try_into()
            {
                Ok(bytes) => Pubkey::new_from_array(bytes),
                Err(_) => continue,
            };
            let migrated_to = match data
                [P39_MIGRATED_TO_OFFSET..P39_MIGRATED_TO_OFFSET + 32]
                .try_into()
            {
                Ok(bytes) => Pubkey::new_from_array(bytes),
                Err(_) => continue,
            };
            let migrated_from = match data
                [P39_MIGRATED_FROM_OFFSET..P39_MIGRATED_FROM_OFFSET + 32]
                .try_into()
            {
                Ok(bytes) => Pubkey::new_from_array(bytes),
                Err(_) => continue,
            };

            let mut active_balances: u32 = 0;
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                if data[base] != 0 {
                    active_balances = active_balances.saturating_add(1);
                }
            }

            let record_claims_it = if record_ptr == Pubkey::default() {
                None
            } else {
                match f.ctx.account_data(&record_ptr) {
                    Ok(r)
                        if r.len() == P39_RECORD_LEN && r[..8] == P39_RECORD_DISCRIMINATOR =>
                    {
                        match r[P39_RECORD_BACKLINK_OFFSET..P39_RECORD_BACKLINK_OFFSET + 32]
                            .try_into()
                        {
                            Ok(bytes) => Some(Pubkey::new_from_array(bytes) == account),
                            Err(_) => Some(false),
                        }
                    }
                    _ => Some(false),
                }
            };
            scout_check!(
                "P-0039",
                "marginfi-account-liquidation_record-pointer-resolves-to-a-record-that-claims-it",
                record_claims_it != Some(false),
                "P-0039: marginfi_account {} has liquidation_record = {}, but that address is not \
                 a live LiquidationRecord whose `marginfi_account` field names {} back. That \
                 pointer is written exactly once, at init_liquid_record.rs:23, alongside \
                 record.marginfi_account at :17, and is the ONLY thing binding the pair in \
                 StartLiquidation/StartDeleverage/EndLiquidation/EndDeleverage (`has_one = \
                 liquidation_record`) -- so a pointer that does not resolve back means the \
                 liquidation bracket's snapshot/settlement pair (liquidate_start.rs:106-109, \
                 liquidate_end.rs:118-152) is operating on the wrong record. flags=0x{:016x}.",
                account,
                record_ptr,
                account,
                flags
            );

            let migrated = migrated_to != Pubkey::default();
            scout_check!(
                "P-0039",
                "migrated_to-set-implies-ACCOUNT_DISABLED-set",
                !migrated || flags & SCOUT_ACCOUNT_DISABLED != 0,
                "P-0039: marginfi_account {} has migrated_to = {} but does NOT carry \
                 ACCOUNT_DISABLED (flags=0x{:016x}). transfer_to_new_account writes the pointer \
                 and sets the flag three lines apart (transfer_account.rs:65 then :68, and :200 \
                 then :203 for the _pda variant), and ACCOUNT_DISABLED has NO clear site in the \
                 program (the only three `unset_flag` calls are flashloan.rs:147 and \
                 liquidate_end.rs:93/:151, none of them for this bit). A migrated shell without \
                 the flag is a live account holding a position that has already been copied to its \
                 successor: deposit.rs:63, borrow.rs:66, repay.rs:61, withdraw.rs:65 and every \
                 other guard on this account gate on exactly that bit.",
                account,
                migrated_to,
                flags
            );
            scout_check!(
                "P-0039",
                "migrated_to-set-implies-no-active-balances",
                !migrated || active_balances == 0,
                "P-0039: marginfi_account {} has migrated_to = {} and still has {} active \
                 balance slot(s) (flags=0x{:016x}). transfer_to_new_account COPIES \
                 old.lending_account into the new account and then zeroes the old one \
                 (transfer_account.rs:60 then :67), so these positions already exist in {} -- the \
                 protocol is now carrying the same position twice. The only wall stopping a \
                 migrated shell from regaining a balance is ACCOUNT_DISABLED, and two instructions \
                 that write balances do not check it: lending_account_liquidate (liquidate.rs:498 \
                 and :519 check only ACCOUNT_IN_RECEIVERSHIP) and lending_pool_handle_bankruptcy \
                 (handle_bankruptcy.rs:235, same).",
                account,
                migrated_to,
                active_balances,
                flags,
                migrated_to
            );

            let source_points_here = if migrated_from == Pubkey::default() {
                None
            } else {
                match f.ctx.account_data(&migrated_from) {
                    Ok(s)
                        if s.len() == P39_ACCOUNT_LEN
                            && s[..8] == P39_ACCOUNT_DISCRIMINATOR =>
                    {
                        match s[P39_MIGRATED_TO_OFFSET..P39_MIGRATED_TO_OFFSET + 32].try_into() {
                            Ok(bytes) => Some(Pubkey::new_from_array(bytes) == account),
                            Err(_) => None,
                        }
                    }
                    _ => None,
                }
            };
            scout_check!(
                "P-0039",
                "migrated_from-source-account-points-forward-at-this-account",
                source_points_here != Some(false),
                "P-0039: marginfi_account {} has migrated_from = {}, but that source account's \
                 own migrated_to does NOT name {} back. Both halves of the edge are written in one \
                 handler (transfer_account.rs:63 and :65, :195 and :200) and re-validated by \
                 nothing afterwards -- AccountAlreadyMigrated (transfer_account.rs:46-51) only \
                 guards that the source has not migrated twice, never that the two pointers still \
                 describe the same edge. A source whose migrated_to names a THIRD account means \
                 one migration silently overwrote another, and the balances copied out of it are \
                 now attributed to the wrong successor.",
                account,
                migrated_from,
                account
            );
        }
    }
    scout_run_property!("P-0039", invariant_p_0039(fixture));
    fn invariant_p_0039_liq_record(f: &mut MarginfiFixture) {
        const LIQ_RECORD_DISCRIMINATOR: [u8; 8] = [95, 116, 23, 132, 89, 210, 245, 162];
        if f.scout_known_next > SCOUT_KNOWN_CAP {
            return;
        }
        let program_id = f.program_id;
        let accounts = f.scout_known_accounts;
        for acct in accounts {
            if acct == Pubkey::default() {
                continue;
            }
            let record_pda =
                Pubkey::find_program_address(&[b"liq_record".as_ref(), acct.as_ref()], &program_id)
                    .0;
            let data = match f.ctx.read_account(&record_pda) {
                Ok(a) => a.data,
                Err(_) => continue,
            };
            if data.len() < 136 || data[0..8] != LIQ_RECORD_DISCRIMINATOR {
                continue;
            }
            let record_key: [u8; 32] = data[8..40].try_into().unwrap_or_default();
            let record_acct: [u8; 32] = data[40..72].try_into().unwrap_or_default();
            let record_receiver: [u8; 32] = data[104..136].try_into().unwrap_or_default();

            scout_check!(
                "P-0039",
                "liq-record-self-key-matches-its-pda",
                record_key == record_pda.to_bytes(),
                "P-0039: LiquidationRecord at {} stores key {} != its own PDA address -- the \
                 record's self-key (init_liquid_record.rs:12) has been aliased onto the wrong \
                 address",
                record_pda,
                Pubkey::new_from_array(record_key)
            );
            scout_check!(
                "P-0039",
                "liq-record-back-pointer-matches-account",
                record_acct == acct.to_bytes(),
                "P-0039: LiquidationRecord at PDA of {} points back at marginfi_account {} -- the \
                 back-edge (init_liquid_record.rs:17) no longer names the account the record is a \
                 PDA of; the record is orphaned or re-pointed",
                acct,
                Pubkey::new_from_array(record_acct)
            );
            scout_check!(
                "P-0039",
                "liq-record-receiver-cleared-at-boundary",
                record_receiver == [0u8; 32],
                "P-0039: LiquidationRecord for {} has liquidation_receiver {} set at a \
                 transaction boundary -- it must be pubkey-default outside an active liquidation \
                 (type-crate/.../liquidation_record.rs:38); a receivership bracket entered but \
                 never ran end_liquidation/end_deleverage to clear it",
                acct,
                Pubkey::new_from_array(record_receiver)
            );
        }
    }
    scout_run_property!("P-0039", invariant_p_0039_liq_record(fixture));
    // SCOUT:INVARIANT:P-0039:END
    // SCOUT:INVARIANT:P-0035:BEGIN
    // P-0035 -- no exposure growth while unhealthy: an account already below maintenance health must not end a transaction with larger liability value on any bank.
    fn invariant_p_0035(f: &mut MarginfiFixture) {
        let zero = fixed::types::I80F48::from_num(0);
        let ten = fixed::types::I80F48::from_num(10);
        let one_share = fixed::types::I80F48::from_num(1);

        let clock_key = Pubkey::new_from_array(SCOUT_P35_CLOCK_SYSVAR_BYTES);
        let clock_data = f.ctx.read_account(&clock_key).unwrap_or_default().data;
        if clock_data.len() < SCOUT_P35_CLOCK_UNIX_TIMESTAMP_OFFSET + 8 {
            return;
        }
        let clock_now = i64::from_le_bytes(
            (&clock_data[SCOUT_P35_CLOCK_UNIX_TIMESTAMP_OFFSET
                ..SCOUT_P35_CLOCK_UNIX_TIMESTAMP_OFFSET + 8])
                .try_into()
                .unwrap_or_default(),
        );

        let subjects = [
            f.marginfi_account,
            f.borrow_marginfi_account,
            f.withdraw_marginfi_account,
            f.withdraw_emissions_marginfi_account,
            f.fee_borrow_marginfi_account,
            f.kamino_withdraw_marginfi_account,
            f.pulse_health_healthy_account,
            f.pulse_health_risk_rejected_account,
        ];

        for subject_index in 0..SCOUT_P35_SUBJECT_COUNT {
            let account = subjects[subject_index];
            if account == Pubkey::default() {
                continue;
            }
            let data = f.ctx.read_account(&account).unwrap_or_default().data;
            if data.len() != SCOUT_P35_ACCOUNT_LEN {
                continue;
            }
            if data[..8] != SCOUT_P35_ACCOUNT_DISCRIMINATOR {
                continue;
            }
            let account_flags = u64::from_le_bytes(
                (&data[SCOUT_P35_ACCOUNT_FLAGS_OFFSET..SCOUT_P35_ACCOUNT_FLAGS_OFFSET + 8])
                    .try_into()
                    .unwrap_or_default(),
            );
            if account_flags & SCOUT_P35_BRACKET_FLAGS != 0 {
                continue;
            }
            let cached_timestamp = i64::from_le_bytes(
                (&data[SCOUT_P35_HC_TIMESTAMP_OFFSET..SCOUT_P35_HC_TIMESTAMP_OFFSET + 8])
                    .try_into()
                    .unwrap_or_default(),
            );
            if cached_timestamp != clock_now {
                continue;
            }
            let cached_assets_maint = fixed::types::I80F48::from_le_bytes(
                (&data[SCOUT_P35_HC_ASSET_MAINT_OFFSET..SCOUT_P35_HC_ASSET_MAINT_OFFSET + 16])
                    .try_into()
                    .unwrap_or_default(),
            );
            let cached_liabs_maint = fixed::types::I80F48::from_le_bytes(
                (&data[SCOUT_P35_HC_LIAB_MAINT_OFFSET..SCOUT_P35_HC_LIAB_MAINT_OFFSET + 16])
                    .try_into()
                    .unwrap_or_default(),
            );
            if cached_liabs_maint <= cached_assets_maint {
                continue;
            }

            let mut usable = true;
            let mut liabs_now = zero;
            let mut active_index = 0;
            for slot in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + slot * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    usable = false;
                    break;
                }
                if data[base] == 0 {
                    continue;
                }
                let price_offset = SCOUT_P35_HC_PRICES_OFFSET + active_index * 8;
                active_index = active_index + 1;
                if data.len() < price_offset + 8 {
                    usable = false;
                    break;
                }
                let bank_key = Pubkey::new_from_array(
                    (&data[base + 1..base + 33]).try_into().unwrap_or_default(),
                );
                let bank_data = f.ctx.read_account(&bank_key).unwrap_or_default().data;
                if bank_data.len() != SCOUT_P35_BANK_LEN {
                    usable = false;
                    break;
                }
                if bank_data[..8] != SCOUT_P35_BANK_DISCRIMINATOR {
                    usable = false;
                    break;
                }
                if bank_data[SCOUT_P35_BANK_ORACLE_SETUP_OFFSET] != SCOUT_P35_ORACLE_SETUP_FIXED {
                    usable = false;
                    break;
                }
                if bank_data[SCOUT_P35_BANK_ASSET_TAG_OFFSET] != SCOUT_P35_ASSET_TAG_DEFAULT {
                    usable = false;
                    break;
                }
                if data[base + 33] != SCOUT_P35_ASSET_TAG_DEFAULT {
                    usable = false;
                    break;
                }
                let decimals = bank_data[SCOUT_P35_BANK_MINT_DECIMALS_OFFSET];
                if decimals > SCOUT_P35_MAX_DECIMALS {
                    usable = false;
                    break;
                }
                let bank_price = fixed::types::I80F48::from_le_bytes(
                    (&bank_data[SCOUT_P35_BANK_FIXED_PRICE_OFFSET
                        ..SCOUT_P35_BANK_FIXED_PRICE_OFFSET + 16])
                        .try_into()
                        .unwrap_or_default(),
                );
                let recorded_price = f64::from_bits(u64::from_le_bytes(
                    (&data[price_offset..price_offset + 8])
                        .try_into()
                        .unwrap_or_default(),
                ));
                if bank_price.to_num::<f64>() != recorded_price {
                    usable = false;
                    break;
                }
                let liability_shares = fixed::types::I80F48::from_le_bytes(
                    (&data[base + 56..base + 72]).try_into().unwrap_or_default(),
                );
                if liability_shares < one_share {
                    continue;
                }
                let liability_share_value = fixed::types::I80F48::from_le_bytes(
                    (&bank_data[SCOUT_P35_BANK_LIABILITY_SHARE_VALUE_OFFSET
                        ..SCOUT_P35_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16])
                        .try_into()
                        .unwrap_or_default(),
                );
                let liability_weight = fixed::types::I80F48::from_le_bytes(
                    (&bank_data[SCOUT_P35_BANK_LIABILITY_WEIGHT_MAINT_OFFSET
                        ..SCOUT_P35_BANK_LIABILITY_WEIGHT_MAINT_OFFSET + 16])
                        .try_into()
                        .unwrap_or_default(),
                );
                let mut scale = fixed::types::I80F48::from_num(1);
                for _digit in 0..decimals {
                    let scaled = scale.checked_mul(ten);
                    if scaled.is_none() {
                        usable = false;
                        break;
                    }
                    scale = scaled.unwrap();
                }
                if !usable {
                    break;
                }
                let amount = liability_shares.checked_mul(liability_share_value);
                if amount.is_none() {
                    usable = false;
                    break;
                }
                let weighted = amount.unwrap().checked_mul(liability_weight);
                if weighted.is_none() {
                    usable = false;
                    break;
                }
                let priced = weighted.unwrap().checked_mul(bank_price);
                if priced.is_none() {
                    usable = false;
                    break;
                }
                let valued = priced.unwrap().checked_div(scale);
                if valued.is_none() {
                    usable = false;
                    break;
                }
                let summed = liabs_now.checked_add(valued.unwrap());
                if summed.is_none() {
                    usable = false;
                    break;
                }
                liabs_now = summed.unwrap();
            }
            if !usable {
                continue;
            }

            scout_check!(
                "P-0035",
                "unhealthy-account-liability-value-never-grows",
                liabs_now <= cached_liabs_maint.saturating_add(SCOUT_P35_VALUE_TOLERANCE),
                "P-0035: MarginfiAccount {} was recorded MAINTENANCE-UNHEALTHY by marginfi's own \
                 risk engine (health_cache.asset_value_maint {} < liability_value_maint {}, \
                 written at unix timestamp {}, which is still the current clock -- so no interest \
                 can have accrued since) and its maintenance liability value has since grown to \
                 {}, recomputed from persisted shares at the SAME per-balance oracle prices the \
                 engine itself recorded in health_cache.prices. Something increased this \
                 account's debt while it was already below the maintenance requirement. All six \
                 gated paths (borrow.rs:191-199, marginfi_account/withdraw.rs:196-201, \
                 kamino/withdraw.rs:203-208, drift/withdraw.rs:244-257, solend/withdraw.rs:\
                 214-220) abort on `risk_result?` and init weights are stricter than maintenance \
                 weights, so an already-unhealthy account cannot pass any of them -- this can only \
                 have come from a path with no RiskEngine gate, or from a suspended one. Account \
                 flags {:#x} (bracket bits are excluded, so none of FLASHLOAN/RECEIVERSHIP/\
                 DELEVERAGE is set here)",
                account,
                cached_assets_maint,
                cached_liabs_maint,
                cached_timestamp,
                liabs_now,
                account_flags
            );
        }
    }
    scout_run_property!("P-0035", invariant_p_0035(fixture));
    // SCOUT:INVARIANT:P-0035:END
    // SCOUT:INVARIANT:P-0037:BEGIN
    // P-0037 -- self-service withdrawal lockout while liquidatable: no self-authorized withdraw/borrow may succeed while maintenance health is negative.
    fn invariant_p_0037(f: &mut MarginfiFixture) {
        if f.scout_hp_kind == SCOUT_HP_KIND_NONE {
            return;
        }
        if !f.scout_hp_pre_valid {
            return;
        }
        if f.scout_hp_kind != SCOUT_HP_KIND_WITHDRAW
            && f.scout_hp_kind != SCOUT_HP_KIND_WITHDRAW_ALL
            && f.scout_hp_kind != SCOUT_HP_KIND_BORROW
        {
            return;
        }
        let pre = fixed::types::I80F48::from_bits(f.scout_hp_pre_health);
        if pre >= fixed::types::I80F48::from_num(0) {
            return;
        }
        scout_check!(
            "P-0037",
            "no-self-authorized-value-removal-succeeds-while-liquidatable",
            !f.scout_hp_succeeded,
            "P-0037: MarginfiAccount {} had MAINTENANCE health {} (negative, i.e. liquidatable) \
             immediately before a self-authorized value-removing instruction (probe arm {}: \
             1 = lending_account_withdraw, 2 = lending_account_withdraw withdraw_all, \
             3 = lending_account_borrow), and that instruction SUCCEEDED. A liquidatable account \
             must not be able to remove further value from itself for any amount: the only health \
             gate on these paths is `RiskEngine::check_account_init_health` \
             (withdraw.rs:198-203, borrow.rs:193-198), which is an INITIAL-margin check on the \
             POST state and is skipped entirely under ACCOUNT_IN_RECEIVERSHIP / \
             ACCOUNT_IN_FLASHLOAN (marginfi_account.rs:549-552) -- neither of which is set here \
             (both are excluded by SCOUT_HP_ACCOUNT_EXEMPT_FLAGS before this point). Every byte of \
             this scenario was written by a real instruction; nothing was byte-patched",
            f.scout_hp_subject,
            pre,
            f.scout_hp_kind
        );
    }
    scout_run_property!("P-0037", invariant_p_0037(fixture));
    // SCOUT:INVARIANT:P-0037:END
    // SCOUT:INVARIANT:P-0038:BEGIN
    // P-0038 -- no self-inflicted health decrease below maintenance: for an account already liquidatable, no successful self-initiated action may leave health lower than before.
    fn invariant_p_0038(f: &mut MarginfiFixture) {
        if f.scout_hp_kind == SCOUT_HP_KIND_NONE {
            return;
        }
        if !f.scout_hp_pre_valid {
            return;
        }
        if !f.scout_hp_succeeded {
            return;
        }
        let zero = fixed::types::I80F48::from_num(0);
        let one = fixed::types::I80F48::from_num(1);
        let pre = fixed::types::I80F48::from_bits(f.scout_hp_pre_health);
        if pre >= zero {
            return;
        }
        let account = f.scout_hp_subject;

        let data = match f.ctx.read_account(&account) {
            Ok(read) => read.data,
            Err(_) => return,
        };
        if data.len() != SCOUT_HP_ACCOUNT_LEN || data[..8] != SCOUT_HP_ACCOUNT_DISCRIMINATOR {
            return;
        }
        let flag_bytes: [u8; 8] = match data
            [SCOUT_HP_ACCOUNT_FLAGS_OFFSET..SCOUT_HP_ACCOUNT_FLAGS_OFFSET + 8]
            .try_into()
        {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        if u64::from_le_bytes(flag_bytes) & SCOUT_HP_ACCOUNT_EXEMPT_FLAGS != 0 {
            return;
        }
        let group_bytes: [u8; 32] = match data
            [SCOUT_HP_ACCOUNT_GROUP_OFFSET..SCOUT_HP_ACCOUNT_GROUP_OFFSET + 32]
            .try_into()
        {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        let group = Pubkey::new_from_array(group_bytes);

        let mut assets = zero;
        let mut liabilities = zero;
        for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
            let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
            if data.len() < base + SCOUT_BALANCE_STRIDE {
                break;
            }
            if data[base] == 0 {
                continue;
            }
            if data[base + 33] != SCOUT_HP_ASSET_TAG_DEFAULT {
                return;
            }
            let bank_bytes: [u8; 32] = match data[base + 1..base + 33].try_into() {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let bank_pk = Pubkey::new_from_array(bank_bytes);
            let bank_data = match f.ctx.read_account(&bank_pk) {
                Ok(read) => read.data,
                Err(_) => return,
            };
            if bank_data.len() != SCOUT_HP_BANK_LEN || bank_data[..8] != SCOUT_HP_BANK_DISCRIMINATOR
            {
                return;
            }
            let bank_group: [u8; 32] = match bank_data
                [SCOUT_HP_BANK_GROUP_OFFSET..SCOUT_HP_BANK_GROUP_OFFSET + 32]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            if Pubkey::new_from_array(bank_group) != group {
                return;
            }
            if bank_data[SCOUT_HP_BANK_ORACLE_SETUP_OFFSET] != SCOUT_HP_ORACLE_SETUP_FIXED
                || bank_data[SCOUT_HP_BANK_ASSET_TAG_OFFSET] != SCOUT_HP_ASSET_TAG_DEFAULT
            {
                return;
            }
            let emode_tag: [u8; 2] = match bank_data
                [SCOUT_HP_BANK_EMODE_TAG_OFFSET..SCOUT_HP_BANK_EMODE_TAG_OFFSET + 2]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let emode_flags: [u8; 8] = match bank_data
                [SCOUT_HP_BANK_EMODE_FLAGS_OFFSET..SCOUT_HP_BANK_EMODE_FLAGS_OFFSET + 8]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            if u16::from_le_bytes(emode_tag) != 0 || u64::from_le_bytes(emode_flags) != 0 {
                return;
            }
            let decimals = bank_data[SCOUT_HP_BANK_MINT_DECIMALS_OFFSET];
            if decimals > 24 {
                return;
            }
            let scale =
                match fixed::types::I80F48::checked_from_num(SCOUT_HP_EXP_10[decimals as usize]) {
                    Some(value) => value,
                    None => return,
                };
            let price_bytes: [u8; 16] = match bank_data
                [SCOUT_HP_BANK_FIXED_PRICE_OFFSET..SCOUT_HP_BANK_FIXED_PRICE_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let price = fixed::types::I80F48::from_le_bytes(price_bytes);
            let asset_share_value_bytes: [u8; 16] = match bank_data
                [SCOUT_HP_BANK_ASSET_SHARE_VALUE_OFFSET..SCOUT_HP_BANK_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let asset_share_value = fixed::types::I80F48::from_le_bytes(asset_share_value_bytes);
            let liability_share_value_bytes: [u8; 16] = match bank_data
                [SCOUT_HP_BANK_LIABILITY_SHARE_VALUE_OFFSET
                    ..SCOUT_HP_BANK_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let liability_share_value =
                fixed::types::I80F48::from_le_bytes(liability_share_value_bytes);
            let asset_weight_bytes: [u8; 16] = match bank_data
                [SCOUT_HP_BANK_ASSET_WEIGHT_MAINT_OFFSET
                    ..SCOUT_HP_BANK_ASSET_WEIGHT_MAINT_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let asset_weight_maint = fixed::types::I80F48::from_le_bytes(asset_weight_bytes);
            let liability_weight_bytes: [u8; 16] = match bank_data
                [SCOUT_HP_BANK_LIABILITY_WEIGHT_MAINT_OFFSET
                    ..SCOUT_HP_BANK_LIABILITY_WEIGHT_MAINT_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let liability_weight_maint =
                fixed::types::I80F48::from_le_bytes(liability_weight_bytes);
            let effective_asset_weight =
                if bank_data[SCOUT_HP_BANK_RISK_TIER_OFFSET] == SCOUT_HP_RISK_TIER_ISOLATED {
                    zero
                } else {
                    asset_weight_maint
                };

            let asset_share_bytes: [u8; 16] = match data[base + 40..base + 56].try_into() {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let asset_shares = fixed::types::I80F48::from_le_bytes(asset_share_bytes);
            let liability_share_bytes: [u8; 16] = match data[base + 56..base + 72].try_into() {
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let liability_shares = fixed::types::I80F48::from_le_bytes(liability_share_bytes);

            if liability_shares >= one {
                let amount = match liability_shares.checked_mul(liability_share_value) {
                    Some(value) => value,
                    None => return,
                };
                let weighted = match amount.checked_mul(liability_weight_maint) {
                    Some(value) => value,
                    None => return,
                };
                let priced = match weighted.checked_mul(price) {
                    Some(value) => value,
                    None => return,
                };
                let value = match priced.checked_div(scale) {
                    Some(value) => value,
                    None => return,
                };
                liabilities = match liabilities.checked_add(value) {
                    Some(value) => value,
                    None => return,
                };
            } else if asset_shares >= one {
                let amount = match asset_shares.checked_mul(asset_share_value) {
                    Some(value) => value,
                    None => return,
                };
                let weighted = match amount.checked_mul(effective_asset_weight) {
                    Some(value) => value,
                    None => return,
                };
                let priced = match weighted.checked_mul(price) {
                    Some(value) => value,
                    None => return,
                };
                let value = match priced.checked_div(scale) {
                    Some(value) => value,
                    None => return,
                };
                assets = match assets.checked_add(value) {
                    Some(value) => value,
                    None => return,
                };
            }
        }
        let post = match assets.checked_sub(liabilities) {
            Some(value) => value,
            None => return,
        };
        let tolerance = fixed::types::I80F48::from_num(0.0001);
        let floor = match pre.checked_sub(tolerance) {
            Some(value) => value,
            None => return,
        };
        scout_check!(
            "P-0038",
            "successful-self-initiated-action-does-not-lower-health-of-a-liquidatable-account",
            post >= floor,
            "P-0038: account {} already liquidatable (health {} < 0); self-initiated action left health at {} (arm {}); weighted assets {} vs liabilities {}",
            account,
            pre,
            post,
            f.scout_hp_kind,
            assets,
            liabilities
        );
    }
    scout_run_property!("P-0038", invariant_p_0038(fixture));
    // SCOUT:INVARIANT:P-0038:END
    // SCOUT:INVARIANT:P-0018:BEGIN
    fn invariant_p_0018(f: &mut MarginfiFixture) {
        const P18_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
        const P18_ACCOUNT_LEN: usize = 8 + 2304;
        let empty_balance_threshold = fixed::types::I80F48::from_num(1);

        let mut doubled = false;
        let mut bad_account = Pubkey::default();
        let mut bad_bank = Pubkey::default();
        let mut bad_slot = 0usize;
        let mut bad_active = 0u8;
        let mut bad_asset = fixed::types::I80F48::ZERO;
        let mut bad_liability = fixed::types::I80F48::ZERO;

        let dirty = f.ctx.dirty_tracker.dirty_accounts().clone();
        for key in dirty.iter() {
            if doubled {
                break;
            }
            let data = match f.ctx.read_account(key) {
                Ok(read) => read.data,
                Err(_) => continue,
            };
            if data.len() != P18_ACCOUNT_LEN {
                continue;
            }
            if data[..8] != P18_ACCOUNT_DISCRIMINATOR[..] {
                continue;
            }
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                if doubled {
                    break;
                }
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                let bank_bytes: [u8; 32] = match data[base + 1..base + 33].try_into() {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                let asset_share_bytes: [u8; 16] = match data[base + 40..base + 56].try_into() {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                let liability_share_bytes: [u8; 16] = match data[base + 56..base + 72].try_into() {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                };
                let asset_shares = fixed::types::I80F48::from_le_bytes(asset_share_bytes);
                let liability_shares = fixed::types::I80F48::from_le_bytes(liability_share_bytes);
                if asset_shares >= empty_balance_threshold
                    && liability_shares >= empty_balance_threshold
                {
                    doubled = true;
                    bad_account = key.clone();
                    bad_bank = Pubkey::new_from_array(bank_bytes);
                    bad_slot = i;
                    bad_active = data[base];
                    bad_asset = asset_shares;
                    bad_liability = liability_shares;
                }
            }
        }

        scout_check!(
                    "P-0018",
                    "balance-holds-at-most-one-side",
                    !doubled,
                    "P-0018: account {} slot {} bank {} active={} holds both sides: asset_shares {} liability_shares {}",
            bad_account,
            bad_slot,
            bad_bank,
            bad_active,
            bad_asset,
            bad_liability
        );
    }
    scout_run_property!("P-0018", invariant_p_0018(fixture));
    // SCOUT:INVARIANT:P-0018:END
    // SCOUT:INVARIANT:P-0033:BEGIN
    fn invariant_p_0033(f: &mut MarginfiFixture) {
        if !f.scout_p33_valid {
            return;
        }
        if f.scout_p33_rounds == 0 {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p33_liquidator)
        {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p33_liquidatee)
        {
            return;
        }

        let gross = fixed::types::I80F48::from_bits(f.scout_p33_gross_bits);
        let gain = fixed::types::I80F48::from_bits(f.scout_p33_gain_bits);
        let loss = fixed::types::I80F48::from_bits(f.scout_p33_loss_bits);
        let worst_gain = fixed::types::I80F48::from_bits(f.scout_p33_worst_gain_bits);
        let worst_gross = fixed::types::I80F48::from_bits(f.scout_p33_worst_gross_bits);
        let rounds = fixed::types::I80F48::from_num(f.scout_p33_rounds);
        let slack = match SCOUT_P33_PER_ROUND_SLACK.checked_mul(rounds) {
            Some(value) => value,
            None => return,
        };

        let gain_allowance = match gross.checked_mul(SCOUT_P33_LIQUIDATOR_FEE) {
            Some(value) => value,
            None => return,
        };
        let gain_bound = match gain_allowance.checked_add(slack) {
            Some(value) => value,
            None => return,
        };
        scout_check!(
            "P-0033",
            "cumulative-liquidator-gain-within-liquidation-fee",
            gain <= gain_bound,
            "P-0033: {} liquidation(s) of {} by {}: liquidator gained {} of {} gross moved (bound {} + slack {}); worst round gained {} on {}",
            f.scout_p33_rounds,
            f.scout_p33_liquidatee,
            f.scout_p33_liquidator,
            gain,
            gross,
            gain_allowance,
            slack,
            worst_gain,
            worst_gross
        );

        let loss_allowance = match gross.checked_mul(SCOUT_P33_LIQUIDATOR_PLUS_INSURANCE_FEE) {
            Some(value) => value,
            None => return,
        };
        let loss_bound = match loss_allowance.checked_add(slack) {
            Some(value) => value,
            None => return,
        };
        scout_check!(
            "P-0033",
            "cumulative-liquidatee-loss-within-liquidation-and-insurance-fees",
            loss <= loss_bound,
            "P-0033: {} liquidation(s), {} lost {} of {} gross moved (bound {} + slack {}); liquidator gained {}",
            f.scout_p33_rounds,
            f.scout_p33_liquidatee,
            loss,
            gross,
            loss_allowance,
            slack,
            gain
        );
    }
    scout_run_property!("P-0033", invariant_p_0033(fixture));
    // SCOUT:INVARIANT:P-0033:END
    // SCOUT:INVARIANT:P-0029:BEGIN
    fn invariant_p_0029(f: &mut MarginfiFixture) {
        if f.scout_p29_pay_valid {
            let dec_insurance = fixed::types::I80F48::from_bits(f.scout_p29_pay_dec_insurance);
            let dec_group = fixed::types::I80F48::from_bits(f.scout_p29_pay_dec_group);
            let dec_program = fixed::types::I80F48::from_bits(f.scout_p29_pay_dec_program);
            let out_insurance = fixed::types::I80F48::from_num(f.scout_p29_pay_out_insurance);
            let out_group = fixed::types::I80F48::from_num(f.scout_p29_pay_out_group);
            let out_program = fixed::types::I80F48::from_num(f.scout_p29_pay_out_program);
            let out_liquidity = fixed::types::I80F48::from_num(f.scout_p29_pay_liquidity_out);
            let dec_total = dec_insurance + dec_group + dec_program;
            let out_total = out_insurance + out_group + out_program;
            scout_check!(
                "P-0029",
                "fee-payout-debits-each-counter-by-exactly-what-it-delivered",
                dec_insurance == out_insurance
                    && dec_group == out_group
                    && dec_program == out_program,
                "P-0029: bank {} debited fees (ins={} grp={} prog={}) but delivered (ins={} grp={} ata={}); succeeded={}",
                f.fee_bank,
                dec_insurance,
                dec_group,
                dec_program,
                out_insurance,
                out_group,
                out_program,
                f.scout_p29_pay_succeeded
            );
            scout_check!(
                "P-0029",
                "fee-payout-is-funded-exactly-by-the-liquidity-vault",
                dec_total == out_liquidity,
                "P-0029: bank {} counters reduced by {} but vault lost {} (delivered total {})",
                f.fee_bank,
                dec_total,
                out_liquidity,
                out_total
            );
            scout_check!(
                "P-0029",
                "a-failed-fee-payout-commits-nothing",
                f.scout_p29_pay_succeeded
                    || (dec_total == fixed::types::I80F48::ZERO
                        && out_liquidity == fixed::types::I80F48::ZERO
                        && out_total == fixed::types::I80F48::ZERO),
                "P-0029: bank {} collect FAILED yet counters moved {}, vault lost {}, delivered {}",
                f.fee_bank,
                dec_total,
                out_liquidity,
                out_total
            );
        }
        if f.scout_p29_gen_valid {
            let expect_group = fixed::types::I80F48::from_bits(f.scout_p29_gen_expect_group);
            let expect_program = fixed::types::I80F48::from_bits(f.scout_p29_gen_expect_program);
            let delta_group = fixed::types::I80F48::from_bits(f.scout_p29_gen_delta_group);
            let delta_program = fixed::types::I80F48::from_bits(f.scout_p29_gen_delta_program);
            let delta_insurance = fixed::types::I80F48::from_bits(f.scout_p29_gen_delta_insurance);
            if f.scout_p29_gen_exact {
                scout_check!(
                    "P-0029",
                    "origination-fee-credits-exactly-the-computed-split",
                    delta_group == expect_group
                        && delta_program == expect_program
                        && delta_insurance == fixed::types::I80F48::ZERO,
                    "P-0029: bank {} origination fee split moved (grp={} prog={} ins={}) vs expected (grp={} prog={})",
                    f.fee_bank,
                    delta_group,
                    delta_program,
                    delta_insurance,
                    expect_group,
                    expect_program
                );
            }
            scout_check!(
                "P-0029",
                "origination-fee-is-never-under-credited",
                delta_group >= expect_group
                    && delta_program >= expect_program
                    && delta_insurance >= fixed::types::I80F48::ZERO,
                "P-0029: bank {} origination fee (grp={} prog={} ins={}) below expected (grp={} prog={}); exact_window={}",
                f.fee_bank,
                delta_group,
                delta_program,
                delta_insurance,
                expect_group,
                expect_program,
                f.scout_p29_gen_exact
            );
        }
    }
    scout_run_property!("P-0029", invariant_p_0029(fixture));
    // SCOUT:INVARIANT:P-0029:END
    // SCOUT:INVARIANT:P-0021:BEGIN
    fn invariant_p_0021(f: &mut MarginfiFixture) {
        if !f.scout_p21_valid {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p21_liquidatee)
        {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p21_liquidator)
        {
            return;
        }

        let loss = fixed::types::I80F48::from_bits(f.scout_p21_loss_bits);
        let gross = fixed::types::I80F48::from_bits(f.scout_p21_gross_bits);
        let allowance = match gross.checked_mul(SCOUT_P33_LIQUIDATOR_PLUS_INSURANCE_FEE) {
            Some(value) => value,
            None => return,
        };
        let bound = match allowance.checked_add(SCOUT_P2123_ROUND_SLACK) {
            Some(value) => value,
            None => return,
        };
        scout_check!(
            "P-0021",
            "single-liquidation-liquidatee-equity-loss-within-discount-plus-insurance-fee",
            loss <= bound,
            "P-0021: liquidation of {} by {} (arm {}, worst of {}) destroyed {} equity moving {} gross (bound {} + slack {})",
            f.scout_p21_liquidatee,
            f.scout_p21_liquidator,
            f.scout_p21_arm,
            f.scout_p21_rounds,
            loss,
            gross,
            allowance,
            SCOUT_P2123_ROUND_SLACK
        );
    }
    scout_run_property!("P-0021", invariant_p_0021(fixture));
    // SCOUT:INVARIANT:P-0021:END
    // SCOUT:INVARIANT:P-0023:BEGIN
    fn invariant_p_0023(f: &mut MarginfiFixture) {
        if !f.scout_p23_valid {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p23_liquidator)
        {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p23_liquidatee)
        {
            return;
        }

        let gain = fixed::types::I80F48::from_bits(f.scout_p23_gain_bits);
        let gross = fixed::types::I80F48::from_bits(f.scout_p23_gross_bits);
        let allowance = match gross.checked_mul(SCOUT_P33_LIQUIDATOR_FEE) {
            Some(value) => value,
            None => return,
        };
        let bound = match allowance.checked_add(SCOUT_P2123_ROUND_SLACK) {
            Some(value) => value,
            None => return,
        };
        scout_check!(
            "P-0023",
            "single-liquidation-liquidator-equity-gain-within-liquidation-discount",
            gain <= bound,
            "P-0023: liquidation of {} by {} (arm {}, worst of {}): liquidator gained {} seizing {} gross (bound {} + slack {})",
            f.scout_p23_liquidatee,
            f.scout_p23_liquidator,
            f.scout_p23_arm,
            f.scout_p23_rounds,
            gain,
            gross,
            allowance,
            SCOUT_P2123_ROUND_SLACK
        );
    }
    scout_run_property!("P-0023", invariant_p_0023(fixture));
    // SCOUT:INVARIANT:P-0023:END
// SCOUT:INVARIANT:P-0030:BEGIN
    fn invariant_p_0030(f: &mut MarginfiFixture) {
        let payout_since_baseline =
            f.scout_p30_collect_seq != f.scout_p30_collect_seq_at_baseline;
        let harness_fee_fabrication_bank = f.bank;

        for slot in 0..SCOUT_P30_BANK_CAP {
            let bank = f.scout_p30_banks[slot];
            if bank == Pubkey::default() {
                continue;
            }
            let account = f.ctx.read_account(&bank);
            if account.is_err() {
                continue;
            }
            let data = account.unwrap().data;
            if data.len() != SCOUT_P30_BANK_LEN {
                continue;
            }
            if data[..8] != SCOUT_P30_BANK_DISCRIMINATOR {
                continue;
            }
            let insurance_bytes: [u8; 16] = data[SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET
                ..SCOUT_COLLECT_BANK_FEES_INSURANCE_OFFSET + 16]
                .try_into()
                .unwrap_or_default();
            let insurance = fixed::types::I80F48::from_le_bytes(insurance_bytes);
            let group_bytes: [u8; 16] = data[SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET
                ..SCOUT_COLLECT_BANK_FEES_GROUP_OFFSET + 16]
                .try_into()
                .unwrap_or_default();
            let group = fixed::types::I80F48::from_le_bytes(group_bytes);
            let program_bytes: [u8; 16] = data[SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET
                ..SCOUT_COLLECT_BANK_FEES_PROGRAM_OFFSET + 16]
                .try_into()
                .unwrap_or_default();
            let program = fixed::types::I80F48::from_le_bytes(program_bytes);

            scout_check!(
                "P-0030",
                "fee-counters-are-never-negative",
                insurance >= fixed::types::I80F48::ZERO
                    && group >= fixed::types::I80F48::ZERO
                    && program >= fixed::types::I80F48::ZERO,
                "P-0030: bank {} has negative fee counter (ins={} grp={} prog={})",
                bank,
                insurance,
                group,
                program
            );

            if bank == harness_fee_fabrication_bank && payout_since_baseline {
                continue;
            }
            let base_insurance = fixed::types::I80F48::from_bits(f.scout_p30_insurance_bits[slot]);
            let base_group = fixed::types::I80F48::from_bits(f.scout_p30_group_bits[slot]);
            let base_program = fixed::types::I80F48::from_bits(f.scout_p30_program_bits[slot]);
            scout_check!(
                "P-0030",
                "fee-counters-never-decrease-outside-a-payout",
                insurance >= base_insurance
                    && group >= base_group
                    && program >= base_program,
                "P-0030: bank {} fee counters dropped from (ins={} grp={} prog={}) to (ins={} grp={} prog={}); payout_since_baseline={} is_shared_bank={}",
                bank,
                base_insurance,
                base_group,
                base_program,
                insurance,
                group,
                program,
                payout_since_baseline,
                bank == harness_fee_fabrication_bank
            );
        }
    }
    scout_run_property!("P-0030", invariant_p_0030(fixture));
    // SCOUT:INVARIANT:P-0030:END
    // SCOUT:INVARIANT:P-0024:BEGIN
    fn invariant_p_0024(f: &mut MarginfiFixture) {
        if f.scout_p24_arm == SCOUT_P24_ARM_NONE {
            return;
        }
        if !f.scout_p24_valid {
            return;
        }
        if !f.scout_p24_pinned {
            return;
        }
        let zero = fixed::types::I80F48::from_num(0);
        let pre = fixed::types::I80F48::from_bits(f.scout_p24_pre_health);
        let post = fixed::types::I80F48::from_bits(f.scout_p24_post_health);
        if pre < zero {
            return;
        }
        let tolerance = fixed::types::I80F48::from_num(0.0001);
        let floor = match zero.checked_sub(tolerance) {
            Some(value) => value,
            None => return,
        };
        scout_check!(
            "P-0024",
            "third-party-action-cannot-make-a-healthy-account-liquidatable",
            post >= floor,
            "P-0024: account {} (authority {}) was healthy (health {}); action by {} left it at {} (arm {}); succeeded={}",
            f.scout_p24_victim,
            f.scout_p24_victim_authority,
            pre,
            f.scout_p24_actor,
            post,
            f.scout_p24_arm,
            f.scout_p24_succeeded
        );
    }
    scout_run_property!("P-0024", invariant_p_0024(fixture));
    // SCOUT:INVARIANT:P-0024:END
    // SCOUT:INVARIANT:P-0025:BEGIN
    fn invariant_p_0025(f: &mut MarginfiFixture) {
        if !f.scout_rl_armed {
            return;
        }
        let one = fixed::types::I80F48::from_num(1);
        let liability_shares = fixed::types::I80F48::from_bits(f.scout_rl_liab_bits);
        if liability_shares < one {
            return;
        }
        if f.scout_rl_wallet < f.scout_rl_amount {
            return;
        }
        if f.scout_rl_bank_state != SCOUT_RL_BANK_STATE_OPERATIONAL {
            return;
        }
        if f.scout_rl_bank_tag != SCOUT_HP_ASSET_TAG_DEFAULT {
            return;
        }
        if f.scout_rl_flags & SCOUT_RL_ACCOUNT_EXEMPT_FLAGS != 0 {
            return;
        }
        scout_check!(
            "P-0025",
            "well-formed-repay-is-admitted-regardless-of-other-banks",
            f.scout_rl_succeeded,
            "P-0025: account {} owed {} shares in bank {} (wallet {} vs amount {}, flags={}); well-formed repay refused (arm {}, other bank {})",
            f.scout_rl_subject,
            liability_shares,
            f.scout_rl_bank,
            f.scout_rl_wallet,
            f.scout_rl_amount,
            f.scout_rl_flags,
            f.scout_rl_arm,
            f.scout_rl_other_bank
        );
    }
    scout_run_property!("P-0025", invariant_p_0025(fixture));
    // SCOUT:INVARIANT:P-0025:END
    // SCOUT:INVARIANT:P-0027:BEGIN
    fn invariant_p_0027(f: &mut MarginfiFixture) {
        if !f.scout_p27_valid {
            return;
        }
        let bank = f.scout_p27_bank;
        if bank == Pubkey::default() {
            return;
        }
        let read = f.ctx.read_account(&bank);
        if read.is_err() {
            return;
        }
        let data = read.unwrap().data;
        if data.len() != SCOUT_PC_BANK_LEN {
            return;
        }
        if data[..8] != SCOUT_PC_BANK_DISCRIMINATOR {
            return;
        }
        if data[SCOUT_PC_BANK_ORACLE_SETUP_OFFSET] != SCOUT_PC_ORACLE_SETUP_FIXED {
            return;
        }

        let input_bytes: [u8; 16] = data
            [SCOUT_PC_BANK_FIXED_PRICE_OFFSET..SCOUT_PC_BANK_FIXED_PRICE_OFFSET + 16]
            .try_into()
            .unwrap_or_default();
        let expected_price = i128::from_le_bytes(input_bytes);
        let expected_confidence: i128 = 0;

        let cached_bytes: [u8; 16] = data
            [SCOUT_PC_BANK_CACHE_PRICE_OFFSET..SCOUT_PC_BANK_CACHE_PRICE_OFFSET + 16]
            .try_into()
            .unwrap_or_default();
        let cached_price = i128::from_le_bytes(cached_bytes);
        let confidence_bytes: [u8; 16] = data
            [SCOUT_PC_BANK_CACHE_CONFIDENCE_OFFSET..SCOUT_PC_BANK_CACHE_CONFIDENCE_OFFSET + 16]
            .try_into()
            .unwrap_or_default();
        let cached_confidence = i128::from_le_bytes(confidence_bytes);
        let stamp_bytes: [u8; 8] = data[SCOUT_PC_BANK_CACHE_TIMESTAMP_OFFSET
            ..SCOUT_PC_BANK_CACHE_TIMESTAMP_OFFSET + 8]
            .try_into()
            .unwrap_or_default();
        let cached_stamp = i64::from_le_bytes(stamp_bytes);

        let asked = f.scout_p27_asked_bits;
        let pre_ts = f.scout_p27_pre_ts;
        let post_ts = f.scout_p27_post_ts;

        scout_check!(
            "P-0027",
            "set-fixed-oracle-price-stores-the-requested-price",
            expected_price == asked,
            "P-0027: bank {} asked fixed price {} but config.fixed_price now holds {}",
            bank,
            asked,
            expected_price
        );
        scout_check!(
            "P-0027",
            "pulse-price-cache-equals-independently-rederived-oracle-price",
            cached_price == expected_price,
            "P-0027: bank {} cached price {} != re-derived {}",
            bank,
            cached_price,
            expected_price
        );
        scout_check!(
            "P-0027",
            "pulse-price-cache-confidence-is-zero-for-a-fixed-oracle",
            cached_confidence == expected_confidence,
            "P-0027: bank {} cached confidence {} != 0",
            bank,
            cached_confidence
        );
        scout_check!(
            "P-0027",
            "pulse-price-cache-timestamp-is-the-clock-at-the-pulse",
            cached_stamp >= pre_ts && cached_stamp <= post_ts,
            "P-0027: bank {} cached timestamp {} outside [{}, {}]",
            bank,
            cached_stamp,
            pre_ts,
            post_ts
        );
    }
    scout_run_property!("P-0027", invariant_p_0027(fixture));
    // SCOUT:INVARIANT:P-0027:END
    // SCOUT:INVARIANT:P-0028:BEGIN
    fn invariant_p_0028(f: &mut MarginfiFixture) {
        if f.scout_p28_arm == SCOUT_P28_ARM_NONE {
            return;
        }
        if !f.scout_p28_measured {
            return;
        }
        scout_check!(
            "P-0028",
            "crank-writes-nothing-outside-its-permitted-set",
            f.scout_p28_account_mask == 0 && f.scout_p28_bank_mask == 0,
            "P-0028: crank arm {} by {} on {} wrote outside permitted set (account_mask={:#x} bank_mask={:#x} first_offset={} succeeded={})",
            f.scout_p28_arm,
            f.scout_p28_actor,
            f.scout_p28_victim,
            f.scout_p28_account_mask,
            f.scout_p28_bank_mask,
            f.scout_p28_first_offset,
            f.scout_p28_succeeded
        );
        if f.scout_p28_health_valid && f.scout_p28_pinned {
            let pre = fixed::types::I80F48::from_bits(f.scout_p28_pre_health);
            let post = fixed::types::I80F48::from_bits(f.scout_p28_post_health);
            let tolerance = fixed::types::I80F48::from_num(0.0001);
            let floor = match pre.checked_sub(tolerance) {
                Some(value) => value,
                None => return,
            };
            scout_check!(
                "P-0028",
                "crank-cannot-lower-maintenance-health",
                post >= floor,
                "P-0028: crank arm {} by {} LOWERED {} health from {} to {}; succeeded={}",
                f.scout_p28_arm,
                f.scout_p28_actor,
                f.scout_p28_victim,
                pre,
                post,
                f.scout_p28_succeeded
            );
        }
        if !f.scout_p28_followup_measured {
            return;
        }
        scout_check!(
            "P-0028",
            "crank-cannot-break-an-unrelated-later-transaction",
            f.scout_p28_followup_ok,
            "P-0028: crank arm {} by {} on {} broke a later unrelated deposit; crank succeeded={}",
            f.scout_p28_arm,
            f.scout_p28_actor,
            f.scout_p28_victim,
            f.scout_p28_succeeded
        );
    }
    scout_run_property!("P-0028", invariant_p_0028(fixture));
    // SCOUT:INVARIANT:P-0028:END
    // SCOUT:INVARIANT:P-0014:BEGIN
    fn invariant_p_0014(f: &mut MarginfiFixture) {
        for slot in 0..SCOUT_SV_BANK_CAP {
            let bank = f.scout_sv_banks[slot];
            if bank == Pubkey::default() {
                continue;
            }
            if f.scout_sv_forged[slot] {
                continue;
            }
            let account = f.ctx.read_account(&bank);
            if account.is_err() {
                continue;
            }
            let data = account.unwrap().data;
            if data.len() != SCOUT_SV_BANK_LEN {
                continue;
            }
            if data[..8] != SCOUT_SV_BANK_DISCRIMINATOR {
                continue;
            }
            let asset_bytes: [u8; 16] = data[SCOUT_SV_ASSET_SHARE_VALUE_OFFSET
                ..SCOUT_SV_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .unwrap_or_default();
            let asset = fixed::types::I80F48::from_le_bytes(asset_bytes);
            let liability_bytes: [u8; 16] = data[SCOUT_SV_LIABILITY_SHARE_VALUE_OFFSET
                ..SCOUT_SV_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .unwrap_or_default();
            let liability = fixed::types::I80F48::from_le_bytes(liability_bytes);
            let base_asset = fixed::types::I80F48::from_bits(f.scout_sv_asset_bits[slot]);
            let base_liability = fixed::types::I80F48::from_bits(f.scout_sv_liability_bits[slot]);

            scout_check!(
                "P-0014",
                "liability-share-value-never-decreases",
                liability >= base_liability,
                "P-0014: bank {} lowered liability_share_value from {} to {}",
                bank,
                base_liability,
                liability
            );

            let mut socialized_in_window = false;
            for entry in 0..SCOUT_SV_SOCIALIZED_CAP {
                if f.scout_sv_socialized[entry] == bank {
                    socialized_in_window = true;
                }
            }
            if socialized_in_window {
                continue;
            }
            scout_check!(
                "P-0014",
                "asset-share-value-decreases-only-on-a-socialized-loss",
                asset >= base_asset,
                "P-0014: bank {} lowered asset_share_value from {} to {} with no socialized loss",
                bank,
                base_asset,
                asset
            );
        }
    }
    scout_run_property!("P-0014", invariant_p_0014(fixture));
    // SCOUT:INVARIANT:P-0014:END
    // SCOUT:INVARIANT:P-0016:BEGIN
    fn invariant_p_0016(f: &mut MarginfiFixture) {
        for slot in 0..SCOUT_SV_BANK_CAP {
            let bank = f.scout_sv_banks[slot];
            if bank == Pubkey::default() {
                continue;
            }
            let account = f.ctx.read_account(&bank);
            if account.is_err() {
                continue;
            }
            let data = account.unwrap().data;
            if data.len() != SCOUT_SV_BANK_LEN {
                continue;
            }
            if data[..8] != SCOUT_SV_BANK_DISCRIMINATOR {
                continue;
            }
            let operational_state = data[SCOUT_SV_OPERATIONAL_STATE_OFFSET];
            if operational_state == SCOUT_SV_KILLED_BY_BANKRUPTCY {
                continue;
            }
            let asset_bytes: [u8; 16] = data[SCOUT_SV_ASSET_SHARE_VALUE_OFFSET
                ..SCOUT_SV_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
                .unwrap_or_default();
            let asset = fixed::types::I80F48::from_le_bytes(asset_bytes);
            scout_check!(
                "P-0016",
                "live-bank-has-a-positive-asset-share-value",
                asset > fixed::types::I80F48::ZERO,
                "P-0016: bank {} in state {} (not KilledByBankruptcy) has asset_share_value = {}",
                bank,
                operational_state,
                asset
            );
        }
    }
    scout_run_property!("P-0016", invariant_p_0016(fixture));
    // SCOUT:INVARIANT:P-0016:END
    // SCOUT:INVARIANT:P-0031:BEGIN
    fn invariant_p_0031(f: &mut MarginfiFixture) {
        if !f.scout_p31_valid {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p31_liquidator)
        {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p31_liquidatee)
        {
            return;
        }

        let collateral_residual =
            fixed::types::I80F48::from_bits(f.scout_p31_collateral_residual_bits);
        let liability_residual =
            fixed::types::I80F48::from_bits(f.scout_p31_liability_residual_bits);
        let collateral_leg = fixed::types::I80F48::from_bits(f.scout_p31_collateral_leg_bits);
        let liquidator_leg =
            fixed::types::I80F48::from_bits(f.scout_p31_liquidator_liab_leg_bits);
        let liquidatee_leg =
            fixed::types::I80F48::from_bits(f.scout_p31_liquidatee_liab_leg_bits);
        let scaled = match liquidator_leg.checked_mul(SCOUT_P31_FINAL_DISCOUNT) {
            Some(value) => value,
            None => return,
        };
        let expected_liquidatee_leg = match scaled.checked_div(SCOUT_P31_LIQUIDATOR_DISCOUNT) {
            Some(value) => value,
            None => return,
        };

        scout_check!(
            "P-0031",
            "liquidation-collateral-leg-nets-to-zero-between-the-two-parties",
            collateral_residual <= SCOUT_P31_LEG_SLACK,
            "P-0031: liquidation of {} by {} (arm {}, worst of {}) moved {} collateral; legs fail to cancel by {} (slack {})",
            f.scout_p31_liquidatee,
            f.scout_p31_liquidator,
            f.scout_p31_arm,
            f.scout_p31_rounds,
            collateral_leg,
            collateral_residual,
            SCOUT_P31_LEG_SLACK
        );

        scout_check!(
            "P-0031",
            "liquidatee-liability-leg-reconstructs-from-the-liquidator-leg",
            liability_residual <= SCOUT_P31_LEG_SLACK,
            "P-0031: liquidation of {} by {} (arm {}, worst of {}): liquidator debited {}; expected liquidatee leg from {} is {}, actual {}, residual {} (slack {})",
            f.scout_p31_liquidatee,
            f.scout_p31_liquidator,
            f.scout_p31_arm,
            f.scout_p31_rounds,
            liquidator_leg,
            liquidator_leg,
            expected_liquidatee_leg,
            liquidatee_leg,
            liability_residual,
            SCOUT_P31_LEG_SLACK
        );
    }
    scout_run_property!("P-0031", invariant_p_0031(fixture));
    // SCOUT:INVARIANT:P-0031:END
    // SCOUT:INVARIANT:P-0032:BEGIN
    fn invariant_p_0032(f: &mut MarginfiFixture) {
        if !f.scout_p32_valid {
            return;
        }
        if !f
            .ctx
            .dirty_tracker
            .dirty_accounts()
            .contains(&f.scout_p32_account)
        {
            return;
        }

        let seized = fixed::types::I80F48::from_bits(f.scout_p32_seized_bits);
        let repaid = fixed::types::I80F48::from_bits(f.scout_p32_repaid_bits);
        let max_fee = fixed::types::I80F48::from_bits(f.scout_p32_max_fee_bits);
        let pre_assets = fixed::types::I80F48::from_bits(f.scout_p32_pre_assets_bits);
        let one = fixed::types::I80F48::ONE;
        let fee_state_ratio = match one.checked_add(max_fee) {
            Some(value) => value,
            None => return,
        };
        let minimum_ratio = match one.checked_add(SCOUT_P32_BONUS_FEE_MINIMUM) {
            Some(value) => value,
            None => return,
        };
        let bracket_ratio = fee_state_ratio.max(minimum_ratio);
        let one_shot_ratio = match one.checked_div(SCOUT_P31_FINAL_DISCOUNT) {
            Some(value) => value,
            None => return,
        };
        let allowed_ratio = bracket_ratio.max(one_shot_ratio);
        let allowance = match repaid.checked_mul(allowed_ratio) {
            Some(value) => value,
            None => return,
        };
        let bound = match allowance.checked_add(SCOUT_P32_VALUE_SLACK) {
            Some(value) => value,
            None => return,
        };

        scout_check!(
            "P-0032",
            "start-end-liquidation-bracket-honours-the-one-shot-liquidation-premium",
            seized <= bound,
            "P-0032: bracket on {} (arm {}, worst of {}) took {} vs repaid {} (one-shot ratio {}, bracket ratio {}); bound {} + slack {}; pre-bracket equity {}",
            f.scout_p32_account,
            f.scout_p32_arm,
            f.scout_p32_brackets,
            seized,
            repaid,
            one_shot_ratio,
            bracket_ratio,
            bound,
            SCOUT_P32_VALUE_SLACK,
            pre_assets
        );
    }
    scout_run_property!("P-0032", invariant_p_0032(fixture));
    // SCOUT:INVARIANT:P-0032:END
    // SCOUT:INVARIANT:P-0019:BEGIN
    fn invariant_p_0019(f: &mut MarginfiFixture) {
        if !f.scout_pir_acc_valid {
            return;
        }
        let bank = f.scout_pir_acc_bank;
        if bank == Pubkey::default() {
            return;
        }
        if !f.ctx.dirty_tracker.dirty_accounts().contains(&bank) {
            return;
        }
        let delta = f.scout_pir_acc_delta;
        if delta == 0 {
            return;
        }
        let irc = f.scout_pir_acc_irc;
        if irc[SCOUT_PIR_IRC_CURVE_TYPE] != SCOUT_PIR_CURVE_SEVEN_POINT {
            return;
        }
        let pre_asv = fixed::types::I80F48::from_bits(f.scout_pir_acc_pre_asv);
        let pre_lsv = fixed::types::I80F48::from_bits(f.scout_pir_acc_pre_lsv);
        let post_asv = fixed::types::I80F48::from_bits(f.scout_pir_acc_post_asv);
        let post_lsv = fixed::types::I80F48::from_bits(f.scout_pir_acc_post_lsv);
        let asset_shares = fixed::types::I80F48::from_bits(f.scout_pir_acc_pre_asset_shares);
        let liability_shares =
            fixed::types::I80F48::from_bits(f.scout_pir_acc_pre_liability_shares);

        let total_assets = match asset_shares.checked_mul(pre_asv) {
            Some(v) => v,
            None => return,
        };
        let total_liabilities = match liability_shares.checked_mul(pre_lsv) {
            Some(v) => v,
            None => return,
        };
        if total_assets == fixed::types::I80F48::ZERO
            || total_liabilities == fixed::types::I80F48::ZERO
        {
            return;
        }
        let ur_raw = match total_liabilities.checked_div(total_assets) {
            Some(v) => v,
            None => return,
        };

        let u32_max = fixed::types::I80F48::from_num(u32::MAX);
        let zero_rate_code =
            match irc[SCOUT_PIR_IRC_ZERO_UTIL_RATE..SCOUT_PIR_IRC_ZERO_UTIL_RATE + 4].try_into() {
                Ok(v) => u32::from_le_bytes(v),
                Err(_) => return,
            };
        let hundred_rate_code = match irc
            [SCOUT_PIR_IRC_HUNDRED_UTIL_RATE..SCOUT_PIR_IRC_HUNDRED_UTIL_RATE + 4]
            .try_into()
        {
            Ok(v) => u32::from_le_bytes(v),
            Err(_) => return,
        };
        let zero_ratio = match fixed::types::I80F48::from_num(zero_rate_code).checked_div(u32_max) {
            Some(v) => v,
            None => return,
        };
        let zero_rate = match zero_ratio.checked_mul(SCOUT_PIR_MILLI_MAX_PERCENT) {
            Some(v) => v,
            None => return,
        };
        let hundred_ratio =
            match fixed::types::I80F48::from_num(hundred_rate_code).checked_div(u32_max) {
                Some(v) => v,
                None => return,
            };
        let hundred_rate = match hundred_ratio.checked_mul(SCOUT_PIR_MILLI_MAX_PERCENT) {
            Some(v) => v,
            None => return,
        };

        let ur = ur_raw
            .max(fixed::types::I80F48::ZERO)
            .min(fixed::types::I80F48::ONE);
        let mut start_x = fixed::types::I80F48::ZERO;
        let mut start_y = zero_rate;
        let mut end_x = fixed::types::I80F48::ONE;
        let mut end_y = hundred_rate;
        for index in 0..SCOUT_PIR_CURVE_POINTS {
            let base = SCOUT_PIR_IRC_POINTS + index * SCOUT_PIR_RATE_POINT_STRIDE;
            let util_code = match irc[base..base + 4].try_into() {
                Ok(v) => u32::from_le_bytes(v),
                Err(_) => return,
            };
            let rate_code = match irc[base + 4..base + 8].try_into() {
                Ok(v) => u32::from_le_bytes(v),
                Err(_) => return,
            };
            if util_code == 0 {
                continue;
            }
            let point_util = match fixed::types::I80F48::from_num(util_code).checked_div(u32_max) {
                Some(v) => v,
                None => return,
            };
            let point_ratio = match fixed::types::I80F48::from_num(rate_code).checked_div(u32_max) {
                Some(v) => v,
                None => return,
            };
            let point_rate = match point_ratio.checked_mul(SCOUT_PIR_MILLI_MAX_PERCENT) {
                Some(v) => v,
                None => return,
            };
            if ur <= point_util {
                end_x = point_util;
                end_y = point_rate;
                break;
            }
            start_x = point_util;
            start_y = point_rate;
        }

        let mut base_rate = start_y;
        if end_x > start_x {
            if ur < start_x {
                return;
            }
            if ur > end_x {
                return;
            }
            if end_y < start_y {
                return;
            }
            let delta_x = match end_x.checked_sub(start_x) {
                Some(v) => v,
                None => return,
            };
            if delta_x != fixed::types::I80F48::ZERO {
                let offset = match ur.checked_sub(start_x) {
                    Some(v) => v,
                    None => return,
                };
                let proportion = match offset.checked_div(delta_x) {
                    Some(v) => v,
                    None => return,
                };
                let delta_y = match end_y.checked_sub(start_y) {
                    Some(v) => v,
                    None => return,
                };
                let scaled = match delta_y.checked_mul(proportion) {
                    Some(v) => v,
                    None => return,
                };
                let interpolated = match start_y.checked_add(scaled) {
                    Some(v) => v,
                    None => return,
                };
                base_rate = interpolated;
            }
        }

        let insurance_ir = fixed::types::I80F48::from_le_bytes(
            match irc[SCOUT_PIR_IRC_INSURANCE_IR..SCOUT_PIR_IRC_INSURANCE_IR + 16].try_into() {
                Ok(v) => v,
                Err(_) => return,
            },
        );
        let insurance_fixed = fixed::types::I80F48::from_le_bytes(
            match irc[SCOUT_PIR_IRC_INSURANCE_FIXED..SCOUT_PIR_IRC_INSURANCE_FIXED + 16].try_into()
            {
                Ok(v) => v,
                Err(_) => return,
            },
        );
        let group_ir = fixed::types::I80F48::from_le_bytes(
            match irc[SCOUT_PIR_IRC_PROTOCOL_IR..SCOUT_PIR_IRC_PROTOCOL_IR + 16].try_into() {
                Ok(v) => v,
                Err(_) => return,
            },
        );
        let group_fixed = fixed::types::I80F48::from_le_bytes(
            match irc[SCOUT_PIR_IRC_PROTOCOL_FIXED..SCOUT_PIR_IRC_PROTOCOL_FIXED + 16].try_into() {
                Ok(v) => v,
                Err(_) => return,
            },
        );
        let program_ir = if f.scout_pir_acc_program_fees {
            fixed::types::I80F48::from_bits(f.scout_pir_acc_program_fee_rate)
        } else {
            fixed::types::I80F48::ZERO
        };
        let program_fixed = if f.scout_pir_acc_program_fees {
            fixed::types::I80F48::from_bits(f.scout_pir_acc_program_fee_fixed)
        } else {
            fixed::types::I80F48::ZERO
        };
        let fee_ir_partial = match insurance_ir.checked_add(group_ir) {
            Some(v) => v,
            None => return,
        };
        let fee_ir = match fee_ir_partial.checked_add(program_ir) {
            Some(v) => v,
            None => return,
        };
        let fee_fixed_partial = match insurance_fixed.checked_add(group_fixed) {
            Some(v) => v,
            None => return,
        };
        let fee_fixed = match fee_fixed_partial.checked_add(program_fixed) {
            Some(v) => v,
            None => return,
        };
        let lending_rate = match base_rate.checked_mul(ur) {
            Some(v) => v,
            None => return,
        };
        let one_plus_fee_ir = match fixed::types::I80F48::ONE.checked_add(fee_ir) {
            Some(v) => v,
            None => return,
        };
        let borrowing_scaled = match base_rate.checked_mul(one_plus_fee_ir) {
            Some(v) => v,
            None => return,
        };
        let borrowing_rate = match borrowing_scaled.checked_add(fee_fixed) {
            Some(v) => v,
            None => return,
        };

        let dt = fixed::types::I80F48::from_num(delta);
        let lending_scaled = match lending_rate.checked_mul(dt) {
            Some(v) => v,
            None => return,
        };
        let lending_per_period = match lending_scaled.checked_div(SCOUT_PIR_SECONDS_PER_YEAR) {
            Some(v) => v,
            None => return,
        };
        let lending_growth = match fixed::types::I80F48::ONE.checked_add(lending_per_period) {
            Some(v) => v,
            None => return,
        };
        let expected_asv = match pre_asv.checked_mul(lending_growth) {
            Some(v) => v,
            None => return,
        };
        let borrowing_scaled_dt = match borrowing_rate.checked_mul(dt) {
            Some(v) => v,
            None => return,
        };
        let borrowing_per_period = match borrowing_scaled_dt.checked_div(SCOUT_PIR_SECONDS_PER_YEAR)
        {
            Some(v) => v,
            None => return,
        };
        let borrowing_growth = match fixed::types::I80F48::ONE.checked_add(borrowing_per_period) {
            Some(v) => v,
            None => return,
        };
        let expected_lsv = match pre_lsv.checked_mul(borrowing_growth) {
            Some(v) => v,
            None => return,
        };

        let observed_asset_delta = match post_asv.checked_sub(pre_asv) {
            Some(v) => v,
            None => return,
        };
        let expected_asset_delta = match expected_asv.checked_sub(pre_asv) {
            Some(v) => v,
            None => return,
        };
        let observed_asset_tokens = match observed_asset_delta.checked_mul(asset_shares) {
            Some(v) => v,
            None => return,
        };
        let expected_asset_tokens = match expected_asset_delta.checked_mul(asset_shares) {
            Some(v) => v,
            None => return,
        };
        let observed_liab_delta = match post_lsv.checked_sub(pre_lsv) {
            Some(v) => v,
            None => return,
        };
        let expected_liab_delta = match expected_lsv.checked_sub(pre_lsv) {
            Some(v) => v,
            None => return,
        };
        let observed_liab_tokens = match observed_liab_delta.checked_mul(liability_shares) {
            Some(v) => v,
            None => return,
        };
        let expected_liab_tokens = match expected_liab_delta.checked_mul(liability_shares) {
            Some(v) => v,
            None => return,
        };

        let asset_gap_signed = match observed_asset_tokens.checked_sub(expected_asset_tokens) {
            Some(v) => v,
            None => return,
        };
        let asset_gap = asset_gap_signed.abs();
        let liab_gap_signed = match observed_liab_tokens.checked_sub(expected_liab_tokens) {
            Some(v) => v,
            None => return,
        };
        let liab_gap = liab_gap_signed.abs();

        scout_check!(
            "P-0019",
            "asset-side-accrual-equals-independent-recomputation",
            asset_gap <= SCOUT_PIR_TOKEN_TOLERANCE,
            "P-0019: bank {} accrued {}s: asset_share_value {} -> {}, credited {} tokens across {} shares (ur={}); expected {} (gap {} vs tolerance {})",
            bank,
            delta,
            f.scout_pir_acc_pre_asv,
            f.scout_pir_acc_post_asv,
            observed_asset_tokens,
            asset_shares,
            ur,
            expected_asset_tokens,
            asset_gap,
            SCOUT_PIR_TOKEN_TOLERANCE
        );
        scout_check!(
            "P-0019",
            "liability-side-accrual-equals-independent-recomputation",
            liab_gap <= SCOUT_PIR_TOKEN_TOLERANCE,
            "P-0019: over the same {}s on bank {}, liability_share_value {} -> {}, charged {} tokens across {} shares; expected {} (gap {})",
            delta,
            bank,
            f.scout_pir_acc_pre_lsv,
            f.scout_pir_acc_post_lsv,
            observed_liab_tokens,
            liability_shares,
            expected_liab_tokens,
            liab_gap
        );
        scout_check!(
            "P-0019",
            "depositors-credited-no-more-than-borrowers-charged",
            observed_asset_tokens <= observed_liab_tokens,
            "P-0019: over the same {}s on bank {}, depositors credited {} but borrowers charged only {} (diff {}); ur={} fee_ir={} fee_fixed={}",
            delta,
            bank,
            observed_asset_tokens,
            observed_liab_tokens,
            asset_gap_signed,
            ur,
            fee_ir,
            fee_fixed
        );
    }
    scout_run_property!("P-0019", invariant_p_0019(fixture));
    // SCOUT:INVARIANT:P-0019:END
    // SCOUT:INVARIANT:P-0007:BEGIN
    fn invariant_p_0007(f: &mut MarginfiFixture) {
        let banks = [
            f.scout_p7_bank,
            f.borrow_liab_bank,
            f.borrow_asset_bank,
            f.fee_bank,
            f.bank,
        ];
        for index in 0..SCOUT_P7_BANK_COUNT {
            if !f.scout_p7_prev_valid[index] {
                continue;
            }
            if !f.scout_p7_prev_both_sided[index] {
                continue;
            }
            let subject = banks[index];
            if subject == Pubkey::default() {
                continue;
            }
            if f.scout_p7_prev_bank[index] != subject {
                continue;
            }
            let read = f.ctx.read_account(&subject);
            if read.is_err() {
                continue;
            }
            let data = read.unwrap().data;
            if data.len() != SCOUT_P7_BANK_LEN {
                continue;
            }
            if data[..8] != SCOUT_P7_BANK_DISCRIMINATOR {
                continue;
            }
            let asset_bytes: [u8; 16] = match data[SCOUT_P7_BANK_TOTAL_ASSET_SHARES_OFFSET
                ..SCOUT_P7_BANK_TOTAL_ASSET_SHARES_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let liability_bytes: [u8; 16] = match data[SCOUT_P7_BANK_TOTAL_LIABILITY_SHARES_OFFSET
                ..SCOUT_P7_BANK_TOTAL_LIABILITY_SHARES_OFFSET + 16]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            if i128::from_le_bytes(asset_bytes) <= 0 {
                continue;
            }
            if i128::from_le_bytes(liability_bytes) <= 0 {
                continue;
            }
            let stamp_bytes: [u8; 8] = match data
                [SCOUT_P7_BANK_LAST_UPDATE_OFFSET..SCOUT_P7_BANK_LAST_UPDATE_OFFSET + 8]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let stamp = i64::from_le_bytes(stamp_bytes);
            let previous = f.scout_p7_prev_last_update[index];
            if stamp <= previous {
                continue;
            }
            let elapsed = stamp.saturating_sub(previous);
            let booked_bytes: [u8; 4] = match data[SCOUT_P7_BANK_INTEREST_ACCUMULATED_FOR_OFFSET
                ..SCOUT_P7_BANK_INTEREST_ACCUMULATED_FOR_OFFSET + 4]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let booked = u32::from_le_bytes(booked_bytes) as i64;
            scout_check!(
                "P-0007",
                "last-update-advances-only-with-a-booking",
                booked == elapsed,
                "P-0007: bank {} advanced last_update {} -> {} ({}s) but interest_accumulated_for={}",
                subject,
                previous,
                stamp,
                elapsed,
                booked
            );
        }
    }
    scout_run_property!("P-0007", invariant_p_0007(fixture));
    fn invariant_p_0007_ext(f: &mut MarginfiFixture) {
        let banks = [
            f.scout_p7_bank,
            f.borrow_liab_bank,
            f.borrow_asset_bank,
            f.fee_bank,
            f.bank,
        ];
        for index in 0..SCOUT_P7_BANK_COUNT {
            if !f.scout_p7_prev_valid[index] {
                continue;
            }
            let subject = banks[index];
            if subject == Pubkey::default() {
                continue;
            }
            if f.scout_p7_prev_bank[index] != subject {
                continue;
            }
            let read = match f.ctx.read_account(&subject) {
                Ok(a) => a,
                Err(_) => continue,
            };
            let data = read.data;
            if data.len() != SCOUT_P7_BANK_LEN || data[..8] != SCOUT_P7_BANK_DISCRIMINATOR {
                continue;
            }
            const P7_ASSET_SHARE_VALUE_OFFSET: usize = 8 + 72;
            const P7_LIABILITY_SHARE_VALUE_OFFSET: usize = 8 + 88;
            let cur_asset_sv: [u8; 16] = match data
                [P7_ASSET_SHARE_VALUE_OFFSET..P7_ASSET_SHARE_VALUE_OFFSET + 16]
                .try_into()
            {
                Ok(b) => b,
                Err(_) => continue,
            };
            let cur_liab_sv: [u8; 16] = match data
                [P7_LIABILITY_SHARE_VALUE_OFFSET..P7_LIABILITY_SHARE_VALUE_OFFSET + 16]
                .try_into()
            {
                Ok(b) => b,
                Err(_) => continue,
            };
            let stamp_bytes: [u8; 8] = match data
                [SCOUT_P7_BANK_LAST_UPDATE_OFFSET..SCOUT_P7_BANK_LAST_UPDATE_OFFSET + 8]
                .try_into()
            {
                Ok(b) => b,
                Err(_) => continue,
            };
            let cur_stamp = i64::from_le_bytes(stamp_bytes);
            let prev_stamp = f.scout_p7_prev_last_update[index];
            let share_value_changed = cur_asset_sv != f.scout_p7_prev_asset_sv[index]
                || cur_liab_sv != f.scout_p7_prev_liab_sv[index];
            if !share_value_changed {
                continue;
            }
            scout_check!(
                "P-0007",
                "share-value-change-requires-last-update-advance",
                cur_stamp > prev_stamp,
                "P-0007: bank {} changed a share value without advancing last_update (prev={} now={})",
                subject,
                prev_stamp,
                cur_stamp
            );
        }
    }
    scout_run_property!("P-0007", invariant_p_0007_ext(fixture));
    // SCOUT:INVARIANT:P-0007:END
    // SCOUT:INVARIANT:P-0009:BEGIN
    fn invariant_p_0009(f: &mut MarginfiFixture) {
        let accounts = [
            f.borrow_marginfi_account,
            f.withdraw_marginfi_account,
            f.pulse_health_healthy_account,
            f.pulse_health_risk_rejected_account,
        ];
        for index in 0..SCOUT_P9_ACCOUNT_COUNT {
            if !f.scout_p9_prev_valid[index] {
                continue;
            }
            if !f.scout_p9_cur_valid[index] {
                continue;
            }
            let subject = accounts[index];
            if subject == Pubkey::default() {
                continue;
            }
            if f.scout_p9_prev_account[index] != subject {
                continue;
            }
            if f.scout_p9_cur_account[index] != subject {
                continue;
            }
            if f.scout_p9_prev_digest[index] == f.scout_p9_cur_digest[index] {
                continue;
            }
            let read = f.ctx.read_account(&subject);
            if read.is_err() {
                continue;
            }
            let data = read.unwrap().data;
            if data.len() != SCOUT_P9_ACCOUNT_LEN {
                continue;
            }
            if data[..8] != SCOUT_P9_ACCOUNT_DISCRIMINATOR {
                continue;
            }
            let stamp_bytes: [u8; 8] = match data
                [SCOUT_P9_HC_TIMESTAMP_OFFSET..SCOUT_P9_HC_TIMESTAMP_OFFSET + 8]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let flags_bytes: [u8; 4] = match data
                [SCOUT_P9_HC_FLAGS_OFFSET..SCOUT_P9_HC_FLAGS_OFFSET + 4]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let mrgn_err_bytes: [u8; 4] = match data
                [SCOUT_P9_HC_MRGN_ERR_OFFSET..SCOUT_P9_HC_MRGN_ERR_OFFSET + 4]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let internal_err_bytes: [u8; 4] = match data
                [SCOUT_P9_HC_INTERNAL_ERR_OFFSET..SCOUT_P9_HC_INTERNAL_ERR_OFFSET + 4]
                .try_into()
            {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let stamp = i64::from_le_bytes(stamp_bytes);
            let flags = u32::from_le_bytes(flags_bytes);
            let mrgn_err = u32::from_le_bytes(mrgn_err_bytes);
            let internal_err = u32::from_le_bytes(internal_err_bytes);
            let version = data[SCOUT_P9_HC_PROGRAM_VERSION_OFFSET];
            let clock = f.scout_p9_clock;
            scout_check!(
                "P-0009",
                "health-cache-write-stamps-the-clock",
                stamp == clock,
                "P-0009: account {} health_cache timestamp {} != clock {}",
                subject,
                stamp,
                clock
            );
            scout_check!(
                "P-0009",
                "health-cache-write-stamps-the-program-version",
                version == SCOUT_P9_PROGRAM_VERSION,
                "P-0009: account {} health_cache.program_version {} != PROGRAM_VERSION {}",
                subject,
                version,
                SCOUT_P9_PROGRAM_VERSION
            );
            if mrgn_err == 0 {
                if internal_err == 0 {
                    scout_check!(
                        "P-0009",
                        "health-cache-error-free-write-sets-engine-ok",
                        (flags & SCOUT_P9_ENGINE_OK) != 0,
                        "P-0009: account {} health_cache error-free but ENGINE_OK clear in flags={}",
                        subject,
                        flags
                    );
                }
            }
        }
    }
    scout_run_property!("P-0009", invariant_p_0009(fixture));
    // SCOUT:INVARIANT:P-0009:END
    // SCOUT:INVARIANT:P-0015:BEGIN
    fn invariant_p_0015(f: &mut MarginfiFixture) {
        let recorded = f.scout_p15_next.min(SCOUT_P15_CAP);
        let group_account = f.ctx.read_account(&f.marginfi_group);
        let group_data = match group_account {
            Ok(account) => account.data,
            Err(_) => Vec::new(),
        };
        let withdrawn_today = match group_data.get(
            SCOUT_P15_GROUP_WITHDRAWN_TODAY_OFFSET..SCOUT_P15_GROUP_WITHDRAWN_TODAY_OFFSET + 4,
        ) {
            Some(slice) => match slice.try_into() {
                Ok(bytes) => u32::from_le_bytes(bytes),
                Err(_) => 0u32,
            },
            None => 0u32,
        };

        for i in 0..recorded {
            let limit = f.scout_p15_limit[i];
            if limit == 0 {
                continue;
            }
            let window_end = f.scout_p15_ts[i];
            let mut total = fixed::types::I80F48::ZERO;
            let mut counted = 0usize;
            for j in 0..(i + 1) {
                let age = window_end.saturating_sub(f.scout_p15_ts[j]);
                if age >= SCOUT_P15_WINDOW_SECONDS {
                    continue;
                }
                total = total
                    .checked_add(fixed::types::I80F48::from_bits(f.scout_p15_value_bits[j]))
                    .unwrap_or(total);
                counted = counted.saturating_add(1);
            }
            let bound =
                fixed::types::I80F48::from_num(limit).saturating_add(SCOUT_P15_VALUE_SLACK);
            let latest = fixed::types::I80F48::from_bits(f.scout_p15_value_bits[i]);
            scout_check!(
                "P-0015",
                "trailing-24h-deleverage-withdrawals-stay-within-the-daily-limit",
                total <= bound,
                "P-0015: {} deleverage withdrawals moved ${} against daily_limit ${} (withdrawn_today={}); last was ${}",
                counted,
                total,
                limit,
                withdrawn_today,
                latest
            );
        }
    }
    scout_run_property!("P-0015", invariant_p_0015(fixture));
    // SCOUT:INVARIANT:P-0015:END
    // SCOUT:INVARIANT:P-0046:BEGIN
    fn invariant_p_0046(f: &mut MarginfiFixture) {
        if f.scout_known_next > SCOUT_KNOWN_CAP {
            return;
        }
        let accounts = f.scout_known_accounts;
        for acct in accounts {
            if acct == Pubkey::default() {
                continue;
            }
            let data = match f.ctx.read_account(&acct) {
                Ok(a) => a.data,
                Err(_) => continue,
            };
            let mut active_pks: [[u8; 32]; SCOUT_BALANCES_PER_ACCOUNT] =
                [[0u8; 32]; SCOUT_BALANCES_PER_ACCOUNT];
            let mut active_n = 0usize;
            for i in 0..SCOUT_BALANCES_PER_ACCOUNT {
                let base = SCOUT_PULSE_FIRST_BALANCE_BASE_OFFSET + i * SCOUT_BALANCE_STRIDE;
                if data.len() < base + SCOUT_BALANCE_STRIDE {
                    break;
                }
                let active = data[base] == 1;
                let pk_bytes: [u8; 32] =
                    data[base + 1..base + 33].try_into().unwrap_or_default();
                let asset_z = data[base + 40..base + 56] == [0u8; 16];
                let liab_z = data[base + 56..base + 72] == [0u8; 16];
                let emis_z = data[base + 72..base + 88] == [0u8; 16];
                let lu_z = data[base + 88..base + 96] == [0u8; 8];
                if active {
                    let dup = active_pks[..active_n].iter().any(|p| *p == pk_bytes);
                    scout_check!(
                        "P-0046",
                        "no-two-active-balances-share-a-bank",
                        !dup,
                        "P-0046: account {} has two active balance slots for bank {}",
                        acct,
                        Pubkey::new_from_array(pk_bytes)
                    );
                    if active_n < SCOUT_BALANCES_PER_ACCOUNT {
                        active_pks[active_n] = pk_bytes;
                        active_n += 1;
                    }
                } else {
                    let pk_z = pk_bytes == [0u8; 32];
                    scout_check!(
                        "P-0046",
                        "closed-balance-slot-is-fully-zeroed",
                        pk_z && asset_z && liab_z && emis_z && lu_z,
                        "P-0046: account {} inactive slot {} not zeroed (bank_pk_zero={} asset={} liab={} emissions={} last_update={})",
                        acct,
                        i,
                        pk_z,
                        asset_z,
                        liab_z,
                        emis_z,
                        lu_z
                    );
                }
            }
        }
    }
    scout_run_property!("P-0046", invariant_p_0046(fixture));
    // SCOUT:INVARIANT:P-0046:END
    // SCOUT:INVARIANT:P-0042:BEGIN
    fn invariant_p_0042(f: &mut MarginfiFixture) {
        const P42_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
        const P42_BANK_LEN: usize = 8 + 1856;
        const P42_EMODE_CONFIG_OFFSET: usize = 944;
        const P42_ENTRY_STRIDE: usize = 40;
        const P42_ENTRY_TAG_OFFSET: usize = 0;
        const P42_ENTRY_ASSET_WEIGHT_INIT_OFFSET: usize = 8;
        const P42_ENTRY_ASSET_WEIGHT_MAINT_OFFSET: usize = 24;

        let registry = f.scout_p06_banks;
        let mut checked: Vec<Pubkey> = Vec::new();
        for bank_pk in std::iter::once(f.bank).chain(registry) {
            if bank_pk == Pubkey::default() || checked.contains(&bank_pk) {
                continue;
            }
            checked.push(bank_pk);
            let data = match f.ctx.account_data(&bank_pk) {
                Ok(d) if d.len() == P42_BANK_LEN && d[..8] == P42_BANK_DISCRIMINATOR => d,
                _ => continue,
            };
            for i in 0..MAX_EMODE_ENTRIES {
                let base = P42_EMODE_CONFIG_OFFSET + i * P42_ENTRY_STRIDE;
                if data.len() < base + P42_ENTRY_STRIDE {
                    break;
                }
                let tag: [u8; 2] = match data[base + P42_ENTRY_TAG_OFFSET..base + P42_ENTRY_TAG_OFFSET + 2].try_into() {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let tag = u16::from_le_bytes(tag);
                if tag == 0 {
                    continue;
                }
                let init_bits: [u8; 16] = match data
                    [base + P42_ENTRY_ASSET_WEIGHT_INIT_OFFSET..base + P42_ENTRY_ASSET_WEIGHT_INIT_OFFSET + 16]
                    .try_into()
                {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let maint_bits: [u8; 16] = match data
                    [base + P42_ENTRY_ASSET_WEIGHT_MAINT_OFFSET..base + P42_ENTRY_ASSET_WEIGHT_MAINT_OFFSET + 16]
                    .try_into()
                {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let init = fixed::types::I80F48::from_le_bytes(init_bits);
                let maint = fixed::types::I80F48::from_le_bytes(maint_bits);
                let one = fixed::types::I80F48::from_num(1);
                scout_check!(
                    "P-0042",
                    "emode-collateral-weight-not-above-one",
                    init <= one && maint <= one,
                    "P-0042: bank {} EmodeEntry tag {} has asset_weight_init={} / asset_weight_maint={} (>1.0)",
                    bank_pk,
                    tag,
                    init,
                    maint
                );
            }
        }
    }
    scout_run_property!("P-0042", invariant_p_0042(fixture));
    // SCOUT:INVARIANT:P-0042:END
    // SCOUT:INVARIANTS:END
}
