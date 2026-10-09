# marginfi v0.1.12-rc1

# Summary

## Variable Borrow Premium

Borrowers can now be charged extra interest, called the "variable borrow premium", on top of a
bank's normal borrow rate. The premium depends on the type of collateral that backs the debt. For
example, borrowing against a volatile asset may cost more than borrowing against a stablecoin. These
funds go to a "fee pool" that is intended to subsidize less risky borrowers.

In the near future, expect to see higher borrowing rates for riskier assets like memecoins,
governance tokens, etc - but also incentives and better rates on bluechip assets and stablecoins.
Expect to see more unique borrowable tokens that are not available on other platforms.

### Quick Example:

Our high-level goal is for users to pay a borrow premium in proportion to the net risk of their
portfolio.

Assume the admin has set a premium of **10% APR** for the (BONK, STABLE) pair, and has set **no**
premium for (STABLE, STABLE). USDC and USDT are both tagged STABLE. Each user borrows **$5 USDT**.

| User            | Collateral            | Premium rate calculation                  | Premium rate | Premium per year on $5 USDT |
|-----------------|-----------------------|-------------------------------------------|--------------|-----------------------------|
| Pure USDC lender | $10 USDC             | ($10 × 0%) / $10                          | 0%           | $0.00                       |
| Pure BONK lender | $10 BONK             | ($10 × 10%) / $10                         | 10%          | $0.50                       |
| Mixed lender     | $10 USDC + $10 BONK  | ($10 × 0% + $10 × 10%) / $20              | 5%           | $0.25                       |

The premium rate is the average of each collateral's pair rate, weighted by that collateral's USD
value. Collateral with no premium pair (here, USDC against USDT) counts at 0% and lowers the
average. The premium is charged on top of the bank's normal borrow interest, as simple
(non-compounding) interest on the amount borrowed.

Note that adding collateral can raise your premium. If the pure USDC lender above deposits $10 BONK,
their $5 USDT loan is now partly backed by BONK, so its premium rises from 0% to 5%. This happens
even though the loan itself has not changed.

The reverse is also true: depositing USDC in a pure BONK account lowers the premium, because a
smaller share of the loan is backed by BONK.

Users now face market pressure to diversify their holdings such that riskier lending positions are
isolated from less risky positions, and generally have an incentivize to lend less risky assets,
especially those with high borrowing demand. 

### Quick Technicals:

- Each bank has a `premium_tag`. The group stores a table (up to 64 entries) that maps a pair of
  tags, (collateral tag, liability tag), to an annual rate.
- Premium applies only to liabilities in banks where premium is switched on (see bank flag bit 13,
  `PREMIUM_ACTIVE`).
- Each liability stores a rate snapshot (`premium_rate_snapshot`). The snapshot is the average of
  the pair rates across the account's collateral, weighted by the collateral's USD value. Untagged
  collateral counts toward the total at a rate of zero, which lowers the average. Isolated
  collateral, and collateral with a maintenance weight of zero, is left out.
- The premium accrues as simple (non-compounding) interest on the liability amount. Accrued premium
  is moved into `Balance.premium_outstanding` each time the balance changes.
- Repayments pay premium first, then principal. Collected premium goes to the fee wallet
  (`FeeState.premium_wallet`).
- On bankruptcy, the outstanding premium is written off. It is not socialized to lenders or paid
  from the insurance fund.

The snapshot is refreshed by any instruction that runs a full health check: borrow, withdraw
(all venues), flashloan end, liquidation, order and rebalance end, and `pulse_health`. Deposit and
repay do not refresh it. `pulse_health` can be used as a permissionless crank to refresh the rate.

After the upgrade, premium is off for every bank until the admin switches it on. See **Breaking
Changes** for how premium affects health, debt display, and repay amounts.

## Auto-rebalance Orders

Users can now place a persistent order that keeps a deposit of one mint in the highest-yield bank
from a list of 2 to 8 banks of that mint. Integration venues (Kamino and Juplend) are supported.
This empowers users to earn the best APY among, for example, all USDC or SOL banks P0 supports,
without having to manually move their position.

Keepers move the deposit when another bank on the list pays more, and earn a SOL tip for doing so.
The order stays open until the user closes it. Anyone can run a Keeper, the process is of moving
funds is permissionless.

Order settings:

- `allowed_banks` - the banks the deposit may move between. Each must use the same mint (for
  integrations, this is the native mint, not the ctoken or ftoken mint). The account must hold a
  deposit in at least one of them, and must not owe a debt to any of them. For example, the user
  might pick [P0 native USDC bank, Kamino main pool USDC bank, Kamino Maple market USDC bank,
  Juplend Earn USDC bank]
- `min_improvement` - how much higher the new bank's rate must be, as an absolute APR. (Default 5%,
  additively, i.e., a deposit of 2% will only move for a rate of 7% or better)
- `cooldown_seconds` - minimum time between moves. Default 1 day.
- `amount` - the most token that can move in one execution, in native units. Default 0 (no limit).
- `keeper_tip` - lamports paid to the keeper per full move. Default 0.

Keeper tips come from a per-account SOL **fee pool**. Anyone can top it up; only the account
authority can withdraw from it. The tip is held in escrow after a move. It is paid to the keeper
only if, over a settlement window (10 minutes to 1 hour), the destination bank actually earned more
than the source bank. Otherwise the tip returns to the fee pool.

An open rebalance order counts as an active order. Like limit orders, having active orders prevents
the account from being closed or transferred.

## Liquidation Tagging

Anyone can now "tag" an unhealthy account with `marginfi_account_tag_liq_record`. Tagging starts a
timer that slowly raises the maximum premium a liquidator can take in a receivership liquidation:

- For the first hour after tagging, the maximum premium is the usual base: the greater of
  `FeeState.liquidation_max_fee` and 5%.
- After that, it rises linearly until it reaches 100% at 7 days after tagging.

This makes hard-to-liquidate positions worth clearing over time. The premium does not grow for
accounts that are already in bad debt, or whose health is boosted by e-mode.

The tag can be cleared when the account becomes healthy. After a liquidation that leaves the account
unhealthy, the timer restarts if the liquidation cleared at least 25% of the health shortfall or
repaid at least 25% of the debt, otherwise it stays active (Note: trying to be sneaky by liquidating
just 24.9% is fair game, but expect to be beaten by a greedier liquidator, as many third-parties
compete for liquidations on P0). 

Classic liquidation (`lending_account_liquidate`) is not affected by tagging.

## Governance Admin Split

This change only affects admin clients. See **Admin Instructions** below.

Group administration is now split between two authorities:

- **Admin** - the existing fast authority. Handles day-to-day, risk-reducing changes: limits,
  interest rates, fees, circuit breakers, pausing a bank or setting it to reduce-only, freezing
  accounts, and premium settings.
- **Governance admin** - a new, slow (timelocked) authority. Handles risk-increasing changes: adding
  banks, oracles, asset weights, risk tiers, e-mode, staked settings, returning a bank to
  `Operational`, and unfreezing accounts.

The Governance admin will be a timelocked multisig.

## Bank Account Resize

`Bank` grows from **1,856** to **3,904** bytes (excluding the Anchor discriminator). All the added
space is reserved padding at the end of the account. All txes will fail until the change is
completed, which we expect will take no more than five minutes after the program goes live to
mainnet. See **Breaking Changes** for more details.

## Other Changes

- **Kamino market emergency.** When a Kamino reserve, or its whole lending market, enters emergency
  mode, collateral in that Kamino bank no longer counts toward new borrows (i.e., its initial weight
  becomes zero). Its maintenance value is unchanged, so this does not make accounts liquidatable.
  Market-level emergency is copied onto the bank by the new permissionless
  `propagate_kamino_market_emergency` instruction.
- **Scope oracles for Kamino and JupLend banks.** Two new oracle setups, `ScopeKamino` and
  `ScopeJuplend`, price the underlying token with Scope and apply the venue's exchange rate, just
  like Pyth variants currently do.
- **Price and yield history.** A new on-chain archive stores hourly price and native APY snapshots
  for each mint over a rolling 7 days. It is written by a single authorized `snapshot_manager`, and
  anyone can read it. This is useful for on-chain consumers trying to read the "net APY" including
  local yields from e.g. LST appreciation.
- **Durable nonces blocked for admin actions.** Admin instructions, and `handle_bankruptcy`, now fail
  if the transaction uses a durable nonce.
- **Disabled accounts** can no longer be moved with `transfer_to_new_account` or
  `transfer_to_new_account_pda`.

# Breaking Changes (everyone)

# Major
 - Health checks, including for liquidation, now also include premium (see `Borrow premium changes
   debt and health`)
 - Borrows now require all collateral oracles if variable borrows premiums are active (see `Borrow
   premium changes debt and health`)

## Required bank resize

Because `Bank` grows (see Summary), the upgraded program cannot load a bank that has not been
resized. **Any transaction that touches an unresized bank will fail.** This includes health checks
for any account that holds a position in that bank.

We will resize all production banks right after the program update lands, using the new
permissionless `lending_pool_resize_bank_account`. We expect the outage to last a few minutes.

The first 1,856 bytes of each bank are unchanged, so existing decoders that read the old layout
will still work. Decoders that check the exact account size must accept the new size.

## Closing an account needs a new account

`marginfi_account_close` and `admin_close_account` require a new trailing account,
`rebalance_fee_pool`. This is a PDA with seeds `["rebalance_fee_pool", marginfi_account]`. The
account is required even if the user never used auto-rebalance.

The close fails unless the fee pool holds zero lamports. If it holds any, the account authority must
first call `marginfi_account_withdraw_rebalance_fee_pool`.

## Borrow premium changes debt and health

These points apply only to liabilities in banks with `PREMIUM_ACTIVE` set. Until any bank has it
set, nothing changes in practice. Integrators should still update now.

- **Debt display.** A borrower's debt is now:

  ```
  principal (liability_shares * liability_share_value)
  + premium_outstanding
  + principal * premium_rate * seconds_elapsed / SECONDS_PER_YEAR
  ```

  - `premium_rate` is `premium_rate_snapshot` decoded with `u32_to_milli` (`u32::MAX` = 1000%).
  - `seconds_elapsed` = now − max(`balance.last_update`, `bank.premium_activated_at`).
  - The type crate exports `accrued_premium_total` and `premium_elapsed_seconds` to do this for you.
  - If the bank does not have `PREMIUM_ACTIVE` set, treat the premium as zero. Any stored premium is
    written off the next time the balance changes.
- **Health.** Health checks now add the projected premium to the liability side. An account that
  does nothing slowly loses health and can become liquidatable. Off-chain health and liquidation
  checks must include the premium.
- **Repay.** `repay` pays premium before principal: repaying exactly the principal leaves a debt
  equal to the premium. `repay_all` pulls principal plus premium, so users need more tokens than the
  principal alone. The `amount` in `LendingAccountRepayEvent` now includes the premium portion.
- **Classic liquidation.** The liquidator's repayment reduces the liquidatee's principal only. The
  liquidatee's premium stays outstanding.
- **New oracle requirement.** If the account has premium-bearing debt, `lending_account_borrow` and
  `lending_account_end_flashloan` now fail with `PremiumSnapshotUnavailable` (6615) when any
  collateral oracle cannot be priced. This applies even to collateral the health check would
  normally ignore, such as stale or reduce-only balances. Withdrawals do not fail in this case.
  Instead, the premium rate can only go up, and any collateral that cannot be priced is charged its
  full pair rate. 
  
  This breaks an old trick that some integrators may have relied upon. Previously:
```
  Lending $5   A 
  Lending $5   B
  Borrowing $1 C
```
Could process a borrow with
```
Valid oracle for A
Stale oracle for B
Valid oracle for C
```
As long as the collateral requirement for C was met, the ix succeeds, so callers might pass lazily
pass A or B, omitting the other one. Now, this ix would fail: if C has variable borrow premium
enabled, B needs a valid oracle. This is true even if B is not part of the variable borrow premium
pairs for C. If B is isolated or zero weight, it can continue to be skipped as before. In summary,
callers should now expect that ALL oracles must be non-stale and valid when processing a borrow ix.

## Balance field repurposed

In `Balance`, the deprecated `emissions_outstanding` field is renamed `premium_outstanding`, and
`last_update` now records the last time premium was added to the balance. The old emissions values
were cleared and are all zero as of this update. Decoders that use the old field names must update.

## Fewer "costly" positions per account

The limit on integration positions (Kamino, Drift, Solend, JupLend) drops from **8 to 4**. Staked
collateral positions now count toward the same limit. The limit is checked when a new balance is
opened. This is to avoid pressure on liquidations, which must pass a large amount of accounts,
leaving them otherwise dependent on expensive bundle txes to land e.g., all the third-party refresh
instructions for external reserves/markets.

Error 6073 is renamed `IntegrationPositionLimitExceeded` -> `CostlyPositionLimitExceeded`, and the
type-crate constant `MAX_INTEGRATION_POSITIONS` is renamed `MAX_COSTLY_POSITIONS`.

# Breaking Changes (liquidators)

- **Tag accounts that aren't profitable to liquidate.** See an account that's unhealthy, but not
  profitable to liquidation? Tag it with `marginfi_account_tag_liq_record` for later.
- **Receivership premium may now improve over time.** Before building an `end_liquidation`, check if
  the account is tagged, and if so, work out the allowed liquidation premium from
  `MarginfiAccount.liquidation_tagged_at` and the formula in the Summary. The base premium applies
  instead if the account's `health_cache.flags` has `EMODE_BOOSTED` (8) set, or if its liabilities
  exceed its assets at equity value. Otherwise, the liquidator may take the increased liquidation
  premium, which is up to 100% after one week!
- **The premium limit now applies to small accounts too.** Before, receivership allowed a full
  closeout of accounts with less than $5 of assets. Now, liquidators are limited to the liquidation
  premium even for these low-value accounts. Small accounts may still be left healthy or fully
  cleared.
- **Classic liquidation signer.** The signer must own the liquidator's marginfi account. This now
  also applies during a circuit-breaker halt (before, the admin and risk admin could skip it there).
- **Liquidator premium.** If a liquidator's own account has premium-bearing debt in the collateral
  bank, seized collateral first pays off that premium. Any new premium-bearing debt the liquidator
  takes on starts at the highest pair rate for its tag, until the rate is next refreshed.
- **New health rule for receivership.** The old rule was "health must not get worse". Health may now
  fall by up to the value the liquidator takes above what they repaid. See below.

### Example: the new receivership health check

**Old check:** after a receivership liquidation, the account's maintenance health must not be lower
than before.

**New check:** two conditions must both hold:

1. Maintenance health may fall, but by no more than the value the liquidator took beyond what they
   repaid (`seized - repaid`, measured at unweighted USD value).
2. If the account's assets covered its debt before the liquidation (at unweighted USD value), they
   must still cover it afterwards.

**Why it changed:** seizing collateral lowers health by `weight * seized`, and repaying debt raises
it by `repaid`. At a maintenance weight of 0.8, any premium above 25% therefore lowers health. The
old check silently capped the premium at 25% for such accounts, so the growing premium from
liquidation tagging could never be used. The new check allows a larger premium, as long as the
liquidator does not push a solvent account into bad debt, to accommodate this new premium growth.

Consider an account with $100 of SOL (maintenance weight 0.8) and $85 of USDC debt. Maintenance
health = 0.8 × $100 − $85 = **−$5**, so the account can be liquidated.

| Liquidation                  | Premium | Allowed when                 | After                     | Health after | Old check                  | New check                                          |
|------------------------------|---------|------------------------------|---------------------------|--------------|----------------------------|----------------------------------------------------|
| A: repay $10, seize $10.50   | 5%      | Any time (base premium)      | $89.50 SOL, $75 debt      | −$3.40       | ✅ Pass (health rose)       | ✅ Pass (health rose)                               |
| B: repay $10, seize $14      | 40%     | About 3 days after tagging   | $86 SOL, $75 debt         | −$6.20       | ❌ Fail (health fell $1.20) | ✅ Pass ($1.20 fall ≤ $4 taken; $86 covers $75)     |
| C: repay $20, seize $40      | 100%    | 7 days after tagging         | $60 SOL, $65 debt         | −$17.00      | ❌ Fail (health fell $12)   | ❌ Fail ($60 no longer covers $65: now bad debt)    |

Liquidation C passes condition 1 (the $12 fall is less than the $20 taken), but fails condition 2:
the account started solvent and would end in bad debt, which lenders would absorb.

The premium limit from liquidation tagging still applies separately. Deleverage is unchanged and
still requires that health does not fall.

# Breaking Changes for marginfi-type-crate Consumers, Rust Consumers Parsing Structs

- `Balance`: `_pad0` → `premium_rate_snapshot: u32`; `emissions_outstanding` →
  `premium_outstanding`.
- `Bank`: `_pad_0` → `collected_premium_outstanding`; the tail padding is replaced by
  `premium_tag`, `_pad3`, `premium_activated_at`, and a larger `_padding_1`.
- `MarginfiAccount`: `_padding0` shrinks; adds `rebalance_execution_seq` and
  `liquidation_tagged_at`.
- `MarginfiGroup`: `_padding_0` and `_padding_1` are removed; `_padding_2` shrinks; adds
  `premium_settings`, `premium_entries`, `governance_admin`, and `_padding_3`. `MarginfiGroup` no
  longer derives `Default`; it has a manual all-zero implementation instead.
- `FeeState`: `_reserved0` shrinks from `[u64; 32]` to `[u64; 28]`; adds `premium_wallet`.
- `OracleSetup` gains `ScopeKamino` and `ScopeJuplend`. Exhaustive matches must handle them.
- `MAX_INTEGRATION_POSITIONS` → `MAX_COSTLY_POSITIONS` (value now 4).
- Default crate features are now `["client", "anchor"]`, so `anchor-lang` is included by default.
- New modules and items: premium (`PremiumEntry`, `PremiumSettings`,
  `MarginfiGroup::find_premium_rate`, `accrued_premium_total`, `premium_elapsed_seconds`),
  rebalance (`RebalanceOrder`, `RebalanceRecord`, `RebalanceMove`, `RebalanceRefBank`), `archive`,
  and `monitor_snapshot`.

# New Instructions

### User Instructions

- `marginfi_account_place_rebalance_order(allowed_banks, min_improvement, cooldown_seconds, amount,
  keeper_tip)` - create an auto-rebalance order. One order per account per mint.
- `marginfi_account_update_rebalance_order(...)` - change an order's settings. `None` leaves a
  setting unchanged.
- `marginfi_account_top_up_rebalance_fee_pool(amount)` (permissionless) - add SOL to an account's
  fee pool. The first top-up also pays the pool's rent-exempt minimum.
- `marginfi_account_withdraw_rebalance_fee_pool(amount)` - authority withdraws SOL from the fee
  pool. If the withdrawal would leave less than the rent-exempt minimum, the whole pool is drained.
- `marginfi_account_close_rebalance_order` - close a rebalance order. The authority can close
their own order at any time, except during a rebalance. Anyone can close an order once the
account no longer holds a balance in any of the order's banks; in that case the caller chooses
fee_recipient and keeps the rent.

Note: To close a marginfi account that has a rebalance order, first close the order, then withdraw
everything from the fee pool. The account cannot be closed or transferred while it has an open order
or a non-empty fee pool.

### Keeper Instructions

- `marginfi_account_start_rebalance(moves, execution_seq)` and `marginfi_account_end_rebalance` -
  the start and end of a rebalance transaction. Between them the keeper uses the normal
  deposit/withdraw instructions for each venue on the rebalanced account. Rules:
  - `execution_seq` must equal the account's `rebalance_execution_seq`.
  - `end_rebalance` must be the last instruction. Neither start/end can be called by CPI.
  - The transaction may only contain marginfi deposit/withdraw instructions on the order's banks,
    compute-budget instructions, token/ATA instructions, and venue refresh instructions.
  - `remaining_accounts` must list every bank on the order's list, not only the banks being moved,
    like most other risk checks.
  - The end signer must be the executor named at start.
- `marginfi_account_settle_rebalance_tip` (permissionless) - after the settlement window, if
  profitability conditions were met, pay the escrowed tip to the keeper, else return it to the fee
  pool. Rent from the rebalance record goes to the keeper as a tiny bonus tip.

### Liquidator / Permissionless Instructions

- `marginfi_account_tag_liq_record` (permissionless) - tag an unhealthy account, or clear the tag on
  an account that is healthy again. Pass the standard risk accounts in `remaining_accounts`. No
  signer is required. Fails with `AccountAlreadyTagged` (6054) if the account is already tagged and
  still unhealthy, or `HealthyAccount` if it is healthy and has no tag.
- `lending_pool_collect_bank_premium_fees` - send collected premium from a bank's liquidity vault
  to the premium wallet's canonical ATA. The ATA must already exist.
- `propagate_kamino_market_emergency` (permissionless) - copy a Kamino lending market's emergency
  flag onto a Kamino bank. Sets or clears bank flag `KAMINO_MARKET_EMERGENCY` (bit 14). While the
  admin will do their best to do so, anyone can run this in a timely fashion if Kamino enters
  emergency mode. As usual, propagation delays or failures to propagate are outside the scope of our
  bug bounty. Note: this is only required for market-level emergency mode, a reserve-level emergency
  mode is automatically picked up instantly with no need to propagate.
- `lending_pool_resize_bank_account` - grow one bank to the new size. The payer covers the extra
  rent. The admin will run this for all banks on the main group
  (4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8), other administrators should expect to run this on
  their own banks.

### Admin Instructions

- `marginfi_group_set_governance_admin(new_governance_admin)` - the admin sets the governance admin
  once, while it is unset. After that, only the governance admin can change it. This ix will be
  removed in a later update.
- `marginfi_group_configure_gov(...)` (governance admin) - sets the admin, e-mode admin, risk
  admin, and e-mode / same-asset e-mode leverage caps (formerly generic group_configure actions).
- `lending_pool_configure_bank_gov(BankConfigGov)` (governance admin) - see **Changes to Existing
  Instructions**.
- `lending_pool_configure_group_premium(collateral_tag, liability_tag, rate)` (admin) - set one
  entry in the premium table. A rate of 0 removes the entry.
- `lending_pool_configure_bank_premium(premium_tag, active)` (admin) - set a bank's premium tag and
  switch premium on or off.
- `edit_fee_state_premium(premium_wallet)` (global fee admin) - set the premium wallet.
- `monitor_archive_initialize(snapshot_manager)` and `monitor_archive_upsert_batch(updates)`
  (snapshot manager) - set up and write the price/APY history archive.

# Changes to Existing Instructions

### User Instructions

- `marginfi_account_close` - new trailing `rebalance_fee_pool` account (see Breaking Changes).
- `transfer_to_new_account` / `transfer_to_new_account_pda` - now fail for disabled accounts. Also
  blocked while a rebalance order is open, as for any active order.
- `repay` / `repay_all` - pay the premium (see Breaking Changes).
- `lending_account_borrow` / `lending_account_end_flashloan` - may fail with
  `PremiumSnapshotUnavailable` (see Breaking Changes).
- Frozen accounts can now be operated only by the governance admin, not the admin.

### Liquidator / Permissionless Instructions

- `lending_pool_handle_bankruptcy` - new trailing `instruction_sysvar` account. Also writes off the
  account's outstanding premium.
- `admin_close_account` - new trailing `rebalance_fee_pool` account, which must be empty.
- `end_liquidation` - see **Breaking Changes (liquidators)**.

### Admin Instructions

These changes affect only admin tooling:

- Almost all admin instructions now take a trailing `instruction_sysvar` account and reject
  durable-nonce transactions (`DurableNonceNotAllowed`, 6605). This includes `start_deleverage`,
  `end_deleverage`, and `purge_deleverage_balance`.
- `marginfi_group_configure` now takes only `new_admin`, `new_curve_admin`, `new_limit_admin`,
  `new_flow_admin`, `new_emissions_admin`, and `new_metadata_admin`. The e-mode admin, risk admin,
  and leverage caps moved to `marginfi_group_configure_gov` (the "slow" admin).
- `lending_pool_configure_bank` now takes `BankConfigFast` instead of `BankConfigOpt`:
  - `BankConfigFast` (admin): deposit/borrow limits, operational state, interest rate config,
    total asset value init limit, permissionless bad debt settlement, liquidation fees, and all
    circuit-breaker settings. It may only change the operational state in a risk-reducing
    direction.
  - `BankConfigGov` (governance admin, via `lending_pool_configure_bank_gov`): asset and liability
    weights, risk tier, asset tag, oracle max confidence and age, tokenless repayments, freeze
    settings, and returning a bank to `Operational`.
- These instructions now need the governance admin instead of the admin or e-mode admin (the
  signer account is renamed `governance_admin`): all `lending_pool_add_bank*` variants,
  `lending_pool_clone_bank`, `lending_pool_configure_bank_oracle`,
  `lending_pool_configure_bank_oracle_scope`, `lending_pool_set_oracle_price`,
  `lending_pool_configure_bank_emode`, `lending_pool_clone_emode`,
  `lending_pool_init_same_asset_emode_registry`,
  `lending_pool_set_bank_same_asset_emode_eligibility`, `init_staked_settings`,
  `edit_staked_settings`, `disable_staked_oracles`, and `enable_staked_oracle_onramp`.
- `marginfi_account_set_freeze` - the admin can freeze an account; only the governance admin can
  unfreeze it.
- `lending_pool_configure_bank_oracle_scope` - picks `Scope`, `ScopeKamino`, or `ScopeJuplend` from
  the bank's asset tag. Kamino and JupLend banks pass their reserve or lending account after the
  Scope feed.
- New config checks: the maximum asset maintenance weight sands-emode drops from 2.0 to 1.0. E-mode
  can still enable weights above 2.0, but e-mode (and same-asset e-mode) leverage must satisfy
  `(total liquidation fee + 0.0001) * maint leverage <= 1.0001`, which caps leverage just under 20x
  at the default 5% total fee (higher leverages can still be achieved lowering either the insurance
  fee or liquidation premium).

# New Accounts

- `RebalanceOrder` - PDA `["rebalance_order", marginfi_account, mint]`. Stores one auto-rebalance
  order.
- Rebalance fee pool - PDA `["rebalance_fee_pool", marginfi_account]`. A system-owned account with
  no data that holds SOL for keeper tips.
- `RebalanceRecord` - PDA `["rebalance_record", marginfi_account, execution_seq (u64 LE)]`.
  Ephemeral; holds one rebalance's escrowed tip until it is settled.
- History archive - a 10 MiB program-owned account (not a PDA). Holds up to 300 mints, each with 168
  hourly snapshots of price and native APY.

# Changes to Existing Accounts

- `Bank`
  - Size grows to 3,904 bytes (see Breaking Changes).
  - Adds `collected_premium_outstanding`, `premium_tag`, and `premium_activated_at`.
  - `flags` adds `PREMIUM_ACTIVE` (bit 13) and `KAMINO_MARKET_EMERGENCY` (bit 14).
- `Balance`
  - `_pad0` → `premium_rate_snapshot: u32`.
  - `emissions_outstanding` → `premium_outstanding`.
  - `last_update` now means "last premium update".
- `MarginfiAccount` (size unchanged)
  - Adds `rebalance_execution_seq: u64` and `liquidation_tagged_at: i64` (0 = not tagged), taken
    from reserved padding.
  - `account_flags` adds `ACCOUNT_IN_REBALANCE` (1 << 8).
  - `active_orders` now also counts rebalance orders.
  - `HealthCache.flags` adds `EMODE_BOOSTED` (8). It is set when e-mode raises any collateral above
    its bank's own maintenance weight.
- `MarginfiGroup` (size unchanged)
  - Adds `premium_settings`, `premium_entries` (64 entries), and `governance_admin`, taken from
    reserved padding. `governance_admin` is empty until it is set after the upgrade.
- `FeeState` (size unchanged)
  - Adds `premium_wallet`, taken from reserved padding.
- `OracleSetup` adds `ScopeKamino` (27) and `ScopeJuplend` (28). These banks take three risk
  accounts: `[bank, Scope OraclePrices, Kamino reserve | JupLend lending]`. The reserve or lending
  account must have been refreshed in the same slot (just like Pyth variants).

# Event Changes

Changed:

- `LendingPoolBankHandleBankruptcyEvent` gains a trailing `premium_written_off: f64`.
- `LendingPoolBankConfigureEvent` keeps its `BankConfigOpt` schema. Each of the two configure
  instructions fills only its own fields.

New:

- Premium: `LendingPoolGroupPremiumConfigureEvent`, `LendingPoolBankPremiumConfigureEvent`,
  `LendingPoolPremiumFeesCollectedEvent`, `LendingAccountPremiumSettledEvent` (emitted when a
  repayment pays or writes off premium, or when a seized-collateral liquidation credit settles
  premium; it is omitted when no premium is paid or written off).
- Rebalance: `MarginfiAccountPlaceRebalanceOrderEvent`, `MarginfiAccountUpdateRebalanceOrderEvent`,
  `MarginfiAccountCloseRebalanceOrderEvent`, `KeeperCloseRebalanceOrderEvent`,
  `RebalanceFeePoolTopUpEvent`, `RebalanceFeePoolWithdrawEvent`, `RebalanceExecutedEvent`,
  `RebalanceTipSettledEvent`.
- Liquidation: `LiquidationTagEvent` (`tagged_at` is 0 when the tag is cleared).
- Governance: `SetGovernanceAdminEvent`.

# Error Changes

Renamed:

```
6054  Vacated3                          → AccountAlreadyTagged
6073  IntegrationPositionLimitExceeded  → CostlyPositionLimitExceeded
```

New:

```
6140  MixedBankConfigAuthority (unused)
6141  InvalidGovernanceAdmin
6142  MixedGroupConfigAuthority (unused)
6143  InvalidFastBankOperationalState
6144  InvalidGovernanceBankOperationalState
6605  DurableNonceNotAllowed
6610  PremiumEntryInvalid
6611  PremiumMatrixFull
6612  InvalidPremiumAta
6613  PremiumWalletNotSet
6614  PremiumEntryNotFound
6615  PremiumSnapshotUnavailable
6700–6720  Rebalance errors (RebalanceVenueUnsupported … RebalanceTaggedBalanceSplit)
```

# Other Information

### Consolidates

#572, #603, #615, #623, #641, #654, #656, #657, #659, #667, #669, #670, #673, #674, #675, #676,
#677, #678, #685, #686

### Minor bugfixes / notes

- Scope-priced Kamino banks now respect Kamino reserve emergency mode. Before, they always kept
  their borrowing power.
- Exponent PT banks whose vault is in emergency mode can now be priced during a risk-admin
  deleverage, so the position can be unwound. Liquidation still refuses to price them.
- Limit-order execution now runs the circuit-breaker price check at the start.
- `MarginfiError::from(6135)` now returns `SlippageTooHigh` (the mapping was missing).
- Added a new fuzz harness (#675).

### Audit Information

TBD

### Release information

Staging - Roughly Sept 29, 2026

Mainnet - ETA Oct 12, 2026 (exact time pending, check again later)
