#!/usr/bin/env bash
# Build Consul static assets on the Pi (or any native host with npm).
# Produces ${ROOT}/consul/dist for install-pi.sh → /opt/marengo/www.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="${1:-$ROOT}"
# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"

if [[ "${SKIP_CONSUL:-false}" == true ]]; then
  echo "build-consul-native: SKIP_CONSUL=true — skipping"
  exit 0
fi

if ! command -v npm >/dev/null 2>&1; then
  # Silent skip left /opt/marengo/www on a stale bundle after Set Limits / UI
  # fixes landed in git (self-update reported success; Apply still ±30 mrad pad).
  echo "error: npm not found — cannot rebuild Consul UI for install-pi www/" >&2
  echo "error: install Node ≥24 on the Pi, or set SKIP_CONSUL=true only for binary-only installs" >&2
  exit 1
fi

if [[ ! -f "${ROOT}/consul/package-lock.json" ]]; then
  echo "error: ${ROOT}/consul/package-lock.json missing" >&2
  exit 1
fi

log_token="$(resolve_vite_marengo_log_token "" || true)"
if [[ -n "$log_token" ]]; then
  export VITE_MARENGO_LOG_TOKEN="$log_token"
  echo "build-consul-native: baking VITE_MARENGO_LOG_TOKEN (${#log_token} chars)"
else
  unset VITE_MARENGO_LOG_TOKEN || true
  echo "build-consul-native: no log token (logs HTTP may 401 if gateway requires one)"
fi

echo "build-consul-native: npm ci + build in ${ROOT}/consul"
(
  cd "${ROOT}/consul"
  npm ci
  env -u VITE_CHAPPE_HTTP_URL -u VITE_CHAPPE_WEBTRANSPORT_URL \
    VITE_AUTO_LEARN_URL= VITE_AUTO_LEARN_TOKEN= \
    npm run build
)

if [[ ! -f "${ROOT}/consul/dist/index.html" ]]; then
  echo "error: consul build did not produce dist/index.html" >&2
  exit 1
fi

"${ROOT}/scripts/check-consul-dist.sh" "${ROOT}/consul/dist"
echo "build-consul-native: ok → ${ROOT}/consul/dist"
