#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
ENV_FILE="$REPO_ROOT/ops/alt-staging/.env"

if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
fi

FEATURES="${ALT_STAGING_BUILD_FEATURES:-stagingalt}"
ANCHOR_EXTRA_ARGS="${ALT_STAGING_ANCHOR_BUILD_ARGS:-}"

cd "$REPO_ROOT"

echo "==> Building program (alt staging)"
echo "Repo: $REPO_ROOT"
echo "Features: $FEATURES"

if ! command -v anchor >/dev/null 2>&1; then
  echo "ERROR: anchor not found on PATH" >&2
  exit 1
fi

# IMPORTANT: Must use --no-default-features because default is mainnet-beta.
# Without it, both mainnet-beta AND stagingalt would be enabled, causing the
# wrong program ID to be compiled in.
if [[ -n "$FEATURES" ]]; then
  anchor build -p marginfi -- --no-default-features --features "$FEATURES"
else
  anchor build -p marginfi -- --no-default-features
fi

echo "==> Build complete"
