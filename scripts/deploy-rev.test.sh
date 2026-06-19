#!/usr/bin/env bash
# TDD tests for deploy-rev staging + install — run: ./scripts/deploy-rev.test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

pass=0
fail=0

assert_file_first_field() {
  local label="$1"
  local path="$2"
  local expected="$3"
  local actual
  actual="$(awk '{print $1}' "${path}")"
  if [[ "$actual" == "$expected" ]]; then
    echo "ok: ${label} (first field ${actual})"
    pass=$((pass + 1))
  else
    echo "FAIL: ${label} — expected first field '${expected}', got '${actual}'" >&2
    fail=$((fail + 1))
  fi
}

# --- staged write uses local repo HEAD ---
STAGING="${TMP}/staging"
mkdir -p "${STAGING}"
EXPECTED_SHA="$(git -C "${ROOT}" rev-parse HEAD)"
stage_deploy_rev "${ROOT}" "${STAGING}"
assert_file_first_field "stage_deploy_rev writes local HEAD" "${STAGING}/.deploy-rev" "${EXPECTED_SHA}"

# --- install copies staged file to install root ---
BUNDLE="${TMP}/bundle"
INSTALL="${TMP}/install"
mkdir -p "${BUNDLE}" "${INSTALL}"
cp "${STAGING}/.deploy-rev" "${BUNDLE}/.deploy-rev"
install_deploy_rev "${BUNDLE}" "${INSTALL}"
assert_file_first_field "install_deploy_rev copies staged SHA" "${INSTALL}/.deploy-rev" "${EXPECTED_SHA}"

# --- pi_native fallback: no staged file, git checkout present ---
GIT_BUNDLE="${TMP}/git-bundle"
GIT_INSTALL="${TMP}/git-install"
mkdir -p "${GIT_INSTALL}"
git init -q "${GIT_BUNDLE}"
git -C "${GIT_BUNDLE}" config user.email "test@marengo.local"
git -C "${GIT_BUNDLE}" config user.name "deploy-rev test"
echo "fixture" >"${GIT_BUNDLE}/README"
git -C "${GIT_BUNDLE}" add README
git -C "${GIT_BUNDLE}" commit -q -m "fixture"
GIT_SHA="$(git -C "${GIT_BUNDLE}" rev-parse HEAD)"
install_deploy_rev "${GIT_BUNDLE}" "${GIT_INSTALL}"
assert_file_first_field "install_deploy_rev falls back to bundle git HEAD" "${GIT_INSTALL}/.deploy-rev" "${GIT_SHA}"

# --- in-place install: stale canonical .deploy-rev must not win over git HEAD ---
INPLACE="${TMP}/inplace"
mkdir -p "${INPLACE}"
git init -q "${INPLACE}"
git -C "${INPLACE}" config user.email "test@marengo.local"
git -C "${INPLACE}" config user.name "deploy-rev test"
echo "v1" >"${INPLACE}/README"
git -C "${INPLACE}" add README
git -C "${INPLACE}" commit -q -m "v1"
printf '%s 2020-01-01T00:00:00Z\n' "0000000000000000000000000000000000000000" >"${INPLACE}/.deploy-rev"
echo "v2" >>"${INPLACE}/README"
git -C "${INPLACE}" add README
git -C "${INPLACE}" commit -q -m "v2"
SECOND_SHA="$(git -C "${INPLACE}" rev-parse HEAD)"
install_deploy_rev "${INPLACE}" "${INPLACE}"
assert_file_first_field "in-place install refreshes from git HEAD (not stale .deploy-rev)" "${INPLACE}/.deploy-rev" "${SECOND_SHA}"

# --- format: SHA + UTC ISO8601 timestamp ---
CONTENT="$(cat "${STAGING}/.deploy-rev")"
if [[ "$CONTENT" =~ ^[0-9a-f]{40}\ [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]]; then
  echo "ok: deploy-rev format is SHA + UTC ISO8601"
  pass=$((pass + 1))
else
  echo "FAIL: deploy-rev format — got '${CONTENT}'" >&2
  fail=$((fail + 1))
fi

echo ""
echo "deploy-rev.test: ${pass} passed, ${fail} failed"
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
