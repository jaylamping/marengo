#!/usr/bin/env bash
# Convert production URDF to MuJoCo MJCF when assets/urdf/marengo.urdf exists.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
URDF="${ROOT}/assets/urdf/marengo.urdf"
OUT="${ROOT}/sim/marengo.xml"

if [[ ! -f "${URDF}" ]]; then
  echo "urdf-to-mjcf: missing ${URDF} — export from SolidWorks first" >&2
  exit 1
fi

echo "TODO: convert ${URDF} -> ${OUT} (MuJoCo compile or third-party tool)"
echo "Until then, CI uses sim/fixtures/minimal.xml"
exit 1
