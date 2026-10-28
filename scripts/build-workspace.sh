#!/usr/bin/env sh
set -e
ROOT=$(git rev-parse --show-toplevel)
cd $ROOT

export CARGO_TARGET_DIR="$ROOT/target/sbf"
export SBF_OUT_DIR="$ROOT/target/sbf/deploy"

cmd="anchor build --no-idl --ignore-keys"
echo "Running: $cmd"
eval "$cmd"

# Build the native CPI fixture separately to avoid workspace feature unification with Anchor.
(
  cd "$ROOT/programs/native-cpi-example"
  CARGO_TARGET_DIR="$ROOT/programs/native-cpi-example/target" cargo build-sbf --arch v2
)
cp "$ROOT/programs/native-cpi-example/target/deploy/native_cpi_example.so" "$SBF_OUT_DIR/"
