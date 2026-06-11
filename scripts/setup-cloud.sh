#!/usr/bin/env bash
# Native toolchain setup for Cursor Cloud VMs (no Docker).
# Mirrors docker/Dockerfile.dev pins; run once per fresh VM, then ./scripts/bootstrap.sh.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

PROTOC_VERSION="${PROTOC_VERSION:-28.3}"
CARGO_DENY_VERSION="${CARGO_DENY_VERSION:-0.16.3}"
ADVISORY_DB_REV="${ADVISORY_DB_REV:-808b5a554ded31fe41863ecf7c9abf4c26e8cfcd}"
ADVISORY_DB_DIR="/usr/local/cargo/advisory-db-pinned/github.com-a946fc29ac602819"

need_sudo() {
  if [[ "${EUID}" -ne 0 ]]; then
    echo "sudo $*"
  else
    echo "$@"
  fi
}

SUDO="$(need_sudo)"

cmd_reports_version() {
  local cmd="$1"
  local expected="$2"
  command -v "${cmd}" >/dev/null 2>&1 && "${cmd}" --version 2>&1 | grep -qF "${expected}"
}

install_protoc() {
  if ! command -v unzip >/dev/null 2>&1; then
    ${SUDO} apt-get update -qq
    ${SUDO} apt-get install -y --no-install-recommends unzip
  fi
  curl -fsSL \
    "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip" \
    -o /tmp/protoc.zip
  ${SUDO} unzip -q -o /tmp/protoc.zip -d /usr/local
  rm -f /tmp/protoc.zip
}

install_cargo_tool() {
  local crate="$1"
  local version="$2"
  cargo install "${crate}" --locked --version "${version}"
}

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
${SUDO} chown -R "$(id -u):$(id -g)" /usr/local/cargo/advisory-db-pinned

echo "==> workspace bootstrap"
"${ROOT}/scripts/bootstrap.sh"

echo "setup-cloud: ok"
