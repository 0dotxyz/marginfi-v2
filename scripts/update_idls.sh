#!/usr/bin/env bash
set -euo pipefail

########################################
# Configurable parameters
########################################

ANCHOR_PROVIDER_URL="${ANCHOR_PROVIDER_URL:-https://api.mainnet-beta.solana.com}"

IDL_DIR="idls"
FIXTURES_DIR="tests/fixtures"

# Map: output file prefix -> Solana program id
declare -A PROGRAMS=(
  [kamino_lending]="KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"
  [kamino_farms]="FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"
)

# Stable iteration order
PROGRAM_ORDER=(
  kamino_lending
  kamino_farms
)

########################################

export ANCHOR_PROVIDER_URL

mkdir -p "${IDL_DIR}" "${FIXTURES_DIR}"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Error: required command not found: $1" >&2
    exit 1
  }
}

require_cmd anchor
require_cmd solana
require_cmd python3

generate_ts_from_idl() {
  local idl_json="$1"
  local out_ts="$2"

  local tmp_ts
  local sibling_ts

  tmp_ts="$(mktemp)"
  sibling_ts="${idl_json%.json}.ts"

  anchor idl type "${idl_json}" > "${tmp_ts}"

  if [[ -s "${tmp_ts}" ]]; then
    mv "${tmp_ts}" "${out_ts}"
  elif [[ -f "${sibling_ts}" ]]; then
    mv "${sibling_ts}" "${out_ts}"
    rm -f "${tmp_ts}"
  else
    rm -f "${tmp_ts}"
    echo "Error: failed to generate TS from ${idl_json}" >&2
    exit 1
  fi
}

download_program_so() {
  local program_id="$1"
  local out_so="$2"

  local tmp_json
  local tmp_bin
  local program_data_account

  tmp_json="$(mktemp)"
  tmp_bin="$(mktemp)"

  solana account \
    --output json \
    --url "${ANCHOR_PROVIDER_URL}" \
    "${program_id}" > "${tmp_json}"

  program_data_account="$(
python3 - <<PY "${tmp_json}"
import json, sys
with open(sys.argv[1]) as f:
    print(json.load(f)["programData"])
PY
)"

  solana account \
    --output-file "${tmp_bin}" \
    --url "${ANCHOR_PROVIDER_URL}" \
    "${program_data_account}" > /dev/null

python3 - <<PY "${tmp_bin}" "${out_so}"
import sys

src, dst = sys.argv[1], sys.argv[2]

with open(src, "rb") as f:
    data = f.read()

# Strip upgradeable loader header
elf = data[45:]

if not elf.startswith(b"\x7fELF"):
    raise SystemExit("Unexpected ProgramData layout")

with open(dst, "wb") as f:
    f.write(elf)
PY

  rm -f "${tmp_json}" "${tmp_bin}"
}

process_program() {
  local name="$1"
  local program_id="$2"

  local raw_idl="${IDL_DIR}/${name}.raw.json"
  local final_idl="${IDL_DIR}/${name}.json"
  local ts_file="${FIXTURES_DIR}/${name}.ts"
  local so_file="${FIXTURES_DIR}/${name}.so"

  echo "Fetching IDL for ${name}..."
  anchor --provider.cluster "${ANCHOR_PROVIDER_URL}" idl fetch -o "${raw_idl}" "${program_id}"

  echo "Converting IDL..."
  anchor idl convert "${raw_idl}" \
    -o "${final_idl}" \
    --program-id "${program_id}"

  rm -f "${raw_idl}"

  echo "Generating TS..."
  generate_ts_from_idl "${final_idl}" "${ts_file}"

#   echo "Downloading program .so..."
#   download_program_so "${program_id}" "${so_file}"

  echo "Generated:"
  echo "  ${final_idl}"
  echo "  ${ts_file}"
  echo "  ${so_file}"
}

########################################

for name in "${PROGRAM_ORDER[@]}"; do
  process_program "${name}" "${PROGRAMS[$name]}"
done

echo "Done."