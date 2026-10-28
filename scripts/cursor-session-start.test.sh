#!/usr/bin/env bash
# The Cursor sessionStart hook must never re-enable the marengo-pi MCP server
# (TOOL-09): run the committed hook JS against a fake workspace whose ensure
# script records any call.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOK="${1:-${ROOT}/.cursor/hooks/session-start-marengo.js}"
TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT
mkdir -p "${TMP}/scripts" "${TMP}/.cursor"
cat > "${TMP}/scripts/ensure-marengo-pi-mcp-enabled.py" <<'PY'
import pathlib, sys
pathlib.Path(__file__).with_name("ensure-called").write_text(" ".join(sys.argv[1:]))
PY
echo '{"mcpServers": {}}' > "${TMP}/.cursor/mcp.json"

out="$(printf '{"workspace_roots": ["%s"]}' "${TMP}" | node "${HOOK}")"

if [[ -f "${TMP}/scripts/ensure-called" ]]; then
  echo "FAIL: sessionStart ran ensure-marengo-pi-mcp-enabled.py $(cat "${TMP}/scripts/ensure-called")" >&2
  exit 1
fi
printf '%s' "${out}" | node -e '
let s = ""; process.stdin.on("data", (c) => (s += c)).on("end", () => {
  const ctx = JSON.parse(s).additional_context;
  if (!ctx.includes("Marengo session environment")) throw new Error("missing context");
});'
echo "cursor sessionStart hook contracts ok"
