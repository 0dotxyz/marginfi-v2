use crate::{
    check,
    errors::MarginfiError,
    math_error,
    prelude::MarginfiResult,
    state::{
        bank::BankImpl,
        bank_config::BankConfigImpl,
        order::{snapshot_balances_outside, verify_balances_outside_unchanged},
        rate::{debt_index_of, realized_apr, yield_index_of},
    },
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        BORROW_ORDER_DEFAULT_COOLDOWN_SECONDS, INTEREST_DEFAULT_WINDOW_SECONDS,
        INTEREST_MAX_WINDOW_SECONDS, INTEREST_MIN_WINDOW_SECONDS,
    },
    types::{Bank, BorrowOrder, BorrowOrderRecord, MarginfiAccount, RateReading},
};

/// Where a fill's borrowed funds land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowDestination {
    /// Deposited into a same-mint bank, so they earn while the order holds them.
    Bank(Pubkey),
    /// Sent to the authority's wallet.
    Wallet,
}

/// Take a rate reading of a native bank at `now`; a no-op inside the ring's spacing.
pub fn record_native_reading(bank: &mut Bank, now: i64) -> MarginfiResult {
    let reading = RateReading::new(
        yield_index_of(bank, I80F48::ONE)?,
        debt_index_of(bank, I80F48::ONE)?,
        now,
    )
    .ok_or_else(math_error!())?;
    bank.record_rate_reading(reading);
    Ok(())
}

/// Tokens a native bank can pay out before its liabilities exceed its assets.
pub fn available_liquidity(bank: &Bank) -> MarginfiResult<I80F48> {
    let assets = bank.get_asset_amount(bank.total_asset_shares.into())?;
    let liabilities = bank.get_liability_amount(bank.total_liability_shares.into())?;
    Ok(assets.saturating_sub(liabilities).max(I80F48::ZERO))
}

/// Tokens a bank can still lend under its borrow limit; unbounded when the limit is off.
pub fn remaining_borrow_capacity(bank: &Bank) -> MarginfiResult<I80F48> {
    if !bank.config.is_borrow_limit_active() {
        return Ok(I80F48::MAX);
    }
    let liabilities = bank.get_liability_amount(bank.total_liability_shares.into())?;
    Ok(I80F48::from_num(bank.config.borrow_limit)
        .saturating_sub(liabilities)
        .saturating_sub(I80F48::ONE)
        .max(I80F48::ZERO))
}

/// The borrow APR `bank` has realized since its youngest reading at least `window` seconds old,
/// given its debt index `debt_index_now`. A gap in the readings lengthens the span, never shortens it.
pub fn realized_borrow_apr(
    bank: &Bank,
    window: u32,
    debt_index_now: I80F48,
    now: i64,
) -> MarginfiResult<I80F48> {
    let reading = bank
        .rate_reading_at_least(i64::from(window), now)
        .ok_or(MarginfiError::BorrowOrderHistoryTooShort)?;
    let elapsed = now
        .checked_sub(reading.timestamp)
        .ok_or_else(math_error!())?;
    realized_apr(reading.debt_index(), debt_index_now, elapsed)
}

pub trait BorrowOrderImpl {
    #[allow(clippy::too_many_arguments)]
    fn initialize(
        &mut self,
        marginfi_account: Pubkey,
        authority: Pubkey,
        bank: Pubkey,
        amount: u64,
        open_below_apr: u32,
        close_above_apr: Option<u32>,
        cooldown_seconds: Option<u32>,
        window_seconds: Option<u32>,
        keeper_tip: Option<u64>,
        destination: BorrowDestination,
        bump: u8,
    ) -> MarginfiResult;

    /// Reject a policy that cannot be acted on: nothing to borrow, a zero open level, a window the
    /// bank ring cannot cover, a close level at or under the open one or without a destination.
    fn validate(&self) -> MarginfiResult;

    fn record_fill(&mut self, amount: u64, shares: I80F48, now: i64) -> MarginfiResult;

    /// A repayment of `shares` against an account that held `account_shares` before it. The order
    /// holds at most what the account did; `filled` comes down by the same fraction as the shares.
    fn record_repay(&mut self, shares: I80F48, account_shares: I80F48, now: i64) -> MarginfiResult;
}

impl BorrowOrderImpl for BorrowOrder {
    fn initialize(
        &mut self,
        marginfi_account: Pubkey,
        authority: Pubkey,
        bank: Pubkey,
        amount: u64,
        open_below_apr: u32,
        close_above_apr: Option<u32>,
        cooldown_seconds: Option<u32>,
        window_seconds: Option<u32>,
        keeper_tip: Option<u64>,
        destination: BorrowDestination,
        bump: u8,
    ) -> MarginfiResult {
        self.marginfi_account = marginfi_account;
        self.authority = authority;
        self.bank = bank;
        self.amount = amount;
        self.filled = 0;
        self.open_below_apr = open_below_apr;
        self.close_above_apr = close_above_apr.unwrap_or(0);
        self.cooldown_seconds = cooldown_seconds.unwrap_or(BORROW_ORDER_DEFAULT_COOLDOWN_SECONDS);
        self.window_seconds = window_seconds.unwrap_or(INTEREST_DEFAULT_WINDOW_SECONDS);
        self.keeper_tip = keeper_tip.unwrap_or(0);
        self.last_exec_timestamp = 0;
        match destination {
            BorrowDestination::Wallet => self.flags = BorrowOrder::DESTINATION_WALLET,
            BorrowDestination::Bank(bank) => self.destination_bank = bank,
        }
        self.bump = bump;
        self.validate()
    }

    fn validate(&self) -> MarginfiResult {
        check!(self.amount > 0, MarginfiError::BorrowOrderInvalidConfig);
        check!(
            self.open_below_apr > 0,
            MarginfiError::BorrowOrderInvalidConfig
        );
        check!(
            (INTEREST_MIN_WINDOW_SECONDS..=INTEREST_MAX_WINDOW_SECONDS)
                .contains(&self.window_seconds),
            MarginfiError::BorrowOrderInvalidConfig
        );
        if self.has_close_side() {
            // The two levels must never both hold for one rate, and a close repays from the
            // destination bank, so it needs one.
            check!(
                self.close_above_apr > self.open_below_apr,
                MarginfiError::BorrowOrderInvalidConfig
            );
            check!(self.to_bank(), MarginfiError::BorrowOrderNoCloseSide);
        }
        // Exactly one destination, and never the borrow bank itself: one balance cannot hold both
        // sides, so redeploying there would net the position out.
        check!(
            self.to_wallet() != self.to_bank(),
            MarginfiError::BorrowOrderInvalidConfig
        );
        check!(
            self.destination_bank != self.bank,
            MarginfiError::BorrowOrderInvalidConfig
        );
        Ok(())
    }

    fn record_fill(&mut self, amount: u64, shares: I80F48, now: i64) -> MarginfiResult {
        check!(
            amount > 0 && amount <= self.remaining(),
            MarginfiError::BorrowOrderExceedsRemaining
        );
        self.filled = self.filled.checked_add(amount).ok_or_else(math_error!())?;
        self.liability_shares = I80F48::from(self.liability_shares)
            .checked_add(shares)
            .ok_or_else(math_error!())?
            .into();
        self.last_exec_timestamp = now;
        Ok(())
    }

    fn record_repay(&mut self, shares: I80F48, account_shares: I80F48, now: i64) -> MarginfiResult {
        let tracked = I80F48::from(self.liability_shares);
        let held = tracked.min(account_shares);
        if shares >= held {
            self.filled = 0;
            self.liability_shares = I80F48::ZERO.into();
        } else {
            let left = held.checked_sub(shares).ok_or_else(math_error!())?;
            let principal = I80F48::from_num(self.filled)
                .checked_mul(left)
                .ok_or_else(math_error!())?
                .checked_div(tracked)
                .ok_or_else(math_error!())?;
            self.filled = principal.round().to_num();
            self.liability_shares = left.into();
        }
        self.last_exec_timestamp = now;
        Ok(())
    }
}

pub trait BorrowOrderRecordImpl {
    /// Snapshot every active balance outside `excluded` (the order's two banks), so `end` can prove
    /// the fill touched nothing else.
    fn snapshot_others(&mut self, account: &MarginfiAccount, excluded: &[Pubkey])
        -> MarginfiResult;

    /// Every snapshotted balance still holds its side and shares, and no active balance exists
    /// outside the snapshot and `excluded`.
    fn verify_others_unchanged(
        &self,
        account: &MarginfiAccount,
        excluded: &[Pubkey],
    ) -> MarginfiResult;
}

impl BorrowOrderRecordImpl for BorrowOrderRecord {
    fn snapshot_others(
        &mut self,
        account: &MarginfiAccount,
        excluded: &[Pubkey],
    ) -> MarginfiResult {
        self.active_balance_count =
            snapshot_balances_outside(&mut self.balance_states, account, |bank| {
                excluded.contains(bank)
            })?;
        Ok(())
    }

    fn verify_others_unchanged(
        &self,
        account: &MarginfiAccount,
        excluded: &[Pubkey],
    ) -> MarginfiResult {
        verify_balances_outside_unchanged(
            &self.balance_states[..self.active_balance_count as usize],
            account,
            |bank| excluded.contains(bank),
            MarginfiError::BorrowOrderUntrackedBalance,
        )
    }
}

/// The order arithmetic a fill moves, and the rate read off the bank's ring.
#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;
    use marginfi_type_crate::types::{milli_to_u32, RateReading};

    const YEAR: i64 = 31_536_000;
    const WINDOW: u32 = INTEREST_MIN_WINDOW_SECONDS;

    fn milli(bps: u32) -> u32 {
        milli_to_u32(I80F48::from_num(bps) / I80F48::from_num(10_000))
    }

    fn place(
        amount: u64,
        open: u32,
        close: Option<u32>,
        window: Option<u32>,
        dest: BorrowDestination,
    ) -> MarginfiResult<BorrowOrder> {
        let mut o = BorrowOrder::zeroed();
        o.initialize(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            amount,
            open,
            close,
            Some(0),
            window,
            None,
            dest,
            0,
        )?;
        Ok(o)
    }

    fn order(open_bps: u32) -> BorrowOrder {
        place(
            1_000,
            milli(open_bps),
            None,
            None,
            BorrowDestination::Bank(Pubkey::new_unique()),
        )
        .unwrap()
    }

    /// A bank whose ring holds one reading at index 1, `age` seconds before `now`.
    fn bank_with_reading(index: I80F48, age: i64, now: i64) -> Bank {
        let mut bank = Bank::zeroed();
        bank.record_rate_reading(RateReading::new(I80F48::ONE, index, now - age).unwrap());
        bank
    }

    /// Growth is annualized over the reading's actual age, which is the youngest reading at least
    /// a window old; a reading older than the window lengthens the span.
    #[test]
    fn the_realized_rate_spans_the_youngest_reading_at_least_a_window_old() {
        let now = 1_700_000_000;
        let per_year = |age: i64| I80F48::from_num(YEAR) / I80F48::from_num(age);

        let bank = bank_with_reading(I80F48::ONE, i64::from(WINDOW), now);
        assert_eq!(
            realized_borrow_apr(&bank, WINDOW, I80F48::from_num(1.0625), now).unwrap(),
            I80F48::from_num(0.0625) * per_year(i64::from(WINDOW))
        );

        let older = bank_with_reading(I80F48::ONE, 2 * i64::from(WINDOW), now);
        assert_eq!(
            realized_borrow_apr(&older, WINDOW, I80F48::from_num(1.0625), now).unwrap(),
            I80F48::from_num(0.0625) * per_year(2 * i64::from(WINDOW))
        );
    }

    #[test]
    fn a_ring_with_no_reading_old_enough_has_no_measurement() {
        let now = 1_700_000_000;
        let young = bank_with_reading(I80F48::ONE, i64::from(WINDOW) - 1, now);
        assert!(realized_borrow_apr(&young, WINDOW, I80F48::ONE, now).is_err());
        assert!(realized_borrow_apr(&Bank::zeroed(), WINDOW, I80F48::ONE, now).is_err());
    }

    #[test]
    fn a_repayment_reduces_the_principal_by_the_share_of_the_debt_it_retired() {
        let mut o = order(500);
        // Shares are booked at an index of 1.25, so 1,000 tokens are 800 shares.
        o.record_fill(1_000, I80F48::from_num(800), 10).unwrap();
        // 320 of 800 shares is 40% of the debt, interest included, so 40% of the principal.
        o.record_repay(I80F48::from_num(320), I80F48::from_num(800), 20)
            .unwrap();
        assert_eq!(o.filled, 600);
        assert_eq!(I80F48::from(o.liability_shares), I80F48::from_num(480));
        assert_eq!(o.last_exec_timestamp, 20);
        // A repayment of everything held clears both, however interest inflated it.
        o.record_repay(I80F48::from_num(481), I80F48::from_num(480), 30)
            .unwrap();
        assert_eq!(o.filled, 0);
        assert_eq!(I80F48::from(o.liability_shares), I80F48::ZERO);
        assert_eq!(o.remaining(), 1_000);
    }

    /// The user repaid 400 of the order's 1,000 shares by hand, so the order holds 600 at most.
    #[test]
    fn a_repayment_after_a_hand_repayment_counts_from_what_the_account_still_owes() {
        let mut o = order(500);
        o.record_fill(1_000, I80F48::from_num(1_000), 10).unwrap();
        // Half of the 600 the account still owes: the principal follows the same ratio.
        o.record_repay(I80F48::from_num(300), I80F48::from_num(600), 20)
            .unwrap();
        assert_eq!(o.filled, 300);
        assert_eq!(I80F48::from(o.liability_shares), I80F48::from_num(300));
        // The rest of it: the order is empty even though it never saw the hand repayment.
        o.record_repay(I80F48::from_num(300), I80F48::from_num(300), 30)
            .unwrap();
        assert_eq!(o.filled, 0);
        assert_eq!(I80F48::from(o.liability_shares), I80F48::ZERO);
    }

    #[test]
    fn fills_move_the_remainder_and_stay_bounded() {
        let mut o = order(500);
        assert_eq!(o.remaining(), 1_000);

        o.record_fill(400, I80F48::from_num(400), 10).unwrap();
        assert_eq!(o.filled, 400);
        assert_eq!(o.remaining(), 600);
        assert_eq!(o.last_exec_timestamp, 10);

        assert!(o.record_fill(601, I80F48::ONE, 20).is_err());
        assert!(o.record_fill(0, I80F48::ZERO, 20).is_err());
        o.record_fill(600, I80F48::from_num(600), 20).unwrap();
        assert_eq!(o.remaining(), 0);
        assert_eq!(I80F48::from(o.liability_shares), I80F48::from_num(1_000));
    }

    #[test]
    fn initialize_defaults_the_policy_and_rejects_it_out_of_range() {
        let to_bank = BorrowDestination::Bank(Pubkey::new_unique());
        let o = place(1_000, milli(500), None, None, to_bank).unwrap();
        assert_eq!(o.window_seconds, INTEREST_DEFAULT_WINDOW_SECONDS);
        assert_eq!(o.cooldown_seconds, 0);
        assert!(o.to_bank() && !o.to_wallet());

        assert!(place(0, milli(500), None, None, to_bank).is_err());
        assert!(place(1_000, 0, None, None, to_bank).is_err());
        assert!(place(1_000, milli(500), None, Some(WINDOW - 1), to_bank).is_err());
        assert!(place(
            1_000,
            milli(500),
            None,
            Some(INTEREST_MAX_WINDOW_SECONDS + 1),
            to_bank
        )
        .is_err());
        // A close level must sit above the open level and needs a destination to repay from.
        assert!(place(1_000, milli(500), Some(milli(500)), None, to_bank).is_err());
        assert!(place(1_000, milli(500), Some(milli(1_500)), None, to_bank).is_ok());
        assert!(place(
            1_000,
            milli(500),
            Some(milli(1_500)),
            None,
            BorrowDestination::Wallet
        )
        .is_err());
        assert!(place(1_000, milli(500), None, None, BorrowDestination::Wallet).is_ok());

        // Redeploying into the borrow bank itself would net the position out.
        let bank = Pubkey::new_unique();
        let mut into_self = BorrowOrder::zeroed();
        assert!(into_self
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                bank,
                1_000,
                milli(500),
                None,
                Some(0),
                None,
                None,
                BorrowDestination::Bank(bank),
                0,
            )
            .is_err());
    }
}
