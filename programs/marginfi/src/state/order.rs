use crate::{
    check, check_eq, constants::MAX_BPS, errors::MarginfiError, prelude::MarginfiResult,
    state::marginfi_account::LendingAccountImpl,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::ORDER_ACTIVE_TAGS,
    types::{
        BalanceSide, ExecuteOrderBalanceRecord, ExecuteOrderRecord, MarginfiAccount, Order,
        OrderTrigger, OrderTriggerType, WrappedI80F48, MAX_EXECUTE_RECORD_BALANCES,
    },
};

pub trait OrderImpl {
    fn initialize(
        &mut self,
        marginfi_account: Pubkey,
        trigger: OrderTrigger,
        tags: [u16; ORDER_ACTIVE_TAGS],
        bump: u8,
    ) -> MarginfiResult;
}

impl OrderImpl for Order {
    fn initialize(
        &mut self,
        marginfi_account: Pubkey,
        trigger: OrderTrigger,
        tags: [u16; ORDER_ACTIVE_TAGS],
        bump: u8,
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

        check!(self.max_slippage < MAX_BPS, MarginfiError::InvalidSlippage);

        self.tags = tags;
        self.bump = bump;

        Ok(())
    }
}

pub trait ExecuteOrderRecordImpl {
    fn initialize(
        &mut self,
        order: Pubkey,
        executor: Pubkey,
        marginfi_account: &MarginfiAccount,
        order_tags: &[u16],
        order_start_health: &I80F48,
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
            if balance.tag != 0 && order_tags.iter().any(|t| *t == balance.tag) {
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
        // This check is not strictly necessary since deposits are not allowed
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
        );

        assert!(
            result.is_ok(),
            "initialize should succeed when only non-order balances are recorded"
        );
    }
}
