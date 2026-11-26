#!/usr/bin/env bash
#
# Assembles the FuzzCorp bundle. Run after ./prepare.sh and
#   cargo build --release --locked --features invariant_test
#
# Usage: ./build-bundle.sh [output-dir]   (default: ./bundle)
#
# FuzzCorp's `crucible` driver runs the single harness binary. Already-reported
# invariants are muted via SCOUT_CHECK_MUTE so campaigns surface only new signal.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUT="${1:-$HERE/bundle}"
BIN="$HERE/target/release/invariant_test"
SYMBOLS="$HERE/programs/marginfi_symbols.so"

for f in "$BIN" "$HERE/programs/marginfi_program.so" "$SYMBOLS"; do
  [ -f "$f" ] || {
    echo "error: missing $f -- run ./prepare.sh and build the harness" >&2
    exit 1
  }
done

case "$(uname -m)" in
x86_64) ARCH=amd64 ;;
aarch64 | arm64) ARCH=arm64 ;;
*)
  echo "error: unsupported arch $(uname -m)" >&2
  exit 1
  ;;
esac

COMMIT="${FUZZ_REVISION:-$(git -C "$REPO" rev-parse HEAD)}"

# Property ids from scout_run_property!("P-...", ...) calls in src/main.rs.
MUTE="P-0039,P-0025,P-0032,P-0015,P-0041,P-0042,P-0018-ESC,P-0020,P-0020-DELEV,P-0014-ESC,P-0018-NOREMEDY,P-0019-T22,P-0019B-T22,P-COMPOUND-BADDEBT,P-PERMDELEGATE,P-STALE-BYSTANDER,P-0035-SOLEND-T22,P-0037-SOLEND"

# Coverage: build-sbf remaps the cwd away, so DWARF paths are repo-relative
# (programs/marginfi/src/..., type-crate/src/..., ...). A symbols file without that line
# table still uploads fine and renders empty coverage, so check it here.
dwarfdump="$(command -v llvm-dwarfdump || command -v dwarfdump || true)"
for c in /usr/lib/llvm-*/bin/llvm-dwarfdump; do
  [ -z "$dwarfdump" ] && [ -x "$c" ] && dwarfdump="$c"
done
[ -n "$dwarfdump" ] || {
  echo "error: llvm-dwarfdump not found (apt-get install llvm)" >&2
  exit 1
}
if ! "$dwarfdump" --debug-line "$SYMBOLS" | grep '"programs/marginfi/src' >/dev/null; then
  echo "error: $SYMBOLS has no programs/marginfi/src line table -- coverage would be empty" >&2
  exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT/harness/programs" "$OUT/harness/idls" "$OUT/srcs"
cp "$BIN" "$OUT/harness/invariant_test"
cp "$HERE/idls/marginfi_program.json" "$OUT/harness/idls/"
cp "$HERE"/programs/*.so "$OUT/harness/programs/"

# SourcesOriginalPath maps programs/marginfi/src/ onto srcs/; every other DWARF path is
# resolved relative to the bundle root, so first-party crates go there at their repo path.
cp -R "$REPO/programs/marginfi/src/." "$OUT/srcs/"
git -C "$REPO" ls-files -z -- 'type-crate/src' 'id-crate/src' 'programs/*-mocks/src/**' |
  (cd "$REPO" && xargs -0 tar -cf -) | tar -xf - -C "$OUT"

# Manifest v3 with snake_case keys: fuzz-up >= 0.1.0 (2026-10-14) matches keys
# case-sensitively and rejects the former PascalCase form ("unsupported manifest version: 0").
# One core: the platform scales by running many single-core replicas, and the harness
# is a single-threaded LiteSVM replay. Memory is what corpus_merge actually needs.
cat >"$OUT/manifest.fc.json" <<JSON
{
  "version": 3,
  "revision": { "commit": "${COMMIT}" },
  "lineages": [{
    "name": "marginfi",
    "confs": [{
      "name": "invariant_test",
      "driver": {
        "type": "crucible",
        "params": {
          "binary_path_in_bundle": "harness/invariant_test",
          "harness_run_dir_in_bundle": "harness",
          "symbols_path_in_bundle": "harness/programs/marginfi_symbols.so",
          "sources_path_in_bundle": "srcs",
          "sources_original_path": "programs/marginfi/src/",
          "extra_env": { "SCOUT_CHECK_MUTE": "${MUTE}" }
        }
      },
      "architecture": { "name": "${ARCH}" },
      "yield_time_minutes": 120,
      "memory_kib": 4194304,
      "cores": 1
    }]
  }]
}
JSON

echo "bundle assembled at $OUT (arch=$ARCH, commit=$COMMIT)"
