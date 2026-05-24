#!/usr/bin/env bash
# Fix ownership on Docker named volumes, then drop to marengo for all commands.
set -euo pipefail

MARENGO_USER="${MARENGO_USER:-marengo}"

ensure_dir_owned_by_marengo() {
  local path="$1"
  mkdir -p "${path}"
  chown -R "${MARENGO_USER}:${MARENGO_USER}" "${path}"
}

if [[ "$(id -u)" -eq 0 ]]; then
  ensure_dir_owned_by_marengo /workspace/consul/node_modules
  ensure_dir_owned_by_marengo /workspace/consul/src/gen
  ensure_dir_owned_by_marengo /workspace/target
  ensure_dir_owned_by_marengo /usr/local/cargo/registry
  ensure_dir_owned_by_marengo /usr/local/cargo/git

  if [[ $# -eq 0 ]]; then
    set -- bash
  fi
  exec gosu "${MARENGO_USER}" "$@"
fi

exec "$@"
