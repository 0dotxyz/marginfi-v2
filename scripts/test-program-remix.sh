#!/usr/bin/env bash
set -euo pipefail

# Fancy pants help screen
usage() {
  cat <<EOF
Usage: $0 -p <program> [-f <features>] [-l <loglevel>] [-c <chain>] [-- <extra test flags>]

Options:
  -p, --program    Cargo package name to test (required)
  -f, --features   Comma-separated feature flags to pass to nextest (default: "test,test-bpf")
  -l, --loglevel   Log level for stable_log (e.g. "warn", default: "debug")
  -c, --chain      Chain to test against (e.g., mainnet-beta). This sets the TEST_CHAIN env variable.
  -j, --jobs       Threads to use (use as many as your CPU has physical cores for best results)
  --               Anything after this is passed as extra test flags (such as filters or test harness flags).
EOF
  exit 1
}

# Default values.
features="test,test-bpf"
loglevel="debug"
program=""
chain=""
jobs=""  # number of parallel test jobs (if your CPU has 8 physical cores, 8 is probably best!)
extra_args=()

# Parse command-line arguments.
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    -p|--program)
      if [[ -z "${2-}" ]]; then
        echo "Error: --program requires a value."
        usage
      fi
      program="$2"
      shift 2
      ;;
    -f|--features)
      if [[ -z "${2-}" ]]; then
        echo "Error: --features requires a value."
        usage
      fi
      features="$2"
      shift 2
      ;;
    -l|--loglevel)
      if [[ -z "${2-}" ]]; then
        echo "Error: --loglevel requires a value."
        usage
      fi
      loglevel="$2"
      shift 2
      ;;
    -c|--chain)
      if [[ -z "${2-}" ]]; then
        echo "Error: --chain requires a value."
        usage
      fi
      chain="$2"
      shift 2
      ;;
    -j)
      if [[ -z "${2-}" ]]; then
        echo "Error: -j requires a value."
        usage
      fi
      jobs="$2"
      shift 2
      ;;
    --)
      shift
      extra_args=("$@")
      break
      ;;
    *)
      echo "Unknown parameter: $1"
      usage
      ;;
  esac
done

if [[ -z "$program" ]]; then
  echo "Error: A program (cargo package) must be specified."
  usage
fi

# Determine the repository root.
ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo ".")
cd "$ROOT"

# Set environment variables for the tests.
export SBF_OUT_DIR="$ROOT/target/sbf/deploy"
export CARGO_TARGET_DIR="$ROOT/target/host"
export RUST_LOG="solana_runtime::message_processor::stable_log=${loglevel}"

# Set the chain environment variable if provided.
if [[ -n "$chain" ]]; then
  export TEST_CHAIN="$chain"
fi

# Build the cargo nextest command.
cmd=(cargo nextest run --no-fail-fast --package "$program" --features "$features" --retries 2)

# Add job limit if specified
if [[ -n "$jobs" ]]; then
  cmd+=(-j "$jobs")
fi

# Append any extra arguments (these are passed after '--')
if [ "${#extra_args[@]}" -gt 0 ]; then
  cmd+=(-- "${extra_args[@]}")
fi

echo "Running: ${cmd[*]}"
exec "${cmd[@]}"
