#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

program_lib_name=${1-}

if [[ -z "${program_lib_name}" ]]; then
  echo "Usage: $0 <program_lib_name|all> [-- <extra test filters/flags for the test binary>]"
  exit 1
fi

# Defaults tuned for a weak 2-core CI
JOBS="${NEXTEST_JOBS:-1}"
RETRIES="${NEXTEST_RETRIES:-2}" 
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"

# CI-only: keep host builds separate from SBF artifacts
export CARGO_TARGET_DIR="$ROOT/target/host"

if [[ "${program_lib_name}" == "all" ]]; then
  package_filter=()
else
  package_filter=(--package "${program_lib_name}")
fi

shift 1 || true
extra_params=( "$@" )

# Minimal logging
export SBF_OUT_DIR="$ROOT/target/sbf/deploy"
export RUST_LOG="solana_runtime::message_processor::stable_log=warn"

cmd=(cargo nextest run
     --no-fail-fast
     "${package_filter[@]}"
     --features=test,test-bpf
     -j "${JOBS}"
     --retries "${RETRIES}"
     --failure-output=immediate
     --success-output=never
)

if [[ ${#extra_params[@]} -gt 0 ]]; then
  cmd+=(-- "${extra_params[@]}")
fi

echo "Running: ${cmd[*]}"
exec "${cmd[@]}"
