#!/usr/bin/env bash
# Cross-build Marengo Pi binaries and rsync to a Pi host.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"
deploy_progress_env

TARGET="${MARENGO_PI_TARGET:-aarch64-unknown-linux-gnu}"
PI_HOST=""
DO_INSTALL=false
SKIP_CONSUL=false

usage() {
  echo "Usage: $0 [--install] [--skip-consul] [user@host]" >&2
  echo "  --install      run sudo install-pi.sh on the Pi after rsync" >&2
  echo "  --skip-consul  skip consul npm build (binary-only deploy)" >&2
  echo "  Env: MARENGO_PI_HOST, MARENGO_INSTALL_ROOT, MARENGO_DEPLOY_VERBOSE=1" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install)
      DO_INSTALL=true
      shift
      ;;
    --skip-consul)
      SKIP_CONSUL=true
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

if [[ "${MARENGO_SKIP_CONSUL:-}" == 1 ]]; then
  SKIP_CONSUL=true
fi

ensure_cross_toolchain() {
  if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
    log_step "Cross toolchain: $(command -v aarch64-linux-gnu-gcc)"
    return 0
  fi
  if [[ "$(uname -s)" == "Darwin" ]]; then
    log_step "Cross linker missing — running setup-mac-pi-cross.sh"
    "${ROOT}/scripts/setup-mac-pi-cross.sh"
    return 0
  fi
  echo "error: aarch64-linux-gnu-gcc not found" >&2
  echo "  WSL2: ./scripts/setup-wsl-pi-cross.sh then ./scripts/deploy-pi.sh" >&2
  echo "  Windows: ./scripts/deploy-pi-docker.sh (cached compose volumes)" >&2
  exit 1
}

consul_lock_hash() {
  sha256sum package-lock.json | awk '{print $1}'
}

stage_copy_tree() {
  local src="$1"
  local dest="$2"
  local delete="${3:-false}"
  mkdir -p "$dest"
  if command -v rsync >/dev/null 2>&1; then
    if [[ "$delete" == true ]]; then
      rsync -a --delete "${src}/" "${dest}/"
    else
      rsync -a "${src}/" "${dest}/"
    fi
    return
  fi
  if [[ "$delete" == true ]] && [[ -d "$dest" ]]; then
    find "$dest" -mindepth 1 -delete 2>/dev/null || rm -rf "${dest:?}/"* 2>/dev/null || true
  fi
  cp -a "${src}/." "$dest/"
}

COMPOSE_SSH_DIR=""
COMPOSE_SSH_IDENTITY=""
COMPOSE_SSH_KNOWN=""

compose_ssh_setup() {
  if [[ "${MARENGO_DEPLOY_VIA_COMPOSE:-}" != 1 ]]; then
    return 0
  fi
  if [[ -n "$COMPOSE_SSH_DIR" ]]; then
    return 0
  fi
  COMPOSE_SSH_DIR="/tmp/marengo-deploy-ssh"
  mkdir -p "$COMPOSE_SSH_DIR"
  chmod 700 "$COMPOSE_SSH_DIR"
  if [[ -d /home/marengo/.ssh ]]; then
    cp -a /home/marengo/.ssh/. "$COMPOSE_SSH_DIR/"
  fi
  chmod 600 "$COMPOSE_SSH_DIR"/* 2>/dev/null || true
  chmod 644 "$COMPOSE_SSH_DIR"/*.pub 2>/dev/null || true
  if [[ -f "${COMPOSE_SSH_DIR}/config" ]]; then
    sed "s|~/.ssh|${COMPOSE_SSH_DIR}|g; s|\${HOME}/.ssh|${COMPOSE_SSH_DIR}|g" \
      "${COMPOSE_SSH_DIR}/config" > "${COMPOSE_SSH_DIR}/config.deploy"
    mv "${COMPOSE_SSH_DIR}/config.deploy" "${COMPOSE_SSH_DIR}/config"
  fi
  for k in id_ed25519 id_rsa id_ed25519_marengo; do
    if [[ -f "${COMPOSE_SSH_DIR}/${k}" ]]; then
      COMPOSE_SSH_IDENTITY="${COMPOSE_SSH_DIR}/${k}"
      break
    fi
  done
  COMPOSE_SSH_KNOWN="${COMPOSE_SSH_DIR}/known_hosts"
  touch "$COMPOSE_SSH_KNOWN"
  chmod 644 "$COMPOSE_SSH_KNOWN"
}

compose_ssh_opts() {
  compose_ssh_setup
  local -n _out=$1
  _out=(-o BatchMode=yes -o StrictHostKeyChecking=accept-new)
  if [[ -f "${COMPOSE_SSH_DIR}/config" ]]; then
    _out+=(-F "${COMPOSE_SSH_DIR}/config")
  elif [[ -n "$COMPOSE_SSH_IDENTITY" ]]; then
    _out+=(-o IdentitiesOnly=yes -i "$COMPOSE_SSH_IDENTITY")
  fi
  if [[ -n "$COMPOSE_SSH_KNOWN" ]]; then
    _out+=(-o UserKnownHostsFile="$COMPOSE_SSH_KNOWN")
  fi
}

compose_ssh() {
  local ssh_opts=()
  compose_ssh_opts ssh_opts
  ssh "${ssh_opts[@]}" "$@"
}

sync_staging_to_pi() {
  local staging="$1"
  local remote_root="$2"
  local ssh_opts=()
  compose_ssh_opts ssh_opts
  if command -v rsync >/dev/null 2>&1; then
    rsync -av --delete -e "ssh ${ssh_opts[*]}" "${staging}/" "${PI_HOST}:${remote_root}/"
    return
  fi
  log_note "rsync missing — using tar over ssh"
  tar -C "$staging" -czf - . | ssh "${ssh_opts[@]}" "$PI_HOST" "mkdir -p ${remote_root} && tar -xzf - -C ${remote_root}"
}

clear_consul_node_modules() {
  if [[ ! -d node_modules ]]; then
    return 0
  fi
  # Compose named volume mounts node_modules — cannot rm the mount point itself.
  if [[ "${MARENGO_DEPLOY_VIA_COMPOSE:-}" == 1 ]] || mountpoint -q node_modules 2>/dev/null; then
    find node_modules -mindepth 1 -delete 2>/dev/null \
      || find node_modules -mindepth 1 -maxdepth 1 -exec rm -rf {} + 2>/dev/null \
      || true
  else
    rm -rf node_modules
  fi
}

# Host-mounted consul/node_modules (Windows npm on Linux bind mount) ships the wrong
# @bufbuild/buf binary. Reinstall only when buf fails or package-lock changed.
ensure_consul_deps() {
  (
    cd "${ROOT}/consul"
    if [[ ! -f package-lock.json ]]; then
      echo "error: consul/package-lock.json missing" >&2
      exit 1
    fi
    local stamp=".marengo-npm-stamp"
    local hash
    hash="$(consul_lock_hash)"
    if [[ -d node_modules ]] && node_modules/.bin/buf --version >/dev/null 2>&1; then
      if [[ -f "$stamp" ]] && [[ "$(cat "$stamp")" == "$hash" ]]; then
        log_step "Consul deps OK (cached, lock ${hash:0:8}…)"
        return 0
      fi
      log_step "Consul deps present but lock changed — npm ci"
    else
      log_step "Consul deps missing or wrong platform — npm ci"
    fi
    clear_consul_node_modules
    npm ci
    if ! node_modules/.bin/buf --version >/dev/null 2>&1; then
      echo "error: buf still unavailable after npm ci" >&2
      exit 1
    fi
    echo "$hash" >"$stamp"
    log_step "Consul deps installed"
  )
}

consul_assets_fresh() {
  local dist="${ROOT}/consul/dist/index.html"
  [[ -f "$dist" ]] || return 1
  if find "${ROOT}/consul/src" "${ROOT}/proto" -type f -newer "$dist" 2>/dev/null | grep -q .; then
    return 1
  fi
  return 0
}

build_consul_assets() {
  if [[ "$SKIP_CONSUL" == true ]]; then
    log_step "Skipping Consul build (--skip-consul)"
    return 0
  fi
  if consul_assets_fresh; then
    log_step "Consul dist up to date — skipping npm build"
    return 0
  fi
  log_step "Building Consul static assets"
  ensure_consul_deps
  (
    cd "${ROOT}/consul"
    npm run build
  )
  if [[ ! -f "${ROOT}/consul/dist/index.html" ]]; then
    echo "error: consul build did not produce dist/index.html" >&2
    exit 1
  fi
  log_step "Consul build done"
}

cd "$ROOT"

if [[ -z "${MARENGO_DEPLOY_VIA_COMPOSE:-}" ]] && ! command -v aarch64-linux-gnu-gcc >/dev/null 2>&1 && [[ "$(uname -s)" != Darwin ]]; then
  log_warn "No native cross GCC — use ./scripts/deploy-pi-docker.sh or ./scripts/setup-wsl-pi-cross.sh"
fi

ensure_cross_toolchain
ensure_cargo_in_path
ensure_pi_cross_target

log_step "cargo build (release, ${TARGET})"
log_note "Packages: marengo-pi, marengo-gateway, motor-repl, imu-probe"
cargo build --release --target "$TARGET" -p marengo-pi -p marengo-gateway -p motor-repl -p imu-probe --features socketcan,linux-i2c
log_step "cargo build done"

build_consul_assets

if [[ -z "$PI_HOST" ]]; then
  log_step "Build complete (no deploy host)"
  echo "  ${ROOT}/target/${TARGET}/release/marengo-pi"
  echo "  ${ROOT}/target/${TARGET}/release/motor-repl"
  echo ""
  echo "Deploy: $0 [--install] joey@marengo.local"
  exit 0
fi

log_step "Staging deploy bundle"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

mkdir -p "$STAGING/target/release"
cp "${ROOT}/target/${TARGET}/release/marengo-pi" "$STAGING/target/release/"
cp "${ROOT}/target/${TARGET}/release/marengo-gateway" "$STAGING/target/release/"
cp "${ROOT}/target/${TARGET}/release/motor-repl" "$STAGING/target/release/"
cp "${ROOT}/target/${TARGET}/release/imu-probe" "$STAGING/target/release/"
stage_copy_tree "${ROOT}/config" "$STAGING/config"
stage_copy_tree "${ROOT}/assets" "$STAGING/assets"
stage_copy_tree "${ROOT}/scripts" "$STAGING/scripts"
mkdir -p "$STAGING/www"
if [[ -d "${ROOT}/consul/dist" ]]; then
  stage_copy_tree "${ROOT}/consul/dist" "$STAGING/www" true
fi

REMOTE_ROOT="${MARENGO_INSTALL_ROOT:-~/marengo}"
log_step "sync → ${PI_HOST}:${REMOTE_ROOT}/"
sync_staging_to_pi "$STAGING" "$REMOTE_ROOT"

if [[ "$DO_INSTALL" == true ]]; then
  log_step "install-pi.sh on ${PI_HOST}"
  # Narrow sudoers allow specific scripts only — not `sudo -n true`.
  if compose_ssh "$PI_HOST" "set -euo pipefail; sudo -n ${REMOTE_ROOT}/scripts/install-pi.sh"; then
    compose_ssh "$PI_HOST" "echo \$(git -C /opt/marengo rev-parse HEAD 2>/dev/null || echo unknown) \$(date -u +%Y-%m-%dT%H:%M:%SZ) > ${REMOTE_ROOT}/.deploy-rev && cat ${REMOTE_ROOT}/.deploy-rev" || true
  else
    log_warn "passwordless sudo install failed (run once on Pi: sudo ${REMOTE_ROOT}/scripts/install-pi.sh)"
  fi
else
  echo "On the Pi: sudo ${REMOTE_ROOT}/scripts/install-pi.sh"
fi

log_step "deploy complete"
