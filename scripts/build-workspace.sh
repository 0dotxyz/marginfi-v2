#!/usr/bin/env sh
set -eu

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

SBF_TARGET_DIR="$ROOT/target/sbf"
export SBF_OUT_DIR="$SBF_TARGET_DIR/deploy"
# cargo-build-sbf uses SBF_OUT_PATH for build output; SBF_OUT_DIR is used by tests.
export SBF_OUT_PATH="$SBF_OUT_DIR"
mkdir -p "$SBF_OUT_DIR"

# Keep this program list in sync with [workspace].members in Anchor.toml.
# Standalone mock programs use different features from marginfi's dependencies;
# keep their Cargo artifacts separate so they do not invalidate marginfi's cache.
export CARGO_TARGET_DIR="$SBF_TARGET_DIR/mocks-cache"
for program in mocks kamino-mocks scope-mocks; do
    echo "Building $program (sBPF v3, mainnet type crate)"
    anchor build -p "$program" --arch v3 --no-idl --ignore-keys -- \
        --features marginfi-type-crate/mainnet-beta
done

echo "Building test_transfer_hook (sBPF v3)"
anchor build -p test_transfer_hook --arch v3 --no-idl --ignore-keys

export CARGO_TARGET_DIR="$SBF_TARGET_DIR"
echo "Building marginfi (sBPF v3, mainnet-beta)"
anchor build -p marginfi --arch v3 --no-idl --ignore-keys -- --features mainnet-beta
