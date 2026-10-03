#!/usr/bin/env bash
# Headless MuJoCo smoke tests — run via: just sim-check / compose profile sim
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

MODEL="${MARENGO_SIM_MODEL:-${ROOT}/sim/fixtures/minimal.xml}"
PRODUCTION_MJCF="${ROOT}/assets/mjcf/marengo.xml"

echo "==> sim python smoke"
if python3 -c "import mujoco" 2>/dev/null; then
  python3 "${ROOT}/sim/scripts/smoke_test.py" "${MODEL}"
  if [[ -f "${PRODUCTION_MJCF}" ]]; then
    echo "==> production MJCF smoke (assets/mjcf/marengo.xml)"
    python3 "${ROOT}/sim/scripts/smoke_test.py" "${PRODUCTION_MJCF}"
  else
    echo "warn: production MJCF missing at ${PRODUCTION_MJCF} — run scripts/export-urdf.sh" >&2
    exit 1
  fi
else
  echo "mujoco python package not installed (use Dockerfile.sim)" >&2
  exit 1
fi

echo "==> sim-harness tests"
cargo test -p sim-harness --quiet

echo "check-sim: ok"
