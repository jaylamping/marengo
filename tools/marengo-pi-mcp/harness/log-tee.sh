#!/usr/bin/env bash
# Remote bench session log tee — sourced or invoked from MCP harness.
# Usage: LOGDIR=/opt/marengo/var/log LABEL=my-run bench-log-tee.sh -- command...
set -euo pipefail

LOGDIR="${LOGDIR:-/opt/marengo/var/log}"
LABEL="${LABEL:-bench}"
mkdir -p "$LOGDIR"
TS=$(date -u +"%Y%m%dT%H%M%SZ")
LOG="$LOGDIR/bench-$TS.log"
JSON="$LOGDIR/bench-$TS.json"

echo "=== bench session $TS ($LABEL) ===" | tee "$LOG"
EXIT=0
if "$@" 2>&1 | tee -a "$LOG"; then
  :
else
  EXIT=$?
fi

ln -sf "$LOG" "$LOGDIR/bench-latest.log"
ln -sf "$JSON" "$LOGDIR/bench-latest.json" 2>/dev/null || true

echo "{\"log\":\"$LOG\",\"json\":\"$JSON\",\"ts\":\"$TS\",\"label\":\"$LABEL\",\"exit\":$EXIT}"
exit "$EXIT"
