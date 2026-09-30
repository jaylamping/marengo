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

probe_iface=""
probe_created=false
probe_add_pending=false
signal_status=0
cleanup_probe() {
  local status=$?
  if [[ "${probe_created}" == true ]]; then
    if ! ip link del dev "${probe_iface}"; then
      echo "vcan-up: failed to remove this invocation's probe ${probe_iface}" >&2
      if [[ "${status}" -eq 0 ]]; then status=1; fi
    fi
  fi
  exit "${status}"
}
handle_signal() {
  # Wait for a pending add result so cleanup knows whether we own the probe.
  if [[ "${probe_add_pending}" == true ]]; then
    signal_status="$1"
  else
    exit "$1"
  fi
}
trap cleanup_probe EXIT
trap 'handle_signal 129' HUP
trap 'handle_signal 130' INT
trap 'handle_signal 143' TERM

ensure_vcan_support() {
  if modprobe vcan; then
    return 0
  fi

  # Host may already have loaded vcan (e.g. CI host setup) while a container lacks
  # /lib/modules — verify the kernel actually supports vcan before proceeding.
  # Linux interface names have a 15-character maximum. A random suffix reduces
  # collisions when separate PID namespaces share a network namespace. A failed
  # creation is never treated as ownership, including a remaining name collision.
  local pid="$$"
  if [[ "${#pid}" -gt 7 ]]; then pid="${pid: -7}"; fi
  probe_iface="vp${pid}-${RANDOM}"
  probe_add_pending=true
  if ip link add dev "${probe_iface}" type vcan; then
    probe_created=true
  fi
  probe_add_pending=false
  if [[ "${signal_status}" -ne 0 ]]; then exit "${signal_status}"; fi
  if [[ "${probe_created}" == true ]]; then
    ip link del dev "${probe_iface}" || fail "failed to remove vcan support probe"
    probe_created=false
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
