#!/usr/bin/env bash
# Regenerate MJCF from production URDF (manual / tool-assisted until automated converter lands).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
URDF="${ROOT}/assets/urdf/marengo.urdf"
MJCF="${ROOT}/assets/mjcf/marengo.xml"

if [[ ! -f "${URDF}" ]]; then
  echo "error: missing ${URDF} — run scripts/export-urdf.sh first" >&2
  exit 1
fi

echo "==> URDF → MJCF"
echo "Source: ${URDF}"
echo "Target: ${MJCF}"
echo ""
echo "For the v0 prototype, MJCF is maintained by hand next to the URDF (2-DOF bench model)."
echo "When the full humanoid URDF lands, use MuJoCo compile or an exporter, then run:"
echo "  cargo test -p sim-harness -- --nocapture"
echo ""

if [[ ! -f "${MJCF}" ]]; then
  echo "error: missing ${MJCF}" >&2
  exit 1
fi

cargo test -p sim-harness production_urdf_and_mjcf_dof_match --quiet
echo "urdf-to-mjcf: ok"
