# NEW DEV QUICKSTART GUIDE

New developer getting started working on the mrgnv2 program side? Read on.

## Dependencies

Install the following versions used by the workspace and CI:

- Rust **1.95.0**, including `rustfmt` and `clippy` (`rust-toolchain.toml` also requests `rust-analyzer`).
- Anchor CLI **1.2.2** and Agave/Solana CLI **4.1.2** (`Anchor.toml`).
- Node **24.15.0** and Yarn **4.14.1** (via Corepack).
- `cargo-nextest` **0.9.81** for Rust integration tests.
- `cargo-fuzz` **0.13.1** and Python 3 for the optional fuzz suite; see its [README](../../programs/marginfi/fuzz/README.md) for the required nightly toolchain.

On Ubuntu, the build also needs `build-essential`, `pkg-config`, and `libudev-dev`, as in CI.

Install Rust before Anchor so AVM's compilation uses the required compiler:

```sh
rustup toolchain install 1.95.0 --component rustfmt --component clippy --component rust-analyzer

# Install the Agave/Solana CLI version configured in Anchor.toml.
sh -c "$(curl -sSfL https://release.anza.xyz/v4.1.2/install)"
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"

# Install Anchor through AVM. Set the toolchain explicitly when running AVM
# outside the workspace, where rust-toolchain.toml does not apply.
RUSTUP_TOOLCHAIN=1.95.0 cargo install avm --git https://github.com/solana-foundation/anchor --locked
RUSTUP_TOOLCHAIN=1.95.0 avm install 1.2.2
avm use 1.2.2

RUSTUP_TOOLCHAIN=1.95.0 cargo install cargo-nextest --version 0.9.81 --locked
# Optional: only needed for fuzzing.
RUSTUP_TOOLCHAIN=1.95.0 cargo install cargo-fuzz --version 0.13.1 --locked
```

`rust-toolchain.toml` selects Rust 1.95.0 inside this repository; it does not change your global default. Anchor may initialize the Solana version specified in `Anchor.toml` for a build even if your active `solana --version` differs. The Solana CLI/toolchain version is separate from the versions of the individual Solana Rust crates resolved in `Cargo.lock`.

Anchor's provider is configured to use `~/.config/solana/id.json`. If you don't already have that keypair, create one with `solana-keygen new`; keep any existing keypair rather than overwriting it.

Install Node 24.15.0 and Corepack, then enable Yarn and install the JavaScript dependencies:

```sh
corepack enable
corepack prepare yarn@4.14.1 --activate
yarn install --immutable
```

Check your setup from the repository root:

```sh
rustc --version
anchor --version
solana --version
node --version
yarn --version
cargo nextest --version
```

The Rust program uses Anchor 1.2.2, while the JavaScript client dependencies in `package.json` use Anchor 0.32.1. Use the versions in the manifests and lockfiles rather than trying to make their version numbers match. Switchboard pull feed fixtures are still used by the tests, but the Rust workspace no longer requires the Switchboard SDK.

# Running tests

The two integration test stacks require separate sBPF v3 program builds: Rust tests use the mainnet program ID, while TypeScript tests use the localnet program ID. Keep their artifacts separate:

```text
target/sbf/deploy/   Rust-test binaries, including mainnet-ID marginfi
target/sbf/          mainnet marginfi SBF build cache
target/sbf/mocks-cache/  standalone mock SBF build cache
target/host/         Rust host test build cache
target/deploy/       localnet binaries used by TypeScript tests
target/idl/          generated IDLs used by TypeScript tests
```

## Rust unit tests

```sh
cargo test -p marginfi --no-default-features --features custom-heap --lib
```

## TypeScript tests (LiteSVM)

```sh
# Run once, and again after changing the corresponding programs.
anchor build -p mocks --arch v3 --ignore-keys
anchor build -p test_transfer_hook --arch v3 --no-idl
anchor build -p marginfi --arch v3 -- --no-default-features --features custom-heap

anchor run basic-tests
# Or run all suites:
anchor run all-tests
```

The marginfi build generates the localnet `target/deploy/marginfi.so`, `target/idl/marginfi.json`, and TypeScript types. Other named suites, including `limits-tests`, `kamino-tests`, and `juplend-tests`, are listed in `Anchor.toml`. Individual specs can have ordering or setup dependencies within their suite.

## Rust integration tests

```sh
# Build mainnet binaries to target/sbf/deploy/.
./scripts/build-workspace.sh

# Run all packages, or select marginfi.
./scripts/test-program.sh all
# ./scripts/test-program.sh marginfi
```

The script builds the standalone mocks and marginfi in separate SBF caches to avoid rebuilding them when their feature sets differ. The first test run also compiles host binaries into `target/host/`; later runs can reuse that cache.

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

0.1.8

| 9700X (8 threads) | `[  53.024s] 622 tests run: 622 passed, 0 skipped`

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

Make sure the correct program ID is baked into each build. Rust integration tests load the mainnet build from `target/sbf/deploy/`, while TypeScript tests load the localnet build from `target/deploy/`. A mismatch can produce `DeclaredProgramIdMismatch` (Anchor error `0x1004`). Rebuild the affected stack using the commands above; don't point Rust tests at the localnet artifact.

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

### Anchor tests fail with `./scripts/test-program.sh: line 40: package_filter[@]: unbound variable`

Just `anchor clean` and `cargo clean` and rebuild. If the error persists it's probably due to macOS default Bash `3.2` + `set -u` that ends up triggering `package_filter[@]: unbound variable` when all uses an empty array. Fix is basically upgrade your Bash to version `5` or higher via Homebrew or similar.

## BlockhashNotFound errors in Rust test suite

On slower machines, or in tests with many txes, this error can be consistent or sometimes intermittent. Never
sign with the cached `ctx.last_blockhash`, which is captured once at startup; fetch a fresh one instead:

```
let blockhash = {
    let banks_client = self.ctx.borrow().banks_client.clone();
    banks_client.get_latest_blockhash().await.unwrap()
};
```

## Validator Crashes at Startup

Usually manifests as something like:

```
Starting LiteSVM setup...
thread 'tokio-runtime-worker' panicked at /usr/local/cargo/registry/src/index.crates.io-.../solana-program-test-4.1.2/src/lib.rs:...:
Program file data not available <SOME GARBAGE>
```

Run `lsof -i :8899` to find a process using the validator port, then stop it if it is no longer needed.

## Rust tests panic with `Program file data not available for marginfi (...)`

Usually manifests as:

```
thread '...' panicked at .../solana-program-test-4.1.2/src/lib.rs:...:
Program file data not available for marginfi (MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA)
```

Root cause: `solana-program-test` cannot find `marginfi.so` in `SBF_OUT_DIR`.
This often happens when artifacts exist at `target/deploy` but `SBF_OUT_DIR` points at `target/sbf/deploy`.

Quick checks:

```
ls -la target/deploy/marginfi.so
ls -la target/sbf/deploy/marginfi.so
```

Rebuild the mainnet artifacts, then rerun the Rust tests:

```sh
./scripts/build-workspace.sh
./scripts/test-program-remix.sh -p marginfi -l warn -c mainnet-beta -f mainnet-beta -j 8
```

If TypeScript tests report missing programs, build the localnet artifacts using the TypeScript build commands above. If account resolution fails with `Unresolved accounts`, regenerate the marginfi IDL with the localnet `anchor build -p marginfi` command.

# Common Footguns

Debugging `I80F48`s by `msg!("val: {:?}", some_val_I80F48);` can cause silent build issues leading to `Program is not deployed`. Convert these values to string or float before printing them.
