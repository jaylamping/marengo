#!/usr/bin/env bash
# Native release build on Raspberry Pi: Rust bins + optional Consul UI.
# Used by pi_build / pi_sync_main pi_native (MCP) and manual bench deploy.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ -f "${HOME}/.cargo/env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
  set +a
fi
export PATH="${HOME}/.cargo/bin:/usr/local/cargo/bin:${PATH:-}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not on PATH (install Rust on Pi or use cross deploy)" >&2
  exit 127
fi

echo "pi-native-build: cargo release (marengo-pi, gateway, log-cli, motor-repl, imu-probe)"
cargo build -p marengo-pi -p marengo-gateway -p marengo-log-cli -p motor-repl -p imu-probe \
  --features socketcan,linux-i2c --release

"${ROOT}/scripts/build-consul-native.sh" "${ROOT}"

echo "pi-native-build: done"
