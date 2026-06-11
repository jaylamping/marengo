#!/usr/bin/env bash
# Prepare GitHub Actions runners for vcan SocketCAN tests.
set -euo pipefail

probe_vcan() {
  sudo ip link add dev vcan-ci-probe type vcan
  sudo ip link del dev vcan-ci-probe
}

load_vcan_module() {
  sudo modprobe vcan
}

if load_vcan_module 2>/dev/null && probe_vcan 2>/dev/null; then
  echo "vcan host setup: ok"
  exit 0
fi

kernel_release="$(uname -r)"
echo "==> vcan unavailable; installing linux-modules-extra-${kernel_release}"
sudo apt-get update -qq
sudo apt-get install -y --no-install-recommends "linux-modules-extra-${kernel_release}"

load_vcan_module
probe_vcan
echo "vcan host setup: ok"
