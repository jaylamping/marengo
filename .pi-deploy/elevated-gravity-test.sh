#!/usr/bin/env bash
set -euo pipefail
if [[ -f /etc/marengo/env ]]; then set -a; source /etc/marengo/env; set +a; fi
export MARENGO_ROOT=/opt/marengo
export MARENGO_CONFIG_DIR=/opt/marengo/config/bringup/shoulder_pitch_right_only
export RUST_LOG=robstride=info,davout=info,berthier=info,marengo_pi=info
cd /opt/marengo
pkill -f /opt/marengo/bin/marengo-pi 2>/dev/null || true
sleep 0.5
LOGDIR=/opt/marengo/var/log
mkdir -p "$LOGDIR"
TS=$(date -u +"%Y%m%dT%H%M%SZ")
LOG="$LOGDIR/bench-$TS.log"
echo "=== elevated gravity test v2 $TS ===" | tee "$LOG"
{
  printf '%s\n' home
  printf '%s\n' 'enable bench'
  printf '%s\n' 'hold-at 0.3'
  sleep 10
  printf '%s\n' status
  printf '%s\n' 'enable bench'
  printf '%s\n' gravity-on
  sleep 25
  printf '%s\n' status
  printf '%s\n' disable
  printf '%s\n' quit
} 2>&1 | timeout 50 bin/marengo-pi | tee -a "$LOG"
ln -sf "$LOG" "$LOGDIR/bench-latest.log"
echo "log=$LOG"
