#!/usr/bin/env bash
# Launcher for Cursor MCP: Cursor's spawn PATH often lacks mise `node` (ENOENT).
#
# Profile / SSH defaults live in src/launch.ts → dist/launch.js — not here and
# not in `.cursor/mcp.json` (env thrash auto-disables the project MCP; ADR 0016).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"

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
ENTRY="${ROOT}/dist/launch.js"
if [[ ! -f "${ENTRY}" ]]; then
  echo "run-mcp.sh: missing ${ENTRY} — run \`just mcp-build\` first." >&2
  exit 1
fi
exec "${NODE}" "${ENTRY}"
