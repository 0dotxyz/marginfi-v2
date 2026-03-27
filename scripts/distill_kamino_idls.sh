#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
IDL_DIR="${REPO_ROOT}/idls"
FULL_IDL_DIR="${REPO_ROOT}/idls-complete"
MOCK_IDL_DIR="${REPO_ROOT}/programs/kamino-mocks/idls"

LENDING_IDL_SOURCE="${FULL_IDL_DIR}/kamino_lending.json"
FARMS_IDL_SOURCE="${FULL_IDL_DIR}/kamino_farms.json"
LENDING_IDL="${IDL_DIR}/kamino_lending.json"
FARMS_IDL="${IDL_DIR}/kamino_farms.json"
MOCK_LENDING_IDL="${MOCK_IDL_DIR}/kamino_lending.json"
MOCK_FARMS_IDL="${MOCK_IDL_DIR}/kamino_farms.json"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Error: required command not found: $1" >&2
    exit 1
  }
}

require_file() {
  [[ -f "$1" ]] || {
    echo "Error: file not found: $1" >&2
    exit 1
  }
}

require_cmd jq
require_file "${LENDING_IDL_SOURCE}"
require_file "${FARMS_IDL_SOURCE}"

tmp_lending="$(mktemp)"
tmp_farms="$(mktemp)"
trap 'rm -f "${tmp_lending}" "${tmp_farms}"' EXIT

jq '
  .instructions |= map(select(.name | IN(
    "init_lending_market",
    "init_reserve",
    "init_farms_for_reserve",
    "update_reserve_config",
    "socialize_loss_v2",
    "refresh_reserve",
    "refresh_reserves_batch",
    "init_user_metadata",
    "init_obligation",
    "init_obligation_farms_for_reserve",
    "borrow_obligation_liquidity",
    "refresh_obligation",
    "deposit_reserve_liquidity_and_obligation_collateral",
    "deposit_reserve_liquidity_and_obligation_collateral_v2",
    "withdraw_obligation_collateral_and_redeem_reserve_collateral",
    "withdraw_obligation_collateral_and_redeem_reserve_collateral_v2"
  )))
  | .instructions |= map(
      if .name == "deposit_reserve_liquidity_and_obligation_collateral_v2" then
        .accounts |= map(if .name == "farms_accounts" then .name = "deposit_farms_accounts" else . end)
      elif .name == "withdraw_obligation_collateral_and_redeem_reserve_collateral_v2" then
        .accounts |= map(if .name == "farms_accounts" then .name = "withdraw_farms_accounts" else . end)
      else
        .
      end
    )
  | .accounts = []
' "${LENDING_IDL_SOURCE}" > "${tmp_lending}"

mv "${tmp_lending}" "${LENDING_IDL}"
cp "${LENDING_IDL}" "${MOCK_LENDING_IDL}"

jq '
  .instructions |= map(select(.name | IN(
    "add_rewards",
    "harvest_reward",
    "initialize_global_config",
    "initialize_reward",
    "refresh_user_state",
    "refresh_farm",
    "update_farm_config"
  )))
  | .accounts = []
' "${FARMS_IDL_SOURCE}" > "${tmp_farms}"

mv "${tmp_farms}" "${FARMS_IDL}"
cp "${FARMS_IDL}" "${MOCK_FARMS_IDL}"

echo "Pruned IDLs:"
echo "  ${LENDING_IDL}"
jq -r '.instructions[].name' "${LENDING_IDL}" | sed 's/^/    - /'
jq -r '.accounts[].name' "${LENDING_IDL}" | sed 's/^/    * /'
echo "  ${MOCK_LENDING_IDL}"
jq -r '.instructions[].name' "${MOCK_LENDING_IDL}" | sed 's/^/    - /'
jq -r '.accounts[].name' "${MOCK_LENDING_IDL}" | sed 's/^/    * /'
echo "  ${FARMS_IDL}"
jq -r '.instructions[].name' "${FARMS_IDL}" | sed 's/^/    - /'
jq -r '.accounts[].name' "${FARMS_IDL}" | sed 's/^/    * /'
echo "  ${MOCK_FARMS_IDL}"
jq -r '.instructions[].name' "${MOCK_FARMS_IDL}" | sed 's/^/    - /'
jq -r '.accounts[].name' "${MOCK_FARMS_IDL}" | sed 's/^/    * /'
