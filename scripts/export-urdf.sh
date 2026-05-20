#!/usr/bin/env bash
# Export URDF from SolidWorks (Brawner add-in), decimate meshes, refresh assets/.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
echo "TODO: run SW exporter → ${ROOT}/assets/urdf/marengo.urdf"
echo "TODO: decimate STLs → ${ROOT}/assets/meshes/collision/"
