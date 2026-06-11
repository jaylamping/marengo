#!/usr/bin/env bash
# Bring up virtual CAN interfaces for SocketCAN integration tests (Linux only).
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "vcan-up: requires Linux (run inside the vcan compose service on macOS/Windows)" >&2
  exit 1
fi

ensure_vcan_support() {
  if modprobe vcan 2>/dev/null; then
    return 0
  fi

  # Host may already have loaded vcan (e.g. CI host setup) while a container lacks
  # /lib/modules — verify the kernel actually supports vcan before proceeding.
  local probe=vcan-up-probe-$$
  if ip link add dev "${probe}" type vcan 2>/dev/null; then
    ip link del dev "${probe}"
    return 0
  fi

  echo "vcan-up: vcan unavailable (modprobe failed and cannot create vcan link)" >&2
  return 1
}

ensure_vcan_support || exit 1

for iface in vcan0 vcan1; do
  if ! ip link show "${iface}" &>/dev/null; then
    ip link add dev "${iface}" type vcan
  fi
  ip link set up "${iface}"

  echo "${iface} is up"
  ip -details link show "${iface}"
done
