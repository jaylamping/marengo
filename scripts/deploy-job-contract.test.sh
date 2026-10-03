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
# write_job_atomic serializes via an embedded Python heredoc (dict kwargs, no
# literal JSON in the shell source): run that writer and inspect its output.
JOB_TMP="$(mktemp -d)"
trap 'rm -rf "${JOB_TMP}"' EXIT
awk '/<<'"'"'PY'"'"'$/ {body = 1; next} /^PY$/ {body = 0} body' "${ENQUEUE}" >"${JOB_TMP}/write_job.py"
run_enqueue_job_writer() {
  python3 - "${JOB_TMP}/deploy-job.json" running "job-1" \
    "0123456789abcdef0123456789abcdef01234567" marengo-self-update enqueued enqueue \
    <"${JOB_TMP}/write_job.py"
}
assert_ok "enqueue job writer emits deploy-job.json" run_enqueue_job_writer
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
for key in state job_id target_sha result_sha unit_name started_at updated_at message phase; do
  assert_ok "enqueue job JSON includes ${key}" \
    python3 -c 'import json, sys; sys.exit(sys.argv[2] not in json.load(open(sys.argv[1])))' \
    "${JOB_TMP}/deploy-job.json" "${key}"
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

# Index mode, not worktree -x. Trust the checkout like check.sh's git_root (CI mounts it under another uid).
git_index_mode() {
  git -c "safe.directory=${ROOT}" -C "${ROOT}" ls-files -s -- "$1" | awk '{print $1}'
}
assert_ok "pi-native-build.sh is executable in git (100755)" \
  test "$(git_index_mode scripts/pi-native-build.sh)" = "100755"
assert_ok "build-consul-native.sh is executable in git (100755)" \
  test "$(git_index_mode scripts/build-consul-native.sh)" = "100755"
assert_ok "self-update runs pi-native-build via file check (not only -x)" \
  grep -q '\[\[ -f ./scripts/pi-native-build.sh \]\]' "${WORKER}"
assert_ok "marengo-deploy job_script_contract tests" \
  cargo test --manifest-path "${ROOT}/Cargo.toml" -p marengo-deploy --test job_script_contract -- --quiet

echo
echo "${pass} passed, ${fail} failed"
[[ "${fail}" -eq 0 ]]
