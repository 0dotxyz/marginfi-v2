use crate::{
    check, errors::MarginfiError, math_error, prelude::MarginfiResult,
    state::marginfi_account::LendingAccountImpl,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::constants::{EXP_10_I80F48, REBALANCE_CONSERVATION_DUST_ATOMS};
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
    /// Record every referenced bank's start underlying-token amount and order tag + the declared
    /// moves, and snapshot every non-empty active balance NOT in the referenced set, so
    /// `end_rebalance` can reconcile the moves against real token deltas, prove conservation, and
    /// prove untouched balances kept side and shares. The referenced set is the order's whole
    /// allowlist, so a bank no move touches is still recorded and must return a zero net delta.
    fn initialize(
        &mut self,
        order: Pubkey,
        marginfi_account_key: Pubkey,
        executor: Pubkey,
        ref_banks: &[RebalanceRefBank],
        pre_rates: &[I80F48],
        moves: &[RebalanceMove],
        marginfi_account: &MarginfiAccount,
    ) -> MarginfiResult;

    /// The declared moves, sliced to `move_count`.
    fn active_moves(&self) -> &[RebalanceMove];

    /// The tolerance for this rebalance's conservation checks, in whole-token UI units:
    /// `REBALANCE_CONSERVATION_DUST_ATOMS` per declared move, scaled by the widest venue multiplier
    /// (venues settle in whole accounting tokens, each worth `multiplier` native units).
    fn conservation_dust(
        &self,
        mint_decimals: u8,
        venue_multiplier: I80F48,
    ) -> MarginfiResult<I80F48>;

    /// Reconcile the declared moves against the observed per-bank underlying-token deltas.
    /// `post_underlying[i]` is the end token amount of `ref_banks[i]`. For every referenced bank the net
    /// declared flow (incoming amounts minus outgoing) must equal `post - pre` within the conservation
    /// dust. Returns `(total_moved, total_ref_pre, dust)`: the tokens that landed (sum of positive net
    /// deltas), the start token amount summed across ALL referenced banks (the tip denominator, stable
    /// against how the keeper splits the move across banks), and the tolerance applied (reused by the
    /// caller's budget-cap cushion).
    fn reconcile(
        &self,
        post_underlying: &[I80F48],
        mint_decimals: u8,
        venue_multiplier: I80F48,
    ) -> MarginfiResult<(I80F48, I80F48, I80F48)>;

    /// Verify the non-referenced balance set is exactly what it was at start: every snapshotted
    /// balance still holds its side, order tag, and shares, and no non-empty balance outside the
    /// referenced set was added.
    fn verify_others_unchanged(&self, marginfi_account: &MarginfiAccount) -> MarginfiResult;

    /// Carry each drained tagged source's order tag onto its destination balance, and verify every
    /// other tagged referenced balance still holds its tag.
    fn carry_tags(&self, marginfi_account: &mut MarginfiAccount) -> MarginfiResult;
}

impl RebalanceRecordImpl for RebalanceRecord {
    fn initialize(
        &mut self,
        order: Pubkey,
        marginfi_account_key: Pubkey,
        executor: Pubkey,
        ref_banks: &[RebalanceRefBank],
        pre_rates: &[I80F48],
        moves: &[RebalanceMove],
        marginfi_account: &MarginfiAccount,
    ) -> MarginfiResult {
        check!(
            pre_rates.len() == ref_banks.len(),
            MarginfiError::IllegalBalanceState
        );
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
        self.marginfi_account = marginfi_account_key;
        self.executor = executor;
        self.ref_banks = [RebalanceRefBank::default(); MAX_REBALANCE_BANKS];
        self.ref_banks[..ref_banks.len()].copy_from_slice(ref_banks);
        self.ref_bank_count = ref_banks.len() as u8;
        self.pre_rate = [WrappedI80F48::default(); MAX_REBALANCE_BANKS];
        for (slot, rate) in self.pre_rate.iter_mut().zip(pre_rates.iter()) {
            *slot = (*rate).into();
        }
        self.moves = [RebalanceMove::default(); MAX_REBALANCE_MOVES];
        self.moves[..moves.len()].copy_from_slice(moves);
        self.move_count = moves.len() as u8;

        let mut active: u8 = 0;
        for balance in marginfi_account.lending_account.balances.iter() {
            if !balance.is_active() {
                continue;
            }
            if ref_banks.iter().any(|r| r.bank == balance.bank_pk) {
                continue;
            }
            let Some(side) = balance.get_side() else {
                continue;
            };
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

    fn conservation_dust(
        &self,
        mint_decimals: u8,
        venue_multiplier: I80F48,
    ) -> MarginfiResult<I80F48> {
        REBALANCE_CONSERVATION_DUST_ATOMS
            .checked_mul(I80F48::from_num(self.move_count))
            .ok_or_else(math_error!())?
            .checked_mul(venue_multiplier)
            .ok_or_else(math_error!())?
            .checked_div(EXP_10_I80F48[mint_decimals as usize])
            .ok_or_else(math_error!())
            .map_err(Into::into)
    }

    fn reconcile(
        &self,
        post_underlying: &[I80F48],
        mint_decimals: u8,
        venue_multiplier: I80F48,
    ) -> MarginfiResult<(I80F48, I80F48, I80F48)> {
        let n = self.ref_bank_count as usize;
        check!(
            post_underlying.len() == n,
            MarginfiError::IllegalBalanceState
        );
        let dust = self.conservation_dust(mint_decimals, venue_multiplier)?;
        let mut total_moved = I80F48::ZERO;
        let mut total_ref_pre = I80F48::ZERO;
        let mut total_actual = I80F48::ZERO;
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
            total_actual = total_actual.checked_add(actual).ok_or_else(math_error!())?;
            total_ref_pre = total_ref_pre.checked_add(pre).ok_or_else(math_error!())?;
            if actual > I80F48::ZERO {
                total_moved = total_moved.checked_add(actual).ok_or_else(math_error!())?;
            }
        }

        check!(total_actual >= -dust, MarginfiError::RebalanceValueLeak);
        Ok((total_moved, total_ref_pre, dust))
    }

    fn verify_others_unchanged(&self, marginfi_account: &MarginfiAccount) -> MarginfiResult {
        // A balance added mid-sandwich occupies no snapshot slot, so the loop below cannot see it.
        let ref_banks = &self.ref_banks[..self.ref_bank_count as usize];
        let untracked = marginfi_account
            .lending_account
            .balances
            .iter()
            .filter(|b| {
                b.is_active()
                    && b.get_side().is_some()
                    && !ref_banks.iter().any(|r| r.bank == b.bank_pk)
            })
            .count();
        check!(
            untracked == self.active_balance_count as usize,
            MarginfiError::RebalanceUntrackedBalance
        );

        for rec in self.balance_states[..self.active_balance_count as usize].iter() {
            let idx = marginfi_account
                .lending_account
                .get_balance_index(&rec.bank)?;
            let balance: &Balance = &marginfi_account.lending_account.balances[idx];
            let side = balance
                .get_side()
                .ok_or(MarginfiError::IllegalBalanceState)?;
            check_eq_u8(rec.is_asset, matches!(side, BalanceSide::Assets) as u8)?;
            check!(rec.tag == balance.tag, MarginfiError::IllegalBalanceState);
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

    fn carry_tags(&self, marginfi_account: &mut MarginfiAccount) -> MarginfiResult {
        let balances = &mut marginfi_account.lending_account.balances;
        let slot_of = |balances: &[Balance], bank: &Pubkey| {
            balances
                .iter()
                .position(|b| b.is_active() && b.bank_pk == *bank)
        };
        let n = self.ref_bank_count as usize;
        for (i, rb) in self.ref_banks[..n].iter().enumerate() {
            if rb.tag == 0 {
                continue;
            }
            match self
                .active_moves()
                .iter()
                .find(|m| m.src_index as usize == i)
            {
                None => {
                    let idx =
                        slot_of(balances, &rb.bank).ok_or(MarginfiError::IllegalBalanceState)?;
                    check!(
                        balances[idx].tag == rb.tag,
                        MarginfiError::IllegalBalanceState
                    );
                }
                Some(m) => {
                    check!(
                        slot_of(balances, &rb.bank).is_none(),
                        MarginfiError::RebalanceTaggedBalanceSplit
                    );
                    let dst = &self.ref_banks[m.dst_index as usize].bank;
                    let idx = slot_of(balances, dst).ok_or(MarginfiError::IllegalBalanceState)?;
                    check!(balances[idx].tag == 0, MarginfiError::IllegalBalanceState);
                    balances[idx].tag = rb.tag;
                }
            }
        }
        Ok(())
    }
}

#[inline]
fn check_eq_u8(a: u8, b: u8) -> MarginfiResult {
    check!(a == b, MarginfiError::IllegalBalanceState);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::RebalanceRecordImpl;
    use crate::errors::MarginfiError;
    use anchor_lang::prelude::Pubkey;
    use bytemuck::Zeroable;
    use fixed::types::I80F48;
    use marginfi_type_crate::constants::EXP_10_I80F48;
    use marginfi_type_crate::types::{
        Balance, MarginfiAccount, RebalanceMove, RebalanceRecord, RebalanceRefBank,
    };

    fn dust(move_count: u8, mint_decimals: u8, multiplier: f64) -> I80F48 {
        let mut record = RebalanceRecord::zeroed();
        record.move_count = move_count;
        record
            .conservation_dust(mint_decimals, I80F48::from_num(multiplier))
            .unwrap()
    }

    /// `native_units` of a mint, in the whole-token units `conservation_dust` returns.
    fn units(native_units: f64, mint_decimals: u8) -> I80F48 {
        I80F48::from_num(native_units) / EXP_10_I80F48[mint_decimals as usize]
    }

    /// The tolerance is `REBALANCE_CONSERVATION_DUST_ATOMS` native units per move, scaled by the
    /// venue's accounting-token size: a venue whose multiplier has grown rounds by proportionally
    /// more native units per leg.
    #[test]
    fn conservation_dust_scales_with_moves_and_multiplier() {
        // A native bank's multiplier of 1 leaves the raw per-move allowance.
        assert_eq!(dust(1, 6, 1.0), units(3.0, 6));
        assert_eq!(dust(4, 6, 1.0), units(12.0, 6));
        // A venue settling in tokens worth 2.5 native units rounds by 2.5x as much per leg.
        assert_eq!(dust(1, 6, 2.5), units(7.5, 6));
        assert_eq!(dust(4, 9, 2.5), units(30.0, 9));
    }

    fn balance(bank: Pubkey, tag: u16) -> Balance {
        let mut balance = Balance::zeroed();
        balance.active = 1;
        balance.bank_pk = bank;
        balance.tag = tag;
        balance.asset_shares = I80F48::ONE.into();
        balance
    }

    fn ref_bank(bank: Pubkey, tag: u16) -> RebalanceRefBank {
        RebalanceRefBank {
            bank,
            pre_underlying: I80F48::ONE.into(),
            tag,
            _pad0: [0; 6],
        }
    }

    /// A record over `ref_banks` with a single whole move from bank 0 to bank 1.
    fn record_for(account: &MarginfiAccount, ref_banks: &[RebalanceRefBank]) -> RebalanceRecord {
        let mv = RebalanceMove {
            src_index: 0,
            dst_index: 1,
            _pad0: [0; 6],
            amount: I80F48::ONE.into(),
        };
        let mut record = RebalanceRecord::zeroed();
        record
            .initialize(
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                Pubkey::new_unique(),
                ref_banks,
                &vec![I80F48::ZERO; ref_banks.len()],
                &[mv],
                account,
            )
            .unwrap();
        record
    }

    /// The snapshot pins each non-referenced balance's order tag alongside its side and shares.
    #[test]
    fn verify_others_unchanged_rejects_a_cleared_tag() {
        let src = Pubkey::new_unique();
        let mut account = MarginfiAccount::zeroed();
        account.lending_account.balances[0] = balance(src, 0);
        account.lending_account.balances[1] = balance(Pubkey::new_unique(), 7);
        let record = record_for(
            &account,
            &[ref_bank(src, 0), ref_bank(Pubkey::new_unique(), 0)],
        );
        assert!(record.verify_others_unchanged(&account).is_ok());

        account.lending_account.balances[1].tag = 0;
        let err = record.verify_others_unchanged(&account).unwrap_err();
        assert_eq!(err, MarginfiError::IllegalBalanceState.into());
    }

    /// A drained tagged source hands its tag to the balance its move opened.
    #[test]
    fn carry_tags_moves_the_tag_of_a_drained_source() {
        let src = Pubkey::new_unique();
        let dst = Pubkey::new_unique();
        let mut account = MarginfiAccount::zeroed();
        account.lending_account.balances[0] = balance(src, 7);
        let record = record_for(&account, &[ref_bank(src, 7), ref_bank(dst, 0)]);

        let err = record.carry_tags(&mut account).unwrap_err();
        assert_eq!(err, MarginfiError::RebalanceTaggedBalanceSplit.into());

        account.lending_account.balances[0] = balance(dst, 0);
        record.carry_tags(&mut account).unwrap();
        assert_eq!(account.lending_account.balances[0].tag, 7);
    }

    /// An active slot below `EMPTY_BALANCE_THRESHOLD` is left out of the snapshot on both sides.
    #[test]
    fn record_ignores_an_empty_slot() {
        let src = Pubkey::new_unique();
        let mut account = MarginfiAccount::zeroed();
        account.lending_account.balances[0] = balance(src, 0);
        let mut empty = balance(Pubkey::new_unique(), 0);
        empty.asset_shares = I80F48::from_num(0.5).into();
        account.lending_account.balances[1] = empty;
        let record = record_for(
            &account,
            &[ref_bank(src, 0), ref_bank(Pubkey::new_unique(), 0)],
        );
        assert_eq!(record.active_balance_count, 0);
        assert!(record.verify_others_unchanged(&account).is_ok());
    }

    /// A tagged referenced bank no move drains must still hold its tag at end.
    #[test]
    fn carry_tags_rejects_a_cleared_tag_on_an_untouched_bank() {
        let src = Pubkey::new_unique();
        let other = Pubkey::new_unique();
        let mut account = MarginfiAccount::zeroed();
        account.lending_account.balances[0] = balance(other, 7);
        account.lending_account.balances[1] = balance(src, 0);
        let record = record_for(
            &account,
            &[
                ref_bank(src, 0),
                ref_bank(Pubkey::new_unique(), 0),
                ref_bank(other, 7),
            ],
        );
        assert!(record.carry_tags(&mut account).is_ok());

        account.lending_account.balances[0].tag = 0;
        let err = record.carry_tags(&mut account).unwrap_err();
        assert_eq!(err, MarginfiError::IllegalBalanceState.into());
    }
}
