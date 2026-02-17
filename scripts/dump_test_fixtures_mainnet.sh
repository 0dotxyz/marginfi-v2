#!/usr/bin/env bash
set -euo pipefail

# Dumps (clones) on-chain program bytecode (.so) from mainnet into tests/fixtures/
# so bankrun tests can load them via Anchor.toml [[test.genesis]].
#
# Usage:
#   ./scripts/dump_test_fixtures_mainnet.sh            # downloads missing files only
#   ./scripts/dump_test_fixtures_mainnet.sh --force     # overwrite existing files
#
# Notes:
# - Uses `solana program dump -u m <PROGRAM_ID> <OUTFILE>`.
# - Requires Solana CLI installed and configured.
#
# References:
#   solana program dump <PROGRAM_ID> <OUTPUT.so>

FORCE=0
if [[ "${1:-}" == "--force" ]]; then
  FORCE=1
  shift
fi

if ! command -v solana >/dev/null 2>&1; then
  echo "error: solana CLI not found. Install Solana/Agave CLI and try again." >&2
  exit 1
fi

# Mainnet-beta by default (moniker form). Override with CLUSTER=m|d|t|l or full URL.
CLUSTER="${CLUSTER:-m}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${OUT_DIR:-"${ROOT_DIR}/tests/fixtures"}"
mkdir -p "${OUT_DIR}"

# ----------------------------------------------------------------------------
# Program IDs (mainnet)
# ----------------------------------------------------------------------------

# Existing integration fixtures
KAMINO_LENDING_PROGRAM_ID="KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"
KAMINO_FARMS_PROGRAM_ID="FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"
DRIFT_V2_PROGRAM_ID="dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"
SOLEND_PROGRAM_ID="So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo"
SPL_SINGLE_POOL_PROGRAM_ID="SVSPxpvHdN29nkVg9rPapPNDddN5DipNLRUFhyjFThE"

# Jupiter Lend (JupLend / Fluid)
JUPLEND_LENDING_PROGRAM_ID="jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9"
JUPLEND_LIQUIDITY_PROGRAM_ID="jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC"
JUPLEND_REWARDS_RATE_MODEL_PROGRAM_ID="jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar"

# Metaplex Token Metadata (required by JupLend init_lending)
TOKEN_METADATA_PROGRAM_ID="metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"

# ----------------------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------------------

dump_program() {
  local program_id="$1"
  local outfile="$2"

  if [[ -f "${outfile}" && "${FORCE}" -eq 0 ]]; then
    echo "skip: ${outfile} (already exists)"
    return 0
  fi

  echo "dump: ${program_id} -> ${outfile}"
  # Common syntax: solana program dump -u m <PROGRAM_ID> <OUTPUT.so>
  solana program dump -u "${CLUSTER}" "${program_id}" "${outfile}"
}

# ----------------------------------------------------------------------------
# Dump all programs used by bankrun tests
# ----------------------------------------------------------------------------

dump_program "${KAMINO_LENDING_PROGRAM_ID}" "${OUT_DIR}/kamino_lending.so"
dump_program "${KAMINO_FARMS_PROGRAM_ID}" "${OUT_DIR}/kamino_farms.so"
dump_program "${DRIFT_V2_PROGRAM_ID}" "${OUT_DIR}/drift_v2.so"
dump_program "${SOLEND_PROGRAM_ID}" "${OUT_DIR}/solend.so"
dump_program "${SPL_SINGLE_POOL_PROGRAM_ID}" "${OUT_DIR}/spl_single_pool.so"

dump_program "${JUPLEND_LENDING_PROGRAM_ID}" "${OUT_DIR}/juplend_lending.so"
dump_program "${JUPLEND_LIQUIDITY_PROGRAM_ID}" "${OUT_DIR}/juplend_liquidity.so"
dump_program "${JUPLEND_REWARDS_RATE_MODEL_PROGRAM_ID}" "${OUT_DIR}/juplend_rewards_rate_model.so"

dump_program "${TOKEN_METADATA_PROGRAM_ID}" "${OUT_DIR}/token_metadata.so"

echo "done."
