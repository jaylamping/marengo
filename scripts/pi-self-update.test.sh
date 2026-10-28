#!/usr/bin/env bash
# Run pi-self-update.sh end to end against a local origin, a fake sudo and
# no-op build/install scripts. Covers (P2B-14, TOOL-15/EDGE-18, EDGE-17):
# - a short SHA that matches two commits is refused, never resolved to one;
# - a second run while one holds the staging lock exits; it marks its own
#   enqueued job failed and never touches another job's ledger entry;
# - every job write waits for deploy-job.json.lock (shared with marengo-deploy);
# - each run starts a fresh log and keeps the previous one as .1.
# Run: ./scripts/pi-self-update.test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORKER="${ROOT}/scripts/pi-self-update.sh"

if ! command -v flock >/dev/null; then
  echo "skip: pi-self-update.test.sh needs util-linux flock (the worker runs on the Pi)"
  exit 0
fi

TMP="$(mktemp -d)"
LOCK_PID=""
cleanup() {
  if [[ -n "${LOCK_PID}" ]]; then kill "${LOCK_PID}" 2>/dev/null || true; fi
  rm -rf "${TMP}"
}
trap cleanup EXIT

pass=0
fail=0
ok() {
  echo "ok: $1"
  pass=$((pass + 1))
}
bad() {
  echo "FAIL: $1" >&2
  fail=$((fail + 1))
}

export GIT_CONFIG_GLOBAL="${TMP}/gitconfig" GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=T GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=T GIT_COMMITTER_EMAIL=t@t
export GIT_AUTHOR_DATE='1700000000 +0000' GIT_COMMITTER_DATE='1700000000 +0000'
: >"${GIT_CONFIG_GLOBAL}"

# Origin with no-op build and install scripts.
SRC="${TMP}/src"
git init -q -b main "${SRC}"
mkdir -p "${SRC}/scripts"
printf '#!/usr/bin/env bash\nexit 0\n' >"${SRC}/scripts/pi-native-build.sh"
printf '#!/usr/bin/env bash\nexit 0\n' >"${SRC}/scripts/install-pi.sh"
chmod +x "${SRC}"/scripts/*.sh
git -C "${SRC}" add scripts
git -C "${SRC}" commit -q -m fixture
MAIN_SHA="$(git -C "${SRC}" rev-parse HEAD)"
# Two parentless commits on the empty tree whose ids share the prefix 40b99b1
# (fixed author, committer and dates above make the ids deterministic).
EMPTY_TREE="$(git -C "${SRC}" hash-object -t tree /dev/null)"
AMB_A="$(git -C "${SRC}" commit-tree "${EMPTY_TREE}" -m c4965)"
AMB_B="$(git -C "${SRC}" commit-tree "${EMPTY_TREE}" -m c9754)"
git -C "${SRC}" update-ref refs/heads/amb-a "${AMB_A}"
git -C "${SRC}" update-ref refs/heads/amb-b "${AMB_B}"
if [[ "${AMB_A:0:7}" != 40b99b1 || "${AMB_B:0:7}" != 40b99b1 ]]; then
  echo "error: fixture commits do not share the prefix 40b99b1 (${AMB_A} ${AMB_B})" >&2
  exit 1
fi

STAGING="${TMP}/staging"
git clone -q "${SRC}" "${STAGING}"
VAR="${TMP}/var"
mkdir -p "${VAR}" "${TMP}/bin"
JOB_FILE="${VAR}/deploy-job.json"
LOG_FILE="${VAR}/self-update.log"
cat >"${TMP}/bin/sudo" <<'CLI'
#!/usr/bin/env bash
[[ "$1" == -n ]] && shift
exec "$@"
CLI
chmod +x "${TMP}/bin/sudo"

run_worker() {
  status=0
  env PATH="${TMP}/bin:${PATH}" HOME="${TMP}" TARGET_SHA="$1" JOB_ID=job-test \
    JOB_FILE="${JOB_FILE}" MARENGO_SELF_UPDATE_LOG="${LOG_FILE}" \
    MARENGO_STAGING_ROOT="${STAGING}" MARENGO_ROOT="${TMP}/opt" \
    bash "${WORKER}" >"${TMP}/out" 2>&1 || status=$?
}
job_field() {
  python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "${JOB_FILE}" "$1"
}

# 1. An ambiguous short SHA is refused at fetch; nothing is checked out.
run_worker 40b99b1
if [[ "${status}" -ne 0 && "$(job_field phase)" == fetch ]] && grep -q 'cannot resolve' "${TMP}/out" &&
  [[ "$(git -C "${STAGING}" rev-parse HEAD)" == "${MAIN_SHA}" ]]; then
  ok "ambiguous short SHA is refused"
else
  bad "ambiguous short SHA resolved (status ${status}, phase $(job_field phase), HEAD $(git -C "${STAGING}" rev-parse HEAD))"
fi

# 2. A full SHA installs, and the log starts fresh with the previous run kept.
echo "previous run" >"${LOG_FILE}"
run_worker "${MAIN_SHA}"
if [[ "${status}" -eq 0 && "$(job_field state)" == succeeded && "$(job_field result_sha)" == "${MAIN_SHA}" ]]; then
  ok "full SHA installs"
else
  cat "${TMP}/out" >&2
  bad "full SHA run failed (status ${status})"
fi
if grep -q 'previous run' "${LOG_FILE}.1" 2>/dev/null && ! grep -q 'previous run' "${LOG_FILE}" &&
  grep -q "pi-self-update: done ${MAIN_SHA}" "${LOG_FILE}"; then
  ok "log holds this run only; the previous run is kept as .1"
else
  bad "log was appended to instead of rotated"
fi

# 3. A second run while the staging lock is held leaves the ledger alone.
echo '{"state":"running","job_id":"other","phase":"build"}' >"${JOB_FILE}"
flock -o "${STAGING}.self-update.lock" sleep 30 &
LOCK_PID=$!
sleep 0.3
run_worker "${MAIN_SHA}"
kill "${LOCK_PID}" 2>/dev/null || true
wait "${LOCK_PID}" 2>/dev/null || true
LOCK_PID=""
if [[ "${status}" -ne 0 && "$(job_field job_id)" == other && "$(job_field state)" == running ]] &&
  grep -q 'another self-update' "${TMP}/out"; then
  ok "second run refuses without touching another job's ledger entry"
else
  bad "second run proceeded or wrote the ledger (status ${status}, job_id $(job_field job_id))"
fi

# 3b. A refused run that the enqueue helper recorded as running/enqueue marks
# its own job failed with the reason, instead of leaving it running.
echo '{"state":"running","job_id":"job-test","phase":"enqueue","started_at":"2026-10-28T00:00:00Z"}' >"${JOB_FILE}"
flock -o "${STAGING}.self-update.lock" sleep 30 &
LOCK_PID=$!
sleep 0.3
run_worker "${MAIN_SHA}"
kill "${LOCK_PID}" 2>/dev/null || true
wait "${LOCK_PID}" 2>/dev/null || true
LOCK_PID=""
if [[ "${status}" -ne 0 && "$(job_field job_id)" == job-test && "$(job_field state)" == failed &&
  "$(job_field phase)" == enqueue ]] && job_field message | grep -q 'another self-update'; then
  ok "refused enqueued job is marked failed with the reason"
else
  bad "refused enqueued job left as $(job_field state)/$(job_field phase) (status ${status})"
fi

# 4. Job writes wait for the shared ledger lock.
echo '{"state":"idle","job_id":"held","phase":"init"}' >"${JOB_FILE}"
flock -o "${JOB_FILE}.lock" sleep 2 &
LOCK_PID=$!
sleep 0.3
run_worker "${MAIN_SHA}" &
WORKER_PID=$!
sleep 1
held_id="$(job_field job_id)"
wait "${WORKER_PID}" || true
wait "${LOCK_PID}" 2>/dev/null || true
LOCK_PID=""
if [[ "${held_id}" == held && "$(job_field state)" == succeeded ]]; then
  ok "job writes wait for deploy-job.json.lock"
else
  cat "${TMP}/out" "${JOB_FILE}" >&2
  bad "job written while the ledger lock was held (saw ${held_id})"
fi

echo ""
echo "pi-self-update.test: ${pass} passed, ${fail} failed"
[[ "${fail}" -eq 0 ]]
