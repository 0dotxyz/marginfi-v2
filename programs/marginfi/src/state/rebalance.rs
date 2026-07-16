use crate::{
    check, errors::MarginfiError, math_error, prelude::MarginfiResult,
    state::marginfi_account::LendingAccountImpl,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    Balance, BalanceSide, MarginfiAccount, RebalanceMove, RebalanceOrder, RebalanceRecord,
    RebalanceRefBank, WrappedI80F48, MAX_ALLOWED_BANKS, MAX_REBALANCE_BANKS, MAX_REBALANCE_MOVES,
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
        keeper_tip: u64,
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
        keeper_tip: u64,
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
        self.keeper_tip = keeper_tip;
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
    /// Record every referenced bank's start underlying-token amount + the declared moves, and snapshot
    /// every active balance NOT in the referenced set, so `end_rebalance` can reconcile the moves
    /// against real token deltas, prove conservation, and prove untouched balances kept side and shares.
    fn initialize(
        &mut self,
        order: Pubkey,
        executor: Pubkey,
        ref_banks: &[(Pubkey, I80F48)],
        moves: &[RebalanceMove],
        marginfi_account: &MarginfiAccount,
    ) -> MarginfiResult;

    /// The declared moves, sliced to `move_count`.
    fn active_moves(&self) -> &[RebalanceMove];

    /// Reconcile the declared moves against the observed per-bank underlying-token deltas.
    /// `post_underlying[i]` is the end token amount of `ref_banks[i]`. For every referenced bank the net
    /// declared flow (incoming amounts minus outgoing) must equal `post - pre` within `dust`. Returns
    /// `(total_moved, total_source_pre)`: the tokens that landed (sum of positive net deltas) and the
    /// start token amount of the net-source banks (the tip denominator for unlimited orders).
    fn reconcile(
        &self,
        post_underlying: &[I80F48],
        dust: I80F48,
    ) -> MarginfiResult<(I80F48, I80F48)>;

    /// Verify every snapshotted non-referenced balance is unchanged (side + shares).
    fn verify_others_unchanged(&self, marginfi_account: &MarginfiAccount) -> MarginfiResult;
}

impl RebalanceRecordImpl for RebalanceRecord {
    fn initialize(
        &mut self,
        order: Pubkey,
        executor: Pubkey,
        ref_banks: &[(Pubkey, I80F48)],
        moves: &[RebalanceMove],
        marginfi_account: &MarginfiAccount,
    ) -> MarginfiResult {
        check!(
            !ref_banks.is_empty()
                && ref_banks.len() <= MAX_REBALANCE_BANKS
                && !moves.is_empty()
                && moves.len() <= MAX_REBALANCE_MOVES,
            MarginfiError::IllegalBalanceState
        );
        // Every move must reference distinct in-range banks and carry a positive amount.
        for m in moves {
            check!(
                (m.src_index as usize) < ref_banks.len()
                    && (m.dst_index as usize) < ref_banks.len()
                    && m.src_index != m.dst_index
                    && I80F48::from(m.amount) > I80F48::ZERO,
                MarginfiError::IllegalBalanceState
            );
        }
        self.order = order;
        self.executor = executor;
        self.ref_banks = [RebalanceRefBank::default(); MAX_REBALANCE_BANKS];
        for (i, (bank, val)) in ref_banks.iter().enumerate() {
            self.ref_banks[i] = RebalanceRefBank {
                bank: *bank,
                pre_underlying: (*val).into(),
            };
        }
        self.ref_bank_count = ref_banks.len() as u8;
        self.moves = [RebalanceMove::default(); MAX_REBALANCE_MOVES];
        self.moves[..moves.len()].copy_from_slice(moves);
        self.move_count = moves.len() as u8;

        let mut active: u8 = 0;
        for balance in marginfi_account.lending_account.balances.iter() {
            if !balance.is_active() {
                continue;
            }
            if ref_banks.iter().any(|(b, _)| *b == balance.bank_pk) {
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
        Ok(())
    }

    fn active_moves(&self) -> &[RebalanceMove] {
        &self.moves[..self.move_count as usize]
    }

    fn reconcile(
        &self,
        post_underlying: &[I80F48],
        dust: I80F48,
    ) -> MarginfiResult<(I80F48, I80F48)> {
        let n = self.ref_bank_count as usize;
        check!(
            post_underlying.len() == n,
            MarginfiError::IllegalBalanceState
        );
        let mut total_moved = I80F48::ZERO;
        let mut total_source_pre = I80F48::ZERO;
        for (i, post) in post_underlying.iter().enumerate().take(n) {
            let mut declared_net = I80F48::ZERO;
            for m in self.active_moves() {
                let amt = I80F48::from(m.amount);
                if m.dst_index as usize == i {
                    declared_net = declared_net.checked_add(amt).ok_or_else(math_error!())?;
                }
                if m.src_index as usize == i {
                    declared_net = declared_net.checked_sub(amt).ok_or_else(math_error!())?;
                }
            }
            let pre = I80F48::from(self.ref_banks[i].pre_underlying);
            let actual = post.checked_sub(pre).ok_or_else(math_error!())?;
            check!(
                (declared_net.checked_sub(actual).ok_or_else(math_error!())?).abs() <= dust,
                MarginfiError::RebalanceValueLeak
            );
            if actual > I80F48::ZERO {
                total_moved = total_moved.checked_add(actual).ok_or_else(math_error!())?;
            } else if actual < I80F48::ZERO {
                total_source_pre = total_source_pre
                    .checked_add(pre)
                    .ok_or_else(math_error!())?;
            }
        }
        Ok((total_moved, total_source_pre))
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
