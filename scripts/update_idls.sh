#!/usr/bin/env bash
set -euo pipefail

########################################
# Configurable parameters
########################################

ANCHOR_PROVIDER_URL="${ANCHOR_PROVIDER_URL:-https://api.mainnet-beta.solana.com}"

IDL_DIR="idls-complete"
FIXTURES_DIR="tests/fixtures"

# One row per program: name | program id | IDL source path. Every artifact is named after the
# program, so `<name>.json`, `<name>.ts` and `<name>.so` all follow from the first field.
#
# Sources are vendor repos, not `anchor idl fetch`: Kamino and JupLend publish no IDL account, and
# Drift's is a 3-instruction stub that would clobber the real file.
RAW="https://raw.githubusercontent.com"

PROGRAMS=(
  "kamino_lending|KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD|Kamino-Finance/klend-sdk/master/src/idl/klend.json"
  "kamino_farms|FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr|Kamino-Finance/farms-sdk/master/src/idl/farms.json"
  "drift|dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH|drift-labs/protocol-v2/master/sdk/src/idl/drift.json"
  "juplend_earn|jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9|jup-ag/jupiter-lend/main/target/idl/lending.json"
  "liquidity|jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC|jup-ag/jupiter-lend/main/target/idl/liquidity.json"
  "lending_reward_rate_model|jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar|jup-ag/jupiter-lend/main/target/idl/lending_reward_rate_model.json"
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
require_cmd curl

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

  # `solana program dump` reads the upgradeable loader's ProgramData account and writes the raw ELF
  # executable — exactly the .so fixture we want.
  solana program dump \
    --url "${ANCHOR_PROVIDER_URL}" \
    "${program_id}" \
    "${out_so}"
}

process_program() {
  local name program_id idl_path
  IFS='|' read -r name program_id idl_path <<<"$1"

  local raw_idl="${IDL_DIR}/${name}.raw.json"
  local final_idl="${IDL_DIR}/${name}.json"
  local ts_file="${FIXTURES_DIR}/${name}.ts"
  local so_file="${FIXTURES_DIR}/${name}.so"

  echo "Fetching IDL for ${name}..."
  curl -sSfL --max-time 120 -o "${raw_idl}" "${RAW}/${idl_path}"

  echo "Converting IDL..."
  # Legacy (pre-Anchor-0.30) IDLs have no instruction discriminators and need converting; modern
  # ones only need the program address stamped in, and `anchor idl convert` rejects them.
  if python3 -c "import json,sys; i=json.load(open(sys.argv[1])).get('instructions') or []; sys.exit(0 if i and not any('discriminator' in x for x in i) else 1)" "${raw_idl}"; then
    anchor idl convert "${raw_idl}" \
      -o "${final_idl}" \
      --program-id "${program_id}"
  else
    python3 -c "import json,sys; d=json.load(open(sys.argv[1])); d['address']=sys.argv[3]; json.dump(d, open(sys.argv[2],'w'), indent=2)" \
      "${raw_idl}" "${final_idl}" "${program_id}"
  fi

  rm -f "${raw_idl}"

  echo "Generating TS..."
  generate_ts_from_idl "${final_idl}" "${ts_file}"

  echo "Downloading program .so..."
  download_program_so "${program_id}" "${so_file}"

  echo "Generated:"
  echo "  ${final_idl}"
  echo "  ${ts_file}"
  echo "  ${so_file}"
}

########################################

for entry in "${PROGRAMS[@]}"; do
  process_program "${entry}"
done

echo "Done."
