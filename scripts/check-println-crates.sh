#!/usr/bin/env bash
# Fail if library crates use println!/eprintln! (bins are exempt).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

HITS=""
if command -v rg >/dev/null 2>&1; then
  HITS="$(rg '(println|eprintln)!' crates/ --glob '*.rs' --glob '!**/build.rs' -l 2>/dev/null || true)"
else
  # No ripgrep (minimal host): grep fallback over the same file set.
  HITS="$(grep -rEl --include='*.rs' '(println|eprintln)!' crates/ 2>/dev/null | grep -v '/build.rs' || true)"
fi
if [[ -n "${HITS}" ]]; then
  echo "error: println!/eprintln! in library crates (use tracing instead):"
  echo "${HITS}"
  exit 1
fi
echo "println guard: ok (no println in crates/)"
