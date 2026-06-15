#!/usr/bin/env sh
# Archive hot bench logs into SQLite + gzip blobs via marengo-log-cli.
#
# Usage: bench-log-archive.sh [LOGDIR] [KEEP]
#   LOGDIR default: /opt/marengo/var/log
#   KEEP default: 50

set -eu

LOGDIR="${1:-/opt/marengo/var/log}"
KEEP="${2:-50}"
ROOT="${MARENGO_ROOT:-/opt/marengo}"
CLI="${MARENGO_ROOT:-/opt/marengo}/bin/marengo-log-cli"

if [ ! -x "$CLI" ]; then
  CLI="$(command -v marengo-log-cli 2>/dev/null || true)"
fi

if [ -z "${CLI:-}" ] || [ ! -x "$CLI" ]; then
  echo "marengo-log-cli not found; skip archive" >&2
  exit 0
fi

export MARENGO_ROOT="$ROOT"
"$CLI" archive --keep "$KEEP"
