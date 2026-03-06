# NEW DEV QUICKSTART GUIDE

New developer getting started working on the mrgnv2 program side? Read on.

## Things to Install (January 2026)

- rust toolchain - 1.79.0
- node - 23.0.0
- yarn - 1.22.22
- avm - 0.30.1 or later
- anchor - 0.31.1
- solana - 2.1.20
- cargo-nextest - use `cargo install cargo-nextest --version "0.9.81" --locked` exactly
- cargo-fuzz - 0.12.0

# Running tests

## For Rust unit tests:

```
cargo test --lib
```

## For the TS test suite:

```
anchor build
anchor build -p marginfi -- --no-default-features --features custom-heap
anchor test --skip-build
```

Note: you may need to build the other programs (mock, liquidity incentive, etc) if you have never
run `anchor build` before.

Note: you need to `yarn install` before your first run

Segmentation fault? Just try again. That happens sometimes, generally on the first run of the day.
Sometimes it happens on the CI pipeline as well, just kick it again it that occurs.

Each letter prefix is referred to as a "suite" and is broadly end-to-end. The localnet tests
multithread with bankrun will create a fairly substantial CPU load. Completetion varies
substantially by hardware. If you your workflow is too slow, go to this portion of `Anchor.toml` and
comment out the top line, comment in the suite you actually want to run:

```
[scripts]
test = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/*.spec.ts --exit --require tests/rootHooks.ts"

# Staked collateral tests only
# test = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/s*.spec.ts --exit --require tests/rootHooks.ts"

# Pyth pull tests only
# test = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/p*.spec.ts --exit --require tests/rootHooks.ts"

# Edmode tests only
# test = "RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/e*.spec.ts --exit --require tests/rootHooks.ts"
```

Note: You cannot run individual tests, most of the tests in a suite must run in order, where the
number after the prefix determines their run order through the magic of filenames.

## For the Anchor test suite:

```
anchor build --no-idl
./scripts/test-program.sh all
```

This is much slower than the remix test command, but stable on any system.

## Customize Your Rust testing experience:

```
./scripts/test-program-remix.sh -p marginfi -l warn -c mainnet-beta -f mainnet-beta -j 8
```

Where the number after j is how many threads you want to use. More threads = more likely to experience random failures for no reason, but it sure is faster!

This will throttle your CPU and may error sporadically as a reminder to buy a better CPU if you try to do anything else (like say, compile another Rust repo) while this is running. It is approximately 10x faster than test-program.sh, so use this one if you value your time and sanity. Feel free to add your flex below:

Benchmarks:
| CPU | Summary |
|---------|---------|
| 9700X | `[   6.302s] 238 tests run: 238 passed, 0 skipped` |
| Apple M4 Pro | `[  11.038s] 225 tests run: 225 passed, 0 skipped` |

0.1.4

| 9700X | `[  12.203s] 226 tests run: 226 passed, 0 skipped`

0.1.6

| 9700X (8 threads) | `[  27.718s] 373 tests run: 373 passed, 0 skipped`

| 9700X (16 threads) | `[  19.343s] 373 tests run: 373 passed (3 flaky), 0 skipped`

## To run just one Rust test:

```
./scripts/single-test.sh marginfi accrue_interest --verbose
./scripts/single-test.sh <program_name> <test_name> --verbose
```

This will run all tests prefixed with the given test_name, and all test cases for them.

For a complete list of all programs/tests, run the following command:

```
cargo nextest list
```

## To run the fuzz suite

Don't.

If you really want to, open the fuzz directory in a new IDE window (it's not part of the workspace,
by design). Run the generate-corpus Python script (you may need to install Python), then cargo
build, then run it. It may take a very long time and use a very large amount of disk space.
Typically, we run this to see if there are errors, and close it early if there are not, there is
typically no need to wait for the suite to finish locally.

See the Readme within the Fuzz directory for more details.

# Common issues

## The TS suite fails with `Environment supports crypto:  false` at the top

Update Node

## All the tests are failing in Rust and/or TS

Make sure you build the correct version, Rust requires the mainnet version (default features), TS wants localnet (no features)

## Program not deployed errors, when build seemingly worked otherwise

Adding a msg! that tries to print any `I80F48` without first converting it to a float or similar will
cause the entire project to silently fail to build, resulting in `Program not deployed` errors
downstream when testing

```
msg!("recorded price: {:?}", price);
```

## Metadata corruption

Seeing this:

```
error[E0786]: found invalid metadata files for crate `transfer_hook`
 --> test-utils/src/lib.rs:9:9
  |
9 | pub use transfer_hook;
  |         ^^^^^^^^^^^^^
  |
  = note: corrupt metadata encountered in /home/fish/mrgn/marginfi-v2/marginfi-v2/target/debug/deps/libtest_transfer_hook.rlib
```

Just `anchor clean` and rebuild. This is particularly likely to occur when switching between build environments e.g. cargo test --lib then anchor build because the former does not use SBF and the latter does.

## Rust tests fail with `Error: simulation error: BlockhashNotFound, logs: [], units_consumed: 0`

Ensure your machine is not in Low Power battery mode (or in any other mode decreasing performance).

## BlockhashNotFound errors in Rust test suite

On slower machines, or in tests with many txes, this error can be consistent or sometimes intermittent. Try
refreshing the blockhash in longer tests: `test_f.refresh_blockhash().await;` and switching usage of
`ctx.last_blockhash` to

```
let blockhash = {
    let banks_client = self.ctx.borrow().banks_client.clone();
    banks_client.get_latest_blockhash().await.unwrap()
};
```

## Validator Crashes at Startup

Usually manifests as something like:

```
Starting bankrun with pure bankrun setup...
thread 'tokio-runtime-worker' panicked at /usr/local/cargo/registry/src/index.crates.io-6f17d22bba15001f/solana-program-test-1.18.0/src/lib.rs:716:17:
Program file data not available <SOME GARBAGE>
```

Run `lsof -i :8899` to find the validator and then `kill -9 VALIDATOR_PID

## Rust tests panic with `Program file data not available for marginfi (...)`

Usually manifests as:

```
thread '...' panicked at .../solana-program-test-2.1.20/src/lib.rs:745:17:
Program file data not available for marginfi (MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA)
```

Root cause: `solana-program-test` cannot find `marginfi.so` in `SBF_OUT_DIR`.
This often happens when artifacts exist at `target/deploy` but `SBF_OUT_DIR` points at `target/sbf/deploy`.

Quick checks:

```
ls -la target/deploy/marginfi.so
ls -la target/sbf/deploy/marginfi.so
```

Fix options:

Actual fix:

```
./scripts/build-workspace.sh
```

Quick fix to just run tests:

```
export SBF_OUT_DIR="$PWD/target/deploy"
cargo nextest run --package marginfi --features mainnet-beta
```

This will run tests without building the full workspace.

Or edit `scripts/test-program-remix.sh` line 93 so `SBF_OUT_DIR` uses
`target/deploy` (not `target/sbf/deploy`), then rerun the remix command:

```
./scripts/test-program-remix.sh -p marginfi -l warn -c mainnet-beta -f mainnet-beta -j 8
```

If artifacts are stale/corrupt:

```
anchor clean
anchor build -p marginfi -- --no-default-features --features mainnet-beta,custom-heap
```

# Common Footguns

Debugging `I80F48`s by `msg!("val: {:?}", some_val_I80F48);` can cause silent build issues leading to `Program is not deployed`. Convert these values to string or float before printing them.
