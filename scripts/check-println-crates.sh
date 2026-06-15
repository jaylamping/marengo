#!/usr/bin/env bash
# Fail if library crates use println!/eprintln! (bins are exempt).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

HITS="$(rg '(println|eprintln)!' crates/ --glob '*.rs' -l 2>/dev/null || true)"
if [[ -n "${HITS}" ]]; then
  echo "error: println!/eprintln! in library crates (use tracing instead):"
  echo "${HITS}"
  exit 1
fi
echo "println guard: ok (no println in crates/)"
