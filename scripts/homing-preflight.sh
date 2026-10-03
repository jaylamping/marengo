#!/usr/bin/env bash
# Read-only homing preflight: calibration record + motor-repl homing-status.
# Does not set-zero (operator must place arm at reference first).
# Skips homing-status while marengo-pi or motor-repl holds SocketCAN: motor-repl
# transmits type-24 active-reporting frames at startup, and a second writer
# beside marengo-pi can latch its persistent Transport fault.
#
# Usage:
#   MARENGO_ROOT=/opt/marengo MARENGO_CONFIG_DIR=... ./scripts/homing-preflight.sh
#   HOMING_PREFLIGHT_STRICT=true  → exit 1 when any joint is not Verified
set -euo pipefail

ROOT="${MARENGO_ROOT:-/opt/marengo}"
CONFIG_DIR="${MARENGO_CONFIG_DIR:-${ROOT}/config}"
STRICT="${HOMING_PREFLIGHT_STRICT:-false}"

if [[ "$CONFIG_DIR" != /* ]]; then
  CONFIG_DIR="${ROOT}/${CONFIG_DIR}"
fi

if [[ ! -d "$CONFIG_DIR" ]]; then
  echo "homing-preflight: config dir missing: ${CONFIG_DIR}" >&2
  if [[ "$STRICT" == true ]]; then
    exit 1
  fi
  exit 0
fi

CAL_RECORD=""
if [[ -f "${CONFIG_DIR}/homing.yaml" ]]; then
  CAL_RECORD="$(grep -E '^[[:space:]]*calibration_record_path:' "${CONFIG_DIR}/homing.yaml" | head -1 | sed 's/.*:[[:space:]]*//' | tr -d "\"'")"
fi
if [[ -z "$CAL_RECORD" ]]; then
  CAL_RECORD="${ROOT}/var/calibration/zero_registry.yaml"
fi
if [[ "$CAL_RECORD" != /* ]]; then
  CAL_RECORD="${ROOT}/${CAL_RECORD}"
fi

echo "=== homing preflight ==="
echo "config: ${CONFIG_DIR}"
if [[ -f "$CAL_RECORD" ]]; then
  echo "calibration record: ${CAL_RECORD} [present]"
else
  echo "calibration record: MISSING at ${CAL_RECORD}"
fi

export MARENGO_ROOT="$ROOT"
export MARENGO_CONFIG_DIR="$CONFIG_DIR"
cd "$ROOT"

if [[ ! -x "${ROOT}/bin/motor-repl" ]]; then
  echo "homing-preflight: motor-repl not installed — skip" >&2
  exit 0
fi

CAN_OWNER="$(pgrep -l -x 'marengo-pi|motor-repl' | head -n 1 || true)"
if [[ -n "$CAN_OWNER" ]]; then
  echo "homing-status skipped: ${CAN_OWNER#* } (pid ${CAN_OWNER%% *}) owns CAN; motor-repl would open SocketCAN beside it"
  echo "  live homing: Consul, or MCP pi_homing_status (marengo-pi RobotState via gateway)"
  if [[ "$STRICT" == true ]]; then
    exit 1
  fi
  exit 0
fi

STATUS="$(bin/motor-repl homing-status 2>&1)" || true
echo "$STATUS"

if echo "$STATUS" | grep -qE 'homing=(Unhomed|Homing|Faulted)'; then
  echo ""
  echo "warning: one or more joints not Verified — place arm at mechanical reference," >&2
  echo "  then: marengo-pi stdin 'home <joint>... sign-tested' (or pi_hold_on set_zero via MCP)" >&2
  echo "  docs: docs/homing.md" >&2
  if [[ "$STRICT" == true ]]; then
    exit 1
  fi
elif ! echo "$STATUS" | grep -q 'homing=Verified'; then
  echo ""
  echo "warning: no Verified joints reported (check motors.yaml / CAN)" >&2
  if [[ "$STRICT" == true ]]; then
    exit 1
  fi
else
  echo ""
  echo "homing preflight: all commissioned joints Verified"
fi
