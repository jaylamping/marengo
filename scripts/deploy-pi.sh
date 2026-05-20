#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cargo build --release -p marengo-pi
echo "TODO: deploy ${ROOT}/target/release/marengo-pi to Pi host"
