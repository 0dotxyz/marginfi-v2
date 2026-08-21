# marginfi crucible fuzz harness

Stateful invariant fuzzer for the marginfi program, built on
[crucible](https://github.com/asymmetric-research/crucible) and run on FuzzCorp.
It replays randomized instruction sequences in an in-process LiteSVM and checks
protocol invariants (conservation, solvency, liquidation, access control, …) after
every step.

Self-contained crate — it has its own `[workspace]`, so it is **not** part of the
marginfi-v2 build. It tracks crucible `main`.

## Build & run

The harness fuzzes a freshly-built program, so build the program from source first:

```bash
anchor build -p marginfi                                    # from repo root
cp target/deploy/marginfi.so fuzz/crucible/programs/marginfi_program.so
cd fuzz/crucible
cargo build --release --features invariant_test
./build-bundle.sh                                           # → ./bundle
```

## CI

`.github/workflows/fuzzcorp.yml` builds the marginfi program **from the current
source**, compiles the harness against it, and uploads the bundle to FuzzCorp — so
every change to the program is compiled and fuzzed. Required repo secrets:

- `CRUCIBLE_TOKEN` — read access to the private `asymmetric-research/crucible`
- `FUZZ_API_KEY` — FuzzCorp API key

Already-reported findings are muted (`SCOUT_CHECK_MUTE`) so campaigns surface only
new signal. If the program's instruction/account interface changes, regenerate the
harness bindings with the generator.
