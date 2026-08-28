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

# Coverage gate: a symbols file with no DWARF still satisfies `[ -f ]` and still
# reports "not stripped" from `file`, then renders EMPTY coverage on the server
# while every CI step reports success. The server's complaint in that case is
# "SourcesOriginalPath ... does not match any source file", which reads like a path
# bug rather than a missing-debug-info bug. Require real compile units instead.
scout_has_dwarf() {
  so="$1"; dd=""
  command -v llvm-dwarfdump >/dev/null 2>&1 && dd=llvm-dwarfdump
  [ -z "$dd" ] && command -v dwarfdump >/dev/null 2>&1 && dd=dwarfdump
  if [ -z "$dd" ]; then
    echo "warning: no llvm-dwarfdump available; cannot verify DWARF in $so" >&2
    return 0
  fi
  n=$($dd --debug-info "$so" 2>/dev/null | grep -c DW_TAG_compile_unit)
  if [ "${n:-0}" -eq 0 ]; then
    echo "warning: $so carries NO DWARF (0 compile units) -- coverage would render" >&2
    echo "         empty. Build the program with CARGO_PROFILE_RELEASE_DEBUG=2 and" >&2
    echo "         CARGO_PROFILE_RELEASE_STRIP=none." >&2
    return 1
  fi
  echo "coverage: $(basename "$so") carries $n compile units"
  return 0
}


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

REPO="$(git -C "$HERE" rev-parse --show-toplevel 2>/dev/null || (cd "$HERE/../.." && pwd))"

rm -rf "$OUT"
mkdir -p "$OUT/harness/programs" "$OUT/harness/idls" "$OUT/srcs"

cp "$BIN"                             "$OUT/harness/invariant_test"
cp "$HERE/idls/marginfi_program.json" "$OUT/harness/idls/"
# Every program the harness registers -- the target plus the drift/kamino/solend
# fixtures -- resolved CWD-relative from HarnessRunDirInBundle.
for so in "$HERE"/programs/*.so; do
  [ -f "$so" ] && cp "$so" "$OUT/harness/programs/"
done

# ---------------------------------------------------------------- coverage ---
# Coverage maps LCOV lines onto the PROGRAM's source, not the harness's. The LCOV
# keys each line on an absolute path (DW_AT_comp_dir + the relative file); the
# driver strips SourcesOriginalPath and looks the remainder up under
# SourcesPathInBundle. comp_dir is wherever the program was compiled, which differs
# between a dev box and a CI runner, so read it from the artifact rather than
# hardcoding it -- a wrong prefix does not error, it silently yields lines_found: 0.
cp -R "$REPO/programs/marginfi/src/." "$OUT/srcs/"

SYMBOLS="$HERE/programs/marginfi_symbols.so"
COVERAGE_PARAMS=""
if [ -f "$SYMBOLS" ] && scout_has_dwarf "$SYMBOLS"; then
  DD=""
  command -v llvm-dwarfdump >/dev/null 2>&1 && DD=llvm-dwarfdump
  [ -z "$DD" ] && command -v dwarfdump >/dev/null 2>&1 && DD=dwarfdump
  COMP_DIR=""
  if [ -n "$DD" ]; then
    COMP_DIR="$($DD --debug-info "$SYMBOLS" 2>/dev/null \
      | grep -o 'DW_AT_comp_dir[[:space:]]*("[^"]*")' \
      | sed 's/.*("\(.*\)")/\1/' \
      | grep -vE '\.cargo|/rustc/|toolchain|bpf-tools' | head -1 || true)"
  fi
  SRC_ORIG="${COMP_DIR:+${COMP_DIR%/}/}programs/marginfi/src/"
  COVERAGE_PARAMS=$'\n          "SymbolsPathInBundle":   "harness/programs/marginfi_symbols.so",\n          "SourcesPathInBundle":   "srcs",\n          "SourcesOriginalPath":   "'"$SRC_ORIG"$'",'
  echo "coverage: comp_dir=${COMP_DIR:-<none>}  SourcesOriginalPath=${SRC_ORIG}"
else
  echo "warning: programs/marginfi_symbols.so missing -- the bundle will run but" >&2
  echo "         render NO source-level coverage." >&2
fi

# One core, not four. The platform scales a lineage by running MANY single-core replicas
# of the same task rather than handing one task several cores, so a 4-core request is
# placeable only on a worker with 4 cores free at once -- and the harness is a
# single-threaded LiteSVM replay that cannot use them anyway. Memory is what it actually
# needs: a corpus_merge over tens of thousands of inputs does not fit in ~1 GiB, and an
# OOM there kills the task without a useful error.
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
          "BinaryPathInBundle": "harness/invariant_test",
          "HarnessRunDirInBundle": "harness",${COVERAGE_PARAMS}
          "ExtraEnv": { "SCOUT_CHECK_MUTE": "${MUTE}" }
        }
      },
      "Architecture": { "Name": "${ARCH}" },
      "YieldTimeMinutes": 120,
      "MemoryKiB": 4194304,
      "Cores": 1
    }]
  }]
}
JSON

echo "bundle assembled at: $OUT  (arch=${ARCH}, crucible=${CRC:-unknown})"
