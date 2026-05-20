#!/usr/bin/env bash
# Bring up virtual CAN interfaces for bench development (Linux only).
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "vcan-up: requires Linux (run inside the vcan compose service on macOS/Windows)" >&2
  exit 1
fi

modprobe vcan 2>/dev/null || true

if ! ip link show vcan0 &>/dev/null; then
  ip link add dev vcan0 type vcan
fi
ip link set up vcan0

echo "vcan0 is up"
ip -details link show vcan0
