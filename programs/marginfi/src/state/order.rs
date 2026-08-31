use crate::{
    check, check_eq, constants::MAX_ORDER_SLIPPAGE, errors::MarginfiError, math_error,
    prelude::MarginfiResult, state::marginfi_account::LendingAccountImpl,
    state::rate::realized_apr,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        INTEREST_ANCHOR_MAX_AGE_WINDOWS, INTEREST_DEFAULT_PATIENCE_SECONDS,
        INTEREST_DEFAULT_WINDOW_SECONDS, INTEREST_MAX_PATIENCE_SECONDS,
        INTEREST_MAX_WINDOW_SECONDS, INTEREST_MIN_WINDOW_SECONDS, ORDER_ACTIVE_TAGS,
        SECONDS_PER_YEAR,
    },
    types::{
        u32_to_milli, BalanceSide, ExecuteOrderBalanceRecord, ExecuteOrderRecord,
        InterestTriggerConfig, MarginfiAccount, Order, OrderTrigger, OrderTriggerType,
        WrappedI80F48, MAX_EXECUTE_RECORD_BALANCES,
    },
};

pub trait OrderImpl {
    fn initialize(
        &mut self,
        marginfi_account: Pubkey,
        trigger: OrderTrigger,
        interest: Option<InterestTriggerConfig>,
        tags: [u16; ORDER_ACTIVE_TAGS],
        bump: u8,
        current_timestamp: i64,
    ) -> MarginfiResult;

    /// Rotate a new anchor in, displacing the current one. Both indices must come from banks
    /// accrued in this instruction, or the span they later measure starts before `now`.
    fn set_interest_anchor(&mut self, asset_index: I80F48, debt_index: I80F48, now: i64);

    /// Seconds since the current anchor was taken. `None` when the order was never armed.
    fn interest_anchor_age(&self, now: i64) -> Option<i64>;

    /// Whether the current anchor is old enough to be rotated out.
    fn interest_window_elapsed(&self, now: i64) -> bool;

    /// `(asset_index, debt_index, elapsed)` from the OLDER anchor aged between one and
    /// `INTEREST_ANCHOR_MAX_AGE_WINDOWS` windows, so an arm cannot displace a matured measurement.
    fn interest_anchor(&self, now: i64) -> Option<(I80F48, I80F48, i64)>;

    /// Net carry for the pair in USD per year, negative when interest is a net cost. `liabs` carries
    /// the accrued premium receivable, which then also pays the borrow rate (accepted overstatement).
    fn realized_carry(
        &self,
        anchor: (I80F48, I80F48, i64),
        asset_index: I80F48,
        debt_index: I80F48,
        assets: I80F48,
        liabs: I80F48,
        premium_apr: I80F48,
    ) -> MarginfiResult<I80F48>;

    /// Whether `carry` clears the arming margin: an annualized loss of at least
    /// `interest_min_negative_apr` measured against the lend leg.
    fn interest_condition_met(&self, carry: I80F48, assets: I80F48) -> MarginfiResult<bool>;

    /// USD the unwind may cost: what the pair loses to `carry` over `interest_patience_seconds`.
    fn interest_allowed_cost(&self, carry: I80F48) -> MarginfiResult<I80F48>;
}

impl OrderImpl for Order {
    fn initialize(
        &mut self,
        marginfi_account: Pubkey,
        trigger: OrderTrigger,
        interest: Option<InterestTriggerConfig>,
        tags: [u16; ORDER_ACTIVE_TAGS],
        bump: u8,
        current_timestamp: i64,
    ) -> MarginfiResult {
        self.marginfi_account = marginfi_account;
        match trigger {
            OrderTrigger::StopLoss {
                threshold,
                max_slippage,
            } => {
                self.trigger = OrderTriggerType::StopLoss;
                self.stop_loss = threshold;
                self.max_slippage = max_slippage;
                self.take_profit = WrappedI80F48::default();
                // Threshold must be > 0
                let val: I80F48 = self.stop_loss.into();
                check!(
                    val > I80F48::ZERO,
                    MarginfiError::InvalidOrderTakeProfitOrStopLoss
                );
            }
            OrderTrigger::TakeProfit {
                threshold,
                max_slippage,
            } => {
                self.trigger = OrderTriggerType::TakeProfit;
                self.take_profit = threshold;
                self.max_slippage = max_slippage;
                self.stop_loss = WrappedI80F48::default();
                // Threshold must be > 0
                let val: I80F48 = self.take_profit.into();
                check!(
                    val > I80F48::ZERO,
                    MarginfiError::InvalidOrderTakeProfitOrStopLoss
                );
            }
            OrderTrigger::Both {
                stop_loss,
                take_profit,
                max_slippage,
            } => {
                self.trigger = OrderTriggerType::Both;
                self.stop_loss = stop_loss;
                self.take_profit = take_profit;
                self.max_slippage = max_slippage;
                // Both thresholds must be > 0 && tp > sl
                let sl: I80F48 = self.stop_loss.into();
                let tp: I80F48 = self.take_profit.into();
                check!(
                    sl > I80F48::ZERO && tp > sl,
                    MarginfiError::InvalidOrderTakeProfitOrStopLoss
                );
            }
        }

        // Orders are capped at MAX_ORDER_SLIPPAGE. Stop-loss execution is also gated by maintenance
        // health. Take-profit execution is additionally bounded by ORDER_EXECUTION_MAX_FEE, which
        // usually dominates the slippage constraint at this cap,
        check!(
            self.max_slippage <= MAX_ORDER_SLIPPAGE,
            MarginfiError::SlippageTooHigh
        );

        if let Some(config) = interest {
            let window = config
                .window_seconds
                .unwrap_or(INTEREST_DEFAULT_WINDOW_SECONDS);
            let patience = config
                .patience_seconds
                .unwrap_or(INTEREST_DEFAULT_PATIENCE_SECONDS);
            check!(
                (INTEREST_MIN_WINDOW_SECONDS..=INTEREST_MAX_WINDOW_SECONDS).contains(&window),
                MarginfiError::OrderInterestInvalidConfig
            );
            check!(
                (1..=INTEREST_MAX_PATIENCE_SECONDS).contains(&patience),
                MarginfiError::OrderInterestInvalidConfig
            );
            self.interest_window_seconds = window;
            self.interest_patience_seconds = patience;
            self.interest_min_negative_apr = config.min_negative_apr.unwrap_or(0);
            self.interest_flags = Order::INTEREST_TRIGGER_ENABLED;
        }

        self.tags = tags;
        self.bump = bump;
        self.created_at = current_timestamp;

        Ok(())
    }

    fn set_interest_anchor(&mut self, asset_index: I80F48, debt_index: I80F48, now: i64) {
        self.interest_prev_asset_index = self.interest_anchor_asset_index;
        self.interest_prev_debt_index = self.interest_anchor_debt_index;
        self.interest_prev_timestamp = self.interest_anchor_timestamp;
        self.interest_anchor_asset_index = asset_index.into();
        self.interest_anchor_debt_index = debt_index.into();
        self.interest_anchor_timestamp = now;
    }

    fn interest_anchor_age(&self, now: i64) -> Option<i64> {
        if self.interest_anchor_timestamp <= 0 {
            return None;
        }
        now.checked_sub(self.interest_anchor_timestamp)
    }

    fn interest_window_elapsed(&self, now: i64) -> bool {
        self.interest_anchor_age(now)
            .is_some_and(|age| age >= i64::from(self.interest_window_seconds))
    }

    fn interest_anchor(&self, now: i64) -> Option<(I80F48, I80F48, i64)> {
        let window = i64::from(self.interest_window_seconds);
        let max_age = window.checked_mul(i64::from(INTEREST_ANCHOR_MAX_AGE_WINDOWS))?;
        let usable = |timestamp: i64, asset: WrappedI80F48, debt: WrappedI80F48| {
            if timestamp <= 0 {
                return None;
            }
            let age = now.checked_sub(timestamp)?;
            (age >= window && age <= max_age).then(|| (asset.into(), debt.into(), age))
        };
        usable(
            self.interest_prev_timestamp,
            self.interest_prev_asset_index,
            self.interest_prev_debt_index,
        )
        .or_else(|| {
            usable(
                self.interest_anchor_timestamp,
                self.interest_anchor_asset_index,
                self.interest_anchor_debt_index,
            )
        })
    }

    fn realized_carry(
        &self,
        anchor: (I80F48, I80F48, i64),
        asset_index: I80F48,
        debt_index: I80F48,
        assets: I80F48,
        liabs: I80F48,
        premium_apr: I80F48,
    ) -> MarginfiResult<I80F48> {
        let (anchor_asset, anchor_debt, elapsed) = anchor;
        let supply_apr = realized_apr(anchor_asset, asset_index, elapsed)?;
        let borrow_apr = realized_apr(anchor_debt, debt_index, elapsed)?
            .checked_add(premium_apr)
            .ok_or_else(math_error!())?;
        let earned = assets.checked_mul(supply_apr).ok_or_else(math_error!())?;
        let paid = liabs.checked_mul(borrow_apr).ok_or_else(math_error!())?;
        earned
            .checked_sub(paid)
            .ok_or_else(math_error!())
            .map_err(Into::into)
    }

    fn interest_condition_met(&self, carry: I80F48, assets: I80F48) -> MarginfiResult<bool> {
        let margin = assets
            .checked_mul(u32_to_milli(self.interest_min_negative_apr))
            .ok_or_else(math_error!())?;
        Ok(carry < -margin)
    }

    fn interest_allowed_cost(&self, carry: I80F48) -> MarginfiResult<I80F48> {
        if carry >= I80F48::ZERO {
            return Ok(I80F48::ZERO);
        }
        carry
            .checked_neg()
            .and_then(|loss| loss.checked_mul(I80F48::from_num(self.interest_patience_seconds)))
            .and_then(|budget| budget.checked_div(SECONDS_PER_YEAR))
            .ok_or_else(math_error!())
            .map_err(Into::into)
    }
}

pub trait ExecuteOrderRecordImpl {
    #[allow(clippy::too_many_arguments)]
    fn initialize(
        &mut self,
        order: Pubkey,
        executor: Pubkey,
        marginfi_account: &MarginfiAccount,
        order_tags: &[u16],
        order_start_health: &I80F48,
        met_conditions: u8,
        interest_carry: I80F48,
    ) -> MarginfiResult;

    fn check_health_and_verify_unchanged(
        &self,
        marginfi_account: &MarginfiAccount,
        closed_order_balances_count: usize,
        order_current_health: &I80F48,
        is_healthy: bool,
    ) -> MarginfiResult;
}

impl ExecuteOrderRecordImpl for ExecuteOrderRecord {
    fn initialize(
        &mut self,
        order: Pubkey,
        executor: Pubkey,
        marginfi_account: &MarginfiAccount,
        order_tags: &[u16],
        order_start_health: &I80F48,
        met_conditions: u8,
        interest_carry: I80F48,
    ) -> MarginfiResult {
        self.order = order;
        self.executor = executor;
        self.balance_states = [ExecuteOrderBalanceRecord::default(); MAX_EXECUTE_RECORD_BALANCES];

        let mut idx: usize = 0;
        let mut inactive_count: u8 = 0;

        for balance in marginfi_account.lending_account.balances.iter() {
            if !balance.is_active() {
                inactive_count += 1;
                continue;
            }

            // Skip balances that belong to this order, they can be changed by the keeper
            if balance.tag != 0 && order_tags.contains(&balance.tag) {
                continue;
            }

            check!(
                idx < self.balance_states.len(),
                MarginfiError::IllegalBalanceState
            );

            let ExecuteOrderBalanceRecord {
                bank,
                tag,
                is_asset,
                shares,
                ..
            } = &mut self.balance_states[idx];

            let side = balance
                .get_side()
                .ok_or_else(|| error!(MarginfiError::IllegalBalanceState))?;

            *bank = balance.bank_pk;
            *tag = balance.tag;
            *is_asset = matches!(side, BalanceSide::Assets) as u8;
            *shares = match side {
                BalanceSide::Assets => balance.asset_shares,
                BalanceSide::Liabilities => balance.liability_shares,
            };

            idx += 1;
        }

        self.order_start_health = (*order_start_health).into();
        self.met_conditions = met_conditions;
        self.interest_carry = interest_carry.into();
        self.inactive_balance_count = inactive_count;
        self.active_balance_count = idx.try_into().unwrap();

        Ok(())
    }

    fn check_health_and_verify_unchanged(
        &self,
        marginfi_account: &MarginfiAccount,
        closed_order_balances_count: usize,
        order_current_health: &I80F48,
        is_healthy: bool,
    ) -> MarginfiResult {
        let order_start_health: I80F48 = self.order_start_health.into();

        check!(
            order_start_health <= *order_current_health || is_healthy,
            MarginfiError::WorseHealthPostExecution
        );

        let inactive_balance_count = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter(|balance| !balance.is_active())
            .count();

        for record in self.balance_states[..self.active_balance_count as usize].iter() {
            let index = marginfi_account
                .lending_account
                .get_balance_index(&record.bank)?;

            let balance = &marginfi_account.lending_account.balances[index];

            let side = balance
                .get_side()
                .ok_or_else(|| error!(MarginfiError::IllegalBalanceState))?;

            let expected_is_asset = matches!(side, BalanceSide::Assets) as u8;

            check_eq!(
                record.is_asset,
                expected_is_asset,
                MarginfiError::IllegalBalanceState
            );

            let expected_shares = match side {
                BalanceSide::Assets => balance.asset_shares,
                BalanceSide::Liabilities => balance.liability_shares,
            };

            check_eq!(
                record.shares,
                expected_shares,
                MarginfiError::IllegalBalanceState
            );
        }

        // This implies that the inactive balances were also not touched.
        // This check is not strictly necessary since deposits & borrows are not allowed
        // during execution and the above has checked that the open balances are
        // still open and the same, but is left here as a sanity check.
        check_eq!(
            self.inactive_balance_count as usize + closed_order_balances_count,
            inactive_balance_count,
            MarginfiError::IllegalBalanceState
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ExecuteOrderRecordImpl;
    use anchor_lang::prelude::Pubkey;
    use bytemuck::Zeroable;
    use fixed::types::I80F48;
    use marginfi_type_crate::types::{Balance, ExecuteOrderRecord, MarginfiAccount};

    fn balance_with_bank_and_tag(bank_byte: u8, tag: u16) -> Balance {
        let mut balance = Balance::zeroed();
        balance.active = 1;
        balance.bank_pk = Pubkey::new_from_array([bank_byte; 32]);
        balance.tag = tag;
        balance.asset_shares = I80F48::from_num(1).into();
        balance.liability_shares = I80F48::ZERO.into();
        balance
    }

    // Catches an edge case in an older implementation where if the tagged banks were in slots
    // 14/15, and none of the other banks were tagged, it would fail to make a ExecuteOrderRecord.
    #[test]
    fn execute_order_record_init_allows_order_balances_sorted_last() {
        let mut account = MarginfiAccount::zeroed();
        let order_tags = [111u16, 222u16];

        let mut slot = 0usize;
        // 14 non-order balances with higher bank pubkeys (take slots 0-13 in descending order).
        for bank_byte in (3u8..=16u8).rev() {
            account.lending_account.balances[slot] = balance_with_bank_and_tag(bank_byte, 0);
            slot += 1;
        }

        // 2 order-tagged balances with lower bank pubkeys (end up in slots 14/15).
        account.lending_account.balances[slot] = balance_with_bank_and_tag(2u8, order_tags[0]);
        slot += 1;
        account.lending_account.balances[slot] = balance_with_bank_and_tag(1u8, order_tags[1]);

        let mut record = ExecuteOrderRecord::zeroed();
        let result = record.initialize(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            &account,
            &order_tags,
            &I80F48::ZERO,
            0,
            I80F48::ZERO,
        );

        assert!(
            result.is_ok(),
            "initialize should succeed when only non-order balances are recorded"
        );
    }
}

/// The carry trigger's arithmetic: what the pair earns net of what it pays, the margin that arms
/// an exit, and the budget patience buys.
#[cfg(test)]
mod interest_trigger {
    use super::OrderImpl;
    use anchor_lang::prelude::Pubkey;
    use bytemuck::Zeroable;
    use fixed::types::I80F48;
    use marginfi_type_crate::constants::{
        INTEREST_ANCHOR_MAX_AGE_WINDOWS, INTEREST_DEFAULT_PATIENCE_SECONDS,
        INTEREST_DEFAULT_WINDOW_SECONDS, INTEREST_MAX_PATIENCE_SECONDS,
        INTEREST_MAX_WINDOW_SECONDS, INTEREST_MIN_WINDOW_SECONDS,
    };
    use marginfi_type_crate::types::{
        milli_to_u32, u32_to_milli, InterestTriggerConfig, Order, OrderTrigger,
    };

    const YEAR: i64 = 31_536_000;
    const ANCHORED_AT: i64 = 1;

    fn f(v: f64) -> I80F48 {
        I80F48::from_num(v)
    }

    /// An order anchored at both indices == 1, so a later index reads directly as its growth.
    fn armed(patience_seconds: u32, min_negative_apr: u32) -> Order {
        let mut order = Order::zeroed();
        order.interest_flags = Order::INTEREST_TRIGGER_ENABLED;
        order.interest_window_seconds = INTEREST_DEFAULT_WINDOW_SECONDS;
        order.interest_patience_seconds = patience_seconds;
        order.interest_min_negative_apr = min_negative_apr;
        order.set_interest_anchor(I80F48::ONE, I80F48::ONE, ANCHORED_AT);
        order
    }

    fn config(window: Option<u32>, patience: Option<u32>) -> InterestTriggerConfig {
        InterestTriggerConfig {
            window_seconds: window,
            patience_seconds: patience,
            min_negative_apr: None,
        }
    }

    fn placed(interest: Option<InterestTriggerConfig>) -> crate::prelude::MarginfiResult<Order> {
        let mut order = Order::zeroed();
        order.initialize(
            Pubkey::new_unique(),
            OrderTrigger::StopLoss {
                threshold: f(100.0).into(),
                max_slippage: 0,
            },
            interest,
            [1, 2],
            0,
            0,
        )?;
        Ok(order)
    }

    /// A full year of span makes each leg's index growth its own annual rate: 6.25% earned on a
    /// 1000 lend against 12.5% paid on a 900 borrow is 62.5 - 112.5.
    #[test]
    fn carry_is_the_pair_rate_difference_and_the_premium_is_a_cost() {
        let order = armed(YEAR as u32, 0);
        let anchor = (I80F48::ONE, I80F48::ONE, YEAR);
        assert_eq!(
            order
                .realized_carry(
                    anchor,
                    f(1.0625),
                    f(1.125),
                    f(1000.0),
                    f(900.0),
                    I80F48::ZERO
                )
                .unwrap(),
            f(-50.0)
        );
        // A 3.125% variable-borrow premium lands on the borrow leg: 900 * 15.625% = 140.625.
        assert_eq!(
            order
                .realized_carry(anchor, f(1.0625), f(1.125), f(1000.0), f(900.0), f(0.03125))
                .unwrap(),
            f(-78.125)
        );
    }

    #[test]
    fn a_profitable_pair_neither_arms_nor_earns_an_exit_budget() {
        let order = armed(YEAR as u32, 0);
        let carry = order
            .realized_carry(
                (I80F48::ONE, I80F48::ONE, YEAR),
                f(1.25),
                f(1.0625),
                f(1000.0),
                f(900.0),
                I80F48::ZERO,
            )
            .unwrap();
        assert_eq!(carry, f(193.75));
        assert!(!order.interest_condition_met(carry, f(1000.0)).unwrap());
        assert_eq!(order.interest_allowed_cost(carry).unwrap(), I80F48::ZERO);
    }

    /// The budget is the loss the pair would take over the patience span, in USD, which is what the
    /// realized unwind cost is measured against.
    #[test]
    fn patience_converts_the_annual_loss_into_a_usd_exit_budget() {
        assert_eq!(
            armed(YEAR as u32, 0)
                .interest_allowed_cost(f(-50.0))
                .unwrap(),
            f(50.0)
        );
        assert_eq!(
            armed(YEAR as u32 / 4, 0)
                .interest_allowed_cost(f(-50.0))
                .unwrap(),
            f(12.5)
        );
    }

    /// The margin is measured against the lend leg and is strict, so a pair losing exactly the
    /// configured rate does not arm.
    #[test]
    fn the_arming_margin_is_strict_and_scales_with_the_lend_leg() {
        let stored = milli_to_u32(f(0.0625));
        let order = armed(YEAR as u32, stored);
        let assets = f(1000.0);
        // The margin round-trips through the u32 encoding, so compare against the stored value.
        let margin = assets * u32_to_milli(stored);
        assert!(!order.interest_condition_met(-margin, assets).unwrap());
        assert!(order
            .interest_condition_met(-margin - I80F48::DELTA, assets)
            .unwrap());

        let no_margin = armed(YEAR as u32, 0);
        assert!(no_margin
            .interest_condition_met(-I80F48::DELTA, assets)
            .unwrap());
        assert!(!no_margin
            .interest_condition_met(I80F48::ZERO, assets)
            .unwrap());
    }

    #[test]
    fn the_window_must_elapse_before_the_anchor_is_a_measurement() {
        let order = armed(YEAR as u32, 0);
        let window = i64::from(INTEREST_DEFAULT_WINDOW_SECONDS);
        assert!(order.interest_anchor(ANCHORED_AT + window - 1).is_none());
        assert!(order.interest_anchor(ANCHORED_AT + window).is_some());

        let mut unarmed = order;
        unarmed.interest_anchor_timestamp = 0;
        assert_eq!(unarmed.interest_anchor_age(ANCHORED_AT + window), None);
        assert!(unarmed.interest_anchor(ANCHORED_AT + window).is_none());
    }

    /// An anchor that has outlived `INTEREST_ANCHOR_MAX_AGE_WINDOWS` stops counting, so a
    /// neglected order cannot fire on a rate regime that has since ended.
    #[test]
    fn an_anchor_past_the_maximum_age_stops_counting() {
        let order = armed(YEAR as u32, 0);
        let window = i64::from(INTEREST_DEFAULT_WINDOW_SECONDS);
        let max_age = window * i64::from(INTEREST_ANCHOR_MAX_AGE_WINDOWS);
        assert!(order.interest_anchor(ANCHORED_AT + max_age).is_some());
        assert!(order.interest_anchor(ANCHORED_AT + max_age + 1).is_none());
    }

    /// Arming rotates and evaluation reads the older anchor, so a re-arm at the very moment an order
    /// came of age cannot reset it.
    #[test]
    fn re_arming_rotates_and_cannot_erase_a_matured_measurement() {
        let mut order = armed(YEAR as u32, 0);
        let window = i64::from(INTEREST_DEFAULT_WINDOW_SECONDS);
        let matured = ANCHORED_AT + window;

        // A griefer arms the instant the standing anchor comes of age.
        order.set_interest_anchor(f(9.0), f(9.0), matured);

        // The displaced anchor still measures, with its own full-window span.
        let (asset, debt, elapsed) = order.interest_anchor(matured).unwrap();
        assert_eq!(asset, I80F48::ONE);
        assert_eq!(debt, I80F48::ONE);
        assert_eq!(elapsed, window);

        // And the fresh one alone is not yet a measurement.
        let mut only_fresh = order;
        only_fresh.interest_prev_timestamp = 0;
        assert!(only_fresh.interest_anchor(matured).is_none());
    }

    #[test]
    fn initialize_defaults_the_policy_and_rejects_it_out_of_range() {
        let order = placed(Some(config(None, None))).unwrap();
        assert!(order.interest_trigger_enabled());
        assert_eq!(
            order.interest_window_seconds,
            INTEREST_DEFAULT_WINDOW_SECONDS
        );
        assert_eq!(
            order.interest_patience_seconds,
            INTEREST_DEFAULT_PATIENCE_SECONDS
        );
        // An unarmed order has no anchor, so it cannot execute until one is written.
        assert_eq!(order.interest_anchor_timestamp, 0);

        assert!(!placed(None).unwrap().interest_trigger_enabled());

        assert!(placed(Some(config(Some(INTEREST_MIN_WINDOW_SECONDS - 1), None))).is_err());
        assert!(placed(Some(config(Some(INTEREST_MAX_WINDOW_SECONDS + 1), None))).is_err());
        assert!(placed(Some(config(Some(INTEREST_MAX_WINDOW_SECONDS), None))).is_ok());
        assert!(placed(Some(config(None, Some(0)))).is_err());
        assert!(placed(Some(config(None, Some(INTEREST_MAX_PATIENCE_SECONDS + 1)))).is_err());
    }
}
