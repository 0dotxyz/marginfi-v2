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
