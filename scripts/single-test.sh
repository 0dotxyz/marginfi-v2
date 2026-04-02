#!/usr/bin/env bash

set -e

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <program_name> <test_name> [--verbose]"
    exit 1
fi

program_name=$1
test_name=$2
verbose=false

if [ "$#" -eq 3 ] && [ "$3" == "--verbose" ]; then
    verbose=true
fi

ROOT=$(git rev-parse --show-toplevel)
cd $ROOT

SBF_OUT_DIR="$ROOT/target/sbf/deploy"
if [ ! -d "$SBF_OUT_DIR" ]; then
    echo "Error: missing SBF output dir: $SBF_OUT_DIR"
    echo "Run ./scripts/build-workspace.sh first."
    echo "If you need IDL artifacts, run ./scripts/build-idl.sh separately."
    exit 1
fi

if [ ! -f "$SBF_OUT_DIR/${program_name}.so" ]; then
    echo "Error: missing program artifact: $SBF_OUT_DIR/${program_name}.so"
    echo "Run ./scripts/build-workspace.sh first."
    echo "If you need IDL artifacts, run ./scripts/build-idl.sh separately."
    exit 1
fi

RUST_LOG="solana_runtime::message_processor::stable_log=debug"
CARGO_CMD="SBF_OUT_DIR=$SBF_OUT_DIR CARGO_TARGET_DIR=$ROOT/target/host RUST_LOG=$RUST_LOG cargo nextest run --package $program_name --features=test,test-bpf --nocapture -- $test_name"

echo "Running: $CARGO_CMD"

if [ "$verbose" == true ]; then
    eval $CARGO_CMD
else
    eval $CARGO_CMD 2>&1 | awk '/PASS/ {print "\033[32m" $0 "\033[39m"} /FAIL/ {print "\033[31m" $0 "\033[39m"}'
fi
