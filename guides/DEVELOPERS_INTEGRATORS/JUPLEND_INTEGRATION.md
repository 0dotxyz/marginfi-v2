# JUPLEND INTEGRATION GENERAL GUIDE

Are you an administrator, risk manager, developer, or power user of the mrgn program's Juplend
integration and want to know more? Read on!

## Terms

- JupLend (by Fluid) - Includes the Lending, Liquidity, and Rewards programs, which are all
  different programs.
- Lending or Lending "State" or Lending "Pool" - A "bank" in JupLend parlance, similar to a Kamino
  "reserve" or Drift "Spot Market". In marginfi, this is the bank's `integration_acc_1` and is also
  stored as `oracle_keys[1]`.
- Token Reserve (aka `supply_token_reserves_liquidity`) - The Liquidity-side reserve account for a
  mint. This is a JupLend pool account, not a marginfi bank.
- fToken - Interest-bearing share token, like a mrgn bank's "share" in JupLend parlance. Deposits
  mint fTokens and withdrawals burn fTokens. The mrgn bank stores these shares in its `fToken
  vault`.
- fToken vault - The bank's `integration_acc_2` account. This vault holds the fTokens representing
  all user deposits in that wrapped bank.
- cToken - The liquidity-layer supply unit, used extensively by Juplend under the hood, and not to
  be confused with fTokens. This integration does not use cTokens, be wary of accidentally using the
  cToken exchange rate instead of the fToken exchange rate when making calculations.
- Liquidity token or underlying token - The token users deposit/withdraw (e.g. USDC, SOL), in
  native decimals.
- Refreshing Lending - The "Lending" account stores information about the exchange rate of
  fTokens/underlying. The refresh instruction is called `updateRate`. Our `juplend_deposit` and
  `juplend_withdraw` handlers call this internally. For liquidation and other risk-sensitive flows,
  include `updateRate` for all involved Juplend banks in the same tx before health checks that read JupLend state.
- Withdraw intermediary ATA - The bank's `integration_acc_3` account. JupLend withdrawals first land
  here, then marginfi forwards tokens to the user's destination token account. Currently, this is an
  ATA owned by `liquidity_vault_authority`, and must be created outside our program before the first
  withdraw. Will be deprecated when Juplend supports PDA withdraw destinations.
- Mrgn-wrapped or wrapped - We refer to mrgn banks that track Juplend collateral assets as
  "mrgn-wrapped" Juplend banks.

## Notable Instructions

- `lending_pool_add_bank_juplend` (admin) - Adds a wrapped JupLend bank to a group. The bank starts
  in `Paused` state and is unusable until `juplend_init_position` runs.
- `juplend_init_position` (permissionless, one-time per bank) - Performs a seed deposit into JupLend
  and flips the bank from `Paused` to `Operational`. This activates the bank for user flows.
- `create_associated_token_account` (SPL ATA program) - Needed to initialize the bank's withdraw
  intermediary ATA (`integration_acc_3`) before first withdraw. This is not created by
  `lending_pool_add_bank_juplend` or `juplend_init_position`. This requirement will be removed when
  juplend allows withdrawal to pda accounts.
- `juplend_deposit(amount)` (user) - Deposit underlying tokens (native amount). Internally calls
  JupLend `updateRate`, deposits through CPI, verifies minted fTokens, and credits the user's
  marginfi position.
- `juplend_withdraw(amount, withdraw_all)` (user) - Withdraw underlying tokens (native amount).
  Internally calls JupLend `updateRate`, burns fTokens via CPI, then transfers tokens from the
  withdraw intermediary ATA to the user's destination token account.
- `lending_account_liquidate` or `start_liquidation` / `end_liquidation` (liquidators) - Liquidation
  still uses the standard marginfi liquidation instructions. If seized collateral includes JupLend
  assets, the liquidator/receiver claims those assets with `juplend_withdraw`.
- Native JupLend `updateRate` (Lending program) - Used to refresh JupLend exchange rates before risk
  checks. Must run before withdraw, borrow, etc for every bank involved, in the same tx.

## Big Picture Overview

Each mrgn-wrapped Juplend bank tracks exactly one Juplend Lending state (plus its associated
Liquidity reserve/position accounts). In practice, a wrapped bank is marginfi's on-chain "adapter"
for one Juplend lending pool.

Users always deposit the raw underlying asset into the wrapped bank (USDC, SOL, etc). Users do not
deposit fTokens directly, and do not interact with Juplend liquidity-layer cToken units in this
integration.

On deposit, marginfi moves underlying into the bank's liquidity vault, CPI deposits into Juplend,
and receives fTokens, which the bank holds in its fToken vault. You can think of a mrgn-wrapped
deposits as owning a share of the bank's stored ftokens. On withdraw, marginfi burns fTokens via
Juplend and forwards the underlying gained to the user's token account.

User balances in wrapped Juplend banks are tracked in fToken-share units. Like other wrapped
integrations, wrapped Juplend banks do not earn interest through marginfi's internal
`asset_share_value`; yield is captured through Juplend's `token_exchange_price` (fToken appreciation
vs underlying).

Wrapped Juplend banks use dedicated deposit/withdraw instructions (`juplend_deposit`,
`juplend_withdraw`) and cannot be borrowed.

For risk checks such as borrows and liquidations, remember that Juplend pricing depends on the
Lending state exchange rate. Include the correct remaining risk accounts and refresh with
`updateRate` in the same transaction for flows that do not already call it internally. Remember that
Juplend risk accounts are the `bank, oracle, Lending` accounts, in that order.


## Juplend Rewards

### Key Behavior

- Rewards are **same-mint rewards**, not a second incentive token. For a USDC lending pool, rewards are
  USDC-denominated and realized through a higher `token_exchange_price` (fToken -> underlying), not
  through separate token transfers to marginfi users.
- In mrgn-wrapped banks, users earn rewards implicitly by holding wrapped Juplend collateral. As
  `token_exchange_price` rises, their collateral value in mrgn rises.

### One-Time Setup (Admin / Integrator)

- Initialize rewards admin authority (Jup Rewards program): `initLendingRewardsAdmin`.
- Initialize per-mint rewards model PDA: `initLendingRewardsRateModel`.
- Link that rewards model to the lending pool: `setRewardsRateModel` (Jup Lending program).
- For mrgn Juplend withdraw compatibility, initialize the claim PDA for
  `liquidity_vault_authority` + mint via `initClaimAccount`. This instruction is permissionless
  (any signer can pay rent to create it).

### Starting Rewards (Admin)

- Call `startRewards(rewardAmount, duration, startTime, startTvl)` (wrapped in our tests as
  `startJuplendRewardsIx`).
- Important constraints:
  - `rewardAmount > 0`
  - `duration > 0`
  - cannot overlap an existing active period
  - if prior rewards existed, Jup syncs rates first (`updateRate`) before applying new params
- `startTvl` is a floor: if TVL is below this threshold, reward rate is effectively 0.

### How Rewards Accrue and When They Materialize

- Rewards accrue over time but are **materialized on rate refresh** (`updateRate`).
- In mrgn integration, `juplend_deposit` and `juplend_withdraw` already call `updateRate`
  internally, so those user actions crank rewards automatically.
- For pure valuation/health freshness outside deposit/withdraw flows, include native Jup
  `updateRate` in the same transaction before risk checks.

### How Users “Get” Rewards in mrgn

- Users do **not** call a Juplend claim instruction through mrgn to receive rewards.
- Rewards appear as higher collateral valuation (e.g. visible after `healthPulse`).
- To realize rewards into wallet tokens, users withdraw (`juplend_withdraw`), which burns fTokens
  at the newer exchange rate and returns more underlying.

### Stopping / Changing Rewards

- `stopRewards` (wrapped as `stopJuplendRewardsIx`) ends the active schedule early:
  - it first syncs `updateRate`
  - then truncates the current rewards window to elapsed time
- It fails if:
  - rewards are already stopped/ended
  - a next rewards period is queued (must resolve queue first)
- To change emissions while active, protocol-side queue/cancel/transition flows exist; in our mrgn
  integration tests we use start/stop.

### Critical Funding Caveat

- `startRewards` configures emissions/rate math; it does **not** by itself transfer funding into the
  reserve vault.
- Rewards become withdrawable only if underlying liquidity is actually present (typically via
  Jup lending-side rebalancer/funding flow).
- Therefore, it is possible to have value that appears “earned” in exchange-rate/health math, but
  fail on token outflow if reserve liquidity is short.
