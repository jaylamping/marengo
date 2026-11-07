#!/usr/bin/env bash
# Run one OpenCode Go model as a read-only reviewer, and fail fast instead of hanging.
# Usage: run_opencode.sh <model> <packet dir> <prompt file> [minutes, default 45]
# opencode run hangs with no output when the Go quota runs out (rev B2, 2026-11-07), so this probes first
# and kills the run as soon as its log shows a usage-limit or retry error.
# Optional spend cap: PRICES="<in> <cache read> <out>" in $/M and CAP_USD=2 kill the run once oc_cost.py says it passed the cap.
# Needs the review agent: copy opencode-review-agent.md to ~/.config/opencode/agents/review.md
set -u
M=$1; [[ $M == */* ]] || M=opencode-go/$M; PK=$2; PF=$3; MIN=${4:-45}; N=${M//\//-}; OUT=$PK/reviews; mkdir -p "$OUT"
FATAL='usage limit|RetryError|Invalid credential|insufficient'
# keys set with [Environment]::SetEnvironmentVariable(...,'User') don't reach shells that were already open
[ -n "${CHEAPERINFERENCE_API_KEY:-}" ] || export CHEAPERINFERENCE_API_KEY=$(powershell -NoProfile -Command "[Environment]::GetEnvironmentVariable('CHEAPERINFERENCE_API_KEY','User')" | tr -d '
')
cd "$PK" || exit 2
timeout 60 opencode run --pure -m "$M" --agent review --print-logs --format json "Reply with the single word OK." \
  </dev/null >"$OUT/opencode-$N.probe.json" 2>"$OUT/opencode-$N.probe.log"
if [ $? -ne 0 ] || grep -Eq "$FATAL" "$OUT/opencode-$N.probe.log"; then
  echo "probe failed: $(grep -Eo "$FATAL[^\"]{0,60}" "$OUT/opencode-$N.probe.log" | head -1)"; exit 3; fi
start=$(date +%s)
timeout "$((MIN*60))" opencode run --pure -m "$M" --variant high --agent review --print-logs --format json "$(cat "$PF")" \
  </dev/null >"$OUT/opencode-$N.json" 2>"$OUT/opencode-$N.log" & pid=$!
while kill -0 $pid 2>/dev/null; do
  if grep -Eq "$FATAL" "$OUT/opencode-$N.log"; then
    echo "fatal: $(grep -Eo "$FATAL[^\"]{0,60}" "$OUT/opencode-$N.log" | head -1)"; kill $pid; taskkill //F //T //PID $pid >/dev/null 2>&1; exit 4; fi
  if [ -n "${CAP_USD:-}" ] && [ -n "${PRICES:-}" ]; then
    c=$(python -X utf8 -I "$(dirname "$0")/oc_cost.py" "$OUT/opencode-$N.json" $PRICES 2>/dev/null || echo 0)
    if python -c "import sys; sys.exit(0 if float('$c')>float('$CAP_USD') else 1)"; then
      echo "spend cap: \$$c over \$$CAP_USD"; kill $pid; exit 5; fi
  fi
  sleep 15
done
wait $pid; rc=$?; echo "exit=$rc minutes=$(( ($(date +%s)-start)/60 ))"
[ -n "${PRICES:-}" ] && echo "cost \$$(python -X utf8 -I "$(dirname "$0")/oc_cost.py" "$OUT/opencode-$N.json" $PRICES)"
exit $rc
