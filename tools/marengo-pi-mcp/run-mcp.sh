#!/usr/bin/env bash
# Launcher for Cursor MCP: Cursor's spawn PATH often lacks mise `node` (ENOENT).
#
# Keep profile / SSH defaults HERE — not in `.cursor/mcp.json`.
# Cursor hashes mcp.json `env` into an approval key; changing that env
# auto-disables the project MCP until you re-enable it in the UI.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"

# Stable connection + bench defaults (override via process env if needed).
export MARENGO_PI_HOST="${MARENGO_PI_HOST:-joey-robot.tail0b414.ts.net}"
export MARENGO_PI_USER="${MARENGO_PI_USER:-joey}"
export MARENGO_PI_ROOT="${MARENGO_PI_ROOT:-/opt/marengo}"
export MARENGO_CONFIG_DIR="${MARENGO_CONFIG_DIR:-/opt/marengo/config/bringup/arm_4dof_right}"
export MARENGO_BENCH_PROFILE="${MARENGO_BENCH_PROFILE:-elbow_attached}"

if [[ -z "${SSH_IDENTITY_FILE:-}" ]]; then
  for candidate in \
    "${HOME}/.ssh/id_ed25519_marengo" \
    "${HOME}/.ssh/id_ed25519"
  do
    if [[ -f "${candidate}" ]]; then
      export SSH_IDENTITY_FILE="${candidate}"
      break
    fi
  done
fi

resolve_node() {
  if [[ -n "${MARENGO_MCP_NODE:-}" && -x "${MARENGO_MCP_NODE}" ]]; then
    printf '%s\n' "${MARENGO_MCP_NODE}"
    return
  fi
  if command -v mise >/dev/null 2>&1; then
    local mise_node
    mise_node="$(mise which node 2>/dev/null || true)"
    if [[ -n "${mise_node}" && -x "${mise_node}" ]]; then
      printf '%s\n' "${mise_node}"
      return
    fi
  fi
  local candidate
  for candidate in \
    "${HOME}/.local/share/mise/shims/node" \
    "${HOME}/.local/share/mise/installs/node/24.16.0/bin/node" \
    "${HOME}/AppData/Local/mise/shims/node.exe" \
    "${HOME}/AppData/Local/mise/installs/node/24.16.0/node.exe" \
    /usr/local/bin/node \
    /usr/bin/node \
    "/c/Program Files/nodejs/node.exe"
  do
    if [[ -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return
    fi
  done
  # Windows: `command -v` may find node.exe without -x on some mounts
  if command -v node >/dev/null 2>&1; then
    command -v node
    return
  fi
  echo "run-mcp.sh: node not found (mise PATH missing under Cursor). Install Node 24 or set MARENGO_MCP_NODE." >&2
  exit 127
}

NODE="$(resolve_node)"
export PATH="$(dirname "${NODE}"):${HOME}/.local/share/mise/shims:${PATH:-}"
exec "${NODE}" "${ROOT}/dist/index.js"
