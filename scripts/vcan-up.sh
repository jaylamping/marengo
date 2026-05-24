#!/usr/bin/env bash
# Bring up virtual CAN interfaces for SocketCAN integration tests (Linux only).
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "vcan-up: requires Linux (run inside the vcan compose service on macOS/Windows)" >&2
  exit 1
fi

modprobe vcan 2>/dev/null || true

for iface in vcan0 vcan1; do
  if ! ip link show "${iface}" &>/dev/null; then
    ip link add dev "${iface}" type vcan
  fi
  ip link set up "${iface}"

  echo "${iface} is up"
  ip -details link show "${iface}"
done
