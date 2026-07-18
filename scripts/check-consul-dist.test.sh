#!/usr/bin/env bash
# TDD tests for scripts/check-consul-dist.sh — run: ./scripts/check-consul-dist.test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GATE="${ROOT}/scripts/check-consul-dist.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

pass=0
fail=0

assert_exit() {
  local label="$1"
  local expected="$2"
  shift 2
  set +e
  "$@" >/dev/null 2>&1
  local actual=$?
  set -e
  if [[ "$actual" -eq "$expected" ]]; then
    echo "ok: ${label} (exit ${actual})"
    pass=$((pass + 1))
  else
    echo "FAIL: ${label} — expected exit ${expected}, got ${actual}" >&2
    fail=$((fail + 1))
  fi
}

assert_stderr_contains() {
  local label="$1"
  local needle="$2"
  shift 2
  local err
  set +e
  err="$("$@" 2>&1 >/dev/null)"
  local actual=$?
  set -e
  if [[ "$actual" -ne 0 ]] && [[ "$err" == *"$needle"* ]]; then
    echo "ok: ${label} (stderr mentions '${needle}')"
    pass=$((pass + 1))
  else
    echo "FAIL: ${label} — expected non-zero exit and stderr containing '${needle}'" >&2
    echo "  exit=${actual} stderr=${err}" >&2
    fail=$((fail + 1))
  fi
}

# --- fixtures ---
mkdir -p "${TMP}/clean/assets"
echo 'console.log("robot-hosted");' >"${TMP}/clean/assets/index.js"

mkdir -p "${TMP}/poison-localhost/assets"
echo 'const u="http://127.0.0.1:8080";' >"${TMP}/poison-localhost/assets/index.js"

mkdir -p "${TMP}/poison-env-name/assets"
echo 'const k="VITE_CHAPPE_HTTP_URL";' >"${TMP}/poison-env-name/assets/index.js"

mkdir -p "${TMP}/missing"

# --- cases ---
assert_exit "clean dist passes" 0 bash "${GATE}" "${TMP}/clean"
assert_exit "missing dist fails" 1 bash "${GATE}" "${TMP}/missing"
assert_stderr_contains "localhost poison rejected" "127.0.0.1:8080" bash "${GATE}" "${TMP}/poison-localhost"
assert_stderr_contains "VITE_CHAPPE_ poison rejected" "VITE_CHAPPE_" bash "${GATE}" "${TMP}/poison-env-name"

echo ""
echo "check-consul-dist.test: ${pass} passed, ${fail} failed"
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
