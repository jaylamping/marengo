#!/usr/bin/env sh
# Estimate MIT CAN frame rate on a Robstride bus (Pi bench).
#
# Usage:
#   measure-can-mit-rate.sh [interface] [seconds]
#   measure-can-mit-rate.sh can0 2
#
# Run while marengo-pi is Active (Position/GravityComp). Counts all extended
# frames on the interface; with one RS03 enabled expect ~2x loop_hz total
# (one OperationControl TX + one OperationStatus RX per tick at 200 Hz → ~400 frames/s).

set -eu

IFACE="${1:-can0}"
SEC="${2:-2}"

if ! command -v candump >/dev/null 2>&1; then
  echo "error: candump not found (can-utils)" >&2
  exit 1
fi

if ! ip link show "$IFACE" 2>/dev/null | grep -q "UP"; then
  echo "error: $IFACE is not UP" >&2
  exit 1
fi

echo "counting frames on $IFACE for ${SEC}s (start marengo-pi hold before running)..."
count="$(timeout "$SEC" candump -t z "$IFACE" 2>/dev/null | wc -l | tr -d ' ')"
total_hz="$(awk "BEGIN { printf \"%.1f\", $count / $SEC }")"
echo "frames=$count  total=${total_hz}/s  (~${total_hz} frames/s on bus)"
echo "expect ~400/s per motor at loop_hz=200 (200 TX + 200 RX MIT frames)"
