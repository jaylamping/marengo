#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURE="${ROOT}/sim/fixtures/minimal.urdf"
if [[ ! -f "${FIXTURE}" ]]; then
  echo "validate-urdf: missing ${FIXTURE}" >&2
  exit 1
fi
if ! grep -q '<robot' "${FIXTURE}"; then
  echo "validate-urdf: invalid fixture" >&2
  exit 1
fi
echo "validate-urdf: ok (${FIXTURE})"
