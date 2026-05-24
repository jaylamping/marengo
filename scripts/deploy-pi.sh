#!/usr/bin/env bash
# Cross-build Marengo Pi binaries and rsync to a Pi host.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${MARENGO_PI_TARGET:-aarch64-unknown-linux-gnu}"
PI_HOST="${1:-}"

cd "$ROOT"

echo "Building marengo-pi + motor-repl for ${TARGET}..."
cargo build --release --target "$TARGET" -p marengo-pi -p motor-repl --features socketcan

if [[ -z "$PI_HOST" ]]; then
  echo "Built:"
  echo "  ${ROOT}/target/${TARGET}/release/marengo-pi"
  echo "  ${ROOT}/target/${TARGET}/release/motor-repl"
  echo ""
  echo "Deploy: $0 user@marengo"
  echo "  or on Pi: git clone + native cargo build + sudo scripts/install-pi.sh"
  exit 0
fi

STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

mkdir -p "$STAGING/target/release"
cp "${ROOT}/target/${TARGET}/release/marengo-pi" "$STAGING/target/release/"
cp "${ROOT}/target/${TARGET}/release/motor-repl" "$STAGING/target/release/"
rsync -a "${ROOT}/config/" "$STAGING/config/"
rsync -a "${ROOT}/assets/" "$STAGING/assets/"
rsync -a "${ROOT}/scripts/" "$STAGING/scripts/"

echo "Syncing to ${PI_HOST}:${MARENGO_INSTALL_ROOT:-~/marengo}..."
rsync -av --delete \
  "$STAGING/target/release/" \
  "$STAGING/config/" \
  "$STAGING/assets/" \
  "$STAGING/scripts/" \
  "${PI_HOST}:${MARENGO_INSTALL_ROOT:-~/marengo}/"

echo "On the Pi:"
echo "  cd ${MARENGO_INSTALL_ROOT:-~/marengo} && sudo MARENGO_INSTALL_ROOT=/opt/marengo ./scripts/install-pi.sh"
