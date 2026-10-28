#!/usr/bin/env bash
# Canonical Pi-native self-update: pin exact SHA → build → staging install-pi.
# Runs as the deploy user (joey), typically via systemd-run from pi-enqueue-self-update.sh.
# MCP pi_native and marengo-gateway both invoke this script; RUN_LOCK below
# keeps them from building in the same checkout at once.
set -euo pipefail

TARGET_SHA="${TARGET_SHA:-${1:-}}"
JOB_FILE="${JOB_FILE:-${MARENGO_DEPLOY_JOB_FILE:-/opt/marengo/var/deploy-job.json}}"
STAGING="${MARENGO_STAGING_ROOT:-${HOME}/marengo}"
OPT_ROOT="${MARENGO_ROOT:-/opt/marengo}"
LOG_FILE="${MARENGO_SELF_UPDATE_LOG:-${OPT_ROOT}/var/self-update.log}"
# Advisory lock shared with marengo-deploy (job_file_lock_path) and the enqueue helper.
JOB_LOCK="${JOB_FILE}.lock"
RUN_LOCK="${STAGING}.self-update.lock"
UNIT_NAME="${MARENGO_SELF_UPDATE_UNIT:-}"
# Public HTTPS by default; override for private mirrors / deploy keys.
REPO_URL="${MARENGO_GIT_URL:-https://github.com/jaylamping/marengo.git}"

if [[ -z "${TARGET_SHA}" ]]; then
  echo "usage: TARGET_SHA=<40-char> $0" >&2
  exit 2
fi
if [[ ! "${TARGET_SHA}" =~ ^[0-9a-fA-F]{7,40}$ ]]; then
  echo "error: TARGET_SHA must be a git SHA" >&2
  exit 2
fi

mkdir -p "$(dirname "${JOB_FILE}")" "$(dirname "${LOG_FILE}")" 2>/dev/null || true

# One run per staging checkout. A second run (Consul Update during an MCP
# pi_native deploy, or the reverse) would build in the same tree and overwrite
# the running job's ledger entries, so it exits before touching either.
exec 7>>"${RUN_LOCK}"
if ! flock -n 7; then
  echo "error: another self-update holds ${RUN_LOCK}; not starting a second build" >&2
  exit 1
fi

# The log holds this run only (the previous one is kept as .1): the gateway
# shows its tail when a job fails, and an appended log grew without bound.
if [[ -s "${LOG_FILE}" ]]; then
  mv -f "${LOG_FILE}" "${LOG_FILE}.1" 2>/dev/null || : >"${LOG_FILE}" 2>/dev/null || true
fi
exec > >(tee -a "${LOG_FILE}") 2>&1

now_iso() { date -u +%Y-%m-%dT%H:%M:%SZ; }

json_escape() {
  # Escape for JSON string content (no surrounding quotes).
  local s=$1
  s=${s//\\/\\\\}
  s=${s//\"/\\\"}
  s=${s//$'\n'/\\n}
  s=${s//$'\r'/\\r}
  s=${s//$'\t'/\\t}
  printf '%s' "${s}"
}

write_job() {
  local state="$1"
  local message="${2:-}"
  local phase="${3:-}"
  local result_sha="${4:-}"
  local tmp msg_esc
  # Same-directory tmp + rename so readers never see a torn JSON object. The
  # rename runs under JOB_LOCK so a gateway reconcile (read, then write) cannot
  # interleave with it. flock opens the lock read-only, so it works whichever
  # user created it.
  tmp="${JOB_FILE}.tmp.$$"
  msg_esc="$(json_escape "${message}")"
  cat >"${tmp}" <<EOF
{
  "state": "${state}",
  "job_id": "${JOB_ID:-}",
  "target_sha": "${TARGET_SHA}",
  "result_sha": "${result_sha}",
  "unit_name": "${UNIT_NAME}",
  "started_at": "${STARTED_AT}",
  "updated_at": "$(now_iso)",
  "message": "${msg_esc}",
  "phase": "${phase}"
}
EOF
  if ! flock -w 30 "${JOB_LOCK}" mv -f "${tmp}" "${JOB_FILE}" 2>/dev/null; then
    if ! sudo -n mv -f "${tmp}" "${JOB_FILE}" 2>/dev/null; then
      rm -f "${tmp}"
      echo "error: cannot write job file ${JOB_FILE}" >&2
      return 1
    fi
  fi
  chmod 664 "${JOB_FILE}" 2>/dev/null || sudo -n chmod 664 "${JOB_FILE}" 2>/dev/null || true
}

fail() {
  local message="$1"
  local phase="${2:-error}"
  echo "error: ${message}" >&2
  write_job "failed" "${message}" "${phase}" || true
  exit 1
}

ensure_staging_git() {
  # Self-update builds from a real clone. Deploy rsync trees under ~/marengo
  # (www/, .deploy-rev, no .git) used to fail opaquely at "git fetch failed".
  if [[ -d "${STAGING}/.git" ]]; then
    return 0
  fi
  write_job "running" "bootstrapping git clone at ${STAGING}" "init"
  if [[ -e "${STAGING}" ]]; then
    local bak="${STAGING}.not-a-git.bak.$(date -u +%Y%m%dT%H%M%SZ)"
    mv "${STAGING}" "${bak}" || fail "cannot move non-git staging aside: ${STAGING}" "init"
    echo "moved non-git staging to ${bak}"
  fi
  GIT_TERMINAL_PROMPT=0 git clone "${REPO_URL}" "${STAGING}" \
    || fail "git clone failed (${REPO_URL} -> ${STAGING})" "init"
}

STARTED_AT="$(now_iso)"
JOB_ID="${JOB_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$}"
write_job "running" "starting self-update" "init"

ensure_staging_git
cd "${STAGING}" || fail "staging root missing: ${STAGING}" "init"

if [[ -n "$(git status --porcelain)" ]]; then
  fail "dirty working tree:$(printf '\n%s' "$(git status --short)")" "dirty"
fi

write_job "running" "fetching ${TARGET_SHA}" "fetch"
git fetch origin || fail "git fetch failed" "fetch"

# Pin exactly one commit (short or full SHA). rev-parse refuses a short SHA
# that matches several objects; never pick one of them.
RESOLVED="$(git rev-parse --verify --quiet "${TARGET_SHA}^{commit}" 2>/dev/null || true)"
[[ -n "${RESOLVED}" ]] || fail "cannot resolve TARGET_SHA ${TARGET_SHA} to exactly one commit" "fetch"

git checkout --detach "${RESOLVED}" || fail "git checkout ${RESOLVED} failed" "fetch"
HEAD_NOW="$(git rev-parse HEAD)"
[[ "${HEAD_NOW}" == "${RESOLVED}" ]] || fail "HEAD ${HEAD_NOW} != target ${RESOLVED}" "fetch"
TARGET_SHA="${RESOLVED}"

if command -v git-lfs >/dev/null 2>&1 || git lfs version >/dev/null 2>&1; then
  write_job "running" "git lfs pull" "lfs"
  git lfs pull || fail "git lfs pull failed" "lfs"
fi

write_job "running" "native build" "build"
# Prefer exec bit; fall back to bash so a 100644 tree never fails as "missing".
if [[ -f ./scripts/pi-native-build.sh ]]; then
  if [[ -x ./scripts/pi-native-build.sh ]]; then
    ./scripts/pi-native-build.sh || fail "pi-native-build.sh failed" "build"
  else
    bash ./scripts/pi-native-build.sh || fail "pi-native-build.sh failed" "build"
  fi
else
  fail "pi-native-build.sh missing" "build"
fi

write_job "running" "install window" "install"
INSTALL_SCRIPT="${STAGING}/scripts/install-pi.sh"
[[ -x "${INSTALL_SCRIPT}" ]] || fail "install script missing: ${INSTALL_SCRIPT}" "install"
# install-pi owns every root step: stopping marengo-pi and restoring its prior
# enabled/active state, .deploy-rev, www/, and the gateway restart.
sudo -n "${INSTALL_SCRIPT}" || fail "install-pi.sh failed" "install"

# Job file after install returns — gateway already restarted inside install-pi;
# this unit runs outside the gateway cgroup so the write still lands.
write_job "succeeded" "installed ${TARGET_SHA}" "done" "${TARGET_SHA}"
echo "pi-self-update: done ${TARGET_SHA}"
