#!/usr/bin/env bash
# Canonical Pi-native self-update: pin exact SHA → build → staging install-pi.
# Runs as the deploy user (joey), typically via systemd-run from pi-enqueue-self-update.sh.
# MCP pi_native and marengo-gateway both invoke this script.
set -euo pipefail

TARGET_SHA="${TARGET_SHA:-${1:-}}"
JOB_FILE="${JOB_FILE:-${MARENGO_DEPLOY_JOB_FILE:-/opt/marengo/var/deploy-job.json}}"
STAGING="${MARENGO_STAGING_ROOT:-${HOME}/marengo}"
OPT_ROOT="${MARENGO_ROOT:-/opt/marengo}"
LOG_FILE="${MARENGO_SELF_UPDATE_LOG:-${OPT_ROOT}/var/self-update.log}"
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
  # Same-directory tmp + rename so readers never see a torn JSON object.
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
  if ! mv -f "${tmp}" "${JOB_FILE}" 2>/dev/null; then
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
sudo git config --global --add safe.directory "${STAGING}" 2>/dev/null || true

if [[ -n "$(git status --porcelain)" ]]; then
  fail "dirty working tree:$(printf '\n%s' "$(git status --short)")" "dirty"
fi

write_job "running" "fetching ${TARGET_SHA}" "fetch"
git fetch origin || fail "git fetch failed" "fetch"

# Pin exact commit (accept short or full SHA from GitHub tip).
RESOLVED="$(git rev-parse --verify "${TARGET_SHA}^{commit}" 2>/dev/null || true)"
if [[ -z "${RESOLVED}" ]]; then
  RESOLVED="$(git rev-parse --verify "origin/${TARGET_SHA}^{commit}" 2>/dev/null || true)"
fi
if [[ -z "${RESOLVED}" ]]; then
  # Try as prefix against origin/main tip objects after fetch.
  RESOLVED="$(git rev-list --all --max-count=5000 | grep -E "^${TARGET_SHA}" | head -1 || true)"
fi
[[ -n "${RESOLVED}" ]] || fail "cannot resolve TARGET_SHA ${TARGET_SHA}" "fetch"

git checkout --detach "${RESOLVED}" || fail "git checkout ${RESOLVED} failed" "fetch"
HEAD_NOW="$(git rev-parse HEAD)"
[[ "${HEAD_NOW}" == "${RESOLVED}" ]] || fail "HEAD ${HEAD_NOW} != target ${RESOLVED}" "fetch"
TARGET_SHA="${RESOLVED}"

if command -v git-lfs >/dev/null 2>&1 || git lfs version >/dev/null 2>&1; then
  write_job "running" "git lfs pull" "lfs"
  git lfs pull || fail "git lfs pull failed" "lfs"
fi

write_job "running" "native build" "build"
if [[ -x ./scripts/pi-native-build.sh ]]; then
  ./scripts/pi-native-build.sh || fail "pi-native-build.sh failed" "build"
else
  fail "pi-native-build.sh missing" "build"
fi

write_job "running" "install window" "install"
# Bench: stop/disable control before install replaces binaries.
sudo systemctl stop marengo-pi.service 2>/dev/null || true
sudo systemctl disable marengo-pi.service 2>/dev/null || true
sudo pkill -f /opt/marengo/bin/marengo-pi 2>/dev/null || true

INSTALL_SCRIPT="${STAGING}/scripts/install-pi.sh"
[[ -x "${INSTALL_SCRIPT}" ]] || fail "install script missing: ${INSTALL_SCRIPT}" "install"
# install-pi owns .deploy-rev, www/, and gateway/pi unit restarts.
sudo -n "${INSTALL_SCRIPT}" || fail "install-pi.sh failed" "install"

# Job file after install returns — gateway already restarted inside install-pi;
# this unit runs outside the gateway cgroup so the write still lands.
write_job "succeeded" "installed ${TARGET_SHA}" "done" "${TARGET_SHA}"
echo "pi-self-update: done ${TARGET_SHA}"
