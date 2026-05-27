#!/usr/bin/env bash
# Pi deploy via dev container (Windows / hosts without aarch64 cross GCC).
#
# Uses compose named volumes for target/, cargo registry, and consul/node_modules so
# repeat deploys reuse compiled artifacts. Do NOT substitute bare `docker run -v .:/workspace`
# — that re-downloads crates and reinstalls npm every time.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PI_HOST="${1:-${MARENGO_PI_HOST:-joey@marengo.local}}"
SSH_DIR="${MARENGO_SSH_DIR:-${USERPROFILE:-${HOME}}/.ssh}"

# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"
deploy_progress_env

log_step "deploy-pi-docker → ${PI_HOST}"
log_note "Cache volumes: cargo-target, cargo-registry, cargo-git, consul-node-modules"
log_note "Live logs: CARGO_TERM_PROGRESS_WHEN=${CARGO_TERM_PROGRESS_WHEN}"
log_note "Verbose npm/cargo: MARENGO_DEPLOY_VERBOSE=1"
log_note "Start UTC: $(date -u +%Y-%m-%dT%H:%M:%SZ)"

cd "$ROOT"

TTY_ARGS=()
if [[ -t 1 ]]; then
  TTY_ARGS=(-t)
fi

# Build image only when Dockerfile changed (BuildKit layer cache otherwise).
docker compose --ansi always build deploy-pi >&2

exec docker compose --ansi always run --rm "${TTY_ARGS[@]}" \
  -e MARENGO_DEPLOY_VIA_COMPOSE=1 \
  -e MARENGO_SKIP_CONSUL \
  -e CARGO_TERM_PROGRESS_WHEN \
  -e CARGO_TERM_COLOR \
  -e NPM_CONFIG_PROGRESS \
  -e NPM_CONFIG_LOGLEVEL \
  -e MARENGO_DEPLOY_VERBOSE \
  -e MARENGO_DEPLOY_START="${MARENGO_DEPLOY_START}" \
  -v "${SSH_DIR}:/root/.ssh:ro" \
  deploy-pi \
  bash -lc 'stdbuf -oL -eL ./scripts/deploy-pi.sh --install '"${PI_HOST}"
