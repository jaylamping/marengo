#!/usr/bin/env bash
# Fail if Consul production dist embeds dev Chappe URLs or VITE_CHAPPE_* literals.
# See docs/decisions/0008-chappe-webtransport-transport.md
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="${1:-${ROOT}/consul/dist}"

if [[ ! -d "${DIST}" ]]; then
  echo "error: consul dist missing at ${DIST}" >&2
  exit 1
fi

mapfile -t JS_FILES < <(find "${DIST}" -type f -name '*.js' 2>/dev/null || true)
if [[ "${#JS_FILES[@]}" -eq 0 ]]; then
  echo "error: no .js files under ${DIST}" >&2
  exit 1
fi

FORBIDDEN=(
  '127.0.0.1:8080'
  'VITE_CHAPPE_'
)

for pattern in "${FORBIDDEN[@]}"; do
  if matches="$(grep -lF "${pattern}" "${JS_FILES[@]}" 2>/dev/null || true)"; then
    if [[ -n "${matches}" ]]; then
      echo "error: consul dist contains forbidden pattern '${pattern}':" >&2
      echo "${matches}" >&2
      exit 1
    fi
  fi
done

echo "check-consul-dist: ok"
