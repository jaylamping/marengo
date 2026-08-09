#!/usr/bin/env bash
# Native toolchain setup for Cursor Cloud VMs (no Docker).
# Mirrors docker/Dockerfile.dev pins; run once per fresh VM, then ./scripts/bootstrap.sh.
set -euo pipefail

# Cloud agents advertise skill paths under /home/cursor; this image runs as ubuntu.
if [[ ! -e /home/cursor ]] && [[ -d /home/ubuntu ]]; then
  ln -sfn /home/ubuntu /home/cursor 2>/dev/null ||
    sudo -n ln -sfn /home/ubuntu /home/cursor 2>/dev/null ||
    true
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

PROTOC_VERSION="${PROTOC_VERSION:-28.3}"
CARGO_DENY_VERSION="${CARGO_DENY_VERSION:-0.16.3}"
ADVISORY_DB_REV="${ADVISORY_DB_REV:-808b5a554ded31fe41863ecf7c9abf4c26e8cfcd}"
ADVISORY_DB_DIR="/usr/local/cargo/advisory-db-pinned/github.com-a946fc29ac602819"
# Match mise.toml / .nvmrc / consul engines (^24.16.0) and docker/Dockerfile.dev.
NODE_MAJOR="${NODE_MAJOR:-24}"
NODE_MIN_VERSION="${NODE_MIN_VERSION:-24.16.0}"

need_sudo() {
  if [[ "${EUID}" -ne 0 ]]; then
    echo "sudo $*"
  else
    echo "$@"
  fi
}

SUDO="$(need_sudo)"

target_owner() {
  if [[ "${EUID}" -eq 0 && -n "${SUDO_UID:-}" && -n "${SUDO_GID:-}" ]]; then
    echo "${SUDO_UID}:${SUDO_GID}"
  else
    echo "$(id -u):$(id -g)"
  fi
}

cmd_reports_version() {
  local cmd="$1"
  local expected="$2"
  command -v "${cmd}" >/dev/null 2>&1 && "${cmd}" --version 2>&1 | grep -qF "${expected}"
}

# True when `node` is on PATH and satisfies NODE_MIN_VERSION (semver major.minor.patch).
node_meets_min() {
  command -v node >/dev/null 2>&1 || return 1
  local current
  current="$(node -p "process.versions.node" 2>/dev/null || true)"
  [[ -n "${current}" ]] || return 1
  # sort -V: if min sorts first (or equal), current >= min.
  [[ "$(printf '%s\n%s\n' "${NODE_MIN_VERSION}" "${current}" | sort -V | head -n1)" == "${NODE_MIN_VERSION}" ]]
}

node_bin_meets_min() {
  local bin="$1"
  [[ -x "${bin}" ]] || return 1
  local current
  current="$("${bin}" -p "process.versions.node" 2>/dev/null || true)"
  [[ -n "${current}" ]] || return 1
  [[ "$(printf '%s\n%s\n' "${NODE_MIN_VERSION}" "${current}" | sort -V | head -n1)" == "${NODE_MIN_VERSION}" ]]
}

# Cursor cloud PATH puts /exec-daemon/node (Node 22) ahead of nvm and /usr/bin.
# Shim a compliant node/npm/npx into the first PATH dir we can write (/usr/local/cargo/bin).
shim_nodejs_on_path() {
  local node_bin="$1"
  local bindir
  bindir="$(cd "$(dirname "${node_bin}")" && pwd)"
  local shim_dir="${NODE_SHIM_DIR:-/usr/local/cargo/bin}"
  if [[ ! -d "${shim_dir}" ]]; then
    ${SUDO} mkdir -p "${shim_dir}"
    ${SUDO} chown "$(target_owner)" "${shim_dir}" 2>/dev/null || true
  fi
  if [[ ! -w "${shim_dir}" ]]; then
    echo "setup-cloud: cannot write Node shims to ${shim_dir}" >&2
    return 1
  fi
  local name
  for name in node npm npx; do
    if [[ -x "${bindir}/${name}" ]]; then
      ln -sfn "${bindir}/${name}" "${shim_dir}/${name}"
    fi
  done
  export PATH="${shim_dir}:${PATH}"
  hash -r 2>/dev/null || true
}

install_nodejs_via_nvm() {
  export NVM_DIR="${NVM_DIR:-${HOME}/.nvm}"
  if [[ ! -s "${NVM_DIR}/nvm.sh" ]]; then
    return 1
  fi
  # shellcheck disable=SC1091
  . "${NVM_DIR}/nvm.sh"
  echo "==> nvm install ${NODE_MIN_VERSION}"
  nvm install "${NODE_MIN_VERSION}"
  nvm alias default "${NODE_MIN_VERSION}"
  nvm use "${NODE_MIN_VERSION}"
  # Do NOT use `nvm which` / `command -v node` here: Cursor places
  # /usr/local/cargo/bin/node → /exec-daemon/node first on PATH, so those
  # resolve to Node 22 even after `nvm use`.
  local nvm_node="${NVM_DIR}/versions/node/v${NODE_MIN_VERSION}/bin/node"
  if [[ ! -x "${nvm_node}" ]]; then
    echo "setup-cloud: nvm node missing at ${nvm_node}" >&2
    return 1
  fi
  # Drop stale exec-daemon shims before rewriting.
  rm -f /usr/local/cargo/bin/node /usr/local/cargo/bin/npm /usr/local/cargo/bin/npx
  shim_nodejs_on_path "${nvm_node}"
}

install_nodejs_via_nodesource() {
  echo "==> NodeSource Node.js ${NODE_MAJOR}.x"
  curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | ${SUDO} bash -
  ${SUDO} apt-get install -y --no-install-recommends nodejs
  hash -r 2>/dev/null || true
  if node_bin_meets_min /usr/bin/node; then
    shim_nodejs_on_path /usr/bin/node
    return 0
  fi
  return 1
}

install_nodejs() {
  if node_meets_min; then
    echo "==> Node.js $(node -v) via $(command -v node) (meets >= ${NODE_MIN_VERSION})"
    return 0
  fi
  if command -v node >/dev/null 2>&1; then
    echo "==> Node.js $(node -v) via $(command -v node) is below ${NODE_MIN_VERSION}"
  else
    echo "==> Node.js missing — need >= ${NODE_MIN_VERSION}"
  fi

  # Prefer nvm on Cursor cloud images (already present); NodeSource otherwise (matches Dockerfile.dev).
  if ! install_nodejs_via_nvm; then
    install_nodejs_via_nodesource || true
  fi

  if ! node_meets_min; then
    echo "setup-cloud: Node.js still below ${NODE_MIN_VERSION} after install" >&2
    echo "setup-cloud: node=$(command -v node 2>/dev/null || echo none) version=$(node -v 2>/dev/null || echo none)" >&2
    echo "setup-cloud: PATH=${PATH}" >&2
    exit 1
  fi
  echo "==> Node.js $(node -v) via $(command -v node)"
  npm --version
}

protoc_arch() {
  case "$(uname -m)" in
    aarch64 | arm64) echo "aarch_64" ;;
    x86_64 | amd64) echo "x86_64" ;;
    *)
      echo "setup-cloud: unsupported CPU architecture for protoc: $(uname -m)" >&2
      exit 1
      ;;
  esac
}

install_protoc() {
  local protoc_arch
  protoc_arch="$(protoc_arch)"
  if ! command -v unzip >/dev/null 2>&1; then
    ${SUDO} apt-get update -qq
    ${SUDO} apt-get install -y --no-install-recommends unzip
  fi
  curl -fsSL \
    "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-${protoc_arch}.zip" \
    -o /tmp/protoc.zip
  ${SUDO} unzip -q -o /tmp/protoc.zip -d /usr/local
  rm -f /tmp/protoc.zip
}

install_cargo_tool() {
  local crate="$1"
  local version="$2"
  cargo install "${crate}" --locked --version "${version}"
}

echo "==> Node.js >= ${NODE_MIN_VERSION} (Consul / bootstrap)"
install_nodejs

echo "==> protoc ${PROTOC_VERSION}"
if cmd_reports_version protoc "${PROTOC_VERSION}"; then
  :
else
  install_protoc
fi
protoc --version

echo "==> rustfmt + clippy"
rustup component add rustfmt clippy

echo "==> cargo-deny ${CARGO_DENY_VERSION}"
if cmd_reports_version cargo-deny "${CARGO_DENY_VERSION}"; then
  :
else
  install_cargo_tool cargo-deny "${CARGO_DENY_VERSION}"
fi
cargo-deny --version

echo "==> cargo-audit"
if command -v cargo-audit >/dev/null 2>&1; then
  :
else
  cargo install cargo-audit --locked
fi
cargo-audit --version

echo "==> advisory-db (pinned for cargo-deny 0.16.x)"
${SUDO} mkdir -p /usr/local/cargo/advisory-db-pinned
if [[ ! -d "${ADVISORY_DB_DIR}/.git" ]]; then
  if [[ -e "${ADVISORY_DB_DIR}" ]]; then
    ${SUDO} rm -rf "${ADVISORY_DB_DIR}"
  fi
  ${SUDO} git clone https://github.com/RustSec/advisory-db.git "${ADVISORY_DB_DIR}"
fi
${SUDO} git -C "${ADVISORY_DB_DIR}" fetch --depth 1 origin "${ADVISORY_DB_REV}" 2>/dev/null || true
${SUDO} git -C "${ADVISORY_DB_DIR}" checkout -f "${ADVISORY_DB_REV}"
${SUDO} git -C "${ADVISORY_DB_DIR}" clean -fd
${SUDO} chown -R "$(target_owner)" /usr/local/cargo/advisory-db-pinned

echo "==> workspace bootstrap"
"${ROOT}/scripts/bootstrap.sh"

echo "setup-cloud: ok"
