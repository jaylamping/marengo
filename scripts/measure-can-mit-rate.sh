#!/usr/bin/env sh
# Estimate MIT CAN frame rate on a Robstride bus (Pi bench).
#
# Usage:
#   measure-can-mit-rate.sh [interface] [seconds]
#   measure-can-mit-rate.sh can0 2
#   measure-can-mit-rate.sh --kernel 10 can0 can1   # kernel RX/TX packet counters (preferred)
#
# Run while marengo-pi is Active (Position/GravityComp). Candump mode counts all
# frames seen by userspace; --kernel uses ip link statistics (wire truth when
# candump drops frames under load). With one RS03 @ loop_hz=200 expect ~400/s
# per interface (200 OperationControl TX + 200 OperationStatus RX).

set -eu

read_can_stats() {
  iface="$1"
  ip -statistics link show dev "$iface" 2>/dev/null | awk '
    /^[[:space:]]+RX:/ { getline; rx=$2 }
    /^[[:space:]]+TX:/ { getline; tx=$2 }
    END { if (rx != "" && tx != "") print rx, tx; else print "0 0" }
  '
}

kernel_mode() {
  shift
  SEC="${1:-10}"
  shift || true
  IFACES="${*:-can0}"
  if ! ip link show can0 >/dev/null 2>&1; then
    echo "error: no CAN interfaces" >&2
    exit 1
  fi
  TMP="${TMPDIR:-/tmp}/marengo-can-kernel-$$"
  mkdir -p "$TMP"
  trap 'rm -rf "$TMP"' EXIT INT TERM

  echo "kernel counter delta over ${SEC}s on: $IFACES"
  echo "(hold must already be running — this script only samples counters)"
  for iface in $IFACES; do
    if ! ip link show "$iface" 2>/dev/null | grep -q "UP"; then
      echo "warn: $iface not UP, skipping" >&2
      continue
    fi
    read_can_stats "$iface" > "$TMP/$iface.start"
    read rx tx <<EOF
$(cat "$TMP/$iface.start")
EOF
    echo "  $iface start: rx=$rx tx=$tx"
  done
  sleep "$SEC"
  total_rx=0
  total_tx=0
  for iface in $IFACES; do
    if ! test -f "$TMP/$iface.start"; then
      continue
    fi
    read rx0 tx0 <<EOF
$(cat "$TMP/$iface.start")
EOF
    read rx1 tx1 <<EOF
$(read_can_stats "$iface")
EOF
    drx=$((rx1 - rx0))
    dtx=$((tx1 - tx0))
    total=$((drx + dtx))
    hz="$(awk "BEGIN { printf \"%.1f\", $total / $SEC }")"
    rx_hz="$(awk "BEGIN { printf \"%.1f\", $drx / $SEC }")"
    tx_hz="$(awk "BEGIN { printf \"%.1f\", $dtx / $SEC }")"
    echo "  $iface end:   rx=$rx1 tx=$tx1  delta_rx=$drx delta_tx=$dtx"
    echo "  $iface rate: total=${hz}/s  rx=${rx_hz}/s  tx=${tx_hz}/s"
    total_rx=$((total_rx + drx))
    total_tx=$((total_tx + dtx))
  done
  agg="$(awk "BEGIN { printf \"%.1f\", ($total_rx + $total_tx) / $SEC }")"
  echo "aggregate: rx+tx=${agg}/s across interfaces (expect ~400/s per motor @ 200 Hz loop)"
}

if [ "${1:-}" = "--kernel" ]; then
  kernel_mode "$@"
  exit 0
fi

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
echo "tip: use --kernel for counter-based rate if candump under-reports"
