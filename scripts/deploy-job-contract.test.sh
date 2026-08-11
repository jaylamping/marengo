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
  grep -q 'write_job_atomic "running" "enqueued" "enqueue"' "${ENQUEUE}"
assert_ok "enqueue job template includes state field" \
  grep -q '"state":' "${ENQUEUE}"
assert_ok "enqueue uses flock for single-flight" \
  grep -q 'flock -n' "${ENQUEUE}"
assert_ok "enqueue refuses active unit instead of stop" \
  grep -q 'already active' "${ENQUEUE}"
assert_ok "enqueue marks failed when systemd-run fails" \
  grep -q 'systemd-run failed' "${ENQUEUE}"
assert_ok "enqueue bootstraps missing git staging clone" \
  grep -q 'ensure_staging_git' "${ENQUEUE}"
assert_ok "enqueue clone uses MARENGO_GIT_URL default" \
  grep -q 'MARENGO_GIT_URL' "${ENQUEUE}"
assert_ok "enqueue sets WorkingDirectory via systemd-run" \
  grep -q -- '--working-directory=' "${ENQUEUE}"
assert_ok "enqueue does not pass --same-dir= (flag takes no argument)" \
  bash -c '! grep -q -- "--same-dir=" "$0"' "${ENQUEUE}"
for key in job_id target_sha result_sha unit_name started_at updated_at message; do
  assert_ok "enqueue job JSON includes ${key}" \
    grep -q "\"${key}\"" "${ENQUEUE}"
done

assert_ok "self-update write_job includes phase field" \
  grep -q '"phase":' "${WORKER}"
assert_ok "self-update uses atomic mv for job file" \
  grep -q 'mv -f' "${WORKER}"
assert_ok "self-update bootstraps missing git staging clone" \
  grep -q 'ensure_staging_git' "${WORKER}"
assert_ok "self-update clone uses MARENGO_GIT_URL default" \
  grep -q 'MARENGO_GIT_URL' "${WORKER}"
assert_ok "self-update moves non-git staging aside before clone" \
  grep -q 'not-a-git.bak' "${WORKER}"
for phase in init dirty fetch lfs build install done; do
  assert_ok "self-update references phase ${phase}" \
    grep -Eq "(write_job|fail).*[\"']${phase}[\"']|[\"']${phase}[\"']" "${WORKER}"
done

assert_ok "marengo-deploy job_script_contract tests" \
  cargo test -p marengo-deploy --test job_script_contract -- --quiet

echo
echo "${pass} passed, ${fail} failed"
[[ "${fail}" -eq 0 ]]
