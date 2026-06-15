#!/usr/bin/env sh
# Drop old bench logs and position traces on the Pi (or local dev).
#
# Usage:
#   bench-log-prune.sh [LOGDIR] [KEEP]
#   LOGDIR default: /opt/marengo/var/log
#   KEEP default: 50 (newest files kept per pattern)
#
# Never removes symlinks bench-latest.log / position-trace-latest.csv.

set -eu

LOGDIR="${1:-/opt/marengo/var/log}"
KEEP="${2:-50}"

prune_pattern() {
  pat="$1"
  count=0
  removed=0
  # shellcheck disable=SC2012
  for f in $(ls -1t "$LOGDIR"/$pat 2>/dev/null); do
    count=$((count + 1))
    if [ "$count" -le "$KEEP" ]; then
      continue
    fi
    rm -f "$f"
    removed=$((removed + 1))
  done
  if [ "$removed" -gt 0 ]; then
    echo "pruned $removed $pat (kept $KEEP newest in $LOGDIR)"
  fi
}

prune_pattern 'bench-*.log'
prune_pattern 'position-trace-*.csv'
prune_pattern 'bench-*.json'
prune_pattern 'candump-*.log'
