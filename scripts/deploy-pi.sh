#!/usr/bin/env bash
# Cross-build Marengo Pi binaries and rsync to a Pi host.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${MARENGO_PI_TARGET:-aarch64-unknown-linux-gnu}"
PI_HOST=""
DO_INSTALL=false

usage() {
  echo "Usage: $0 [--install] [user@host]" >&2
  echo "  --install  run sudo install-pi.sh on the Pi after rsync (MARENGO_INSTALL_ROOT=/opt/marengo)" >&2
  echo "  Env: MARENGO_PI_HOST, MARENGO_INSTALL_ROOT (remote staging, default ~/marengo)" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install)
      DO_INSTALL=true
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      PI_HOST="$1"
      shift
      ;;
  esac
done

if [[ -z "$PI_HOST" ]]; then
  PI_HOST="${MARENGO_PI_HOST:-}"
fi

ensure_cross_toolchain() {
  if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
    return 0
  fi
  if [[ "$(uname -s)" == "Darwin" ]]; then
    echo "Cross linker missing — running scripts/setup-mac-pi-cross.sh ..."
    "${ROOT}/scripts/setup-mac-pi-cross.sh"
    return 0
  fi
  echo "error: aarch64-linux-gnu-gcc not found (use Docker dev image or install cross GCC)" >&2
  exit 1
}

cd "$ROOT"
ensure_cross_toolchain

echo "Building marengo-pi + marengo-gateway + motor-repl for ${TARGET}..."
cargo build --release --target "$TARGET" -p marengo-pi -p marengo-gateway -p motor-repl --features socketcan

if [[ -z "$PI_HOST" ]]; then
  echo "Built:"
  echo "  ${ROOT}/target/${TARGET}/release/marengo-pi"
  echo "  ${ROOT}/target/${TARGET}/release/motor-repl"
  echo ""
  echo "Deploy: $0 [--install] joey@marengo.local"
  echo "  or on Pi: git clone + native cargo build + sudo scripts/install-pi.sh"
  exit 0
fi

STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

mkdir -p "$STAGING/target/release"
cp "${ROOT}/target/${TARGET}/release/marengo-pi" "$STAGING/target/release/"
cp "${ROOT}/target/${TARGET}/release/marengo-gateway" "$STAGING/target/release/"
cp "${ROOT}/target/${TARGET}/release/motor-repl" "$STAGING/target/release/"
rsync -a "${ROOT}/config/" "$STAGING/config/"
rsync -a "${ROOT}/assets/" "$STAGING/assets/"
rsync -a "${ROOT}/scripts/" "$STAGING/scripts/"

REMOTE_ROOT="${MARENGO_INSTALL_ROOT:-~/marengo}"
echo "Syncing to ${PI_HOST}:${REMOTE_ROOT}/ ..."
rsync -av --delete \
  "$STAGING/" \
  "${PI_HOST}:${REMOTE_ROOT}/"

if [[ "$DO_INSTALL" == true ]]; then
  echo "Installing on Pi (sudo install-pi.sh → /opt/marengo)..."
  if ssh "$PI_HOST" "sudo -n true" 2>/dev/null; then
    ssh "$PI_HOST" "set -euo pipefail; sudo ${REMOTE_ROOT}/scripts/install-pi.sh"
    ssh "$PI_HOST" "echo \$(git -C /opt/marengo rev-parse HEAD 2>/dev/null || echo unknown) \$(date -u +%Y-%m-%dT%H:%M:%SZ) > ${REMOTE_ROOT}/.deploy-rev && cat ${REMOTE_ROOT}/.deploy-rev" || true
  else
    echo ""
    echo "warn: passwordless sudo not available — on the Pi (already logged in), run:"
    echo "  sudo ${REMOTE_ROOT}/scripts/install-pi.sh"
    echo ""
    echo "If install-pi.sh is missing, re-run deploy from Mac: just deploy-pi"
    echo "Quick binary-only fix on Pi:"
    echo "  sudo install -m 755 ${REMOTE_ROOT}/target/release/marengo-pi /opt/marengo/bin/marengo-pi"
    echo "  sudo install -m 755 ${REMOTE_ROOT}/target/release/motor-repl /opt/marengo/bin/motor-repl"
  fi
else
  echo "On the Pi:"
  echo "  cd ${REMOTE_ROOT} && sudo MARENGO_INSTALL_ROOT=/opt/marengo ./scripts/install-pi.sh"
  echo "Or re-run from Mac: $0 --install ${PI_HOST}"
fi
