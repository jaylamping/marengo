#!/usr/bin/env bash
# Start Marengo Auto Learn BFF (loopback). Used by marengo-auto-learn.service.
set -euo pipefail

NODE_BIN="${MARENGO_AUTO_LEARN_NODE:-/opt/marengo/tools/node/bin/node}"
SERVER_JS="${MARENGO_AUTO_LEARN_SERVER:-/opt/marengo/tools/compound-auto-learn/dist/server.js}"

if [[ -z "${CURSOR_API_KEY:-}" || -z "${AUTO_LEARN_TOKEN:-}" ]]; then
  echo "marengo-auto-learn: CURSOR_API_KEY and AUTO_LEARN_TOKEN required in /etc/marengo/env — not starting" >&2
  # Exit 0 so Restart=on-failure does not crash-loop when secrets are unset.
  exit 0
fi

if [[ ! -x "$NODE_BIN" ]]; then
  echo "marengo-auto-learn: node not found at ${NODE_BIN}" >&2
  exit 1
fi
if [[ ! -f "$SERVER_JS" ]]; then
  echo "marengo-auto-learn: server not found at ${SERVER_JS}" >&2
  exit 1
fi

exec "$NODE_BIN" "$SERVER_JS"
