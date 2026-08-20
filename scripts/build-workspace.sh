#!/usr/bin/env sh
set -e
ROOT=$(git rev-parse --show-toplevel)
cd $ROOT

export CARGO_TARGET_DIR="$ROOT/target/sbf"
export SBF_OUT_DIR="$ROOT/target/sbf/deploy"

cmd="anchor build --no-idl --ignore-keys"
echo "Running: $cmd"
eval "$cmd"

# Detached from the workspace, so anchor never sees it. Built with its own target dir to keep the
# graph anchor-free, then dropped alongside the rest for `SBF_OUT_DIR`.
echo "Running: cargo build-sbf (native-cpi-example)"
(
  cd "$ROOT/programs/native-cpi-example" \
    && CARGO_TARGET_DIR="$ROOT/programs/native-cpi-example/target" cargo build-sbf --arch v2
) && cp "$ROOT/programs/native-cpi-example/target/deploy/native_cpi_example.so" "$SBF_OUT_DIR/"
