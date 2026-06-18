# Summary

### Staked Collateral / SVSP oracle upgrade

This release stages the migration of staked-collateral (SVSP / SPL single-pool) oracle pricing from reading the validator stake account directly to the canonical single-pool NAV formula, which includes the pool's **on-ramp** lamports. The migration is gated behind two new group flags and rolled out in three steps:

1. `disable_staked_oracles` (admin) sets `DISABLE_STAKE` — all staked-bank pricing temporarily panics while the rollout happens.
2. Banks are backfilled with their validator vote account (`lending_pool_backfill_staked_bank_validator_vote_account`).
3. `enable_staked_oracle_onramp` (admin) sets `ENABLE_ONRAMP` (which auto-clears `DISABLE_STAKE`), switching every staked oracle to the new NAV formula.

The whole SVSP-transition surface is temporary and slated for removal once rollout completes (likely 1.10).

### Emissions wind-down (completed)

The emissions removal begun last release is finished. `lending_pool_reclaim_emissions_vault` and `lending_account_clear_emissions` are gone, replaced by a same-bank `lending_pool_emissions_deposit` (deposits residual emissions directly into the liquidity vault, raising `asset_share_value`). `migrate_curve` was also removed.

### Indexer flags

`MarginfiAccount` gains an on-chain `IndexerFlags` block (one byte per flag, `memcmp`-filterable) describing the account: lending-only, empty, single-borrower, ever-liquidated/deleveraged/bankrupted, isolated/staked/kamino/drift/juplend exposure, liquidatable/underwater, 30d/60d activity, trivial balance. Balance-derived flags sync automatically on every balance-mutating ix; activity/risk flags update at `pulse_health`. A permissionless `sync_indexer_flags` batch-backfills existing accounts.

### Account lifecycle

- `admin_close_account` (permissionless): closes an empty account inactive >60 days, rent → group fee wallet.
- `marginfi_account_close_liq_record` (permissionless): closes a liquidation record PDA, rent → original payer.

### Bank provenance & metadata

Banks now record their PDA `bank_seed` and carry `IS_T22` / `BANK_SEED_KNOWN` flags, with permissionless backfills for legacy banks. Bank metadata can now be created/written **before** the bank exists (`init_bank_metadata` relaxed, new `write_bank_metadata_pre_init`).

### Other

- **FeeStateV2**: a new (currently unused) fee-state PDA mirroring `FeeState` with extra padding, plus `init_global_fee_state_v2` / `copy_fee_state_to_v2`.
- **Pause delegation**: a `pause_delegate_admin` can pause (but not unpause or edit fees) the protocol. Pause auto-expiry extended 30 min → 6 hours; consecutive-pause cap 2 → 4.
- **Solana 3 upgrade**: the whole program was ported to Solana/Agave 3 (`AccountInfo` → `UncheckedAccount`, collapsed context lifetimes). No account ordering changes from this, but Rust integrators must recompile.
- Lending deposit/withdraw/borrow/repay events now include `share_amount`.
- Trident fuzz test suite added.

# Breaking Changes (everyone)

- **Staked-collateral banks now require 4 risk/oracle accounts instead of 3.** Anywhere you pack risk accounts for a `StakedWithPythPush` bank (deposit, withdraw, borrow, liquidate, pulse_health, etc.), the 4th account is now the SPL single-pool **on-ramp** account (derived from the bank's validator vote account), where it was previously unused/omitted. This takes effect once the group's `ENABLE_ONRAMP` flag is set; before that, the legacy 3-account behavior still holds, and while `DISABLE_STAKE` is set staked pricing reverts. See `guides/.../PACKING_RISK_ACCOUNTS.md`. Non-staked banks are unaffected.

- The following instructions were **removed**:
  - `lending_pool_reclaim_emissions_vault` → use `lending_pool_emissions_deposit`
  - `lending_account_clear_emissions` → emissions fields are already zeroed; use `admin_close_account` for account cleanup
  - `migrate_curve` (legacy curve migration, no longer needed)

- **JupLend `f_token_vault` PDA seed changed** from `"juplend_f_token_vault"` to `"f_token_vault"`. Anyone deriving this PDA off-chain must update the seed. (`EMISSIONS_AUTH_SEED` was also removed.)

# Breaking Changes (when group rate limits are enabled)

- `lending_account_withdraw`, `lending_account_borrow`, `kamino_withdraw`, `drift_withdraw`, `solend_withdraw`, and `juplend_withdraw` now require the withdrawn/borrowed bank's **oracle group** in `remaining_accounts` (used for USD net-outflow pricing) whenever the group rate limiter is enabled. No effect when group rate limits are off.

# Breaking Changes (Rust integrators only)

- **Solana 3**: all instruction handlers had their context lifetimes collapsed (`Context<'_, '_, 'info, 'info, T>` → `Context<'info, T>`) and `AccountInfo` swapped for `UncheckedAccount`. Source-level recompile required; on-chain account layouts/ordering unchanged.
- `edit_global_fee_state`: every argument changed from required to `Option<_>` (`None` = leave unchanged), and a new trailing `pause_delegate_admin: Option<Pubkey>` arg was added.
- `kamino_withdraw`: the final arg changed from `withdraw_all: Option<bool>` to **`flags: Option<u8>`** — bit 0 (`0x01`) = withdraw all, bit 1 (`0x02`) = refresh reserve via batch refresh. Callers passing `Some(true)` must now pass `Some(1)`.
- `kamino_deposit`: gained a trailing `refresh_reserve: Option<bool>` arg.

# New Accounts

- `FeeStateV2` — V2 fee-state PDA, derived from `b"feestate_v2"`. Currently unused by protocol logic; mirrors `FeeState` with 256 bytes of additional padding.
  - Key fields: `global_fee_admin`, `global_fee_wallet`, `bank_init_flat_sol_fee`, `liquidation_max_fee`, `program_fee_fixed`, `program_fee_rate`, `panic_state`, `liquidation_flat_sol_fee`, `order_init_flat_sol_fee`, `order_execution_max_fee`, `pause_delegate_admin`.

# New Instructions

### Staked / SVSP (temporary, removal expected ~1.10)

- `disable_staked_oracles` / `disable_staked_oracles` (admin) — sets `DISABLE_STAKE`; all staked-bank operations panic during rollout.
- `enable_staked_oracle_onramp` / `enable_staked_oracle_onramp` (admin) — sets `ENABLE_ONRAMP` (auto-clears `DISABLE_STAKE`); every staked oracle switches to the single-pool NAV formula.
- `lending_pool_backfill_staked_bank_validator_vote_account` / same (permissionless) — backfills the validator vote account on existing staked banks; no-op if already set.

### Banks / metadata

- `lending_pool_backfill_bank_is_t22_flag` / same (permissionless) — backfills `IS_T22` on pre-flag banks; optionally backfills `bank_seed` in the same call (`None` skips, `Some(seed)` writes, including `Some(0)`).
- `write_bank_metadata_pre_init` / same (metadata admin) — write ticker/description before a canonical seeded bank is initialized.

### Emissions

- `lending_pool_emissions_deposit(amount)` / same (permissionless) — deposit same-bank emissions directly into the liquidity vault, raising `asset_share_value`. Replaces `lending_pool_reclaim_emissions_vault`.

### Fee state

- `init_global_fee_state_v2` / `initialize_fee_state_v2` (runs once) — initialize the `FeeStateV2` PDA.
- `copy_fee_state_to_v2` / same (permissionless) — copy current `FeeState` values into `FeeStateV2`.

### Account lifecycle / indexing

- `admin_close_account` / same (permissionless) — close an empty account inactive >60 days with no blocking flags; rent → group global fee wallet. Replaces `lending_account_clear_emissions`.
- `marginfi_account_close_liq_record` / `close_liquidation_record` (permissionless) — close a liquidation-record PDA; rent → `record_payer`. Fails if the account is in receivership or deleverage.
- `sync_indexer_flags` / same (permissionless) — batch-sync balance-derived `IndexerFlags` for accounts passed as writable `remaining_accounts`.

# Changes to Existing Instructions

### Admin-only

- `marginfi_group_configure`: `new_emissions_admin` is now **deprecated** and has no on-chain effect (the arg and the `delegate_emissions_admin` field are retained for layout/compat only).
- `edit_global_fee_state`: args are now all `Option<_>`; added `pause_delegate_admin` (see Breaking Changes above).
- `init_bank_metadata`: the bank account no longer needs to exist — callers can pre-create metadata for an upcoming bank pubkey at their own rent expense; the PDA is verified once the bank's seed is on-chain.
- `write_bank_metadata`: now requires the bank to exist and verifies the canonical PDA when the seed is on-chain.

### User

- `kamino_deposit` / `kamino_withdraw`: new `refresh_reserve` / `flags` arguments (see Breaking Changes).
- `lending_account_withdraw` / `lending_account_borrow` / integration withdrawals: oracle-group `remaining_accounts` requirement when group rate limits are enabled (see Breaking Changes).
- Deposit / withdraw / borrow / repay events now carry `share_amount`.

### Pause

- `panic_pause`: callable by `global_fee_admin` **or** `pause_delegate_admin`; auto-expiry 30 min → 6 hours; consecutive-pause limit 2 → 4 (still 3/day).

# Changes to Existing Accounts

- `FeeState`
  - Last 32 reserved bytes are now `pause_delegate_admin: Pubkey` — can pause (not unpause) the protocol; cannot modify fee config.
- `MarginfiGroup`
  - `group_flags`: added bit 1 `DISABLE_STAKE` and bit 2 `ENABLE_ONRAMP` (SVSP transition).
  - `delegate_emissions_admin`: **deprecated**, no on-chain authority (kept for layout/history).
- `Bank`
  - `flags`: added bit 7 `IS_T22` and bit 8 `BANK_SEED_KNOWN`.
  - Added `bank_seed: u64` (consumes reserved padding) — the PDA seed for seed-derived banks; `0` for legacy keypair/pre-backfill banks. Validate provenance with `flags & BANK_SEED_KNOWN`.
  - `integration_acc_1` now also stores the validator vote account for staked-collateral banks.
- `BankCache`
  - Added `price_multiplier: WrappedI80F48` (consumes reserved padding) — integration cToken/token exchange rate; real price = `price_multiplier * last_oracle_price`.
- `BankConfig` / `BankOperationalState`
  - `BankOperationalState`: added `Uninitialized` variant (awaiting JupLend `juplend_init_position` seed deposit; all ops blocked; not reachable via `lending_pool_configure_bank`).
- `MarginfiAccount`
  - Added `active_orders: u8` (count of open Orders, max 255; consumes a padding byte).
  - Added `indexer_flags: IndexerFlags` (24 bytes, consumes reserved padding) — see Indexer flags summary above.

# Other Information

### Consolidates

#526, #528, #532, #540, #542, #543, #545, #557, #558, #559, #561, #563, #564, #568, #569, #570, #573, #574, #575, #576, #578, #580, #581, #589, #591, #592, #594, #597, #598

### Minor bugfixes / notes

- Fix `i128 → i64` overflow panic in `StakedWithPythPush` pricing (#559).
- Minor fixed-price footgun fix (#545).
- "Deposit up to limit" fix (#532).
- Removed the legacy super-admin instructions (#557).
- Docs reorganized: new `p0-cli` (replacing `marginfi-cli` docs), expanded `PERMISSIONS_AND_ROLES.md`, `PACKING_RISK_ACCOUNTS.md`, `BANK_STATE.md`, and `RATE_LIMITS_AND_DELEVERAGE_WITHDRAW_LIMITS.md`.

### Audit Information

TBD

### Release information

TBD
