#!/usr/bin/env bash
# Export URDF from SolidWorks (Brawner add-in), decimate meshes, refresh assets/.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
URDF="${ROOT}/assets/urdf/marengo.urdf"
MESH_VIS="${ROOT}/assets/meshes/visual"
MESH_COL="${ROOT}/assets/meshes/collision"

echo "==> Marengo URDF export"
echo "Target: ${URDF}"
echo ""
echo "Manual workflow (until SW automation is wired):"
echo "  1. Open hardware/cad/assemblies/marengo.sldasm in SolidWorks."
echo "  2. Export URDF via your exporter (Brawner / sw2urdf) to ${URDF}."
echo "  3. Copy visual STLs to ${MESH_VIS}/ and decimated collision STLs to ${MESH_COL}/."
echo "  4. Run: ${ROOT}/scripts/urdf-to-mjcf.sh"
echo "  5. Run: just check"
echo ""

if [[ -f "${URDF}" ]]; then
  echo "Current URDF present ($(wc -c < "${URDF}") bytes)."
  echo "After export: run MCP marengo_urdf_export_postcheck (solidworks-mcp) before urdf-to-mjcf."
else
  echo "No URDF at ${URDF} — export required before production sim tests."
  exit 1
fi

mkdir -p "${MESH_VIS}" "${MESH_COL}"
echo "export-urdf: ok (URDF exists; mesh dirs ready)"
