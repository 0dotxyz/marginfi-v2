#!/usr/bin/env bash
#
# Assembles a FuzzCorp bundle for the crucible marginfi invariant harness.
#
# Prereq: the harness binary must already be built, e.g.
#   cargo build --release --features invariant_test
#
# Usage: ./build-bundle.sh [output-dir]   (default: ./bundle)
#
# The bundle uses FuzzCorp's native `crucible` driver: it drives the single
# harness binary via FUZZ_* env vars and parses the [FUZZ_*] stderr protocol.
# The noisy already-reported invariants are muted via SCOUT_CHECK_MUTE (ExtraEnv)
# so a long campaign surfaces only NEW signal.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${1:-$HERE/bundle}"
BIN="$HERE/target/release/invariant_test"

if [ ! -f "$BIN" ]; then
  echo "error: harness binary not found at $BIN" >&2
  echo "       build it first: (cd $HERE && cargo build --release --features invariant_test)" >&2
  exit 1
fi

if [ ! -f "$HERE/programs/marginfi_program.so" ]; then
  echo "error: target program not found at $HERE/programs/marginfi_program.so" >&2
  echo "       build it from source first, e.g. from the repo root:" >&2
  echo "         anchor build -p marginfi && cp target/deploy/marginfi.so $HERE/programs/marginfi_program.so" >&2
  exit 1
fi

# amd64 on a GitHub ubuntu-latest runner, arm64 on Apple Silicon, etc.
case "$(uname -m)" in
  x86_64)  ARCH=amd64 ;;
  aarch64|arm64) ARCH=arm64 ;;
  *) echo "unsupported arch $(uname -m)" >&2; exit 1 ;;
esac

# Commit of the source under test (the marginfi program), recorded in the manifest.
# Override with FUZZ_REVISION (the CI passes the repo SHA); else use the checkout's HEAD.
CRC="${FUZZ_REVISION:-$(git -C "$HERE" rev-parse HEAD 2>/dev/null || echo 0000000)}"

# The full set of already-triaged / written-up invariants. Muting them keeps the
# campaign clean so any NEW finding is visible. Keep in sync with PROPERTIES.md.
MUTE="P-0039,P-0025,P-0032,P-0015,P-0041,P-0042,P-0018-ESC,P-0020,P-0020-DELEV,P-0014-ESC,P-0018-NOREMEDY,P-0019-T22,P-0019B-T22,P-COMPOUND-BADDEBT,P-PERMDELEGATE,P-STALE-BYSTANDER,P-0035-SOLEND-T22,P-0037-SOLEND"

rm -rf "$OUT"
mkdir -p "$OUT/fuzz/marginfi/target/release" \
         "$OUT/fuzz/marginfi/programs" \
         "$OUT/fuzz/marginfi/idls" \
         "$OUT/fuzz/marginfi/src"

cp "$BIN"                              "$OUT/fuzz/marginfi/target/release/"
cp "$HERE/programs/marginfi_program.so" "$OUT/fuzz/marginfi/programs/"
cp "$HERE/idls/marginfi_program.json"   "$OUT/fuzz/marginfi/idls/"
cp -R "$HERE/src/." "$OUT/fuzz/marginfi/src/"

cat > "$OUT/manifest.fc.json" <<JSON
{
  "Version": 3,
  "Revision": { "Commit": "${CRC}" },
  "Lineages": [{
    "Name": "marginfi",
    "Confs": [{
      "Name": "invariant_test",
      "Driver": {
        "Type": "crucible",
        "Params": {
          "BinaryPathInBundle": "fuzz/marginfi/target/release/invariant_test",
          "HarnessRunDirInBundle": "fuzz/marginfi/",
          "SourcesPathInBundle": "fuzz/marginfi/src/",
          "SourcesOriginalPath": "/build/src/",
          "ExtraEnv": { "SCOUT_CHECK_MUTE": "${MUTE}" }
        }
      },
      "Architecture": { "Name": "${ARCH}" },
      "YieldTimeMinutes": 120,
      "MemoryKiB": 1062144,
      "Cores": 4
    }]
  }]
}
JSON

echo "bundle assembled at: $OUT  (arch=${ARCH}, crucible=${CRC:-unknown})"
