#!/usr/bin/env bash
# Contract: enqueue/self-update job JSON stays aligned with marengo-deploy DeployJob.
# Run: ./scripts/deploy-job-contract.test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
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

ENQUEUE="${ROOT}/scripts/pi-enqueue-self-update.sh"
WORKER="${ROOT}/scripts/pi-self-update.sh"

assert_ok "enqueue script writes phase enqueue" \
  grep -q '"phase": "enqueue"' "${ENQUEUE}"
assert_ok "enqueue script writes state running" \
  grep -q '"state": "running"' "${ENQUEUE}"
for key in job_id target_sha result_sha unit_name started_at updated_at message; do
  assert_ok "enqueue job JSON includes ${key}" \
    grep -q "\"${key}\"" "${ENQUEUE}"
done

assert_ok "self-update write_job includes phase field" \
  grep -q '"phase":' "${WORKER}"
for phase in init dirty fetch lfs build install done; do
  assert_ok "self-update references phase ${phase}" \
    grep -Eq "(write_job|fail).*[\"']${phase}[\"']|[\"']${phase}[\"']" "${WORKER}"
done

assert_ok "marengo-deploy job_script_contract tests" \
  cargo test -p marengo-deploy --test job_script_contract -- --quiet

echo
echo "${pass} passed, ${fail} failed"
[[ "${fail}" -eq 0 ]]
