#!/usr/bin/env bash
# Root-owned helper: enqueue Pi self-update as the deploy user outside the gateway cgroup.
# Invoked via sudo -n /usr/local/libexec/marengo/pi-enqueue-self-update.sh <target_sha> <job_id>
set -euo pipefail

TARGET_SHA="${1:-}"
JOB_ID="${2:-}"
DEPLOY_USER="${MARENGO_DEPLOY_USER:-joey}"
STAGING="${MARENGO_STAGING_ROOT:-/home/${DEPLOY_USER}/marengo}"
OPT_ROOT="${MARENGO_ROOT:-/opt/marengo}"
JOB_DIR="${MARENGO_DEPLOY_JOB_DIR:-${OPT_ROOT}/var}"
JOB_FILE="${MARENGO_DEPLOY_JOB_FILE:-${JOB_DIR}/deploy-job.json}"
# Enqueue's root-owned lock must never live in runtime-writable state.
LOCK_DIR="/run/marengo-self-update-enqueue"
LOCK_FILE="${LOCK_DIR}/lock"
SCRIPT="${STAGING}/scripts/pi-self-update.sh"
UNIT="marengo-self-update"
REPO_URL="${MARENGO_GIT_URL:-https://github.com/jaylamping/marengo.git}"

if [[ "$(id -u)" -ne 0 ]]; then
  echo "error: must run as root (via sudo -n)" >&2
  exit 1
fi
if [[ "$#" -ne 2 || -z "${TARGET_SHA}" || -z "${JOB_ID}" ]]; then
  echo "usage: $0 <target_sha> <job_id>" >&2
  exit 2
fi
if [[ ! "${TARGET_SHA}" =~ ^[0-9a-fA-F]{7,40}$ ]]; then
  echo "error: target_sha must be a git SHA" >&2
  exit 2
fi
if [[ ! "${JOB_ID}" =~ ^[A-Za-z0-9._:-]+$ ]]; then
  echo "error: job_id contains unsafe characters" >&2
  exit 2
fi

ensure_staging_git() {
  # Deploy rsync trees under ~/marengo lack .git; heal before looking for the worker.
  if [[ -d "${STAGING}/.git" ]]; then
    return 0
  fi
  echo "bootstrapping git clone at ${STAGING}"
  if [[ -e "${STAGING}" ]]; then
    local bak="${STAGING}.not-a-git.bak.$(date -u +%Y%m%dT%H%M%SZ)"
    mv "${STAGING}" "${bak}"
    echo "moved non-git staging to ${bak}"
  fi
  if ! sudo -u "${DEPLOY_USER}" -H env GIT_TERMINAL_PROMPT=0     git clone "${REPO_URL}" "${STAGING}"; then
    echo "error: git clone failed (${REPO_URL} -> ${STAGING})" >&2
    exit 1
  fi
}

ensure_staging_git

if [[ ! -x "${SCRIPT}" ]]; then
  echo "error: self-update script missing or not executable: ${SCRIPT}" >&2
  exit 1
fi

runuser -u "${DEPLOY_USER}" -- mkdir -p "${JOB_DIR}"

# Refuse overlapping workers — do not stop an in-flight unit.
if systemctl is-active --quiet "${UNIT}.service" 2>/dev/null; then
  echo "error: unit ${UNIT}.service already active" >&2
  exit 1
fi

if [[ -e "${LOCK_DIR}" ]] || [[ -L "${LOCK_DIR}" ]]; then
  owner="$(stat -c '%u' -- "${LOCK_DIR}")"
  permissions="$(stat -c '%a' -- "${LOCK_DIR}")"
  if [[ -L "${LOCK_DIR}" || ! -d "${LOCK_DIR}" ]] || (( owner != 0 || (8#${permissions} & 8#022) != 0 )); then
    echo "error: unsafe enqueue lock directory" >&2
    exit 1
  fi
fi
install -d -o root -g root -m 0700 "${LOCK_DIR}"
if [[ -L "${LOCK_FILE}" ]]; then
  echo "error: unsafe enqueue lock file" >&2
  exit 1
fi
exec 9>>"${LOCK_FILE}"
if ! flock -n 9; then
  echo "error: another enqueue holds ${LOCK_FILE}" >&2
  exit 1
fi

write_job_atomic() {
  local state="$1"
  local message="$2"
  local phase="$3"
  # All writes under runtime-writable paths run without root authority. The
  # descriptor stays open through serialization; chmod cannot follow a replaced
  # name or symlink. The deploy user is the trusted installation principal.
  runuser -u "${DEPLOY_USER}" -- python3 - \
    "${JOB_FILE}" "${state}" "${JOB_ID}" "${TARGET_SHA}" "${UNIT}" "${message}" "${phase}" <<'PY'
import datetime
import json
import os
from pathlib import Path
import sys
import tempfile

path = Path(sys.argv[1])
state, job_id, target_sha, unit, message, phase = sys.argv[2:]
now = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
payload = dict(state=state, job_id=job_id, target_sha=target_sha,
               result_sha='', unit_name=unit, started_at=now, updated_at=now,
               message=message, phase=phase)
descriptor, temporary = tempfile.mkstemp(prefix=path.name + '.tmp.', dir=path.parent)
try:
    with os.fdopen(descriptor, 'w') as output:
        os.fchown(output.fileno(), -1, path.parent.stat().st_gid)
        os.fchmod(output.fileno(), 0o664)
        json.dump(payload, output)
        output.write('\n')
    os.replace(temporary, path)
finally:
    if os.path.lexists(temporary):
        os.unlink(temporary)
PY
}

systemctl reset-failed "${UNIT}.service" 2>/dev/null || true

write_job_atomic "running" "enqueued" "enqueue"

# Detached from caller cgroup; runs as deploy user with that user's HOME/cargo.
# --same-dir is a flag (no =false). Omit it; WorkingDirectory= sets the cwd.
if ! systemd-run \
  --unit="${UNIT}" \
  --uid="${DEPLOY_USER}" \
  --gid="${DEPLOY_USER}" \
  --collect \
  --working-directory="${STAGING}" \
  --setenv="TARGET_SHA=${TARGET_SHA}" \
  --setenv="JOB_ID=${JOB_ID}" \
  --setenv="JOB_FILE=${JOB_FILE}" \
  --setenv="MARENGO_STAGING_ROOT=${STAGING}" \
  --setenv="MARENGO_ROOT=${OPT_ROOT}" \
  --setenv="MARENGO_SELF_UPDATE_UNIT=${UNIT}" \
  --setenv="HOME=/home/${DEPLOY_USER}" \
  --setenv="USER=${DEPLOY_USER}" \
  /bin/bash "${SCRIPT}"; then
  write_job_atomic "failed" "systemd-run failed" "error"
  echo "error: systemd-run failed for ${UNIT}" >&2
  exit 1
fi

echo "enqueued unit=${UNIT} job_id=${JOB_ID} target=${TARGET_SHA}"
