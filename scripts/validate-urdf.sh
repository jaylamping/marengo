#!/usr/bin/env bash
# Validate sim fixtures, production assets, and config via Rust tests.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

cargo test -p armee-kinematics --quiet
cargo test -p sim-harness --quiet
cargo test -p marengo-config --quiet

if [[ ! -f "${ROOT}/assets/urdf/marengo.urdf" ]]; then
  echo "error: missing production URDF at assets/urdf/marengo.urdf" >&2
  exit 1
fi

echo "validate-fixtures: ok"
