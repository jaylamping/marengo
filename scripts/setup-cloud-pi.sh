#!/usr/bin/env bash
# Cloud-agent Pi access: Tailscale userspace networking, SSH key, cross-deploy deps.
# Requires secrets in Cursor Cloud Agents dashboard (see docs/cloud-pi-tailscale.md).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=cloud-pi-lib.sh
source "${ROOT}/scripts/cloud-pi-lib.sh"

MODE="${1:-all}"

run_prepare() {
  cloud_pi_install_packages
  cloud_pi_install_tailscale
  cloud_pi_ensure_cross_target

  if [[ -d "${ROOT}/tools/marengo-pi-mcp" ]]; then
    cloud_pi_log "Building marengo-pi-mcp"
    (cd "${ROOT}/tools/marengo-pi-mcp" && npm install && npm run build)
  fi
}

ensure_tailscale_ready() {
  # Install can fail mid-bootstrap (e.g. old Node) and leave start with no binaries.
  if ! cloud_pi_tailscale_installed; then
    cloud_pi_warn "Tailscale not installed — running prepare (packages + Tailscale)"
    run_prepare
  fi
}

run_start_daemon() {
  ensure_tailscale_ready
  cloud_pi_start_tailscaled || true
  if [[ -n "${TAILSCALE_AUTH_KEY:-}" ]]; then
    cloud_pi_tailscale_up || true
  fi
}

run_connect() {
  ensure_tailscale_ready
  cloud_pi_tailscale_up
  cloud_pi_write_ssh_config
}

case "${MODE}" in
  --prepare | prepare)
    run_prepare
    ;;
  --start-daemon | start-daemon)
    run_start_daemon
    ;;
  --connect | connect)
    run_connect
    ;;
  --verify | verify)
    if ! cloud_pi_tailscale_installed; then
      run_prepare
    fi
    run_connect
    cloud_pi_verify
    ;;
  --all | all | "")
    run_prepare
    run_start_daemon
    if [[ -n "${TAILSCALE_AUTH_KEY:-}" ]]; then
      run_connect
      cloud_pi_verify || true
    else
      cloud_pi_warn "TAILSCALE_AUTH_KEY not set — prepared packages only"
      cloud_pi_warn "Add secrets per docs/cloud-pi-tailscale.md, then re-run: ./scripts/setup-cloud-pi.sh --verify"
    fi
    ;;
  -h | --help)
    cat <<'EOF'
Usage: setup-cloud-pi.sh [mode]

Modes:
  (default)       prepare + start tailscaled + connect/verify when secrets exist
  --prepare       apt packages, tailscale install, aarch64 cross target, mcp build
  --start-daemon  background tailscaled (userspace networking)
  --connect       tailscale up + SSH config from secrets
  --verify        connect + ssh/gateway/log API checks

Secrets (Cursor Cloud Agents → Secrets):
  TAILSCALE_AUTH_KEY              Runtime Secret — ephemeral/reusable auth key
  MARENGO_PI_SSH_PRIVATE_KEY_B64  Runtime Secret — base64 ed25519 deploy key
  MARENGO_PI_HOST                 Environment Variable (default joey-robot.tail0b414.ts.net)
  MARENGO_GATEWAY_LOG_TOKEN       Runtime Secret (optional, if set on Pi gateway)
EOF
    ;;
  *)
    echo "error: unknown mode: ${MODE}" >&2
    exit 2
    ;;
esac
