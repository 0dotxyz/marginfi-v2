# Fuzz Tests

These fuzz tests test the soundness of the accounting of the marginfi protocol.
The tests work by interacting directly with the smart contract instructions and at the end comparing if the internal accounting matches the real balances in the token accounts.

The tests simulates all actions that are involved in internal accounting:

- Depositing
- Withdrawing
- Liquidate
- Accrue Interest
- Update Price Oracle
- Handle Bankruptcy

## How it works?

### Program Interaction

The framework directly invokes the functions that are normally invoked by the onchain program entry point, and stubs the cpi invoke calls to directly invoke the spl token instructions.

### State

The framework uses a bump allocator for account storage. All `AccountInfo` objects are referencing data in the bump allocator.

When an instruction is invoked we direct pass in the `AccountInfo` objects referencing allocated state.
Before the invoke we also copy to a local cache and revert the state if the instructions fail.

### Actions

The framework uses the arbitrary library to generate a random sequence of actions that are then processed on the same state.

### How to Run

Run `python3 ./generate_corpus.py`. You may use python if you don't have python3 installed, or you may need to install python.

Build with `cargo build`.

If this fails, you probably need to update your Rust toolchain:

`rustup install nightly-2025-08-01`

And possibly:

`rustup component add rust-src --toolchain nightly-2025-08-01-aarch64-apple-darwin`
or
`rustup component add rust-src --toolchain nightly-2025-08-01-x86_64-unknown-linux-gnu`

Run with `cargo +nightly-2025-08-01 fuzz run lend --strip-dead-code --no-cfg-fuzzing -- -max_total_time=300 -timeout_exitcode=100 -error_exitcode=101`

If your machine has 8 cores, yolo with:

```
cargo +nightly-2025-08-01 fuzz run lend \
--strip-dead-code --no-cfg-fuzzing \
  -- -max_total_time=30 \
     -timeout_exitcode=100 \
     -error_exitcode=101 \
     -print_pcs=0 \
     -print_final_stats=1 \
     -close_fd_mask=1 \
     -jobs=8 \
     -workers=8
```

This will crush your machine for 30 seconds instead, but 30 \* 8 = 240 ~= 300 so you are getting the
same number of tests more or less. You will only get stdError instead of having your console spammed
for an hour with buffered stdOut.

To rerun some tests after a failure: `cargo +nightly-2025-08-01 fuzz run lend --strip-dead-code --no-cfg-fuzzing artifacts/lend/crash-ae5084b9433152babdaf7dcd75781eacd7ea55c7`, replacing the hash after crash- with the one you see in the terminal.

## It Failed Now What

If the error isn't a fairly obvious panic when the run started, consider enabling more stack trace:

```
export RUSTFLAGS="-C debuginfo=2"
export RUST_BACKTRACE=full
```

compile and run again.

If that fails, remove all log restrictions, e.g. remove:
```
     -print_pcs=0 \
     -print_final_stats=1 \
     -close_fd_mask=1 \
```

open fuzz-0.log and see where the error actually occurs.
