# Code Review — `variable-borrow-premium` → `0.1.10-main`

**Reviewer:** Claude Opus 5
**Date:** 2026-07-28
**Branch:** `variable-borrow-premium` @ `d3233560`
**Base:** `0.1.10-main`
**Scope:** 17 commits, 71 files, +6417 / −140
**Spec:** *Variable Borrow Premium* (`30cff20a24da802cb776f634bf72dac8.md`)
**Note:** review is static only — no builds or test runs were performed (per instruction).

---

## Table of contents

1. [Verdict](#verdict)
2. [What the implementation does](#what-the-implementation-does)
3. [Findings — ranked](#findings--ranked)
   - [F1 (BLOCKING) — legacy `emissions_outstanding` resurrects as premium debt](#f1-blocking--legacy-emissions_outstanding-resurrects-as-premium-debt)
   - [F2 (High) — liquidation forgives the premium instead of collecting it](#f2-high--liquidation-forgives-the-premium-instead-of-collecting-it)
   - [F3 (Medium) — borrow hard-reverts on any stale collateral oracle](#f3-medium--borrow-hard-reverts-on-any-stale-collateral-oracle)
   - [F4 (Medium) — new debt paths that skip the freshness gate](#f4-medium--new-debt-paths-that-skip-the-freshness-gate)
   - [F5 (Medium) — deactivating `PREMIUM_ACTIVE` silently destroys receivables](#f5-medium--deactivating-premium_active-silently-destroys-receivables)
   - [F6 (Medium) — no upper bound on the configured premium rate](#f6-medium--no-upper-bound-on-the-configured-premium-rate)
   - [F7 (Low) — `end_receivership` accounting skew from mid-liquidation write-offs](#f7-low--end_receivership-accounting-skew-from-mid-liquidation-write-offs)
   - [F8 (Low) — stack pressure from `PremiumScratch`](#f8-low--stack-pressure-from-premiumscratch)
   - [F9 (Low) — adversarially timeable snapshotting](#f9-low--adversarially-timeable-snapshotting)
   - [F10 (Low) — requirement-type inconsistency in the collateral denominator](#f10-low--requirement-type-inconsistency-in-the-collateral-denominator)
   - [F11 (Info) — breaking event-schema change](#f11-info--breaking-event-schema-change)
   - [F12 (Nits)](#f12-nits)
4. [Spec fitness — story by story](#spec-fitness--story-by-story)
5. [Things I checked that are correct](#things-i-checked-that-are-correct)
6. [Test coverage assessment](#test-coverage-assessment)
7. [Pre-merge checklist](#pre-merge-checklist)

---

## Verdict

**Sound design, good engineering, one blocking issue.**

The core mechanism is well-built and in one important respect **better than the spec**: the
implementation claims accrued premium at the *old* snapshot rate before overwriting it
([`premium.rs:249-251`](programs/marginfi/src/state/premium.rs#L249-L251)), which eliminates the
retroactive over/under-charge that spec §3.6 and Story 3 explicitly accepted as a known flaw.

Storage reuse is migration-free and pinned by byte-offset tests. Failure modes (partial health
pass, stale oracles, flag toggling, clock skew, `last_update == 0`, overflow on dormant
mega-positions) are each considered and tested. Test coverage is genuinely strong.

One finding ([F1](#f1-blocking--legacy-emissions_outstanding-resurrects-as-premium-debt)) should
block merge; several others need an explicit product/risk decision rather than a code change.

---

## What the implementation does

| Concern | Implementation |
|---|---|
| Pair matrix storage | Group-level (spec Option B), 64 entries in reclaimed `_padding_0`/`_padding_1` — [`type-crate/src/types/premium.rs`](type-crate/src/types/premium.rs), [`type-crate/src/types/group.rs`](type-crate/src/types/group.rs) |
| Lookup | Sorted array + binary search, `find_premium_rate` — [`marginfi_group.rs:270-280`](programs/marginfi/src/state/marginfi_group.rs#L270-L280) |
| Per-balance state | `premium_rate_snapshot: u32` (old `_pad0`), `premium_outstanding: WrappedI80F48` (old `emissions_outstanding`), shared `last_update` — [`type-crate/src/types/user_account.rs`](type-crate/src/types/user_account.rs) |
| Bank state | `premium_tag: u16`, `premium_activated_at: i64`, `collected_premium_outstanding: WrappedI80F48`, flag `PREMIUM_ACTIVE = 1 << 13` |
| Accrual | Lazy simple interest, claimed on any balance mutation and on every snapshot rewrite — [`premium.rs:167-183`](programs/marginfi/src/state/premium.rs#L167-L183) |
| Snapshot recompute | Piggybacks the health loop via `PremiumScratch`, written post-check in the handler — [`premium.rs:193-290`](programs/marginfi/src/state/premium.rs#L193-L290) |
| Health projection | `outstanding + pending`, weighted, added to liabilities — [`marginfi_account.rs:593`](programs/marginfi/src/state/marginfi_account.rs#L593) |
| Settlement | Repay only (`repay_all` in full, partial repay premium-first) → `bank.collected_premium_outstanding` |
| Sweep | Permissionless → canonical ATA of `FeeState.premium_wallet` — [`collect_bank_premium_fees.rs`](programs/marginfi/src/instructions/marginfi_group/collect_bank_premium_fees.rs) |
| Crank | `lending_account_pulse_health` (permissionless) doubles as the refresh crank — [`pulse_health.rs:42-63`](programs/marginfi/src/instructions/marginfi_account/pulse_health.rs#L42-L63) |
| Write-off | Bankruptcy, tokenless repay, liability→asset flip, premium-inactive bank |

New instructions: `lending_pool_configure_group_premium`, `lending_pool_configure_bank_premium`,
`lending_pool_collect_bank_premium_fees`, `edit_fee_state_premium`.

New errors `6610`–`6615`. New events: `LendingPoolGroupPremiumConfigureEvent`,
`LendingPoolBankPremiumConfigureEvent`, `LendingPoolPremiumFeesCollectedEvent`,
`LendingAccountPremiumSettledEvent`, plus a new field on `LendingPoolBankHandleBankruptcyEvent`.

---

## Findings — ranked

### F1 (BLOCKING) — legacy `emissions_outstanding` resurrects as premium debt

**Severity:** High · **Confidence:** High (code path certain; on-chain exposure needs verification)
**Type:** Correctness / user funds

`Balance.emissions_outstanding` was renamed **in place** to `premium_outstanding`
([`user_account.rs`](type-crate/src/types/user_account.rs)). Defense-in-depth against residual
legacy values is a write-off in the balance-mutation helpers — but the guard only covers
premium-**inactive** banks:

[`marginfi_account.rs:2374-2387`](programs/marginfi/src/state/marginfi_account.rs#L2374-L2387)
(`increase_balance_internal`) and
[`marginfi_account.rs:2499-2514`](programs/marginfi/src/state/marginfi_account.rs#L2499-L2514)
(`decrease_balance_internal`):

```rust
if bank.get_flag(PREMIUM_ACTIVE) && had_liabs {
    claim_premium(balance, current_liability_amount, bank.premium_activated_at, now)?;
} else if !bank.get_flag(PREMIUM_ACTIVE)
    && I80F48::from(balance.premium_outstanding) != I80F48::ZERO
{
    balance.premium_outstanding = I80F48::ZERO.into();
}
```

The quadrant `PREMIUM_ACTIVE == true && had_liabs == false` matches **neither** arm. A legacy
value on an asset-side balance survives untouched.

`BankAccountWrapper::claim_premium`
([`marginfi_account.rs:2043-2056`](programs/marginfi/src/state/marginfi_account.rs#L2043-L2056))
has the same shape — it writes off only when the flag is clear.

#### Exploit / failure path

1. Account holds an **asset-side** balance on bank X with a non-zero legacy value at byte offset 72.
2. Risk enables `PREMIUM_ACTIVE` on bank X.
3. User borrows X against that same balance. `decrease_balance_internal` runs with
   `had_liabs == false` → no claim, no write-off. Asset shares fall, liability shares rise.
4. The balance is now a liability carrying `premium_outstanding = <legacy value>`.
5. `update_premium_snapshots` → `claim_premium` **preserves** `outstanding` and adds pending on
   top ([`premium.rs:174-180`](programs/marginfi/src/state/premium.rs#L174-L180)).
6. It is projected into health as a liability
   ([`marginfi_account.rs:593`](programs/marginfi/src/state/marginfi_account.rs#L593)),
   charged on `repay_all`
   ([`marginfi_account.rs:2234-2250`](programs/marginfi/src/state/marginfi_account.rs#L2234-L2250)),
   and swept to the protocol premium wallet.

The user is charged real tokens for a value that was never a premium.

#### Evidence this is not hypothetical

[`programs/marginfi/tests/misc/regression.rs:157-160`](programs/marginfi/tests/misc/regression.rs#L157-L160)
— a real mainnet account fixture (group `4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8`,
authority `3T1kGHp7CrdeW9Qj1t8NMc2Ks233RyvzVhoaUPWoBEFK`):

```rust
assert_eq!(I80F48::from(balance_1.asset_shares),        "470.952530958931234");
assert_eq!(I80F48::from(balance_1.liability_shares),    "0");
assert_eq!(I80F48::from(balance_1.premium_outstanding), "26891413.388324654086347");
```

An **asset-side** balance with a non-zero legacy value — precisely the unguarded quadrant.

The author was aware of the value: the same literal appears in the unit test
[`premium.rs:632`](programs/marginfi/src/state/premium.rs#L632)
(`snapshot_pass_writes_off_inactive_liability_receivable`) — which covers only the
premium-**inactive** case. The active case is untested.

The struct-layout test comment at
[`marginfi_group.rs:378-383`](programs/marginfi/src/state/marginfi_group.rs#L378-L383) asserts:

> *"both zeroed on-chain by the emissions wind-down migration … As defense-in-depth the engine
> additionally honors `premium_outstanding` only on `PREMIUM_ACTIVE` banks."*

That is exactly backwards for this path: on a `PREMIUM_ACTIVE` bank the legacy value is honored.

#### Additional aggravating factor

The permissionless "zero out `emissions_outstanding`" instruction introduced in
`f28865dc` (*Remove and cleanup emissions*, #521) **no longer exists** in the tree —
`grep -rn "emissions_outstanding" programs/marginfi/src` returns nothing, and
[`instructions/marginfi_account/emissions.rs`](programs/marginfi/src/instructions/marginfi_account/emissions.rs)
now contains only `marginfi_account_update_emissions_destination_account`. There is no remaining
on-chain cleanup path for a missed balance.

#### Caveat on exposure

The fixture is from commit `2ada93a9` (*State structs regression test*, #186, 2024) and therefore
**predates** the wind-down. It proves such values existed historically, not that any remain today.
The code defect is real regardless of on-chain exposure.

#### Recommended fix

Make the write-off unconditional on the liability side being absent:

```rust
if bank.get_flag(PREMIUM_ACTIVE) && had_liabs {
    claim_premium(balance, current_liability_amount, bank.premium_activated_at, now)?;
} else if I80F48::from(balance.premium_outstanding) != I80F48::ZERO {
    // premium-inactive bank OR asset-side balance: no receivable can legitimately exist
    balance.premium_outstanding = I80F48::ZERO.into();
}
```

Apply the same change in
[`BankAccountWrapper::claim_premium`](programs/marginfi/src/state/marginfi_account.rs#L2043),
and add a unit test for the `PREMIUM_ACTIVE && !had_liabs` quadrant.

**Also do before enabling the flag anywhere:** scan mainnet for balances with non-zero bytes at
`Balance` offset 72 (`premium_outstanding`) and `active == 1`, and confirm the set is empty.

---

### F2 (High) — liquidation forgives the premium instead of collecting it

**Severity:** Medium-High · **Confidence:** High
**Type:** Spec deviation / revenue leak / silent state change

[`liquidate.rs:440`](programs/marginfi/src/instructions/marginfi_account/liquidate.rs#L440) reduces
the liquidatee's debt with a plain `repay(liab_amount_final)` book transfer. That materializes
premium into `premium_outstanding` (via `increase_balance_internal`) but **never settles it into
`bank.collected_premium_outstanding`**, because settlement is intentionally restricted to paths
where real tokens enter the liquidity vault.

Worse, if the transfer pushes `liability_shares` below `EMPTY_BALANCE_THRESHOLD` (= `1`), the
receivable is silently zeroed:

[`marginfi_account.rs:2461-2467`](programs/marginfi/src/state/marginfi_account.rs#L2461-L2467)

```rust
// Below EMPTY_BALANCE_THRESHOLD health treats the liability as empty, so clear the
// premium too — otherwise a book-transfer that leaves dust (liquidation) strands a
// receivable that health never projects.
if had_liabs && I80F48::from(balance.liability_shares) < EMPTY_BALANCE_THRESHOLD {
    balance.premium_outstanding = I80F48::ZERO.into();
    balance.premium_rate_snapshot = 0;
}
```

**Spec conflict.** Story 5 → *Internal Actions (When Liquidated)*:

> 3. `claim_premium` runs inside the liquidation balance mutations: materializes `0.41 USDC`…
> 4. Liquidation proceeds: collateral transferred, **debt repaid (including premium)**.

and *Result*: "Her remaining BONK collateral covers the base debt **+ premium** + liquidation penalty."

So the premium can *cause* the liquidation (it is projected into maintenance liabilities) and then
evaporate. The protocol books the risk signal but not the revenue.

**Observability gap.** Bankruptcy emits `premium_written_off`
([`handle_bankruptcy.rs:200-238`](programs/marginfi/src/instructions/marginfi_group/handle_bankruptcy.rs#L200-L238)),
and tokenless repay emits it via `LendingAccountPremiumSettledEvent`
([`repay.rs:194-208`](programs/marginfi/src/instructions/marginfi_account/repay.rs#L194-L208)).
The liquidation write-off emits **nothing** — not even a `msg!`.

**Options:**
- (a) Settle premium into `collected_premium_outstanding` during liquidation, funded from the
  seized collateral leg; or
- (b) accept the forgiveness as policy, document it in the spec, and emit an event so accounting
  can reconcile.

Either way this should be a deliberate decision, not an emergent consequence of the
`EMPTY_BALANCE_THRESHOLD` guard.

---

### F3 (Medium) — borrow hard-reverts on any stale collateral oracle

**Severity:** Medium · **Confidence:** High
**Type:** Liveness regression / product decision

[`borrow.rs:226-231`](programs/marginfi/src/instructions/marginfi_account/borrow.rs#L226-L231):

```rust
// New debt needs a real rate: revert if a stale oracle left the premium pass unpriceable
check!(
    !premium_scratch.refresh_unavailable(),
    MarginfiError::PremiumSnapshotUnavailable
);
```

`refresh_unavailable()`
([`premium.rs:104-115`](programs/marginfi/src/state/premium.rs#L104-L115)) is true when the health
pass was incomplete **and** the account holds any premium-active liability. `scratch.complete` is
set only if no balance recorded an `err_code`
([`marginfi_account.rs:1182-1187`](programs/marginfi/src/state/marginfi_account.rs#L1182-L1187)),
and for `RequirementType::Initial` a stale collateral oracle sets `err_code` while valuing the
asset at zero
([`marginfi_account.rs:1660-1667`](programs/marginfi/src/state/marginfi_account.rs#L1660-L1667)).

**Behaviour change:** previously the borrow proceeded with that collateral valued at zero — a
conservative, safe degradation. Now it reverts outright. Any account holding one asset with a
stale oracle cannot borrow *anything* once premium is live on its liability bank.

This is intentional — there is a test, `premium_borrow_rejected_when_collateral_oracle_stale`
([`premium.rs:1577`](programs/marginfi/tests/user_actions/premium.rs#L1577)) — and withdraw and
liquidation were deliberately loosened in commit `474ba2fc` ("Losen the premium freshness on
withdrawal and liquidations"). Flagging so the trade-off is explicit: an oracle outage on a single
collateral asset now becomes a borrow outage for every account holding it.

`end_flashloan` enforces the same gate
([`flashloan.rs:133-137`](programs/marginfi/src/instructions/marginfi_account/flashloan.rs#L133-L137)).

---

### F4 (Medium) — new debt paths that skip the freshness gate

**Severity:** Medium-Low · **Confidence:** High
**Type:** Consistency / revenue leak

Two paths create liabilities but do **not** check `refresh_unavailable`, so if the scratch is
incomplete the snapshot write is a silent no-op and the new debt keeps `premium_rate_snapshot == 0`
until someone cranks `pulse_health`:

- **Liquidator** —
  [`liquidate.rs:597-616`](programs/marginfi/src/instructions/marginfi_account/liquidate.rs#L597-L616)
  (`check_liquidator_health_and_refresh_premium`). The liquidator assumes the liquidatee's debt, so
  this is genuinely new debt on a fresh balance (snapshot 0 at creation).
- **Order execution** —
  [`order.rs:519-526`](programs/marginfi/src/instructions/marginfi_account/order.rs#L519-L526)
  (`end_execute_order`).

Mitigating: both use `RequirementType::Maintenance`, where a stale collateral oracle propagates as
a hard `Err` rather than an `err_code`, so `complete` is effectively always true on the success
path. The gap is therefore narrow — but it depends on a non-obvious interaction between
`calc_weighted_asset_value_standalone`'s requirement-type branch and the completeness flag. Worth
either adding the gate for symmetry or documenting why it is unnecessary.

Note also that `check_liquidator_health_and_refresh_premium` passes `&mut None` for the health
cache, so the liquidator's health cache is not written — pre-existing behaviour, unchanged.

---

### F5 (Medium) — deactivating `PREMIUM_ACTIVE` silently destroys receivables

**Severity:** Medium · **Confidence:** High
**Type:** Admin footgun / observability

Clearing the flag via
[`config_bank_premium.rs:30-32`](programs/marginfi/src/instructions/marginfi_group/config_bank_premium.rs#L30-L32)
means the *next touch* of any borrower's balance writes off their entire accrued premium:

- [`marginfi_account.rs:2043-2056`](programs/marginfi/src/state/marginfi_account.rs#L2043-L2056) — `debug!` only
- [`premium.rs:240-247`](programs/marginfi/src/state/premium.rs#L240-L247) — silent

This is a defensible policy (health no longer projects it, so repay must not collect it), and it is
correctly tested (`premium_deactivated_bank_writes_off_receivable_instead_of_settling`). But it is a
single-parameter admin action that irreversibly destroys accrued protocol revenue across every
borrower on the bank, with no event and no confirmation.

Suggest: emit an event on write-off, and document that deactivation is destructive (operators may
expect "pause accrual", not "forgive everything accrued").

---

### F6 (Medium) — no upper bound on the configured premium rate

**Severity:** Medium · **Confidence:** High
**Type:** Privileged-role blast radius

[`config_group_premium.rs:17-27`](programs/marginfi/src/instructions/marginfi_group/config_group_premium.rs#L17-L27)
accepts any `u32` for `rate`. `milli_to_u32` tops out at 1000 % APR
([`interest_rate.rs:103-108`](type-crate/src/types/interest_rate.rs#L103-L108)), so
`rate = u32::MAX` is a valid, silently-accepted 1000 % surcharge. The only gate is
`has_one = emode_admin`.

Because premium is projected into health liabilities, a mis-set or compromised emode admin can make
every account in a `(collateral_tag, liability_tag)` pair progressively liquidatable — a slower but
broader lever than the weight changes emode already permits.

By comparison, emode entries *are* validated against `group.emode_max_init_leverage` /
`emode_max_maint_leverage`
([`config_bank_emode.rs:29-33`](programs/marginfi/src/instructions/marginfi_group/config_bank_emode.rs#L29-L33)).
A parallel `group.premium_max_rate` rail would be cheap and consistent. Note there is currently no
timelock on any emode-admin operation in this branch, so this is the only available guard.

---

### F7 (Low) — `end_receivership` accounting skew from mid-liquidation write-offs

**Severity:** Low · **Confidence:** Medium
**Type:** Accounting precision

[`liquidate_end.rs:117-182`](programs/marginfi/src/instructions/marginfi_account/liquidate_end.rs#L117-L182)

`pre_liabs*` come from `liq_record.cache` (written at `start_receivership`, premium included).
`post_liabilities_equity` is recomputed at the end (premium included). If a premium receivable is
written off mid-receivership (F2's threshold rule), then:

- `repaid = pre_liabs_equity - post_liabilities_equity` **overstates** the actual repayment by the
  written-off amount. This feeds `LiquidationRecord` entries and the deleverage-limit accounting.
- The `pre_health > post_health` gate becomes marginally easier to pass, letting the liquidator
  extract slightly more collateral.

Both are bounded by the premium size (small relative to base debt), and the pre/post computations
happen in the same transaction so time-based drift is nil. Resolving F2 resolves this.

---

### F8 (Low) — stack pressure from `PremiumScratch`

**Severity:** Low · **Confidence:** Medium
**Type:** Robustness

`PremiumScratch` is `[PremiumScratchEntry; 16]` where the `Liability` variant carries
`Pubkey` (32B) + `u16` + `I80F48` (16B, 16-aligned) + `i64` + `bool` — roughly 80 bytes per entry,
so ~1.3 KB, ~32 % of the 4 KiB SBF frame
([`premium.rs:48-92`](programs/marginfi/src/state/premium.rs#L48-L92)).

The PR already had to work around this twice:

- [`liquidate.rs:569-573`](programs/marginfi/src/instructions/marginfi_account/liquidate.rs#L569-L573)
  — `#[inline(never)]` wrappers, with the comment *"inlined into the handler it overflows the
  4096-byte frame"*.
- [`add_pool_permissionless.rs:28-36`](programs/marginfi/src/instructions/marginfi_group/add_pool_permissionless.rs#L28-L36)
  — an unrelated handler refactored into `write_staked_bank` for the same reason, which is a signal
  the build sits near the edge generally.

But `borrow.rs`, `withdraw.rs`, `pulse_health.rs`, `flashloan.rs`, `order.rs` and all four
integration withdraws hold the scratch inline with no such guard. They are one added local away
from a runtime access violation. Consider boxing the entry array or applying `#[inline(never)]`
prophylactically to the check-and-refresh sequence in each handler.

Related: the 8×8 worst case is measured at **< 1.3 M CU**
([`vb05_premiumWorstCase8x8.spec.ts:350-382`](tests/specs/premium/vb05_premiumWorstCase8x8.spec.ts#L350-L382))
against a 1.4 M ceiling — ~7 % headroom for a 16-position account.

---

### F9 (Low) — adversarially timeable snapshotting

**Severity:** Low · **Confidence:** High
**Type:** Griefing / design property

`lending_account_pulse_health` is permissionless and rewrites premium snapshots
([`pulse_health.rs:42-63`](programs/marginfi/src/instructions/marginfi_account/pulse_health.rs#L42-L63)).
Anyone can therefore choose *when* a given account's rate is locked in, and the rate persists until
the next refresh.

Collateral USD in the scratch uses the **low-biased** price
([`marginfi_account.rs:631`](programs/marginfi/src/state/marginfi_account.rs#L631) →
`collect_premium_scratch_entry`, fed the `price` returned by the risk engine). Because the formula
is a ratio, a uniform bias is neutral — but a transiently wide confidence band on the *good*
collateral shrinks its weight and raises the victim's rate.

Bounded and symmetric (the victim can re-crank immediately, and the mispricing only applies to the
interval between snapshots), so severity is low. Worth documenting as a known property.

---

### F10 (Low) — requirement-type inconsistency in the collateral denominator

**Severity:** Low · **Confidence:** High
**Type:** Consistency

`ReduceOnly` collateral is valued at zero (and returns `price = 0`) for `RequirementType::Initial`
([`marginfi_account.rs:1636-1642`](programs/marginfi/src/state/marginfi_account.rs#L1636-L1642))
but is counted normally for `Maintenance`. Since `collect_premium_scratch_entry` derives
`usd_value` from that same `price`, the premium denominator differs by requirement type:

- Borrow / withdraw (Initial) → `ReduceOnly` collateral excluded → **higher** premium
- Liquidation / order end (Maintenance) → included → **lower** premium
- `end_receivership` (Equity) → included

So the same basket produces different rates depending on which instruction refreshed it. Harmless
directionally (Initial is the conservative one) but non-obvious; worth a comment on
[`PremiumScratchEntry::Asset`](programs/marginfi/src/state/premium.rs#L56).

`RiskTier::Isolated` collateral is always excluded (price 0 in both branches) — correct, since it
cannot back a cross-position anyway.

---

### F11 (Info) — breaking event-schema change

`LendingPoolBankHandleBankruptcyEvent` gained a `premium_written_off: f64` field
([`events.rs:146-151`](programs/marginfi/src/events.rs#L146-L151)). Anchor events are
Borsh-serialized positionally, so this breaks any downstream indexer decoding the old layout.
Needs coordination with the indexer team before deploy.

Also note `EndLiquidationInstructionAccounts::new` argument order changed in the fuzz harness
([`trident-tests/fuzz_0/methods/core.rs`](trident-tests/fuzz_0/methods/core.rs)) — `record` and
`marginfi_group` swapped. Confirm this reflects a regenerated IDL rather than a latent bug.

---

### F12 (Nits)

| # | Location | Note |
|---|---|---|
| 1 | [`solend/withdraw.rs:265-274`](programs/marginfi/src/instructions/solend/withdraw.rs#L265-L274) | Re-loads `group` inside the block, shadowing the outer `Ref`. Works (two immutable `Ref`s), but unnecessary — the other three integrations reuse the outer binding. |
| 2 | [`trident-tests/fuzz_0/invariants/premium.rs:60-64`](trident-tests/fuzz_0/invariants/premium.rs#L60-L64) | Dangling empty block after `continue;` — leftover from a removed invariant. |
| 3 | [`trident-tests/fuzz_0/test_fuzz.rs:1078-1080`](trident-tests/fuzz_0/test_fuzz.rs#L1078-L1080) | Comment claims the invariant checks a receivable "bounded by the configured cap" — there is no cap (spec §4.4 chose none). |
| 4 | [`marginfi_account.rs:1092-1093`](programs/marginfi/src/state/marginfi_account.rs#L1092-L1093) | `clock.as_ref().map(...).unwrap_or(0)` — all three `HealthPriceMode` arms now yield `Some`, so the `unwrap_or(0)` is dead. Prefer an explicit unwrap or restructure so a future fourth arm can't silently produce `now = 0`. |
| 5 | [`marginfi_account.rs:966`](programs/marginfi/src/state/marginfi_account.rs#L966) | `HealthPriceMode::Cached` now calls `Clock::get()`. Comment asserts this path is on-chain-only; worth confirming no `client`-feature simulation reaches it. |
| 6 | [`errors.rs:456`](programs/marginfi/src/errors.rs#L456) | `PremiumEntryInvalid = 610` uses an explicit discriminant to jump the 6605–6609 gap. Correct, but fragile if someone later appends to the circuit-breaker block. |
| 7 | [`config_group_premium.rs`](programs/marginfi/src/instructions/marginfi_group/config_group_premium.rs) | One pair per instruction means configuring the full 8×8 matrix costs 64 transactions. Fine for auditability (mirrors emode), but worth confirming the ops workflow. |
| 8 | [`collect_bank_premium_fees.rs:56-63`](programs/marginfi/src/instructions/marginfi_group/collect_bank_premium_fees.rs#L56-L63) | The sweep can drain the liquidity vault to zero via `min(outstanding, available_liquidity)`, blocking lender withdrawals until repayments arrive. Consistent with `collect_bank_fees`, so not a new risk — just noting the shared property. |

---

## Spec fitness — story by story

| Story | Verdict | Notes |
|---|---|---|
| **1** — Basic premium (BONK → USDC, 1 %) | ✅ | Exact numbers pinned in [`snapshot_story1_single_collateral`](programs/marginfi/src/state/premium.rs#L482) and [`vb02_premiumBorrow.spec.ts`](tests/specs/premium/vb02_premiumBorrow.spec.ts). Snapshot is written *after* the health check, so the borrow itself is unaffected — matches the spec walkthrough. |
| **1 (Bob/Carol premium fund)** | ✅ | Realized premium lands in `bank.collected_premium_outstanding` and is swept to `FeeState.premium_wallet`. Redistribution mechanics correctly punted per §4.2. Permissionless realization ("an ix Bob could send") is satisfied by `pulse_health`. |
| **2** — Mixed collateral weighting | ✅ | [`snapshot_story2_mixed_collateral_weighted`](programs/marginfi/src/state/premium.rs#L504) reproduces the 0.2 % result. |
| **3** — Improving collateral | ⚠️ **Partial** | Deposit does **not** refresh snapshots (the deposit ix carries no oracles), so the rate does not drop "instantly" — it drops on the next refreshing ix or `pulse_health`. The spec's own note permits this ("front end will probably do this anyways by appending a sync before deposit"). **However the implementation is strictly better than the spec here**: because `update_premium_snapshots` claims at the OLD rate before overwriting ([`premium.rs:249-251`](programs/marginfi/src/state/premium.rs#L249-L251)), Charlie is *correctly* charged 1.0 % for the 30-day window instead of the spec's acknowledged undercharge. §3.6 is effectively fixed. **Action:** confirm the front end appends a `pulse_health` before deposits. |
| **4** — Multiple borrows, independent premiums | ✅ | Per-liability snapshots against a shared denominator, exactly as specified. |
| **4.5** — Missing pairs default to 0 % | ✅ | [`snapshot_story4_5_missing_pairs_default_zero`](programs/marginfi/src/state/premium.rs#L521) reproduces 0.4000 % / 0.7167 % / 0.0000 % to spec precision. `PREMIUM_TAG_EMPTY = 0` never matches ([`marginfi_group.rs:271-273`](programs/marginfi/src/state/marginfi_group.rs#L271-L273)). |
| **5** — Dormant account degrades | ⚠️ **Partial** | Health projection ✅ ([`marginfi_account.rs:593`](programs/marginfi/src/state/marginfi_account.rs#L593)); permissionless crank ✅; uncapped accrual per §4.4 ✅. **But** step 4 ("debt repaid *including premium*") is not implemented — see [F2](#f2-high--liquidation-forgives-the-premium-instead-of-collecting-it). |
| **6** — Repay all | ✅ | `total_owed = liability + premium`, `ceil()` remainder → insurance fees, balance closed, `collected_premium_outstanding` credited ([`marginfi_account.rs:2234-2270`](programs/marginfi/src/state/marginfi_account.rs#L2234-L2270)). Verified in `premium_repay_all_settles_and_sweep_pays_premium_wallet`. |

### Spec section coverage

| Spec § | Decision taken | Assessment |
|---|---|---|
| §2.2 storage | `_pad0` → `premium_rate_snapshot`, `emissions_outstanding` → `premium_outstanding` | ✅ zero-migration, pinned by offset tests — but see [F1](#f1-blocking--legacy-emissions_outstanding-resurrects-as-premium-debt) |
| §2.2b config location | **Option B** (group-level matrix) | ✅ good call; fits in reclaimed padding, no group resize |
| §3.4 no retroactive premium | `premium_activated_at` clamp + zero-rate claim bumps `last_update` | ✅ well handled and well tested (`premium_activation_never_charges_retroactively`, `premium_reactivation_never_charges_for_deactivated_window`) |
| §3.6 snapshot-overwrite staleness | **Fixed** — claims at old rate first | ✅ better than spec |
| §4.2 where premiums go | Separate protocol wallet via sweep | ✅ matches "punt on usage" |
| §4.3 partial repay | **Option B** (premium before principal), not the recommended Option A | ⚠️ deviation, favourable to protocol; flag as deliberate |
| §4.4 premium cap | None | ✅ matches recommendation |
| §4.6 pause behaviour | Premium accrues through pauses | ⚠️ implicit — no `MIN_PREMIUM_START_TIME`. Spec left this open; confirm the choice. |
| §4.7 keeper refresh | `pulse_health` doubles as the crank | ✅ no new ix needed |
| §4.8 emode interaction | Separate tags/tables; composes in health | ✅ `premium_composes_with_emode` explicitly tests both directions incl. "emode enables the leverage, premium erodes it to liquidatable" |
| §4.9 dedicated `premium_tag` | `Bank.premium_tag: u16` | ✅ |
| §4.10 liability→asset flip | Receivable zeroed at `EMPTY_BALANCE_THRESHOLD` | ✅ for the flip; ⚠️ same rule creates the liquidation forgiveness in [F2](#f2-high--liquidation-forgives-the-premium-instead-of-collecting-it) |

---

## Things I checked that are correct

Recording these so they don't get re-litigated:

1. **Migration safety — group.** `premium_settings` (32 B) + `premium_entries` (512 B) exactly fill
   former `_padding_0` + `_padding_1`; `_padding_2` still starts at `V1_LEN = 1056`. Verified by
   [`group_premium_field_layout`](programs/marginfi/src/state/marginfi_group.rs#L370). History check
   (`git log -S`) confirms both regions were only ever *shrunk from*, never written — so no live
   group can carry a bogus `entry_count`.
2. **Migration safety — bank.** `collected_premium_outstanding` takes the former `_pad_0: [u8; 16]`,
   itself carved from zero padding in `1b43c9ff` (rate limiter). `premium_tag` /
   `premium_activated_at` take the first 16 B of `_padding_1: [u64; 3]`. Both asserted against the
   real mainnet fixture in [`regression.rs:719-746`](programs/marginfi/tests/misc/regression.rs#L719-L746).
3. **Migration safety — fee state.** `premium_wallet` sits at exactly `FeeState::V1_LEN`, so
   `resize_global_fee_state` zero-fill yields `Pubkey::default()` = sweeps disabled
   ([`fee_state.rs:101-115`](programs/marginfi/src/state/fee_state.rs#L101-L115)).
4. **No double-credit on repay.** `repay.rs` claims once at [:93](programs/marginfi/src/instructions/marginfi_account/repay.rs#L93);
   `repay_all` re-claims with `elapsed == 0` (no-op) and credits exactly once. Partial repay settles
   before `repay(principal)`, whose internal claim is also a no-op.
5. **No free escape via partial repay.** `settle_premium(amount)` runs first, so reaching
   `liability_shares < EMPTY_BALANCE_THRESHOLD` requires `amount >= debt + premium`. `RepayOnly`
   rejects over-repayment, so the flip write-off is unreachable from the user-controlled repay path.
6. **`close_balance` cannot strand a receivable.** Requires `liability_amount ≈ 0`, which implies
   the premium was already zeroed by the threshold rule.
7. **Vault accounting is balanced.** Premium tokens enter the liquidity vault without any share
   change and are tracked separately; the fuzz solvency invariant correctly folds
   `collected_premium_outstanding` into `outstanding_fees`
   ([`solvency.rs:57-66`](trident-tests/fuzz_0/invariants/solvency.rs#L57-L66)).
8. **Overflow handling.** `accrued_premium_total` divides `elapsed / SECONDS_PER_YEAR` *first*, with
   a test for a 1e15-unit position at the 1000 % ceiling dormant 50 years
   ([`premium.rs:653-664`](programs/marginfi/src/state/premium.rs#L653-L664)).
9. **Matrix insert/delete preserves sort order.** `rotate_right` on `[i..=count]` for insert,
   `rotate_left` on `[i..count]` + zero the tail for delete; `count < MAX` checked before insert.
   `find_premium_rate` clamps to `MAX_PREMIUM_ENTRIES`, so no OOB even with a corrupt `entry_count`.
10. **Every collateral-reducing path refreshes.** Core withdraw, all four integration withdraws,
    borrow, liquidation (both sides), `end_execute_order`, `end_flashloan`, `end_receivership`.
    Withdraw correctly *defers* while `ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_ORDER_EXECUTION` is set,
    with the deferred owner handling it.
11. **Flashloan path.** `check_account_init_health` early-returns under `ACCOUNT_IN_FLASHLOAN`,
    leaving the scratch empty → `refresh_unavailable()` false and `update_premium_snapshots` a
    no-op; `end_flashloan` performs both the gate and the refresh after unsetting the flag.
12. **Scratch/account binding.** `liquidate.rs` correctly uses two separate scratches for liquidatee
    and liquidator; `update_premium_snapshots` re-finds balances by `bank_pk`, so `sort_balances()`
    between the check and the write is harmless.
13. **`PremiumScratch::push` bounds.** Capacity equals `MAX_LENDING_ACCOUNT_BALANCES`, so no silent
    truncation is reachable.
14. **Sweep authorization.** ATA derived from `FeeState.premium_wallet` + `bank.mint` +
    `token_program`, rejected when the wallet is unset; vault/authority validated by seeds.
15. **Premium does not compound on itself** — accrual is simple interest on `liability_amount` only,
    matching the spec's "the less Alice interacts, the less her debt compounds".

---

## Test coverage assessment

**Strong.**

| Layer | Location | Count |
|---|---|---|
| Unit (rate math, snapshot, elapsed, gate) | [`premium.rs` `mod tests`](programs/marginfi/src/state/premium.rs#L292) | 15 |
| Unit (layout, matrix lookup) | [`marginfi_group.rs` `mod tests`](programs/marginfi/src/state/marginfi_group.rs#L318) | 6 |
| Rust integration — user actions | [`tests/user_actions/premium.rs`](programs/marginfi/tests/user_actions/premium.rs) | 23 |
| Rust integration — admin | [`tests/admin_actions/premium_config.rs`](programs/marginfi/tests/admin_actions/premium_config.rs) | 3 |
| TS / LiteSVM | [`tests/specs/premium/vb0{1,2,3,5}*.spec.ts`](tests/specs/premium/) | 4 files |
| Fuzz invariants | [`trident-tests/fuzz_0/invariants/premium.rs`](trident-tests/fuzz_0/invariants/premium.rs) | wired into `test_fuzz.rs` |

Particularly good: spec-exact numeric reproduction of Stories 1/2/4.5/6; adversarial oracle-staleness
tests across borrow / withdraw / flashloan / liquidation / pulse; emode composition in both
directions; an explicit 8×8 worst-case CU/tx-size test.

**Gaps:**

1. **`PREMIUM_ACTIVE && !had_liabs`** — the [F1](#f1-blocking--legacy-emissions_outstanding-resurrects-as-premium-debt)
   quadrant. Only the inactive counterpart is tested.
2. **Bankruptcy premium write-off** — [`handle_bankruptcy.rs:200-212`](programs/marginfi/src/instructions/marginfi_group/handle_bankruptcy.rs#L200-L212)
   has no test in `premium.rs`, and the new `premium_written_off` event field is unasserted.
3. **Liquidation that fully clears the debt** — the [F2](#f2-high--liquidation-forgives-the-premium-instead-of-collecting-it)
   forgiveness path is untested; existing liquidation tests exercise partial repayment only.
4. **`vb04`** — the spec numbering skips from `vb03` to `vb05`. Confirm nothing was dropped.
5. **Group `entry_capacity` growth path** — documented as "future" but untested.

---

## Pre-merge checklist

**Must fix**

- [ ] **F1** — unconditional write-off when `!had_liabs`, in both `increase_balance_internal`
      ([:2374](programs/marginfi/src/state/marginfi_account.rs#L2374)) and
      `decrease_balance_internal` ([:2499](programs/marginfi/src/state/marginfi_account.rs#L2499)),
      plus `BankAccountWrapper::claim_premium` ([:2043](programs/marginfi/src/state/marginfi_account.rs#L2043)).
- [ ] **F1** — add a unit test for the `PREMIUM_ACTIVE && !had_liabs` quadrant.
- [ ] **F1** — mainnet scan for residual non-zero `Balance` offset-72 values before enabling the flag.

**Must decide (product / risk)**

- [ ] **F2** — settle premium during liquidation, or accept forgiveness + emit an event.
- [ ] **F3** — confirm borrow should hard-revert on stale collateral oracles.
- [ ] **F5** — confirm `PREMIUM_ACTIVE` deactivation should be destructive; add an event.
- [ ] **F6** — decide whether to add a `group.premium_max_rate` bound.
- [ ] **§4.6** — confirm premium accruing through protocol pauses is intended.
- [ ] **Story 3** — confirm the front end will append `pulse_health` before deposits.

**Should do**

- [ ] **F4** — add the freshness gate to the liquidator/order-end paths, or document why it's moot.
- [ ] **F8** — box `PremiumScratch::entries` or apply `#[inline(never)]` to the check-and-refresh
      sequence in the un-guarded handlers.
- [ ] **F11** — coordinate the `LendingPoolBankHandleBankruptcyEvent` schema change with indexing.
- [ ] Test gaps 2 and 3 above.
- [ ] **F12** nits.

**Build/test verification (not performed in this review)**

- [ ] `./scripts/build-workspace.sh` + `./scripts/test-program.sh all`
- [ ] `anchor build -p marginfi -- --no-default-features --features custom-heap` + `anchor run all-tests`
- [ ] `cargo test -p marginfi --no-default-features --features custom-heap --lib`
- [ ] Confirm the 8×8 CU headroom on the final build (test asserts < 1.3 M against a 1.4 M ceiling).
