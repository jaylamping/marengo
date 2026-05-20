#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cargo build --release -p marengo-jetson
echo "TODO: deploy ${ROOT}/target/release/marengo-jetson to Jetson host"
