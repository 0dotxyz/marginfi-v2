#!/usr/bin/env sh
ROOT=$(git rev-parse --show-toplevel)
cd $ROOT

export CARGO_TARGET_DIR="$ROOT/target/sbf"
export SBF_OUT_DIR="$ROOT/target/sbf/deploy"

cmd="anchor build --no-idl"
echo "Running: $cmd"
eval "$cmd"
