#!/usr/bin/env bash
# Launcher for Cursor MCP: Cursor's spawn PATH often lacks mise `node` (ENOENT).
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
    /usr/local/bin/node \
    /usr/bin/node
  do
    if [[ -x "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return
    fi
  done
  echo "run-mcp.sh: node not found (mise PATH missing under Cursor). Install Node 24 or set MARENGO_MCP_NODE." >&2
  exit 127
}

NODE="$(resolve_node)"
exec "${NODE}" "${ROOT}/dist/index.js"
