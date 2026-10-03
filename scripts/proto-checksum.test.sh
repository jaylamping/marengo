#!/usr/bin/env bash
# Contract: proto-checksum.sh never self-blesses a missing checksum (L-armee-proto-03).
# A present codegen file with no checksum must fail, locally and in CI.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

GEN="consul/src/gen/marengo/v1/marengo_pb.ts"
CHECKSUM="consul/src/gen/.checksum"

HAD_GEN=true
[[ -f "${GEN}" ]] || HAD_GEN=false
HAD_SUM=true
[[ -f "${CHECKSUM}" ]] || HAD_SUM=false

BACKUP_DIR="$(mktemp -d)"
trap 'rm -rf -- "${BACKUP_DIR}"' EXIT
[[ "${HAD_GEN}" == true ]] && cp "${GEN}" "${BACKUP_DIR}/marengo_pb.ts"
[[ "${HAD_SUM}" == true ]] && cp "${CHECKSUM}" "${BACKUP_DIR}/checksum"

restore() {
  if [[ "${HAD_GEN}" == true ]]; then
    cp "${BACKUP_DIR}/marengo_pb.ts" "${GEN}"
  else
    rm -f "${GEN}"
  fi
  if [[ "${HAD_SUM}" == true ]]; then
    cp "${BACKUP_DIR}/checksum" "${CHECKSUM}"
  else
    rm -f "${CHECKSUM}"
  fi
}

# Case under test: codegen output exists, checksum file is missing.
mkdir -p "consul/src/gen/marengo/v1"
printf '// proto-checksum contract fixture (not real codegen)\n' > "${GEN}"
rm -f "${CHECKSUM}"

if CI= ./scripts/proto-checksum.sh >/dev/null 2>&1; then
  restore
  echo "error: proto-checksum.sh exited 0 with a missing checksum (self-blessing)" >&2
  exit 1
fi
if [[ -f "${CHECKSUM}" ]]; then
  restore
  echo "error: proto-checksum.sh wrote a checksum file on failure" >&2
  exit 1
fi
restore

# Untracked-state guard: the committed checksum must still verify when codegen is real.
if [[ "${HAD_GEN}" == true ]] && [[ "${HAD_SUM}" == true ]]; then
  ./scripts/proto-checksum.sh >/dev/null
fi

echo "proto-checksum contract: ok"
