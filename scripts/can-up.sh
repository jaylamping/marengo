#!/usr/bin/env bash
# Bring up production SocketCAN interfaces (1 Mbit/s). Idempotent.
#
# Default txqueuelen is 10 on many kernels — too small for mcp251x + multi-motor
# MIT at 200 Hz (ENOBUFS / "No buffer space available" on enable bursts).
set -euo pipefail

BITRATE="${MARENGO_CAN_BITRATE:-1000000}"
# SocketCAN TX queue depth; override with MARENGO_CAN_TXQUEUELEN if needed.
TXQUEUELEN="${MARENGO_CAN_TXQUEUELEN:-1000}"
# Auto-recover mcp251x bus-off (0 = stay down until manual ip link bounce).
RESTART_MS="${MARENGO_CAN_RESTART_MS:-100}"
if [ $# -eq 0 ]; then
  INTERFACES=(can0 can1)
else
  INTERFACES=("$@")
fi

for iface in "${INTERFACES[@]}"; do
  if ! ip link show "$iface" &>/dev/null; then
    echo "interface $iface not found (check CAN HAT overlay)" >&2
    exit 1
  fi
  ip link set "$iface" down 2>/dev/null || true
  ip link set "$iface" type can bitrate "$BITRATE" restart-ms "$RESTART_MS"
  ip link set "$iface" txqueuelen "$TXQUEUELEN"
  ip link set "$iface" up
  ip -br link show "$iface"
done
