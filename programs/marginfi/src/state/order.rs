use crate::{
    check, check_eq, constants::MAX_ORDER_SLIPPAGE, errors::MarginfiError, math_error,
    prelude::MarginfiResult, state::marginfi_account::LendingAccountImpl,
    state::rate::realized_apr,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        INTEREST_DEFAULT_EXIT_BUDGET_SECONDS, INTEREST_DEFAULT_WINDOW_SECONDS,
        INTEREST_MAX_EXIT_BUDGET_SECONDS, INTEREST_MAX_WINDOW_SECONDS, INTEREST_MIN_WINDOW_SECONDS,
        ORDER_ACTIVE_TAGS, SECONDS_PER_YEAR,
    },
    types::{
        u32_to_milli, BalanceSide, ExecuteOrderBalanceRecord, ExecuteOrderRecord,
        InterestTriggerConfig, MarginfiAccount, Order, OrderTrigger, OrderTriggerType,
        WrappedI80F48, MAX_EXECUTE_RECORD_BALANCES,
    },
};

/// One leg's share index at the bank reading it is measured from and now, `elapsed` seconds later.
pub struct LegSpan {
    pub start: I80F48,
    pub end: I80F48,
    pub elapsed: i64,
}

impl LegSpan {
    /// The time-weighted rate realized over the span.
    pub fn apr(&self) -> MarginfiResult<I80F48> {
        realized_apr(self.start, self.end, self.elapsed)
    }
}

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

    /// Net carry for the pair in USD per year, negative when interest is a net cost. `liabs` carries
    /// the accrued premium receivable, which then also pays the borrow rate (accepted overstatement).
    fn realized_carry(
        &self,
        asset: &LegSpan,
        debt: &LegSpan,
        assets: I80F48,
        liabs: I80F48,
        premium_apr: I80F48,
    ) -> MarginfiResult<I80F48>;

    /// Whether `carry` clears the trigger margin: an annualized loss of at least
    /// `interest_min_negative_apr` measured against the lend leg.
    fn interest_condition_met(&self, carry: I80F48, assets: I80F48) -> MarginfiResult<bool>;

    /// USD the unwind may cost: what the pair loses to `carry` over `interest_exit_budget_seconds`.
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
            let exit_budget = config
                .exit_budget_seconds
                .unwrap_or(INTEREST_DEFAULT_EXIT_BUDGET_SECONDS);
            check!(
                (INTEREST_MIN_WINDOW_SECONDS..=INTEREST_MAX_WINDOW_SECONDS).contains(&window),
                MarginfiError::OrderInterestInvalidConfig
            );
            check!(
                (1..=INTEREST_MAX_EXIT_BUDGET_SECONDS).contains(&exit_budget),
                MarginfiError::OrderInterestInvalidConfig
            );
            self.interest_window_seconds = window;
            self.interest_exit_budget_seconds = exit_budget;
            self.interest_min_negative_apr = config.min_negative_apr.unwrap_or(0);
            self.interest_flags = Order::INTEREST_TRIGGER_ENABLED;
        }

        self.tags = tags;
        self.bump = bump;
        self.created_at = current_timestamp;

        Ok(())
    }

    fn realized_carry(
        &self,
        asset: &LegSpan,
        debt: &LegSpan,
        assets: I80F48,
        liabs: I80F48,
        premium_apr: I80F48,
    ) -> MarginfiResult<I80F48> {
        let supply_apr = asset.apr()?;
        let borrow_apr = debt
            .apr()?
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
            .and_then(|loss| loss.checked_mul(I80F48::from_num(self.interest_exit_budget_seconds)))
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

/// Snapshot every active balance whose bank is not `excluded` into `slots`, in account order.
/// Returns how many were written.
pub fn snapshot_balances_outside(
    slots: &mut [ExecuteOrderBalanceRecord],
    account: &MarginfiAccount,
    excluded: impl Fn(&Pubkey) -> bool,
) -> MarginfiResult<u8> {
    let mut count: u8 = 0;
    for balance in account.lending_account.balances.iter() {
        if !balance.is_active() || excluded(&balance.bank_pk) {
            continue;
        }
        let side = balance
            .get_side()
            .ok_or(MarginfiError::IllegalBalanceState)?;
        let slot = slots
            .get_mut(count as usize)
            .ok_or(MarginfiError::IllegalBalanceState)?;
        slot.bank = balance.bank_pk;
        slot.tag = balance.tag;
        slot.is_asset = matches!(side, BalanceSide::Assets) as u8;
        slot.shares = match side {
            BalanceSide::Assets => balance.asset_shares,
            BalanceSide::Liabilities => balance.liability_shares,
        };
        count = count.saturating_add(1);
    }
    Ok(count)
}

/// Every snapshotted balance still holds its side and shares, and no active balance exists outside
/// the snapshot and `excluded`; a balance the snapshot cannot see reports `untracked_err`.
pub fn verify_balances_outside_unchanged(
    slots: &[ExecuteOrderBalanceRecord],
    account: &MarginfiAccount,
    excluded: impl Fn(&Pubkey) -> bool,
    untracked_err: MarginfiError,
) -> MarginfiResult {
    let untracked = account
        .lending_account
        .balances
        .iter()
        .filter(|b| b.is_active() && !excluded(&b.bank_pk))
        .count();
    check!(untracked == slots.len(), untracked_err);

    for rec in slots.iter() {
        let idx = account.lending_account.get_balance_index(&rec.bank)?;
        let balance = &account.lending_account.balances[idx];
        let side = balance
            .get_side()
            .ok_or(MarginfiError::IllegalBalanceState)?;
        let shares = match side {
            BalanceSide::Assets => balance.asset_shares,
            BalanceSide::Liabilities => balance.liability_shares,
        };
        check!(
            rec.is_asset == matches!(side, BalanceSide::Assets) as u8
                && I80F48::from(rec.shares) == I80F48::from(shares),
            MarginfiError::IllegalBalanceState
        );
    }
    Ok(())
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

#[cfg(test)]
mod interest_trigger {
    use super::{LegSpan, OrderImpl};
    use anchor_lang::prelude::Pubkey;
    use bytemuck::Zeroable;
    use fixed::types::I80F48;
    use marginfi_type_crate::constants::{
        INTEREST_DEFAULT_EXIT_BUDGET_SECONDS, INTEREST_DEFAULT_WINDOW_SECONDS,
        INTEREST_MAX_EXIT_BUDGET_SECONDS, INTEREST_MAX_WINDOW_SECONDS, INTEREST_MIN_WINDOW_SECONDS,
    };
    use marginfi_type_crate::types::{
        milli_to_u32, u32_to_milli, InterestTriggerConfig, Order, OrderTrigger,
    };

    const YEAR: i64 = 31_536_000;

    fn f(v: f64) -> I80F48 {
        I80F48::from_num(v)
    }

    /// A leg whose index grew from 1 to `end` over `elapsed`, so at a year `end - 1` is its rate.
    fn grew(end: f64, elapsed: i64) -> LegSpan {
        LegSpan {
            start: I80F48::ONE,
            end: f(end),
            elapsed,
        }
    }

    fn order(exit_budget_seconds: u32, min_negative_apr: u32) -> Order {
        let mut order = Order::zeroed();
        order.interest_flags = Order::INTEREST_TRIGGER_ENABLED;
        order.interest_window_seconds = INTEREST_DEFAULT_WINDOW_SECONDS;
        order.interest_exit_budget_seconds = exit_budget_seconds;
        order.interest_min_negative_apr = min_negative_apr;
        order
    }

    fn config(window: Option<u32>, exit_budget: Option<u32>) -> InterestTriggerConfig {
        InterestTriggerConfig {
            window_seconds: window,
            exit_budget_seconds: exit_budget,
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

    #[test]
    fn carry_is_the_pair_rate_difference_and_the_premium_is_a_cost() {
        let order = order(YEAR as u32, 0);
        let (asset, debt) = (grew(1.0625, YEAR), grew(1.125, YEAR));
        // 6.25% earned on a 1000 lend against 12.5% paid on a 900 borrow: 62.5 - 112.5.
        assert_eq!(
            order
                .realized_carry(&asset, &debt, f(1000.0), f(900.0), I80F48::ZERO)
                .unwrap(),
            f(-50.0)
        );
        // A 3.125% variable-borrow premium lands on the borrow leg: 900 * 15.625% = 140.625.
        assert_eq!(
            order
                .realized_carry(&asset, &debt, f(1000.0), f(900.0), f(0.03125))
                .unwrap(),
            f(-78.125)
        );
    }

    #[test]
    fn each_leg_annualizes_over_its_own_span() {
        // The same 6.25% growth over half a year is a 12.5% rate: 62.5 - 112.5.
        assert_eq!(
            order(YEAR as u32, 0)
                .realized_carry(
                    &grew(1.0625, YEAR),
                    &grew(1.0625, YEAR / 2),
                    f(1000.0),
                    f(900.0),
                    I80F48::ZERO
                )
                .unwrap(),
            f(-50.0)
        );
    }

    #[test]
    fn a_profitable_pair_neither_fires_nor_earns_an_exit_budget() {
        let order = order(YEAR as u32, 0);
        let carry = order
            .realized_carry(
                &grew(1.25, YEAR),
                &grew(1.0625, YEAR),
                f(1000.0),
                f(900.0),
                I80F48::ZERO,
            )
            .unwrap();
        assert_eq!(carry, f(193.75));
        assert!(!order.interest_condition_met(carry, f(1000.0)).unwrap());
        assert_eq!(order.interest_allowed_cost(carry).unwrap(), I80F48::ZERO);
    }

    #[test]
    fn the_budget_span_converts_the_annual_loss_into_usd() {
        assert_eq!(
            order(YEAR as u32, 0)
                .interest_allowed_cost(f(-50.0))
                .unwrap(),
            f(50.0)
        );
        assert_eq!(
            order(YEAR as u32 / 4, 0)
                .interest_allowed_cost(f(-50.0))
                .unwrap(),
            f(12.5)
        );
    }

    #[test]
    fn the_trigger_margin_is_strict_and_scales_with_the_lend_leg() {
        let stored = milli_to_u32(f(0.0625));
        let order = order(YEAR as u32, stored);
        let assets = f(1000.0);
        // The margin round-trips through the u32 encoding, so compare against the stored value.
        let margin = assets * u32_to_milli(stored);
        assert!(!order.interest_condition_met(-margin, assets).unwrap());
        assert!(order
            .interest_condition_met(-margin - I80F48::DELTA, assets)
            .unwrap());

        let no_margin = self::order(YEAR as u32, 0);
        assert!(no_margin
            .interest_condition_met(-I80F48::DELTA, assets)
            .unwrap());
        assert!(!no_margin
            .interest_condition_met(I80F48::ZERO, assets)
            .unwrap());
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
            order.interest_exit_budget_seconds,
            INTEREST_DEFAULT_EXIT_BUDGET_SECONDS
        );

        assert!(!placed(None).unwrap().interest_trigger_enabled());

        assert!(placed(Some(config(Some(INTEREST_MIN_WINDOW_SECONDS - 1), None))).is_err());
        assert!(placed(Some(config(Some(INTEREST_MAX_WINDOW_SECONDS + 1), None))).is_err());
        assert!(placed(Some(config(Some(INTEREST_MAX_WINDOW_SECONDS), None))).is_ok());
        assert!(placed(Some(config(None, Some(0)))).is_err());
        assert!(placed(Some(config(
            None,
            Some(INTEREST_MAX_EXIT_BUDGET_SECONDS + 1)
        )))
        .is_err());
    }
}
