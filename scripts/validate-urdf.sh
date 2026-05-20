#!/usr/bin/env bash
# Validate sim fixtures and config via Rust tests (URDF parse + URDF/MJCF DOF parity).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

cargo test -p armee-kinematics --quiet
cargo test -p sim-harness --quiet
cargo test -p marengo-config --quiet

echo "validate-fixtures: ok"
