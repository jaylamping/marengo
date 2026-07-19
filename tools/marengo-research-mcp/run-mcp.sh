#!/usr/bin/env sh
# Marengo research MCP launcher — injects GITHUB_TOKEN from `gh auth token` when available.
set -eu
ROOT="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$ROOT/../.." && pwd)"

export MARENGO_RESEARCH_CACHE_DIR="${MARENGO_RESEARCH_CACHE_DIR:-$REPO_ROOT/.marengo-research}"
export MARENGO_WORKSPACE="${MARENGO_WORKSPACE:-$REPO_ROOT}"

if [ -z "${GITHUB_TOKEN:-}" ] && command -v gh >/dev/null 2>&1; then
  token="$(gh auth token 2>/dev/null || true)"
  if [ -n "$token" ]; then
    export GITHUB_TOKEN="$token"
  fi
fi

cd "$ROOT"
exec uv run python -m marengo_research_mcp.server
