#!/usr/bin/env bash
# Contract: the Pi deploy never stages the profile write lock
# (config/.marengo-profile.lock) into the bundle. A revision read takes the
# lock, and the lock file must not ship to Pi staging (install-pi.sh already
# excludes it at install time).
# Run: ./scripts/deploy-config-lock.test.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPLOY_PI="${ROOT}/scripts/deploy-pi.sh"
cd "${ROOT}"

pass=0
fail=0

assert_ok() {
  local label="$1"
  shift
  if "$@"; then
    echo "ok: ${label}"
    pass=$((pass + 1))
  else
    echo "FAIL: ${label}" >&2
    fail=$((fail + 1))
  fi
}

assert_ok "config staging excludes .marengo-profile.lock" \
  grep -q 'stage_copy_tree "${ROOT}/config" "\$STAGING/config" false .marengo-profile.lock' \
  "${DEPLOY_PI}"

STAGING_FUNCS="$(mktemp -d)"
trap 'rm -rf "${STAGING_FUNCS}"' EXIT
# Source only the function definition (avoids executing the deploy body).
eval "$(sed -n '/^stage_copy_tree() {/,/^}/p' "${DEPLOY_PI}")"
FIX_SRC="${STAGING_FUNCS}/src"
FIX_DEST="${STAGING_FUNCS}/dest"
mkdir -p "${FIX_SRC}"
echo "motors: []" >"${FIX_SRC}/motors.yaml"
: >"${FIX_SRC}/.marengo-profile.lock"
stage_copy_tree "${FIX_SRC}" "${FIX_DEST}" false .marengo-profile.lock
assert_ok "staged config file is copied" \
  test -f "${FIX_DEST}/motors.yaml"
assert_ok "profile lock is excluded from staging" \
  test '!' -e "${FIX_DEST}/.marengo-profile.lock"

echo ""
echo "deploy-config-lock.test: ${pass} passed, ${fail} failed"
[[ "${fail}" -eq 0 ]]
