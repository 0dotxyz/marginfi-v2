# marginfi crucible fuzz harness

Stateful invariant fuzzer for the marginfi program, built on
[crucible](https://github.com/asymmetric-research/crucible) and run on FuzzCorp.
It replays randomized instruction sequences in an in-process LiteSVM and checks
protocol invariants (conservation, solvency, liquidation, access control, …) after
every step.

Self-contained crate — it has its own `[workspace]`, so it is **not** part of the
marginfi-v2 build. It tracks crucible `main`.

## Build & run

`prepare.sh` builds everything the harness loads from the current source — the marginfi
program (mainnet id, plus an unstripped copy for coverage), `solend_mocks`, the fixture
programs it CPIs into, and `idls/marginfi_program.json`. CI runs the same script.

```bash
fuzz/crucible/prepare.sh                                    # from repo root
cd fuzz/crucible
cargo build --release --features invariant_test
SCOUT_CHECK_RUN=local ./target/release/invariant_test       # ctrl-C to stop
./build-bundle.sh                                           # → ./bundle
```

The IDL is generated, not committed: `declare_fuzz_program!` embeds it at compile time,
so rerun `prepare.sh` after pulling program changes (`build.rs` makes cargo rebuild when
it changes). Without an argument it runs `anchor idl build` (a few minutes); if you already
have a fresh `target/idl/marginfi.json` from `anchor build -p marginfi`, pass its path to
skip that. If instruction signatures changed, `cargo build` then fails at the drifted call
sites in `src/main.rs` — update the harness in the same PR.

## CI

The `fuzz-crucible` job in `.github/workflows/test.yaml` runs on PRs into `main` or a
`*-main` release branch (same guard as trident). It takes the IDL from the `anchor-build`
artifact, runs `prepare.sh`, builds the harness, smoke-runs it for 60s (it must still be
fuzzing when killed — a `setup()` panic would otherwise ship and die on every worker) and
runs `build-bundle.sh`. That is the whole check for PRs into a release branch. The release
PR into `main` also uploads the bundle to FuzzCorp; every feature merged into the release
branch updates that PR, so each merge gets a fuzz campaign. The only secret is
`FUZZ_API_KEY`.

Already-reported findings are muted (`SCOUT_CHECK_MUTE`) so campaigns surface only
new signal.

Account layout constants: bank-reading invariants take their account length from
`SCOUT_BANK_ACCOUNT_LEN = 8 + size_of::<marginfi::state::Bank>()`, so program resizes
(e.g. #667's Bank V1→V2 growth) track automatically. Field offsets are V1-prefix
offsets; the program asserts the prefix stays byte-identical (`offset_of!(Bank,
_padding_1) == Bank::V1_LEN` in marginfi_group.rs). Hardcoding `8 + 1856` instead
silently disarmed ~13 bank-reading property families between #667 and 0.1.12 — do not
reintroduce literal layout sizes.

## Stateful campaigns

The native order/rebalance state machine (`src/native_orders.rs`, focused binary
`--features native_orders_test`) is driven by the same runner as the general fixture.
A focused local campaign looks like:

```bash
cd fuzz/crucible
cargo build --release --features native_orders_test
FUZZ_STATEFUL=1 FUZZ_SEED=42 FUZZ_TIMEOUT_SECS=600 FUZZ_MAX_DEPTH=40 \
  FUZZ_STOP_ON_CRASH=1 SCOUT_CHECK_RUN=local ./target/release/invariant_test
```

Replay caveats (crucible ≤ 086b4235):

- Replay reconstructs fixtures from `setup()` with fresh `OsRng` keypairs and does not
  re-enable stateful chain mode, so a recorded crash sequence is best-effort, not a
  guarantee: addresses differ and `false`-returning actions no longer terminate the chain.
- Keep `FUZZ_STATE_POOL_SIZE` large (default 256k). When the pool is exhausted, state
  reclaim (`swap_remove`) corrupts `parent_idx` lineage, and the recorded sequence can
  disagree with the stored bytes — campaign-42's crash_7285 replayed reordered and did
  not reproduce for this reason.
- The native world must be created in `setup()` (the general fixture calls
  `native_ensure_for_campaign()` at the end of `setup()`), never lazily mid-campaign:
  the stateful pool snapshots the SVM right after setup, and restores lose accounts
  created later through direct `ctx.create_*` calls. A lazily-built world surfaced as
  `native model: missing token account` harness panics in the general fixture.
- Therefore: treat every campaign crash as a prompt to write a deterministic regression
  test in `src/native_orders.rs` (the `tests` module fixtures are self-contained), and
  treat the unit suite as the durable record. Use `FUZZ_DEBUG=1` /
  `FUZZ_DEBUG_REPLAY=1` for state-hash diagnostics when a replay disagrees.

### Native world coverage and what remains

The native world (`NativeWorld` in `src/native_orders.rs`) covers: order placement
validation, trigger evaluation driven by real oracle price moves (`native_move_price`),
execution bounds, cancel/keeper-close/`set_keeper_close_flags`, single- and multi-move
rebalances (1->N, N->1, duplicate-direction, and order-tagged whole moves) with the
proportional-tip and settlement-delay models and a bit-exact model of start_rebalance's
rate gates (per-move improvement and the best-venue rule, including the dust-leg skip),
plus post-move maintenance-health modeling at share granularity (end_rebalance checks the
post-move balance set, so price-moved destinations can reject) and per-side-truncated
equity/health arithmetic matching `calc_value`'s operation order,
the fee pool, liquidation composed with
live order tags (tag survival, dust closure, composition-gate agreement on the chained
execution) and a share-granular model of the liquidation accept/reject gates
(`TooSevereLiquidation`, `ExhaustedLiability`, `WorseHealthPostLiquidation`, and the
repay path's `OperationRepayOnly` dust rule), and accounting/health/rollback invariants
checked after every action.

Not yet covered: **venue rebalance legs** (Kamino/Drift/JupLend banks inside the
rebalance allowlist). The general fixture stages these venue programs and drives them
through scenario actions, and `prepare.sh` already builds the artifacts. What a venue
leg needs in the native world:

1. A sixth bank added via `lending_pool_add_bank_kamino` (see `scout_valid_kamino_config`
   and the staged reserve/obligation/oracle accounts in `main.rs`'s setup) so the
   allowlist can span a venue bank with a distinct rate source.
2. Venue-aware leg instructions in the rebalance sandwich (`kamino_deposit` /
   `kamino_withdraw` account blocks: reserve, obligation, farms state) instead of the
   native `LendingAccountDeposit`/`LendingAccountWithdraw` legs used today.
3. Model support for `venue_multiplier != 1`: `native_underlying`, the reconciliation
   dust bound, and the settlement yield index currently assume native banks
   (`multiplier == 1`); a Kamino leg reads the reserve's cToken exchange rate via
   `venue_multiplier`/`yield_index_of` in `rebalance.rs`, which the model must mirror
   from the staged reserve bytes.
4. Rate ordering must then come from the venue exchange rate instead of native
   utilization, so the "destination beats source" pre/post checks keep meaning
   something for the venue leg.

### Kamino venue world (general fixture)

The general fixture drives the real klend mainnet build (`programs/kamino_lending.so`,
copied from `tests/fixtures/` by `prepare.sh` — the old `kamino_lending_nocpi.so` stub
lacks the `*_v2` instructions marginfi CPIs). `setup()` stands up a self-consistent
klend world for the kamino-ops bank entirely through real instructions:

1. A fabricated lending market + reserve. The fabricated bytes must satisfy the real
   binary, not just the mock layout: `reserve.version = 1` (klend `PROGRAM_VERSION`,
   else `ReserveDeprecated`), `last_update.slot = 0` (a fabricated `u64::MAX` makes
   klend's `slots_elapsed` subtraction overflow), the market's `bump_seed` (u64 @ 8+8)
   must equal the real `["lma", market]` bump (klend validates the authority PDA with
   `bump = lending_market.bump_seed`), and `liquidity.market_price_sf` must be nonzero
   (klend's post-deposit/post-withdraw obligation invariants divide by the deposited
   market value; a zero price is a u256 division-by-zero panic inside the venue CPI).
2. Real `lending_pool_add_bank_kamino` and a real `kamino_init_obligation` seed
   deposit, so the obligation, klend user metadata, and collateral mint state are all
   program-created (never forged), and the stateful pool's setup snapshot contains them.

Three actions then run against that world: `KaminoInitObligation` (re-init must always
be rejected, with full state rollback), `KaminoDeposit` and `KaminoWithdraw`, each with
a `KAMINO-OPS` property checked on success (signer/obligation/total-share accounting to
within the venue's ±1 rounding) and a full venue-state rollback check on rejection.

Slot-staleness caveat: klend requires the reserve *and* obligation to be refreshed in
the current slot (`STALE_AFTER_SLOTS_ELAPSED = 1`), and marginfi's venue instructions
never refresh the obligation — production clients bundle permissionless klend
`refresh_reserves_batch` + `refresh_obligation` cranks into the same transaction. The
deposit/withdraw actions do the same via `raw_call` + `send_batch`; without the cranks
every post-setup action fails `ObligationStale`. The cranks use
`skip_price_updates = true` (the fabricated `token_info` is zeroed and would fail
oracle validation), which is also what marginfi itself passes for its internal refreshes.

Not covered here: `kamino_harvest_reward` (needs a full farms world), venue banks inside
native rebalance allowlists (see above), and the pre-existing `kamino_withdraw` fixture
accounts, which are state-only scaffolding for P-0005/P-0035 and are not driven by any
action.


