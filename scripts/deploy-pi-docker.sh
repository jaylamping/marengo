#!/usr/bin/env bash
# Pi deploy via dev container (Windows / hosts without aarch64 cross GCC).
#
# Uses compose named volumes for target/, cargo registry, and consul/node_modules so
# repeat deploys reuse compiled artifacts. Do NOT substitute bare `docker run -v .:/workspace`
# — that re-downloads crates and reinstalls npm every time.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"

PI_HOST="${1:-$(resolve_deploy_pi_host)}"

SSH_DIR="$(resolve_ssh_dir)"
export MARENGO_SSH_DIR="${SSH_DIR}"

DOCKER_SSH_MOUNT="$(docker_ssh_mount_src "$SSH_DIR")"
DOCKER_PLATFORM="${DOCKER_PLATFORM:-linux/amd64}"

deploy_progress_env

log_step "deploy-pi-docker → ${PI_HOST}"
log_note "SSH mount: ${DOCKER_SSH_MOUNT} → /home/marengo/.ssh"
if [[ ! -f "${SSH_DIR}/config" ]] && [[ ! -f "${SSH_DIR}/id_ed25519_marengo" ]] \
  && [[ ! -f "${SSH_DIR}/id_ed25519" ]]; then
  log_warn "SSH preflight: host shell cannot read ${SSH_DIR} (continuing — Docker mount may still work)"
fi
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

# Git Bash converts -v C:/... paths before they reach Docker Desktop; disable for compose run.
if [[ "$(uname -s 2>/dev/null)" == MINGW* ]] || [[ "${OSTYPE:-}" == msys* ]]; then
  export MSYS_NO_PATHCONV=1
fi

# Run through the image entrypoint (gosu marengo) — do not wrap in bash -lc (drops PATH).
exec "${COMPOSE_RUN[@]}" \
  -e MARENGO_DEPLOY_VIA_COMPOSE=1 \
  -e MARENGO_PI_HOST \
  -e MARENGO_PI_USER \
  -e MARENGO_SKIP_CONSUL \
  -e VITE_MARENGO_LOG_TOKEN \
  -e MARENGO_GATEWAY_LOG_TOKEN \
  -e CARGO_TERM_PROGRESS_WHEN \
  -e CARGO_TERM_COLOR \
  -e NPM_CONFIG_PROGRESS \
  -e NPM_CONFIG_LOGLEVEL \
  -e MARENGO_DEPLOY_VERBOSE \
  -e MARENGO_DEPLOY_START="${MARENGO_DEPLOY_START}" \
  -e MARENGO_SSH_DIR \
  -v "${DOCKER_SSH_MOUNT}:/home/marengo/.ssh:ro" \
  deploy-pi \
  "${DEPLOY_ARGS[@]}"
