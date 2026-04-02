#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

# Only this script should produce IDLs.
# Keep host artifacts isolated from normal host/debug and SBF outputs.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/idl-host}"

# Optional override if your IDL flow requires nightly.
# Example: IDL_RUSTUP_TOOLCHAIN=nightly ./scripts/build-idl.sh
if [[ -n "${IDL_RUSTUP_TOOLCHAIN:-}" ]]; then
  export RUSTUP_TOOLCHAIN="$IDL_RUSTUP_TOOLCHAIN"
fi

program="${1:-marginfi}"
shift || true
extra_args=("$@")

cmd=(anchor idl build -p "$program")
if [[ ${#extra_args[@]} -gt 0 ]]; then
  cmd+=("${extra_args[@]}")
fi

echo "Running: ${cmd[*]}"
echo "CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
if [[ -n "${RUSTUP_TOOLCHAIN:-}" ]]; then
  echo "RUSTUP_TOOLCHAIN=$RUSTUP_TOOLCHAIN"
fi

"${cmd[@]}"
