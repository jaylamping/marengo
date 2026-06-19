#!/usr/bin/env bash
# TDD tests: deploy path always rebuilds Consul (no mtime short-circuit).
# Run: ./scripts/deploy-consul-rebuild.test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPLOY_PI="${ROOT}/scripts/deploy-pi.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

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

# Replica of removed consul_assets_fresh — documents when old deploy would skip.
legacy_consul_assets_fresh() {
  local repo_root="$1"
  local dist="${repo_root}/consul/dist/index.html"
  [[ -f "$dist" ]] || return 1
  if find "${repo_root}/consul/src" "${repo_root}/proto" -type f -newer "$dist" 2>/dev/null | grep -q .; then
    return 1
  fi
  return 0
}

assert_legacy_would_skip() {
  local label="$1"
  local repo="$2"
  if legacy_consul_assets_fresh "$repo"; then
    echo "ok: ${label}"
    pass=$((pass + 1))
  else
    echo "FAIL: ${label} — fixture should look fresh to legacy mtime check" >&2
    fail=$((fail + 1))
  fi
}

# --- fixture: dist newer than all consul/src (old deploy skipped here) ---
mkdir -p "${TMP}/consul/dist" "${TMP}/consul/src" "${TMP}/proto"
echo '// src' >"${TMP}/consul/src/app.ts"
echo '<html></html>' >"${TMP}/consul/dist/index.html"
sleep 1
touch "${TMP}/consul/dist/index.html"

assert_legacy_would_skip "fixture dist newer than src (legacy would skip)" "${TMP}"

# --- deploy-pi.sh must not short-circuit ---
assert_grep_absent "no consul_assets_fresh helper" 'consul_assets_fresh' "${DEPLOY_PI}"
assert_grep_absent "no skip-consul-build log" 'skipping npm build' "${DEPLOY_PI}"

echo ""
echo "deploy-consul-rebuild.test: ${pass} passed, ${fail} failed"
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
