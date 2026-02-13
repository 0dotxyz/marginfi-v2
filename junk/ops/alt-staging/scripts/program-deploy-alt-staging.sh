#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
ENV_FILE="$REPO_ROOT/ops/alt-staging/.env"

if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
fi

: "${ALT_STAGING_RPC_URL:?ALT_STAGING_RPC_URL is required}"
: "${ALT_STAGING_PROGRAM_ID:?ALT_STAGING_PROGRAM_ID is required}"
: "${ALT_STAGING_UPGRADE_AUTHORITY_KEYPAIR:?ALT_STAGING_UPGRADE_AUTHORITY_KEYPAIR is required}"

DEPLOY_PAYER="${ALT_STAGING_DEPLOY_PAYER_KEYPAIR:-$ALT_STAGING_UPGRADE_AUTHORITY_KEYPAIR}"

cd "$REPO_ROOT"

if ! command -v solana >/dev/null 2>&1; then
  echo "ERROR: solana CLI not found on PATH" >&2
  exit 1
fi

SO_PATH="${ALT_STAGING_PROGRAM_SO_PATH:-}"

if [[ -z "$SO_PATH" ]]; then
  # Try to auto-detect the built .so
  if [[ ! -d "$REPO_ROOT/target/deploy" ]]; then
    echo "ERROR: target/deploy not found. Build first (anchor build) or set ALT_STAGING_PROGRAM_SO_PATH." >&2
    exit 1
  fi

  mapfile -t SO_FILES < <(find "$REPO_ROOT/target/deploy" -maxdepth 1 -type f -name "*.so" 2>/dev/null || true)

  if [[ ${#SO_FILES[@]} -eq 0 ]]; then
    echo "ERROR: No .so found under target/deploy. Build first or set ALT_STAGING_PROGRAM_SO_PATH." >&2
    exit 1
  fi

  if [[ ${#SO_FILES[@]} -gt 1 ]]; then
    echo "ERROR: Multiple .so files found. Set ALT_STAGING_PROGRAM_SO_PATH to one of:" >&2
    printf '  - %s\n' "${SO_FILES[@]}" >&2
    exit 1
  fi

  SO_PATH="${SO_FILES[0]}"
fi

# If relative, resolve relative to repo root
if [[ "$SO_PATH" != /* ]]; then
  SO_PATH="$REPO_ROOT/$SO_PATH"
fi

if [[ ! -f "$SO_PATH" ]]; then
  echo "ERROR: Program binary not found: $SO_PATH" >&2
  exit 1
fi

echo "==> Deploying program to alt staging"
echo "RPC:     $ALT_STAGING_RPC_URL"
echo "Program: $ALT_STAGING_PROGRAM_ID"
echo "Binary:  $SO_PATH"
echo "Payer:   $DEPLOY_PAYER"

echo
solana program deploy "$SO_PATH" \
  --url "$ALT_STAGING_RPC_URL" \
  --program-id "$ALT_STAGING_PROGRAM_ID" \
  --upgrade-authority "$ALT_STAGING_UPGRADE_AUTHORITY_KEYPAIR" \
  --keypair "$DEPLOY_PAYER"

echo
echo "==> Program account"
solana program show --url "$ALT_STAGING_RPC_URL" "$ALT_STAGING_PROGRAM_ID" || true
