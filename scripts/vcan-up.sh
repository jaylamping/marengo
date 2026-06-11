#!/usr/bin/env bash
# Bring up virtual CAN interfaces for SocketCAN integration tests (Linux only).
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "vcan-up: requires Linux (run inside the vcan compose service on macOS/Windows)" >&2
  exit 1
fi

fail() {
  echo "vcan-up: $*" >&2
  exit 1
}

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

  fail "vcan unavailable (modprobe failed and cannot create vcan link)"
}

need_create=false
for iface in vcan0 vcan1; do
  if ! ip link show "${iface}" &>/dev/null; then
    need_create=true
    break
  fi
done

if [[ "${need_create}" == true ]]; then
  ensure_vcan_support
fi

for iface in vcan0 vcan1; do
  if ! ip link show "${iface}" &>/dev/null; then
    ip link add dev "${iface}" type vcan \
      || fail "failed to create ${iface} (is vcan loaded on the host?)"
  fi
  ip link set up "${iface}" || fail "failed to bring ${iface} up"

  echo "${iface} is up"
  ip -details link show "${iface}"
done
