# JupLend integration TS test plan (program-first)

This is a **planning doc**, not an implementation.

Goal: add a JupLend integration test suite that matches the existing Drift / Solend patterns, while improving the ergonomics of the setup (builder-style helpers, fewer repeated boilerplate blocks).

---

## 1. What we need to test

### 1.1 Baseline integration behavior (must-have)

1) **Add bank**
- Validate bank initialization works and the bank stores the minimal JupLend state we care about (e.g. the `lending` account pubkey).

2) **Init position**
- Permissionless init must create any needed protocol accounts (if any), and seed deposit should work.
- For JupLend specifically, seed deposit is deliberately simple and does not need additional checks beyond what other init instructions do.

2b) **Activation gating (paused -> operational)**
- Newly added JupLend banks start **paused** and must be activated via `juplend_init_position`.
- Creating the bank-owned fToken ATA is **permissionless** and must *not* activate the bank by itself.
- Deposits/withdrawals must fail with `BankPaused` until the bank is activated.

3) **Deposit**
- Must call `update_rate` first (same-slot freshness requirement) and then deposit.
- Must verify the exact post-CPI balances match the program-side expectations:
  - underlying debited from user
  - fTokens minted/credited to the bank authority vault (or wherever JupLend puts them)
  - marginfi `asset_shares` delta matches expected

4) **Withdraw**
- Must call `update_rate` first (same-slot freshness requirement) and then withdraw.
- Must verify the exact post-CPI balances match the program-side expectations:
  - fTokens burned/debited
  - underlying credited back to user
  - marginfi `asset_shares` delta matches expected

5) **Receivership / liquidation compatibility**
- The TX-level allowlisting must permit a JupLend `update_rate` instruction *before* `start_liquidation`/`start_deleverage`.
- The TX-level allowlisting must permit the Marginfi `juplend_withdraw` instruction during receivership.

> The Rust unit test added in `liquidate_start.rs` covers the ordering allowlist. A TS test can still be added later as an end-to-end safety net once we have a real JupLend program fixture available in bankrun.

---

### 1.2 Interest accrual / exchange-rate correctness (must-have)

We need at least one test proving the integration math stays correct *as the exchange rate changes*.

Proposed structure:

- User deposits X underlying.
- Warp forward some slots (or otherwise simulate accrual).
- Ensure JupLend exchange price changes (by calling `update_rate`, potentially after warp).
- Then withdraw Y underlying (or withdraw-all), and verify:
  - the computed expected shares burned matches the actual shares burned
  - the underlying returned matches the expected amount **exactly** (or the ix fails)

Important: for the **same-slot freshness enforcement**, we should construct a single tx like:

- `juplend.update_rate(...)`
- `marginfi.juplend_deposit(...)`

and similarly for withdraw. No tolerance: stale exchange rate must fail.

---

### 1.3 Rewards (non-same-mint) tests (later)

We are explicitly deferring merkle distributor / non-underlying-mint reward accounting.

When we pick this up, we’ll add:
- A test where reward mint != underlying mint.
- A custom “claim + swap + re-deposit / fee routing” flow (likely not part of baseline deposit/withdraw).

---

## 2. Proposed helper/builder structure (to reduce boilerplate)

The current tests have a lot of repeated setup logic (especially around protocol init + bank init + remaining accounts). For JupLend, we can improve this by creating a focused builder that cleanly separates concerns.

### 2.1 Files to add (proposal)

- `tests/utils/juplend/juplend-pdas.ts`
  - All PDA derivations (lending, fToken mint, lending admin, etc.) based on seeds from the IDL.

- `tests/utils/juplend/juplend-instructions.ts`
  - Typed instruction builders for:
    - `update_rate`
    - protocol init instructions (whatever JupLend requires to stand up a market)
    - protocol deposit / withdraw

- `tests/utils/juplend/juplend-state.ts`
  - Minimal parsing helpers for the JupLend `Lending` account fields that Marginfi cares about:
    - token exchange price
    - last update slot/timestamp
    - totals needed for sanity checks

- `tests/utils/juplend/juplend-test-env.ts`
  - A **builder** for creating a fully-functional JupLend market for a given underlying mint.
  - Returns a struct with:
    - all relevant addresses
    - convenience methods to build txs:
      - `txUpdateRate()`
      - `txDeposit()` / `txWithdraw()`
      - `txUpdateRateThenDeposit()` (important for same-slot freshness)

- `tests/utils/integration-builder.ts` (optional)
  - A higher-level helper to create:
    - group
    - bank config
    - add bank ix
    - init position ix
    - and standardized remaining accounts ordering

We can follow the same style as `tests/utils/solend-instructions.ts` and `tests/utils/drift-instructions.ts`, but keep JupLend-specific structure in its own subfolder.

---

## 3. Test phases and suite layout

A recommended sequence that keeps debugging straightforward:

### Phase A — Stand up a JupLend market (protocol-only)
- Create a market for a single mint (e.g. USDC).
- Prove that:
  - deposit mints fTokens
  - withdraw burns fTokens
  - update_rate succeeds and updates the exchange price

> This isolates protocol correctness from Marginfi integration issues.

### Phase B — Marginfi bank integration (bank + init position)
- Add a Marginfi bank wired to the protocol market.
- Run init position.

### Phase C — Deposit/withdraw through Marginfi (math checks)
- Single tx: update_rate + deposit
- Single tx: update_rate + withdraw
- Verify exact amounts returned.

### Phase D — Exchange rate moves (interest accrual)
- Warp slot/time, update_rate, withdraw.

### Phase E — Receivership compatibility
- Ensure the transaction allowlists allow:
  - update_rate before start liquidation
  - juplend_withdraw during receivership

---

## 4. Open questions / dependencies (blocking TS e2e tests)

1) **Bankrun fixture**
- For Drift/Kamino/Solend, we load real program `.so` files via `Anchor.toml` `[[test.genesis]]` entries.
- For JupLend we need to decide:
  - compile and add a real JupLend `.so` fixture, or
  - create a minimal-purpose test-double program that implements the subset of instructions we CPI into.

2) **Which program IDs are involved**
- JupLend appears to have multiple programs in the ecosystem (lending, vaults, merkle distributor, etc.).
- For the first milestone, we only need the **lending** program (update_rate, deposit, withdraw).
- Merkle distributor + rewards are deferred.

3) **Same-slot freshness mechanics in bankrun**
- We need to confirm the best pattern for ensuring same-slot:
  - include update_rate + deposit/withdraw in the same tx (preferred)
  - avoid relying on separate txs landing in the same slot

---

## 5. Suggested first TS test file names

Following existing naming conventions (and matching what we’ve implemented so far):

- `jl01_juplend_basic.spec.ts` — add bank + init position + basic deposit/withdraw
- `jl02_juplend_interest.spec.ts` — interest accrual + exact share maths
- `jl03_juplend_stale_health_pulse.spec.ts` — strict same-slot freshness for health pulse
- `jl04_juplend_oracle_price_conversion.spec.ts` — oracle price conversion scales with `token_exchange_price`
- `jl07_juplend_position_limit.spec.ts` — 8 JupLend positions limit + health pulse ordering
- `jl08_juplend_activation_gate.spec.ts` — bank starts paused; permissionless ATA doesn't activate; activation required

- `jl09_juplend_receivership_allowlist.spec.ts` — receivership allowlist: update_rate can be prepended to start_deleverage


