#!/usr/bin/env bash
# Run the minimal Kamino prerequisite setup tests (k01-k06) followed by the
# scenario benchmarks (k_scenarios).
#
# Usage:
#   ./scripts/run-kamino-scenarios.sh
#
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

RUST_LOG= yarn run ts-mocha \
  -p ./tsconfig.json \
  -t 1000000 \
  tests/k01_kaminoInit.spec.ts \
  tests/k02_kaminoUser.spec.ts \
  tests/k03_kaminoDeposit.spec.ts \
  tests/k04_kaminoMrgnUser.spec.ts \
  tests/k05_kaminoBankInit.spec.ts \
  tests/k06_MrgnKaminoDeposit.spec.ts \
  tests/k_scenarios.spec.ts \
  --exit \
  --require tests/rootHooks.ts
