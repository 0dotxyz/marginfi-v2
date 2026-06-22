use crate::{
    check, errors::MarginfiError, prelude::MarginfiResult,
    state::marginfi_account::LendingAccountImpl,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    Balance, BalanceSide, MarginfiAccount, RebalanceOrder, RebalanceRecord, WrappedI80F48,
    MAX_ALLOWED_BANKS,
};

pub trait RebalanceOrderImpl {
    #[allow(clippy::too_many_arguments)]
    fn initialize(
        &mut self,
        marginfi_account: Pubkey,
        authority: Pubkey,
        mint: Pubkey,
        allowed_banks: &[Pubkey],
        min_improvement: WrappedI80F48,
        cooldown_seconds: u64,
        amount: u64,
        bump: u8,
    ) -> MarginfiResult;

    /// Replace the venue allowlist, validating the count and zeroing unused slots.
    fn set_allowed_banks(&mut self, allowed_banks: &[Pubkey]) -> MarginfiResult;
}

impl RebalanceOrderImpl for RebalanceOrder {
    fn initialize(
        &mut self,
        marginfi_account: Pubkey,
        authority: Pubkey,
        mint: Pubkey,
        allowed_banks: &[Pubkey],
        min_improvement: WrappedI80F48,
        cooldown_seconds: u64,
        amount: u64,
        bump: u8,
    ) -> MarginfiResult {
        check!(
            I80F48::from(min_improvement) >= I80F48::ZERO,
            MarginfiError::RebalanceInvalidMinImprovement
        );
        self.marginfi_account = marginfi_account;
        self.authority = authority;
        self.mint = mint;
        self.set_allowed_banks(allowed_banks)?;
        self.min_improvement = min_improvement;
        self.cooldown_seconds = cooldown_seconds;
        self.amount = amount;
        self.last_exec_timestamp = 0;
        self.bump = bump;
        Ok(())
    }

    fn set_allowed_banks(&mut self, allowed_banks: &[Pubkey]) -> MarginfiResult {
        check!(
            (2..=MAX_ALLOWED_BANKS).contains(&allowed_banks.len()),
            MarginfiError::InvalidBalanceCount
        );
        self.allowed_banks = [Pubkey::default(); MAX_ALLOWED_BANKS];
        self.allowed_bank_count = allowed_banks.len() as u8;
        for (slot, bank) in self.allowed_banks.iter_mut().zip(allowed_banks.iter()) {
            *slot = *bank;
        }
        Ok(())
    }
}

pub trait RebalanceRecordImpl {
    #[allow(clippy::too_many_arguments)]
    fn initialize(
        &mut self,
        order: Pubkey,
        executor: Pubkey,
        src_bank: Pubkey,
        dst_bank: Pubkey,
        pre_src_value: I80F48,
        pre_dst_value: I80F48,
        src_rate_pre: I80F48,
        dst_rate_pre: I80F48,
        marginfi_account: &MarginfiAccount,
    ) -> MarginfiResult;

    /// Verify every snapshotted non-{src,dst} balance is unchanged (side + shares).
    fn verify_others_unchanged(&self, marginfi_account: &MarginfiAccount) -> MarginfiResult;
}

impl RebalanceRecordImpl for RebalanceRecord {
    /// Record the {src,dst} pre-move values/rates and snapshot every OTHER active balance, so
    /// `end_rebalance` can prove value conservation and that untouched balances are byte-identical.
    fn initialize(
        &mut self,
        order: Pubkey,
        executor: Pubkey,
        src_bank: Pubkey,
        dst_bank: Pubkey,
        pre_src_value: I80F48,
        pre_dst_value: I80F48,
        src_rate_pre: I80F48,
        dst_rate_pre: I80F48,
        marginfi_account: &MarginfiAccount,
    ) -> MarginfiResult {
        self.order = order;
        self.executor = executor;
        self.src_bank = src_bank;
        self.dst_bank = dst_bank;
        self.pre_src_value = pre_src_value.into();
        self.pre_dst_value = pre_dst_value.into();
        self.src_rate_pre = src_rate_pre.into();
        self.dst_rate_pre = dst_rate_pre.into();

        let mut active: u8 = 0;
        let mut inactive: u8 = 0;
        for balance in marginfi_account.lending_account.balances.iter() {
            if !balance.is_active() {
                inactive = inactive.saturating_add(1);
                continue;
            }
            if balance.bank_pk == src_bank || balance.bank_pk == dst_bank {
                continue;
            }
            let side = balance
                .get_side()
                .ok_or(MarginfiError::IllegalBalanceState)?;
            let slot = self
                .balance_states
                .get_mut(active as usize)
                .ok_or(MarginfiError::IllegalBalanceState)?;
            slot.bank = balance.bank_pk;
            slot.is_asset = matches!(side, BalanceSide::Assets) as u8;
            slot.tag = balance.tag;
            slot.shares = if matches!(side, BalanceSide::Assets) {
                balance.asset_shares
            } else {
                balance.liability_shares
            };
            active = active.saturating_add(1);
        }
        self.active_balance_count = active;
        self.inactive_balance_count = inactive;
        Ok(())
    }

    fn verify_others_unchanged(&self, marginfi_account: &MarginfiAccount) -> MarginfiResult {
        for rec in self.balance_states[..self.active_balance_count as usize].iter() {
            let idx = marginfi_account
                .lending_account
                .get_balance_index(&rec.bank)?;
            let balance: &Balance = &marginfi_account.lending_account.balances[idx];
            let side = balance
                .get_side()
                .ok_or(MarginfiError::IllegalBalanceState)?;
            check_eq_u8(rec.is_asset, matches!(side, BalanceSide::Assets) as u8)?;
            let now: WrappedI80F48 = if matches!(side, BalanceSide::Assets) {
                balance.asset_shares
            } else {
                balance.liability_shares
            };
            check!(
                I80F48::from(rec.shares) == I80F48::from(now),
                MarginfiError::IllegalBalanceState
            );
        }
        Ok(())
    }
}

#[inline]
fn check_eq_u8(a: u8, b: u8) -> MarginfiResult {
    check!(a == b, MarginfiError::IllegalBalanceState);
    Ok(())
}
