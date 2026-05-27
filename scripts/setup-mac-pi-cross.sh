#!/usr/bin/env bash
# One-time (idempotent) macOS setup for Pi cross-build: aarch64-unknown-linux-gnu.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "setup-mac-pi-cross: skip (not macOS)" >&2
  exit 0
fi

if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
  echo "aarch64-linux-gnu-gcc: $(command -v aarch64-linux-gnu-gcc)"
else
  if ! command -v brew >/dev/null 2>&1; then
    echo "error: Homebrew required — https://brew.sh" >&2
    exit 1
  fi
  echo "Installing aarch64-unknown-linux-gnu cross toolchain (Homebrew)..."
  brew tap messense/macos-cross-toolchains 2>/dev/null || true
  brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
fi

if ! rustup target list --installed | grep -q '^aarch64-unknown-linux-gnu$'; then
  echo "Adding Rust target aarch64-unknown-linux-gnu..."
  rustup target add aarch64-unknown-linux-gnu
fi

echo "Cross-build smoke..."
cd "$ROOT"
cargo build --release --target aarch64-unknown-linux-gnu -p marengo-pi -p motor-repl -p imu-probe --features socketcan,linux-i2c

echo "setup-mac-pi-cross: ok"
echo "Deploy: ./scripts/deploy-pi.sh joey@marengo.local --install"
