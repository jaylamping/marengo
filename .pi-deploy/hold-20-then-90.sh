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
  printf '%s\n' home 'enable bench' 'hold-at 0.35'
  sleep 35
  printf '%s\n' 'hold-at 1.570796'
  sleep 80
  printf '%s\n' status disable quit
} | timeout 125 bin/marengo-pi
