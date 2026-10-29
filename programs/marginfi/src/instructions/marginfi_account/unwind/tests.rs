use super::*;
use anchor_lang::error::Error as AnchorError;
use solana_stake_interface::stake_flags::StakeFlags;
use solana_stake_interface::stake_history::{StakeHistory, StakeHistoryEntry};
use solana_stake_interface::state::{Meta, Stake};
use spl_stake_pool::state::{Fee, StakePool};

#[test]
fn test_pool_tokens_for_exceeds_supply() {
    let pool = StakePool {
        pool_token_supply: 1_000,
        total_lamports: 10_000,
        ..Default::default()
    };
    assert!(min_pool_tokens_for_lamports(&pool, 1_000_000_000, true).is_err());
}

#[test]
fn test_pool_tokens_for_zero_supply() {
    let pool = StakePool {
        pool_token_supply: 0,
        total_lamports: 0,
        ..Default::default()
    };
    assert!(min_pool_tokens_for_lamports(&pool, 1_000, true).is_err());
}

fn bank_with_shares(shares: u64, share_value: I80F48) -> Bank {
    Bank {
        total_asset_shares: I80F48::from_num(shares).into(),
        asset_share_value: share_value.into(),
        ..Default::default()
    }
}

#[test]
fn test_apply_pnl_to_asset_share_value_positive_increases_share_value() {
    let mut bank = bank_with_shares(1_000_000, I80F48::ONE);
    bank.apply_pnl_to_asset_share_value(I80F48::from_num(500_000))
        .unwrap();
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::from_num(1.5));
    assert_eq!(
        I80F48::from(bank.total_asset_shares),
        I80F48::from_num(1_000_000)
    );
}

#[test]
fn test_apply_pnl_to_asset_share_value_zero_shares_preserves_value() {
    let mut bank = bank_with_shares(0, I80F48::from_num(2));
    bank.apply_pnl_to_asset_share_value(I80F48::from_num(500_000))
        .unwrap();
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::from_num(2));
}

#[test]
fn test_apply_pnl_to_asset_share_value_negative_lowers_and_clamps() {
    let mut bank = bank_with_shares(1_000_000, I80F48::ONE);
    bank.apply_pnl_to_asset_share_value(I80F48::from_num(-2_000_000))
        .unwrap();
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::ZERO);
}

#[test]
fn test_apply_pnl_to_asset_share_value_zero_shares_with_negative_preserves_value() {
    let mut bank = bank_with_shares(0, I80F48::from_num(2));
    bank.apply_pnl_to_asset_share_value(I80F48::from_num(-500_000))
        .unwrap();
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::from_num(2));
}

fn stake_state_deactivated_at(epoch: u64) -> StakeStateV2 {
    let mut stake = Stake::default();
    stake.delegation.stake = 1_000;
    stake.delegation.activation_epoch = u64::MAX;
    stake.delegation.deactivation_epoch = epoch;
    StakeStateV2::Stake(Meta::default(), stake, StakeFlags::empty())
}

#[test]
fn test_check_stake_deactivated_after_epoch_passes() {
    assert!(require_stake_deactivation_elapsed(
        &stake_state_deactivated_at(5),
        6,
        &StakeHistory::default()
    )
    .is_ok());
}

#[test]
fn test_check_stake_deactivated_same_epoch_fails() {
    assert!(require_stake_deactivation_elapsed(
        &stake_state_deactivated_at(5),
        5,
        &StakeHistory::default()
    )
    .is_err());
}

#[test]
fn test_check_stake_still_cooling_down_after_epoch_fails() {
    let mut stake_history = StakeHistory::default();
    stake_history.add(5, StakeHistoryEntry::with_deactivating(1_000));

    let stake_state = stake_state_deactivated_at(5);
    let StakeStateV2::Stake(_, stake, _) = &stake_state else {
        unreachable!();
    };
    let old_rate_status =
        stake
            .delegation
            .stake_activating_and_deactivating(6, &stake_history, None);
    let new_rate_status =
        stake
            .delegation
            .stake_activating_and_deactivating(6, &stake_history, Some(0));
    assert!(old_rate_status.deactivating > 0);
    assert!(new_rate_status.deactivating > 0);

    assert!(require_stake_deactivation_elapsed(&stake_state, 6, &stake_history).is_err());
}

#[test]
fn test_check_stake_deactivated_never_deactivated_fails() {
    assert!(require_stake_deactivation_elapsed(
        &stake_state_deactivated_at(u64::MAX),
        6,
        &StakeHistory::default()
    )
    .is_err());
}

#[test]
fn test_check_stake_deactivated_non_stake_state_fails() {
    assert!(require_stake_deactivation_elapsed(
        &StakeStateV2::Uninitialized,
        6,
        &StakeHistory::default()
    )
    .is_err());
}

#[test]
fn test_validate_params_boundaries() {
    assert!(validate_unwind_config_params(MIN_UNWIND_FEE_MULTIPLIER_BPS).is_ok());
    assert!(validate_unwind_config_params(MAX_UNWIND_FEE_MULTIPLIER_BPS).is_ok());
    assert!(validate_unwind_config_params(MIN_UNWIND_FEE_MULTIPLIER_BPS - 1).is_err());
    assert!(validate_unwind_config_params(MAX_UNWIND_FEE_MULTIPLIER_BPS + 1).is_err());
    assert!(validate_unwind_config_params(5_000).is_err());
}

fn apr_to_u32(apr: f64) -> u32 {
    ((apr / 10.0) * u32::MAX as f64).round() as u32
}

fn quote_fixture() -> (Bank, Pubkey, MarginfiGroup, UnwindConfig, MarginfiAccount) {
    let sol_bank_key = Pubkey::new_unique();
    let mut sol_bank = Bank {
        total_asset_shares: I80F48::from_num(1_000).into(),
        asset_share_value: I80F48::ONE.into(),
        total_liability_shares: I80F48::from_num(400).into(),
        liability_share_value: I80F48::ONE.into(),
        pending_unwind_lamports: 100,
        ..Bank::default()
    };
    sol_bank.config.interest_rate_config.zero_util_rate = apr_to_u32(0.1);
    sol_bank.config.interest_rate_config.hundred_util_rate = apr_to_u32(0.5);
    sol_bank.config.interest_rate_config.curve_type =
        marginfi_type_crate::types::INTEREST_CURVE_SEVEN_POINT;

    let mut marginfi_account = MarginfiAccount::zeroed();
    let liability = &mut marginfi_account.lending_account.balances[0];
    liability.set_active(true);
    liability.bank_pk = sol_bank_key;
    liability.liability_shares = I80F48::from_num(100).into();

    (
        sol_bank,
        sol_bank_key,
        MarginfiGroup::default(),
        UnwindConfig {
            fee_multiplier_bps: 20_000,
            min_unwind_lamports: 0,
            bump: 0,
        },
        marginfi_account,
    )
}

#[test]
fn test_unwind_quote_uses_pending_utilization_and_fee_multiplier() {
    let (sol_bank, sol_bank_key, group, config, marginfi_account) = quote_fixture();
    let half_year_secs = 15_768_000;

    let quote = quote_unwind_debt_and_fee(
        &sol_bank,
        &sol_bank_key,
        &group,
        &config,
        &marginfi_account,
        half_year_secs,
    )
    .unwrap();
    assert_eq!(quote.debt_lamports, 100);
    assert_eq!(quote.fee_lamports, 31);

    let mut bank_without_pending = sol_bank;
    bank_without_pending.pending_unwind_lamports = 0;
    let quote_without_pending = quote_unwind_debt_and_fee(
        &bank_without_pending,
        &sol_bank_key,
        &group,
        &config,
        &marginfi_account,
        half_year_secs,
    )
    .unwrap();
    assert_eq!(
        quote_without_pending.fee_lamports, 27,
        "pending exposure must increase the quoted borrowing rate"
    );
}

#[test]
fn test_unwind_quote_rounds_fee_up_and_charges_nothing_at_epoch_end() {
    let (sol_bank, sol_bank_key, group, mut config, mut marginfi_account) = quote_fixture();
    config.fee_multiplier_bps = 15_000;
    marginfi_account.lending_account.balances[0].liability_shares = I80F48::ONE.into();

    let half_year_secs = 15_768_000;
    let quote = quote_unwind_debt_and_fee(
        &sol_bank,
        &sol_bank_key,
        &group,
        &config,
        &marginfi_account,
        half_year_secs,
    )
    .unwrap();
    assert_eq!(quote.debt_lamports, 1);
    assert_eq!(quote.fee_lamports, 1, "fractional lamport fee rounds up");

    let quote_at_epoch_end = quote_unwind_debt_and_fee(
        &sol_bank,
        &sol_bank_key,
        &group,
        &config,
        &marginfi_account,
        0,
    )
    .unwrap();
    assert_eq!(quote_at_epoch_end.fee_lamports, 0);
}

fn bank_with_pending_unwind(pending: u64) -> Bank {
    let mut bank = bank_with_shares(1_000_000, I80F48::ONE);
    bank.pending_unwind_lamports = pending;
    bank
}

#[test]
fn test_book_settlement_exact_receipt_changes_nothing_but_pending() {
    let mut bank = bank_with_pending_unwind(500);
    book_unwind_settlement(&mut bank, 500, 500).unwrap();
    assert_eq!(bank.pending_unwind_lamports, 0);
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::ONE);
}

#[test]
fn test_book_settlement_surplus_accrues_to_lenders() {
    let mut bank = bank_with_pending_unwind(500);
    book_unwind_settlement(&mut bank, 600, 500).unwrap();
    assert_eq!(bank.pending_unwind_lamports, 0);
    let expected = I80F48::ONE + I80F48::from_num(100) / I80F48::from_num(1_000_000);
    assert_eq!(I80F48::from(bank.asset_share_value), expected);
}

#[test]
fn test_book_settlement_shortfall_is_socialized_to_lenders() {
    let mut bank = bank_with_pending_unwind(500);
    book_unwind_settlement(&mut bank, 400, 500).unwrap();
    assert_eq!(bank.pending_unwind_lamports, 0);
    let expected = I80F48::ONE - I80F48::from_num(100) / I80F48::from_num(1_000_000);
    assert_eq!(I80F48::from(bank.asset_share_value), expected);
}

#[test]
fn test_book_settlement_releases_only_the_ticket_principal() {
    let mut bank = bank_with_pending_unwind(800);
    book_unwind_settlement(&mut bank, 500, 500).unwrap();
    assert_eq!(bank.pending_unwind_lamports, 300);
}

#[test]
fn test_book_settlement_rejects_principal_above_pending() {
    let mut bank = bank_with_pending_unwind(100);
    let err = book_unwind_settlement(&mut bank, 500, 500).unwrap_err();
    let AnchorError::AnchorError(err) = err else {
        panic!("expected an Anchor error");
    };
    assert_eq!(
        err.error_code_number,
        u32::from(MarginfiError::InternalLogicError)
    );
}

fn lamports_redeemed_for(pool: &StakePool, tokens: u64) -> u64 {
    let fee = pool.calc_pool_tokens_stake_withdrawal_fee(tokens).unwrap();
    pool.calc_lamports_withdraw_amount(tokens - fee).unwrap()
}

#[test]
fn test_pool_tokens_for_is_minimal_across_rates_and_fees() {
    let cases: [(u64, u64, (u64, u64), u64); 7] = [
        (1_000_000_000_000, 1_000_000_000_000, (0, 1_000), 1),
        (
            1_000_000_000_000,
            1_000_000_000_000,
            (0, 1_000),
            7_777_777_777,
        ),
        (
            1_000_000_000_000,
            1_150_000_000_000,
            (0, 1_000),
            8_000_000_000,
        ),
        (
            1_000_000_000_000,
            1_150_000_000_000,
            (5, 1_000),
            8_000_000_000,
        ),
        (3_333_333_333_333, 3_999_999_999_999, (1, 400), 123_456_789),
        (
            500_000_000_000,
            1_000_000_000_000,
            (25, 1_000),
            2_000_000_000,
        ),
        (1_000_000, 10_000_000_000, (0, 1_000), 1_000_000_000),
    ];

    for (supply, total, (num, den), required) in cases {
        let pool = StakePool {
            pool_token_supply: supply,
            total_lamports: total,
            stake_withdrawal_fee: Fee {
                numerator: num,
                denominator: den,
            },
            ..Default::default()
        };
        let (tokens, pool_fee) = min_pool_tokens_for_lamports(&pool, required, true).unwrap();

        assert!(
        lamports_redeemed_for(&pool, tokens) >= required,
        "tokens={tokens} must redeem >= {required} (supply={supply} total={total} fee={num}/{den})"
    );
        if tokens > 1 {
            assert!(
                lamports_redeemed_for(&pool, tokens - 1) < required,
                "tokens={tokens} is not minimal (supply={supply} total={total} fee={num}/{den})"
            );
        }
        assert_eq!(
            pool_fee,
            pool.calc_pool_tokens_stake_withdrawal_fee(tokens).unwrap()
        );
        if num == 0 {
            assert_eq!(pool_fee, 0);
        }
    }
}

#[test]
fn test_pool_tokens_for_charges_more_tokens_when_fee_is_set() {
    let base = StakePool {
        pool_token_supply: 1_000_000_000_000,
        total_lamports: 1_100_000_000_000,
        ..Default::default()
    };
    let with_fee = StakePool {
        stake_withdrawal_fee: Fee {
            numerator: 5,
            denominator: 1_000,
        },
        ..base.clone()
    };
    let (no_fee_tokens, _) = min_pool_tokens_for_lamports(&base, 10_000_000_000, true).unwrap();
    let (waived_fee_tokens, waived_fee) =
        min_pool_tokens_for_lamports(&with_fee, 10_000_000_000, false).unwrap();
    let (fee_tokens, fee) = min_pool_tokens_for_lamports(&with_fee, 10_000_000_000, true).unwrap();
    assert_eq!(waived_fee_tokens, no_fee_tokens);
    assert_eq!(waived_fee, 0);
    assert!(fee > 0);
    assert!(fee_tokens > no_fee_tokens);
}

#[test]
fn test_lamport_unwind_fee_conversion_rounds_up_to_lst_base_unit() {
    assert_eq!(convert_lamport_fee_to_lst(1, 3, 2).unwrap(), 2);
    assert_eq!(convert_lamport_fee_to_lst(0, 3, 2).unwrap(), 0);
    assert!(convert_lamport_fee_to_lst(1, 3, 0).is_err());
}
