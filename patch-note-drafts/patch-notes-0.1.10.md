# Summary

## Per-bank Oracle Circuit Breaker

The admin can now configure price circuit breakers on banks. The breaker temporarily halts all
risk-increasing operations (e.g. borrow, withdraw, liquidate, etc) if a Bank's price changes more
than a set percentage in a set time window. Interest does not accrue during the halt. The duration
of the halt depends on how much the price has moved. If the price moves a large amount, or
repeatedly, then the halt is permanent until cleared by the admin.

Circuit-breaker reference prices use the same multiplier-adjusted price as the risk engine,
including integration exchange-rate multipliers, which means a large movement in an integration
venue's exchange rate can also cause a breaker to trip.

## Same-asset e-mode

Banks using the same mint and oracle can now be placed into same-asset emode, enabling higher
leverage. For example, a Kamino SOL, Juplend SOL, and P0 SOL bank might enter same-asset emode,
enabling leverages of 10x or more when lending SOL to any bank and borrowing from P0. This enables
high-yield loop arbitrage when some bank has a higher lending yield in the same asset as the borrow
rate for that asset. 

A read-only archive of banks (`SameAssetEmodeRegistry`) tracks all banks that have opted in.

Expect assets such as SOL and most Stables to have same-asset emode configured soon.

## Account-size Migration

`MarginfiGroup` and `FeeState` are expanding to add more reserved space. All prod accounts will
immediately be resized just after deployment; see **Breaking Changes** for more details.

## Configurable Fees

Classic-liquidation fees are now configurable per liability bank. The liquidator fee and the
insurance fee both default to the historical 2.5%. Higher risk banks may see a higher fee in the
near future.

Account-transfer fees are also now configurable in `FeeState`, the legacy 5,000,000-lamport applies
currently.

## Minor Oracle and Accounting Fixes

- Switchboard feeds no longer use `std_dev`, which Switchboard no longer populates. For Switchboard
  banks, `oracle_max_confidence` now configures a static confidence spread used for price biasing
  (zero disables it; nonzero is capped at 5% of price). Pyth is unchanged and continues to use
  reported oracle confidence. We expect certain volatile Switchboard assets may have a small static
  confidence applied.
- Various preparation for upcoming SVSP upgrade.
- Kamino and Solend withdrawals now record bank rate-limit outflow in underlying-liquidity units,
  rather than collateral-share units.

# Breaking Changes (everyone)

## Required account-size migration

`MarginfiGroup` grows from **1,056** to **9,248** bytes and `FeeState` grows from **256** to **512**
bytes (excluding the Anchor discriminator). The old layout is byte-identical to the new layout, all
space being added is reserved padding at the end of the account. 

Anchor's exact-size account deserialization check means the upgraded program cannot load an
unresized account, which will briefly cause an outage. We will immediately resize the accounts after
the program update lands, so the duration should be minimal (we expect five minutes or less).

Consumers attempting to deserialize a resized account should generally still be able to, but will
need to upgrade when actual data is added to the new area, which should be expected in the next
version release.

# Existing instruction changes

### Admin Instructions

- `marginfi_group_configure` - now has two trailing arguments: `same_asset_emode_init_leverage` and
  `same_asset_emode_maint_leverage`, both `Option<WrappedI80F48>`.
- `lending_pool_configure_bank` - `BankConfigOpt` has new fields. Can now set
  `liquidation_liquidator_fee`, `liquidation_insurance_fee`, `circuit_breaker_enabled`, three
  deviation thresholds, three halt durations, the escalation multiplier, EMA alpha, long-window
  duration, and long-window up/down caps. Each liquidation fee uses the `u32_to_centi` encoding
  (`u32::MAX` = 100%); zero selects the default 2.5% and each fee is capped at 50%.
- A bank opted into same-asset e-mode cannot have its primary oracle key or oracle feed family
  changed until eligibility is disabled.
- `lending_pool_close_bank` - adds the trailing `force_close: Option<bool>` argument. `Some(true)`
  is an admin-only emergency escape hatch for legacy/non-authoritative position counts; it still
  requires zero asset shares, liability shares, and remaining emissions. It can permanently strand
  user balances or vault funds if used incorrectly. The admin will only use this flag to close banks
  that are genuinely defunct or misconfigured and have no users.
- `edit_global_fee_state` - adds trailing `account_transfer_fee: Option<u32>`.

### User Instructions

- `lending_account_end_flashloan` - now requires `group` account.
- `lending_account_pulse_health` - now requires `group`. Now accrues interest before refreshing the
  price cache.
- `transfer_to_new_account` and `transfer_to_new_account_pda` - now require the global `fee_state`
  PDA so they can read the configurable transfer fee.
- Borrowing, risk-carrying withdrawals, order execution, permissionless bankruptcy, and direct
  liquidation are protected by the circuit-breaker price gate. During a halt, direct liquidation is
  limited to the risk admin; repayment and liability-free withdrawal remain available; depositing is
  allowed only for assets where the user already has an existing position.


### Liquidator Instructions

- `lending_account_start_liquidation` - now requires `group`.
- `lending_account_end_liquidation` - now requires `group`. Adds optional `fee_payer:
  Option<Signer>`. When supplied, this **writable signer** pays the flat liquidation fee; otherwise
  the existing `liquidation_receiver` pays it. `lending_account_liquidate` - calculates the
classic-liquidation discount from the liability bank's individually configured liquidator and
  insurance fees. Existing banks retain the 2.5% + 2.5% default while their stored fee fields are
  zero.

# New Instructions

### Admin Instructions

- `lending_pool_clear_circuit_breaker(reseed_reference)` (group admin or risk admin) - clear an
  active halt or `CircuitBroken` state. Set `reseed_reference` when accepting a valid new price
  level that would otherwise immediately trip the breaker again.
- `lending_pool_init_same_asset_emode_registry` (group admin or e-mode admin) - create the
  group-specific registry PDA.
- `lending_pool_set_bank_same_asset_emode_eligibility(enabled)` (group admin or e-mode admin) - opt
  a bank in or out of same-asset e-mode and maintain the registry. Validates the bank is eligible.
- `lending_pool_resize_group_account` and `resize_global_fee_state`(permissionless) - grow one
  existing group/fee state to the new layout; its payer supplies the incremental rent.


# New Accounts

- `SameAssetEmodeRegistry` - per-group PDA derived from `[b"same_asset_emode_registry", group]`.
  Archives the same-asset-e-mode bank groups configured by admins (up to 32 mint/oracle groups and
  128 banks).

# Changes to Existing Accounts

- `MarginfiGroup`
  - Size grows to 9,248 bytes; gains `same_asset_emode_init_leverage` and
    `same_asset_emode_maint_leverage`, plus reserved space. Existing groups will be resized.
- `FeeState`
  - Size grows to 512 bytes; it gains `account_transfer_fee` and reserved space. Existing fee state
    will be resized.
- `Bank`
  - `flags` adds `CIRCUIT_BREAKER_ENABLED` (bit 11) and
    `BANK_SAME_ASSET_EMODE_ELIGIBLE` (bit 12).
  - Adds per-liability-bank liquidation-fee fields and the circuit-breaker reference state. `Bank`
    remains 1,856 bytes but may grow in a future update.
  - `BankConfig` adds various fields for circuit-breaker configuration.
  - `BankOperationalState` adds `CircuitBroken`.
- `Order`
  - The existing reserved timestamp slot is now `created_at: i64`, orders created prior to this
    upgrade will display 0.


## Event and state changes

- `LendingAccountLiquidateEvent.pre_balances` and `.post_balances` - gain
  `liquidator_liability_bank_asset_balance`.
- `LendingPoolBankConfigureEvent` - gains new fields from `BankConfigOpt`
- `BankOperationalState` - new enum value `CircuitBroken` expresses risk operations are paused util
  a price circuit breaker is lifted.
- `oracle_max_confidence` - value unchanged, but the meaning is now different for Switchboard banks:
  Zero does nothing (as prior to this update), a configured value applies a static confidence to all
  prices, e.g. a 1% value here indicates that if the price is ten dollars, the confidence will be
  +/- ten cents.

# Other Information

### Consolidates

#567, #585, #596, #599, #601, #607, #612, #613, #616, #619, #620, #622, #625, #633, #635, #636

### Minor bugfixes / notes

- Fixes an edge case where banks with a very large asset share value would allow a 1-satoshi
  withdrawal that would burn zero shares.
- The CLI's Kamino reserve-oracle derivation now uses the complete Kamino IDL and selects the
  correct oracle slot.
- Fixes a bug where `withdraw_all` and `repay_all` could leave small amounts of dust in certain edge
  cases.

### Audit Information

TBD

### Release information

TBD
