### Changes Since RC1

* `group` no longer needed on flashloan/liquidation
* SVSP upgrade flags now on `StakedSettings` instead of the `group`

# Summary

## Staked Collateral / SVSP oracle upgrade

In an upcoming update, SVSP will include "staging" free SOL in the on-ramp in pricing. This release
handles the migration to enabled reading the validator stake account value which now includes the
pool's **on-ramp** lamports. Note: The minimum delegation is also no longer 1 SOL.

### SVSP Migration Plan

The migration is gated behind two new staked-settings flags that are copied onto staked-bank
`Bank.flags` and will be rolled out in three steps:

1. `disable_staked_oracles` (admin) sets `STAKED_ORACLE_DISABLED` on `StakedSettings` (and clears
   `STAKED_ORACLE_PRICE_USES_ONRAMP`) — once propagated to staked banks, all staked-bank pricing
   temporarily panics while the rollout happens. We intend to set this state just before SVSP
   upgrades, for as little duration as possible.

**_Foundation updates the SVSP program_**

2. Banks are backfilled with their validator vote account (now stored as a fourth oracle key)
   (`lending_pool_backfill_staked_bank_validator_vote_account`).
3. `enable_staked_oracle_onramp` (admin) sets `STAKED_ORACLE_PRICE_USES_ONRAMP` on
   `StakedSettings` (and clears `STAKED_ORACLE_DISABLED`). Once propagated, staked banks switch to
   the new NAV formula.

The whole SVSP-transition surface is temporary and slated for removal once rollout completes (likely 1.10).

## Emissions and Legacy Curve Wind-down

The emissions removal begun last release is finished. `lending_pool_reclaim_emissions_vault` and
`lending_account_clear_emissions` are removed. `migrate_curve` was also removed, all banks in the
main group now utilize the new seven-point curve.

## Indexer Flags

`MarginfiAccount` gains a variety of helpful flags for backend consumers that want to quickly fetch
relevant accounts with a single memcmp or learn more about an account at-a-glance.

Flags available (one byte per flag, to enable `memcmp`-filtering):

- is_lending_only - no borrows
- is_empty - no lending or borrowing positions
- is_single_borrower - borrowing just one asset
- has_ever_been_liquidated/deleveraged, has_been_bankrupted - account has ever been subject to
  liquidation, deleverage, or bankruptcy
- has_isolated/staked/kamino/drift/juplend - has a position in the given asset/risk category
- was_liquidatable/underwater - at last health pulse, not canonical if health has not recently been pulsed!
- was_active_30d/60d - idle for given time. Combined with is_empty, this flag indicates an account
  can be closed permissionlessly
- has_trivial_balance - Worth less than $1 in Equity terms, but more than $0

The permissionless `sync_indexer_flags` instruction will batch-backfill existing accounts shortly
after this update goes live. Flags that require time or health information should not be treated as
canonical and update on a best-effort basis.

## Other Changes

### Account Lifecycle

- `admin_close_account` (permissionless): closes an empty account inactive for more than 60 days,
  rent goes to the global fee wallet. Accounts must have no liquidation record or pending orders.
- `marginfi_account_close_liq_record` (permissionless): closes a liquidation record, rent returns to
  the original payer.

### Bank Provenance

Banks now record their PDA `bank_seed` and Token22 status (`IS_T22`), with permissionless backfills
for legacy banks. Banks with the flag `BANK_SEED_KNOWN` have been successfully backfilled.

### Misc

- **FeeStateV2**: a new (currently unused) fee-state PDA mirroring `FeeState` with extra padding,
  plus `init_global_fee_state_v2` / `copy_fee_state_to_v2`.
- **Pause delegation**: a `pause_delegate_admin` can pause (but not unpause or edit fees) the
  protocol. Pause auto-expiry extended from 30 min to 6 hours; consecutive-pause cap from 2 to 4.
  This enables a special pause MultiSig to keep the pause authority and react more quickly during
  emergencies, without putting any other systems at risk.
- **Solana 3 upgrade**: the whole program was ported to Solana/Agave 3 (`AccountInfo` ->
  `UncheckedAccount`, collapsed context lifetimes). Rust integrators must recompile.
- Lending deposit/withdraw/borrow/repay events now include `share_amount`.
- Trident fuzz test suite added.

# Breaking Changes (everyone)

- **After SVSP Upgrade, Staked-collateral Banks require 4 risk/oracle accounts instead of 3.**
  Anywhere you pack risk accounts for a `StakedWithPythPush` bank (deposit, withdraw, borrow,
  liquidate, pulse_health, etc.), the 4th account is now the SPL single-pool **on-ramp** account
  (derived from the bank's validator vote account), where it was previously unused/omitted. Remember
  that all accounts can be read from the oracles_keys array.

  Before the SVSP upgrade takes effect, callers may technically pass any placeholder account
  (including pubkey default) in the on-ramp account slot. The actual on-ramp account is required
  once the bank's `STAKED_ORACLE_PRICE_USES_ONRAMP` flag is set. See `PACKING_RISK_ACCOUNTS.md`
  for more details.

  While a staked bank's `STAKED_ORACLE_DISABLED` flag is set, all staked pricing reverts and SVSP
  related txes will fail regardless. Non-staked banks are unaffected.
  `lending_pool_add_bank_permissionless` now requires `pool_onramp`.

# Breaking Changes (when group rate limits are enabled)

At some point in the near future, we may enable the group-level rate limiter, which protects the
protocol against large withdrawals in dollar terms. When enabled:

- `lending_account_withdraw`, `lending_account_borrow`, `kamino_withdraw`, `drift_withdraw`,
  `solend_withdraw`, and `juplend_withdraw` now require a partial risk check: a non-stale oracle for
  the withdrawn asset must appear in `remaining_accounts` (used for USD net-outflow pricing).

# Breaking Changes (Rust integrators only)

- **Solana 3**: all instruction handlers had their context lifetimes collapsed (`Context<'_, '_,
'info, 'info, T>` changed to `Context<'info, T>`) and `AccountInfo` swapped for
  `UncheckedAccount`. Source-level recompile required; on-chain account layouts/ordering unchanged.
- `kamino_withdraw`: the final arg changed from `withdraw_all: Option<bool>` to **`flags:
Option<u8>`** — bit 0 (`0x01`) = withdraw all, bit 1 (`0x02`) = refresh reserve via batch refresh.
  Callers passing `Some(true)` must now pass `Some(1)`. No update is needed for most consumers
  otherwise, unless they wish to switch to the (more efficient) batch refresh.
- `kamino_deposit`: gained a trailing `refresh_reserve: Option<bool>` arg.

# New Accounts

- `FeeStateV2` — V2 fee-state PDA, derived from `b"feestate_v2"`. Currently unused by protocol
  logic; mirrors `FeeState` with 256 bytes of additional padding.

# New Instructions

### Staked / SVSP (temporary, removal expected ~1.10)

- `disable_staked_oracles` (admin) — sets `STAKED_ORACLE_DISABLED` on `StakedSettings`; after
  propagation, all staked-bank pricing panics during SVSP upgrade rollout.
- `enable_staked_oracle_onramp` (admin) — sets `STAKED_ORACLE_PRICE_USES_ONRAMP` on
  `StakedSettings` (auto-clears `STAKED_ORACLE_DISABLED`); after propagation, every staked oracle
  switches to the single-pool NAV formula.
- `lending_pool_backfill_staked_bank_validator_vote_account` (permissionless) — backfills the
  validator vote account on existing staked banks; no-op if already set.

### Banks / metadata

- `lending_pool_backfill_bank_is_t22_flag` (permissionless) — backfills `IS_T22` on pre-flag banks;
  optionally backfills `bank_seed` in the same call (`None` skips, `Some(seed)` writes, including
  `Some(0)`).
- `write_bank_metadata_pre_init` (metadata admin) — write ticker/description before a canonical
  seeded bank is initialized. Bank metadata can now be created/written **before** the bank exists.

### Emissions

- `lending_pool_emissions_deposit(amount)` (permissionless) — deposit same-bank emissions directly
  into the liquidity vault, raising `asset_share_value`.

### Drift

- `drift_claim_bad_debt(amount, proof)` (permissionless) — claims a Drift bad-debt portal
  allocation for a Drift bank. The bank's `liquidity_vault_authority` PDA must be the claimant in
  Drift's merkle tree. The instruction creates the claimant/global-fee ATAs idempotently, prefunds
  the Drift distributor `ClaimStatus` rent from the payer, claims through Drift's merkle distributor,
  sweeps the claimed tokens to the global fee wallet's canonical ATA, and emits
  `DriftClaimBadDebtEvent`.

### Fee state

- `init_global_fee_state_v2` (runs once) — initialize the `FeeStateV2` PDA.
- `copy_fee_state_to_v2` (permissionless) — copy current `FeeState` values into `FeeStateV2`.

### Account lifecycle / indexing

- `admin_close_account` (permissionless) — close an empty account inactive >60 days with no blocking
  flags, no liquidation record, and no open orders; rent goes to the group global fee wallet.
- `marginfi_account_close_liq_record` (permissionless) — close a liquidation-record PDA; rent goes
  to `record_payer`. Fails if the account is in receivership or deleverage.
- `sync_indexer_flags` (permissionless) — batch-sync balance-derived `IndexerFlags` for accounts
  passed as writable `remaining_accounts`. Note: some flags only sync on risk operations (such as
  health pulse).

# Changes to Existing Instructions

### Admin-only

- `marginfi_group_configure`: `new_emissions_admin` is now **deprecated** and has no on-chain effect
  (the arg and the `delegate_emissions_admin` field are retained for layout/compat only).
- `edit_global_fee_state`: args are now all `Option<_>`; added `pause_delegate_admin`.
- `init_bank_metadata`: the bank account no longer needs to exist — callers can pre-create metadata
  for an upcoming bank pubkey at their own rent expense; the PDA is verified once the bank's seed is
  on-chain.
- `write_bank_metadata`: requires the bank to exist, and verifies the canonical PDA when the seed is
  on-chain. Metadata is now writeable
- `lending_pool_add_bank_permissionless` - now requires `pool_onramp`
- `kamino_init_obligation` - no longer requires oracle accounts (pyth, switchboard, switchboard
  twap, or scope), as it now refreshes through the no-oracle-required batch refresh.

### User

- `kamino_deposit` / `kamino_withdraw`: new `refresh_reserve` / `flags` arguments (see Breaking
  Changes).
- `lending_account_withdraw` / kamino/drift/solend/juplend withdrawals: `remaining_accounts`
  requirement when group rate limits are enabled (see Breaking Changes).
- Deposit / withdraw / borrow / repay events now carry `share_amount`.
- `marginfi_account_close_order`/`marginfi_account_set_keeper_close_flags` - now requires `group`
- `keeper_close_order` - Marginfi account is now mutable

### Liquidators

`start_liquidation`/`end_liquidation` - no longer requires `group`

### Pause

- `panic_pause`: callable by `global_fee_admin` **or** `pause_delegate_admin`; auto-expiry was 30
  min now 6 hours; consecutive-pause limit was 2 now 4 (still 3/day).

# Changes to Existing Accounts

- `FeeState`
  - Last 32 reserved bytes are now `pause_delegate_admin: Pubkey` — can pause (not unpause) the
    protocol; cannot modify anything else.
- `MarginfiGroup`
  - `delegate_emissions_admin`: **deprecated**, no on-chain authority (kept for layout/history).
- `Bank`
  - `flags`: added bit 7 `IS_T22`, bit 8 `BANK_SEED_KNOWN`, bit 9
    `STAKED_ORACLE_DISABLED`, and bit 10 `STAKED_ORACLE_PRICE_USES_ONRAMP`.
  - Added `bank_seed: u64` (consumes reserved padding) — the PDA seed for seed-derived banks; `0`
    for legacy keypair/pre-backfill banks. Validate provenance with `flags & BANK_SEED_KNOWN`.
  - `integration_acc_1` now also stores the validator vote account for staked-collateral banks.
- `BankCache`
  - Added `price_multiplier: WrappedI80F48` (consumes reserved padding) — integration cToken/token
    exchange rate; real price = `price_multiplier * last_oracle_price`.
- `BankConfig` / `BankOperationalState`
  - `BankOperationalState`: added `Uninitialized` variant (awaiting JupLend `juplend_init_position`
    seed deposit; all ops blocked; not reachable via `lending_pool_configure_bank`).
  - `BankOperationalState`: added `ReduceOnlyWithBorrowingPower`, which blocks the same new
    bank-side operations as `ReduceOnly` but still lets existing collateral count toward initial
    health for new borrows.
- `StakedSettings`
  - Added `flags: u64` for desired staked-bank transition flags. `propagate_staked_settings` copies
    these bits onto `Bank.flags`, and new staked banks inherit them at creation.
- `MarginfiAccount`
  - Added `active_orders: u8` (count of open Orders, max 255; consumes a padding byte).
  - Added `indexer_flags: IndexerFlags` (24 bytes, consumes reserved padding) — see Indexer flags
    summary above.

# Other Information

### Consolidates

#526, #528, #532, #540, #542, #543, #545, #557, #558, #559, #561, #563, #564, #568, #569, #570,
#573, #574, #575, #576, #578, #580, #581, #589, #591, #592, #594, #597, #598, #604, #605

### Minor bugfixes / notes

- JupLend Ftoken Vault PDA Seed was `"juplend_f_token_vault"` in type-crate instead of
  `"f_token_vault"`, the actual seed used-chain is `f_token_vault`.
- Fix `i128 → i64` overflow panic in `StakedWithPythPush` pricing (#559).
- Minor fixed-price footgun fix (#545).
- "Deposit up to limit" fix (#532).
- Removed the legacy super-admin instructions (#557).
- Docs reorganized: new `p0-cli` (replacing `marginfi-cli` docs), expanded
  `PERMISSIONS_AND_ROLES.md`, `PACKING_RISK_ACCOUNTS.md`, `BANK_STATE.md`, and
  `RATE_LIMITS_AND_DELEVERAGE_WITHDRAW_LIMITS.md`.
- Now caps max slippage when creating a stop loss (now 10%), fixing a footgun where the user sets
  100% slippage and loses all their funds to the keeper.

### Audit Information

TBD

### Release information

TBD
