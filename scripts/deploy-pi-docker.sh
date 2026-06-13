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
DOCKER_PLATFORM="${DOCKER_PLATFORM:-linux/amd64}"

# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"
deploy_progress_env

log_step "deploy-pi-docker → ${PI_HOST}"
log_note "Platform: ${DOCKER_PLATFORM}"
log_note "Cache volumes: cargo-target, cargo-registry, cargo-git, consul-node-modules"
log_note "Live logs: CARGO_TERM_PROGRESS_WHEN=${CARGO_TERM_PROGRESS_WHEN}"
log_note "Verbose npm/cargo: MARENGO_DEPLOY_VERBOSE=1"
log_note "Start UTC: $(date -u +%Y-%m-%dT%H:%M:%SZ)"

cd "$ROOT"

export DOCKER_DEFAULT_PLATFORM="${DOCKER_DEFAULT_PLATFORM:-${DOCKER_PLATFORM}}"

COMPOSE_RUN=(docker compose run --rm -T)
# Deploy is non-interactive; cargo/npm progress uses CARGO_TERM_PROGRESS_WHEN=always.
# Do not pass --ansi always or -t: Docker Desktop on Windows PowerShell fails with
# "creating a console from a file is not supported on windows".

DEPLOY_ARGS=(./scripts/deploy-pi.sh --install "${PI_HOST}")
if [[ "${MARENGO_SKIP_CONSUL:-}" == 1 ]]; then
  DEPLOY_ARGS=(./scripts/deploy-pi.sh --install --skip-consul "${PI_HOST}")
fi

ensure_deploy_pi_image() {
  if [[ "${MARENGO_FORCE_IMAGE_BUILD:-}" == 1 ]]; then
    log_step "docker compose build deploy-pi (forced)"
    docker compose build deploy-pi >&2
    return
  fi
  if docker image inspect marengo-dev:local >/dev/null 2>&1; then
    log_note "Reusing marengo-dev:local (set MARENGO_FORCE_IMAGE_BUILD=1 to rebuild)"
    return
  fi
  log_step "docker compose build deploy-pi (image missing)"
  if ! docker compose build deploy-pi >&2; then
    echo "error: marengo-dev:local missing and docker compose build failed" >&2
    exit 1
  fi
}

ensure_deploy_pi_image

# Run through the image entrypoint (gosu marengo) — do not wrap in bash -lc (drops PATH).
exec "${COMPOSE_RUN[@]}" \
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
  "${DEPLOY_ARGS[@]}"
