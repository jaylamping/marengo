#!/usr/bin/env bash
# One-time WSL2 / Debian setup for native Pi cross-build (no Docker deploy).
set -euo pipefail

if [[ -f /proc/version ]] && grep -qi microsoft /proc/version; then
  : # WSL
elif [[ "$(uname -s)" != Linux ]]; then
  echo "setup-wsl-pi-cross: requires Linux or WSL2" >&2
  exit 1
fi

if ! command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
  echo "Installing aarch64-linux-gnu cross GCC (sudo)..."
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends gcc-aarch64-linux-gnu
fi

if ! rustup target list --installed | grep -q '^aarch64-unknown-linux-gnu$'; then
  echo "Adding Rust target aarch64-unknown-linux-gnu..."
  rustup target add aarch64-unknown-linux-gnu
fi

echo "setup-wsl-pi-cross: ok"
echo "Deploy: ./scripts/deploy-pi.sh --install joey@marengo.local"
