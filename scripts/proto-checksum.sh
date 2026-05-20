#!/usr/bin/env bash
# Verify generated TypeScript matches committed checksum (src/gen/ is gitignored).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GEN="${ROOT}/consul/src/gen/marengo_pb.ts"
CHECKSUM_FILE="${ROOT}/consul/src/gen/.checksum"

if [[ ! -f "${GEN}" ]]; then
  echo "proto-checksum: missing ${GEN} — run npm run gen:proto" >&2
  exit 1
fi

ACTUAL="$(shasum -a 256 "${GEN}" | awk '{print $1}')"
if [[ -f "${CHECKSUM_FILE}" ]]; then
  EXPECTED="$(tr -d '[:space:]' < "${CHECKSUM_FILE}")"
  if [[ "${ACTUAL}" != "${EXPECTED}" ]]; then
    echo "proto-checksum: mismatch" >&2
    echo "  expected: ${EXPECTED}" >&2
    echo "  actual:   ${ACTUAL}" >&2
    echo "  run: cd consul && npm run gen:proto && shasum -a 256 src/gen/marengo_pb.ts > src/gen/.checksum" >&2
    exit 1
  fi
  echo "proto-checksum: ok"
else
  echo "proto-checksum: no ${CHECKSUM_FILE} — writing checksum for first time"
  mkdir -p "$(dirname "${CHECKSUM_FILE}")"
  echo "${ACTUAL}" > "${CHECKSUM_FILE}"
fi
