#!/usr/bin/env bash
# Fail if library crate sources use println!/eprintln! (bins are exempt).
# Scope is lib code only (crates/*/src): tests/, examples/ and benches/ print
# ordinary test output, not operator-visible runtime logs, and are exempt.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

HITS=""
if command -v rg >/dev/null 2>&1; then
  HITS="$(rg '(println|eprintln)!' crates/ --glob '*.rs' --glob '!**/build.rs' --glob '!**/tests/**' --glob '!**/examples/**' --glob '!**/benches/**' -l 2>/dev/null || true)"
else
  # No ripgrep (minimal host, and the dev container): grep fallback over the
  # same file set. Without this the guard silently passes (empty hit list).
  HITS="$(grep -rEl --include='*.rs' '(println|eprintln)!' crates/ 2>/dev/null | grep -v '/build.rs' | grep -Ev '/(tests|examples|benches)/' || true)"
fi
if [[ -n "${HITS}" ]]; then
  echo "error: println!/eprintln! in library crate sources (use tracing instead):"
  echo "${HITS}"
  exit 1
fi
echo "println guard: ok (no println in crates/*/src/)"
