#!/usr/bin/env bash
# Profile marengo-pi loop rate and tick-phase breakdown on the Pi bench.
# Usage on Pi: ./scripts/profile-pi-loop.sh [A|B|C]
#   A = minimal (no trace, no candump)
#   B = position trace CSV only
#   C = full bench wrapper (trace + candump + tee)
set -euo pipefail

MODE="${1:-A}"
ROOT="${MARENGO_ROOT:-/opt/marengo}"
CFG="${MARENGO_CONFIG_DIR:-$ROOT/config/bringup/arm_3dof_right}"
export MARENGO_ROOT="$ROOT"
export MARENGO_CONFIG_DIR="$CFG"

if [[ -f /etc/marengo/env ]]; then
  set -a
  # shellcheck source=/dev/null
  source /etc/marengo/env
  set +a
fi

# Override bench env after /etc/marengo/env (profile runs must not inherit production RUST_LOG).
export RUST_LOG="${PROFILE_RUST_LOG:-marengo_pi=debug,berthier=debug,marengo_pi::imu=warn}"

cd "$ROOT"
pkill -f '[m]arengo-pi' 2>/dev/null || true
sleep 0.5
bin/motor-repl disable 2>/dev/null || true

run_pipe() {
  {
    printf '%s\n' home
    printf '%s\n' "enable bench"
    printf '%s\n' "hold-at 0.1"
    sleep 12
    printf '%s\n' disable
    printf '%s\n' quit
  } | timeout 25 bin/marengo-pi
}

filter_timing() {
  grep -E 'loop timing|tick phase' || true
}

case "$MODE" in
  A)
    echo "=== PROFILE A: minimal ==="
    unset MARENGO_POSITION_TRACE
    LOG="/tmp/profile-A-$$.log"
    run_pipe >"$LOG" 2>&1 || true
    grep -E 'loop timing|tick phase|error|failed|enabled' "$LOG" | tail -30 || true
    echo "(full log: $LOG lines=$(wc -l <"$LOG"))"
    ;;
  B)
    echo "=== PROFILE B: position trace ==="
    export MARENGO_POSITION_TRACE="/tmp/position-trace-profile-$$.csv"
    LOG="/tmp/profile-B-$$.log"
    run_pipe >"$LOG" 2>&1 || true
    grep -E 'loop timing|tick phase' "$LOG" | tail -20 || true
    rm -f "$MARENGO_POSITION_TRACE"
    ;;
  C)
    echo "=== PROFILE C: trace + candump + tee ==="
    LOGDIR="$ROOT/var/log"
    TS=$(date -u +"%Y%m%dT%H%M%SZ")
    export MARENGO_POSITION_TRACE="$LOGDIR/position-trace-profile-$TS.csv"
    CANDUMP="$LOGDIR/candump-profile-$TS.log"
    if ip link show can0 2>/dev/null | grep -q 'UP'; then
      candump -t z can0 ${CANDUMP_CAN1:-} >"$CANDUMP" 2>&1 &
      CDPID=$!
    else
      CDPID=
    fi
    LOG="$LOGDIR/profile-$TS.log"
    { run_pipe; } >"$LOG" 2>&1 || true
    grep -E 'loop timing|tick phase' "$LOG" | tail -20 || true
    if [[ -n "${CDPID:-}" ]]; then
      kill "$CDPID" 2>/dev/null || true
      wait "$CDPID" 2>/dev/null || true
    fi
    ;;
  *)
    echo "usage: $0 [A|B|C]" >&2
    exit 1
    ;;
esac

echo "=== kernel CAN rate (10s sample; hold not required) ==="
if [[ -x "$ROOT/scripts/measure-can-mit-rate.sh" ]]; then
  "$ROOT/scripts/measure-can-mit-rate.sh" --kernel 5 can0 || true
fi
