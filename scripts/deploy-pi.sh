#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cargo build --release -p marengo-pi --features socketcan
echo "TODO: deploy ${ROOT}/target/release/marengo-pi to Pi host"
echo "Bring-up: MARENGO_CONFIG_DIR=config/bringup/shoulder_pitch_dual ./target/release/marengo-pi"
