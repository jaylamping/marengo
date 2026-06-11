#!/usr/bin/env bash
# Prepare GitHub Actions runners for vcan SocketCAN tests.
# CAN is required in CI — this script fails loudly if vcan cannot be brought up.
set -euo pipefail

fail() {
  echo "vcan host setup: FAILED — $*" >&2
  echo "CAN integration tests require working vcan support; do not skip or bypass this job." >&2
  exit 1
}

probe_vcan() {
  sudo ip link add dev vcan-ci-probe type vcan
  sudo ip link del dev vcan-ci-probe
}

load_vcan_module() {
  sudo modprobe vcan
}

bring_up_vcan_host() {
  # Host creates vcan0/vcan1; docker --network host reuses them in CI.
  sudo env PATH="${PATH}" ./scripts/vcan-up.sh \
    || fail "vcan-up failed on CI host"
}

if ! load_vcan_module || ! probe_vcan; then
  kernel_release="$(uname -r)"
  echo "==> vcan module not loaded; installing linux-modules-extra-${kernel_release}"
  sudo apt-get update -qq
  sudo apt-get install -y --no-install-recommends "linux-modules-extra-${kernel_release}" \
    || fail "linux-modules-extra-${kernel_release} is not available on this runner"

  load_vcan_module || fail "modprobe vcan failed after installing linux-modules-extra"
  probe_vcan || fail "ip link type vcan probe failed — kernel lacks CAN support"
fi

bring_up_vcan_host
echo "vcan host setup: ok"
