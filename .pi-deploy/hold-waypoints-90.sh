#!/bin/bash
set -euo pipefail
export MARENGO_ROOT=/opt/marengo
export MARENGO_CONFIG_DIR=/opt/marengo/config/bringup/shoulder_pitch_right_only
export RUST_LOG=robstride=info,davout=info,berthier=info,marengo_pi=info
cd /opt/marengo
pkill -f /opt/marengo/bin/marengo-pi 2>/dev/null || true
sleep 0.3
bin/motor-repl disable 2>/dev/null || true
{
  printf '%s\n' home 'enable bench'
  for target in 0.35 0.70 1.05 1.570796; do
    printf '%s\n' "hold-at $target"
    sleep 28
  done
  printf '%s\n' status disable quit
} | timeout 130 bin/marengo-pi
