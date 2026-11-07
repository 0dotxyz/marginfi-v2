#!/usr/bin/env bash
#
# Builds everything the harness loads from this checkout's source: the marginfi program
# (stripped + unstripped for coverage), solend_mocks, the fixture programs it CPIs into,
# and the IDL that declare_fuzz_program! embeds at compile time. CI runs this same script.
#
# Usage: fuzz/crucible/prepare.sh [marginfi.json]
#   With an IDL path (CI passes the one anchor-build already produced for the commit) that
#   IDL is used; without one, `anchor idl build` generates it (a few minutes, host build).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
cd "$REPO"

# Own target dir: target/deploy holds the localnet-id build the TS tests use, and the
# harness needs the mainnet-id (default features) build.
export CARGO_TARGET_DIR="$REPO/target/fuzz"
# Debug info for FuzzCorp source coverage. Exported for both SBF builds so they share one
# set of compiled dependencies (a different profile would compile them all twice).
export CARGO_PROFILE_RELEASE_DEBUG=2 CARGO_PROFILE_RELEASE_STRIP=none
OUT="$HERE/programs"
mkdir -p "$OUT" "$HERE/idls"

log="$(mktemp)"
cargo build-sbf --manifest-path programs/marginfi/Cargo.toml 2>&1 | tee "$log"
# build-sbf reports frame-size violations but still exits 0; such a binary faults at
# runtime where the real program does not.
if grep -E 'exceeded max offset|overwrites values in the frame' "$log"; then
  echo "error: SBF stack-frame violation in the marginfi build (see above)" >&2
  exit 1
fi
cargo build-sbf --manifest-path programs/solend-mocks/Cargo.toml

symbols=("$CARGO_TARGET_DIR"/*/release/marginfi.so)
if [ "${#symbols[@]}" -ne 1 ] || [ ! -f "${symbols[0]}" ]; then
  echo "error: expected exactly one unstripped marginfi.so, found: ${symbols[*]}" >&2
  exit 1
fi

cp "$CARGO_TARGET_DIR/deploy/marginfi.so" "$OUT/marginfi_program.so"
cp "${symbols[0]}" "$OUT/marginfi_symbols.so"
cp "$CARGO_TARGET_DIR/deploy/solend_mocks.so" "$OUT/solend_mocks.so"
cp tests/fixtures/drift.so "$OUT/drift_v2.so"
cp tests/fixtures/kamino_farms.so "$OUT/"
cp tests/fixtures/kamino_lending_nocpi.so "$OUT/"
cp tests/fixtures/solend.so "$OUT/"

# anchor leaves `address` empty because declare_id! lives in id-crate, so take the
# mainnet-beta id (the default-features build above) from there.
program_id="$(sed -n '/feature = "mainnet-beta"/{n;s/.*declare_id!("\([^"]*\)").*/\1/p;}' \
  id-crate/src/lib.rs)"
if [ -z "$program_id" ]; then
  echo "error: no mainnet-beta declare_id! found in id-crate/src/lib.rs" >&2
  exit 1
fi
idl="${1:-}"
if [ -z "$idl" ]; then
  # --lib: the IDL comes from the lib's generated test, so skip compiling the integration
  # test suite. No debug info: nothing runs under a debugger here and it halves the build.
  idl="$(mktemp)"
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
    anchor idl build -p marginfi -o "$idl" -- --lib
fi
jq --arg id "$program_id" '.address = $id' "$idl" >"$HERE/idls/marginfi_program.json"

echo "harness inputs ready in $OUT and $HERE/idls"
