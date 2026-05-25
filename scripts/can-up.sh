#!/usr/bin/env bash
# Bring up production SocketCAN interfaces (1 Mbit/s). Idempotent.
set -euo pipefail

BITRATE="${MARENGO_CAN_BITRATE:-1000000}"
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
  ip link set "$iface" type can bitrate "$BITRATE"
  ip link set "$iface" up
  ip -br link show "$iface"
done
