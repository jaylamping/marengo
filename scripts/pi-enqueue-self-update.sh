#!/usr/bin/env bash
# Root-owned helper: enqueue Pi self-update as the deploy user outside the gateway cgroup.
# Invoked by marengo-gateway via: sudo -n /opt/marengo/scripts/pi-enqueue-self-update.sh <target_sha> <job_id>
set -euo pipefail

TARGET_SHA="${1:-}"
JOB_ID="${2:-}"
DEPLOY_USER="${MARENGO_DEPLOY_USER:-joey}"
STAGING="${MARENGO_STAGING_ROOT:-/home/${DEPLOY_USER}/marengo}"
OPT_ROOT="${MARENGO_ROOT:-/opt/marengo}"
JOB_DIR="${MARENGO_DEPLOY_JOB_DIR:-${OPT_ROOT}/var}"
JOB_FILE="${MARENGO_DEPLOY_JOB_FILE:-${JOB_DIR}/deploy-job.json}"
LOCK_FILE="${MARENGO_DEPLOY_JOB_LOCK:-${JOB_DIR}/deploy-job.lock}"
SCRIPT="${STAGING}/scripts/pi-self-update.sh"
UNIT="marengo-self-update"

if [[ "$(id -u)" -ne 0 ]]; then
  echo "error: must run as root (via sudo -n)" >&2
  exit 1
fi
if [[ -z "${TARGET_SHA}" || -z "${JOB_ID}" ]]; then
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
if [[ ! -x "${SCRIPT}" ]]; then
  echo "error: self-update script missing or not executable: ${SCRIPT}" >&2
  exit 1
fi

mkdir -p "${JOB_DIR}"
# Gateway (marengo) and deploy user both need to read/write job status.
if getent group marengo >/dev/null 2>&1; then
  chgrp marengo "${JOB_DIR}" 2>/dev/null || true
  chmod 775 "${JOB_DIR}" 2>/dev/null || true
fi

# Refuse overlapping workers — do not stop an in-flight unit.
if systemctl is-active --quiet "${UNIT}.service" 2>/dev/null; then
  echo "error: unit ${UNIT}.service already active" >&2
  exit 1
fi

exec 9>"${LOCK_FILE}"
if ! flock -n 9; then
  echo "error: another enqueue holds ${LOCK_FILE}" >&2
  exit 1
fi

write_job_atomic() {
  local state="$1"
  local message="$2"
  local phase="$3"
  local now tmp
  now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  tmp="${JOB_FILE}.tmp.$$"
  cat >"${tmp}" <<EOF
{
  "state": "${state}",
  "job_id": "${JOB_ID}",
  "target_sha": "${TARGET_SHA}",
  "result_sha": "",
  "unit_name": "${UNIT}",
  "started_at": "${now}",
  "updated_at": "${now}",
  "message": "${message}",
  "phase": "${phase}"
}
EOF
  mv -f "${tmp}" "${JOB_FILE}"
  chgrp marengo "${JOB_FILE}" 2>/dev/null || true
  chmod 664 "${JOB_FILE}" 2>/dev/null || true
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
