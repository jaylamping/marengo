#!/usr/bin/env bash
# Deploy path always rebuilds Consul (no mtime short-circuit).
# Run: ./scripts/deploy-consul-rebuild.test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPLOY_PI="${ROOT}/scripts/deploy-pi.sh"

pass=0
fail=0

assert_grep_absent() {
  local label="$1"
  local pattern="$2"
  local file="$3"
  if grep -qE "${pattern}" "${file}"; then
    echo "FAIL: ${label} — '${pattern}' still present in ${file}" >&2
    fail=$((fail + 1))
  else
    echo "ok: ${label}"
    pass=$((pass + 1))
  fi
}

assert_grep_absent "no consul_assets_fresh helper" 'consul_assets_fresh' "${DEPLOY_PI}"
assert_grep_absent "no skip-consul-build log" 'skipping npm build' "${DEPLOY_PI}"

echo ""
echo "deploy-consul-rebuild.test: ${pass} passed, ${fail} failed"
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
