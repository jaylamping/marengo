#!/usr/bin/env bash
# Install Marengo runtime layout on Raspberry Pi (run on the Pi as root).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"
INSTALL_ROOT="${MARENGO_INSTALL_ROOT:-/opt/marengo}"
RUN_USER="${MARENGO_USER:-marengo}"
DEPLOY_USER="${MARENGO_DEPLOY_USER:-${SUDO_USER:-joey}}"

if [[ "$(id -u)" -ne 0 ]]; then
  echo "run as root: sudo $0" >&2
  exit 1
fi

echo "Installing Marengo to ${INSTALL_ROOT} (user ${RUN_USER})"

# Bench default: stop always-on control so manual/MCP sessions own marengo-pi.
systemctl stop marengo-pi.service 2>/dev/null || true
pkill -f "${INSTALL_ROOT}/bin/marengo-pi" 2>/dev/null || true

if ! id "$RUN_USER" &>/dev/null; then
  useradd --system --home "$INSTALL_ROOT" --shell /usr/sbin/nologin "$RUN_USER"
fi
usermod -aG dialout "$RUN_USER" || true
usermod -aG i2c "$RUN_USER" || true
if id "$DEPLOY_USER" &>/dev/null; then
  usermod -aG "$RUN_USER" "$DEPLOY_USER" || true
  usermod -aG i2c "$DEPLOY_USER" || true
fi

mkdir -p \
  "${INSTALL_ROOT}/bin" \
  "${INSTALL_ROOT}/config" \
  "${INSTALL_ROOT}/assets" \
  "${INSTALL_ROOT}/www" \
  "${INSTALL_ROOT}/var/log" \
  "${INSTALL_ROOT}/var/log/blobs" \
  "${INSTALL_ROOT}/var/calibration" \
  "${INSTALL_ROOT}/var/gateway/tls"
chmod 775 "${INSTALL_ROOT}/var" "${INSTALL_ROOT}/var/log" "${INSTALL_ROOT}/var/calibration" 2>/dev/null || true
chown root:"${RUN_USER}" "${INSTALL_ROOT}/var" "${INSTALL_ROOT}/var/log" "${INSTALL_ROOT}/var/calibration" 2>/dev/null || true

PI_BIN="${ROOT}/target/release/marengo-pi"
GATEWAY_BIN="${ROOT}/target/release/marengo-gateway"
LOG_CLI_BIN="${ROOT}/target/release/marengo-log-cli"
REPL_BIN="${ROOT}/target/release/motor-repl"
IMU_PROBE_BIN="${ROOT}/target/release/imu-probe"
# Flat deploy layout (legacy rsync): binaries at repo root
if [[ ! -f "$PI_BIN" && -f "${ROOT}/marengo-pi" ]]; then
  PI_BIN="${ROOT}/marengo-pi"
fi
if [[ ! -f "$REPL_BIN" && -f "${ROOT}/motor-repl" ]]; then
  REPL_BIN="${ROOT}/motor-repl"
fi
if [[ ! -f "$PI_BIN" ]]; then
  echo "error: marengo-pi not found under ${ROOT}/target/release/ or ${ROOT}/" >&2
  exit 1
fi
install -m 755 "$PI_BIN" "${INSTALL_ROOT}/bin/marengo-pi"
if [[ -f "$GATEWAY_BIN" ]]; then
  install -m 755 "$GATEWAY_BIN" "${INSTALL_ROOT}/bin/marengo-gateway"
fi
if [[ -f "$LOG_CLI_BIN" ]]; then
  install -m 755 "$LOG_CLI_BIN" "${INSTALL_ROOT}/bin/marengo-log-cli"
fi
if [[ -f "$REPL_BIN" ]]; then
  install -m 755 "$REPL_BIN" "${INSTALL_ROOT}/bin/motor-repl"
fi
if [[ -f "$IMU_PROBE_BIN" ]]; then
  install -m 755 "$IMU_PROBE_BIN" "${INSTALL_ROOT}/bin/imu-probe"
fi

# Set Limits Apply writes durable hard/soft (+ expand-only URDF) on the Pi.
# Backup those envelopes before rsync so deploy cannot clobber them (ADR 0012/0017).
# Opt out: MARENGO_REPLACE_LIMITS=1
TAUGHT_BACKUP=""
if [[ "${MARENGO_REPLACE_LIMITS:-0}" != "1" ]] && [[ -f "${INSTALL_ROOT}/config/motors.yaml" ]]; then
  TAUGHT_BACKUP="$(mktemp -d)"
  mkdir -p "${TAUGHT_BACKUP}/config" "${TAUGHT_BACKUP}/assets/urdf"
  cp -a "${INSTALL_ROOT}/config/motors.yaml" "${TAUGHT_BACKUP}/config/" || true
  if [[ -f "${INSTALL_ROOT}/config/control.yaml" ]]; then
    cp -a "${INSTALL_ROOT}/config/control.yaml" "${TAUGHT_BACKUP}/config/" || true
  fi
  if [[ -f "${INSTALL_ROOT}/assets/urdf/marengo.urdf" ]]; then
    cp -a "${INSTALL_ROOT}/assets/urdf/marengo.urdf" "${TAUGHT_BACKUP}/assets/urdf/" || true
  fi
  echo "install-pi: backed up taught limits for preserve merge"
fi

rsync -a --delete "${ROOT}/config/" "${INSTALL_ROOT}/config/"
rsync -a "${ROOT}/assets/" "${INSTALL_ROOT}/assets/"
rsync -a "${ROOT}/scripts/" "${INSTALL_ROOT}/scripts/"

if [[ -n "${TAUGHT_BACKUP}" ]]; then
  if [[ -f "${ROOT}/scripts/preserve-taught-limits.py" ]]; then
    python3 "${ROOT}/scripts/preserve-taught-limits.py" \
      --previous "${TAUGHT_BACKUP}" \
      --install-root "${INSTALL_ROOT}" \
      || echo "warning: preserve-taught-limits failed; Pi may have lost taught Set Limits" >&2
  else
    echo "warning: preserve-taught-limits.py missing from deploy bundle" >&2
  fi
  rm -rf "${TAUGHT_BACKUP}"
fi

WWW_SRC=""
if [[ -d "${ROOT}/consul/dist" ]] && [[ -f "${ROOT}/consul/dist/index.html" ]]; then
  WWW_SRC="${ROOT}/consul/dist"
elif [[ -d "${ROOT}/www" ]] && [[ -f "${ROOT}/www/index.html" ]]; then
  WWW_SRC="${ROOT}/www"
fi
if [[ -n "$WWW_SRC" ]]; then
  rsync -a --delete "${WWW_SRC}/" "${INSTALL_ROOT}/www/"
else
  echo "warning: no Consul UI (consul/dist or www/index.html missing — run pi-native-build or cross deploy)" >&2
fi
chmod 755 "${INSTALL_ROOT}/scripts/can-up.sh"
chmod 755 "${INSTALL_ROOT}/scripts/homing-preflight.sh" 2>/dev/null || true
chown -R root:"${RUN_USER}" "${INSTALL_ROOT}/config" "${INSTALL_ROOT}/assets" "${INSTALL_ROOT}/scripts"
chmod -R g+rwX "${INSTALL_ROOT}/config" "${INSTALL_ROOT}/assets" "${INSTALL_ROOT}/scripts"
# Sudo targets for gateway must not be group-writable by ${RUN_USER}.
if [[ -f "${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh" ]]; then
  chown root:root "${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh"
  chmod 0755 "${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh"
fi
if [[ -f "${INSTALL_ROOT}/scripts/pi-enqueue-self-update.sh" ]]; then
  chown root:root "${INSTALL_ROOT}/scripts/pi-enqueue-self-update.sh"
  chmod 0755 "${INSTALL_ROOT}/scripts/pi-enqueue-self-update.sh"
fi
mkdir -p "${INSTALL_ROOT}/var"
if getent group "${RUN_USER}" >/dev/null 2>&1; then
  chgrp "${RUN_USER}" "${INSTALL_ROOT}/var" 2>/dev/null || true
  chmod 775 "${INSTALL_ROOT}/var" 2>/dev/null || true
fi

if id "$DEPLOY_USER" &>/dev/null; then
  SUDOERS_PATH="/etc/sudoers.d/marengo-${DEPLOY_USER}"
  SUDOERS_TMP="$(mktemp)"
  cat >"$SUDOERS_TMP" <<EOF
# Marengo bench automation (${DEPLOY_USER}) - generated by scripts/install-pi.sh
${DEPLOY_USER} ALL=(root) NOPASSWD: ${INSTALL_ROOT}/scripts/can-up.sh *
${DEPLOY_USER} ALL=(root) NOPASSWD: ${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh
${DEPLOY_USER} ALL=(root) NOPASSWD: ${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh *
${DEPLOY_USER} ALL=(root) NOPASSWD: ${INSTALL_ROOT}/scripts/install-pi.sh
${DEPLOY_USER} ALL=(root) NOPASSWD: /home/${DEPLOY_USER}/marengo/scripts/install-pi.sh
EOF
  chmod 440 "$SUDOERS_TMP"
  if visudo -cf "$SUDOERS_TMP" >/dev/null; then
    install -m 440 "$SUDOERS_TMP" "$SUDOERS_PATH"
  else
    echo "warning: generated sudoers failed validation; not installing ${SUDOERS_PATH}" >&2
  fi
  rm -f "$SUDOERS_TMP"
fi

# Gateway (User=${RUN_USER}) may restart marengo-pi and enqueue self-update only.
if id "$RUN_USER" &>/dev/null; then
  RUN_SUDOERS_PATH="/etc/sudoers.d/marengo-${RUN_USER}-restart"
  RUN_SUDOERS_TMP="$(mktemp)"
  cat >"$RUN_SUDOERS_TMP" <<EOF
# Marengo gateway control-loop restart + self-update enqueue (${RUN_USER}) - generated by scripts/install-pi.sh
${RUN_USER} ALL=(root) NOPASSWD: ${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh restart
${RUN_USER} ALL=(root) NOPASSWD: ${INSTALL_ROOT}/scripts/pi-enqueue-self-update.sh
${RUN_USER} ALL=(root) NOPASSWD: ${INSTALL_ROOT}/scripts/pi-enqueue-self-update.sh *
EOF
  chmod 440 "$RUN_SUDOERS_TMP"
  if visudo -cf "$RUN_SUDOERS_TMP" >/dev/null; then
    install -m 440 "$RUN_SUDOERS_TMP" "$RUN_SUDOERS_PATH"
  else
    echo "warning: generated restart sudoers failed validation; not installing ${RUN_SUDOERS_PATH}" >&2
  fi
  rm -f "$RUN_SUDOERS_TMP"
fi

mkdir -p /etc/marengo
if [[ ! -f /etc/marengo/env ]]; then
  install -m 640 "${ROOT}/scripts/env.example" /etc/marengo/env
  chown root:"${RUN_USER}" /etc/marengo/env
fi
# Migrate legacy bringup profile paths carefully.
# Only auto-migrate profiles that are equivalent to master right 4-DOF.
# Non-equivalent benches (3-DOF, left, weighted, …) require an explicit ack.
MARENGO_ALLOW_NONEQUIV_BRINGUP_MIGRATE="${MARENGO_ALLOW_NONEQUIV_BRINGUP_MIGRATE:-0}"
if [[ -f /etc/marengo/env ]] && grep -q 'config/bringup/' /etc/marengo/env 2>/dev/null; then
  bringup_slugs="$(
    grep -oE 'config/bringup/[^[:space:]"'\'']+' /etc/marengo/env 2>/dev/null \
      | sed 's|.*/||' | sort -u || true
  )"
  if [[ -n "${bringup_slugs}" ]]; then
    install -m 640 /etc/marengo/env "/etc/marengo/env.bak.$(date -u +%Y%m%dT%H%M%SZ)"
  fi
  for slug in ${bringup_slugs}; do
    case "${slug}" in
      arm_4dof_right|arm_2dof_right)
        echo "install-pi: migrating bringup/${slug} → master config/"
        sed -i "s|/opt/marengo/config/bringup/${slug}|/opt/marengo/config|g" /etc/marengo/env
        sed -i "s|config/bringup/${slug}|config|g" /etc/marengo/env
        ;;
      arm_3dof_right)
        if [[ "${MARENGO_ALLOW_NONEQUIV_BRINGUP_MIGRATE}" == "1" ]]; then
          echo "install-pi: migrating bringup/${slug} → master config/ with JOINT_SUBSET (explicit ack)"
          sed -i "s|/opt/marengo/config/bringup/${slug}|/opt/marengo/config|g" /etc/marengo/env
          sed -i "s|config/bringup/${slug}|config|g" /etc/marengo/env
          if ! grep -q '^MARENGO_JOINT_SUBSET=' /etc/marengo/env 2>/dev/null; then
            printf '\n# Auto-set on 3-DOF → master migration (install-pi.sh)\nMARENGO_JOINT_SUBSET=right_shoulder_roll,right_shoulder_pitch,right_upper_arm_yaw\n' \
              >> /etc/marengo/env
          fi
        else
          echo "error: /etc/marengo/env points at bringup/${slug}, which is not equivalent to master right 4-DOF." >&2
          echo "error: re-run with MARENGO_ALLOW_NONEQUIV_BRINGUP_MIGRATE=1 to migrate and set MARENGO_JOINT_SUBSET, or edit env manually." >&2
          exit 1
        fi
        ;;
      *)
        if [[ "${MARENGO_ALLOW_NONEQUIV_BRINGUP_MIGRATE}" == "1" ]]; then
          echo "warning: migrating non-equivalent bringup/${slug} → master config/ (explicit ack; verify CAN map / JOINT_SUBSET)" >&2
          sed -i "s|/opt/marengo/config/bringup/${slug}|/opt/marengo/config|g" /etc/marengo/env
          sed -i "s|config/bringup/${slug}|config|g" /etc/marengo/env
        else
          echo "error: /etc/marengo/env points at bringup/${slug}; refusing blind cutover to master right 4-DOF." >&2
          echo "error: set MARENGO_ALLOW_NONEQUIV_BRINGUP_MIGRATE=1 after confirming the joint/CAN map, or point MARENGO_CONFIG_DIR at /opt/marengo/config yourself." >&2
          exit 1
        fi
        ;;
    esac
  done
fi

install -m 644 "${ROOT}/scripts/systemd/marengo-can.service" /etc/systemd/system/marengo-can.service
install -m 644 "${ROOT}/scripts/systemd/marengo-pi.service" /etc/systemd/system/marengo-pi.service
install -m 644 "${ROOT}/scripts/systemd/marengo-gateway.service" /etc/systemd/system/marengo-gateway.service
install -m 644 "${ROOT}/scripts/systemd/marengo-log-maintenance.service" /etc/systemd/system/marengo-log-maintenance.service
install -m 644 "${ROOT}/scripts/systemd/marengo-log-maintenance.timer" /etc/systemd/system/marengo-log-maintenance.timer
sed -i "s|WorkingDirectory=.*|WorkingDirectory=${INSTALL_ROOT}|" /etc/systemd/system/marengo-pi.service
sed -i "s|User=.*|User=${RUN_USER}|" /etc/systemd/system/marengo-pi.service
sed -i "s|ExecStart=.*|ExecStart=${INSTALL_ROOT}/bin/marengo-pi|" /etc/systemd/system/marengo-pi.service
sed -i "s|WorkingDirectory=.*|WorkingDirectory=${INSTALL_ROOT}|" /etc/systemd/system/marengo-gateway.service
sed -i "s|User=.*|User=${RUN_USER}|" /etc/systemd/system/marengo-gateway.service
sed -i "s|ExecStart=.*|ExecStart=${INSTALL_ROOT}/bin/marengo-gateway --http-listen [::]:8080 --https-listen [::]:8444 --web-root ${INSTALL_ROOT}/www --wt-listen [::]:8443 --chappe-socket /run/marengo/chappe.sock|" /etc/systemd/system/marengo-gateway.service

chown -R "${RUN_USER}:${RUN_USER}" "${INSTALL_ROOT}"
chown -R root:"${RUN_USER}" "${INSTALL_ROOT}/config" "${INSTALL_ROOT}/assets" "${INSTALL_ROOT}/scripts" "${INSTALL_ROOT}/var" 2>/dev/null || true
chmod -R g+rwX "${INSTALL_ROOT}/config" "${INSTALL_ROOT}/assets" "${INSTALL_ROOT}/scripts" "${INSTALL_ROOT}/var" 2>/dev/null || true

# Re-harden sudo targets AFTER recursive scripts ownership (must stay root:root 0755).
if [[ -f "${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh" ]]; then
  chown root:root "${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh"
  chmod 0755 "${INSTALL_ROOT}/scripts/pi-restart-marengo-pi.sh"
fi
if [[ -f "${INSTALL_ROOT}/scripts/pi-enqueue-self-update.sh" ]]; then
  chown root:root "${INSTALL_ROOT}/scripts/pi-enqueue-self-update.sh"
  chmod 0755 "${INSTALL_ROOT}/scripts/pi-enqueue-self-update.sh"
fi

install_deploy_rev "${ROOT}" "${INSTALL_ROOT}"
chown root:root "${INSTALL_ROOT}/.deploy-rev"
chmod 644 "${INSTALL_ROOT}/.deploy-rev"

systemctl daemon-reload
systemctl enable --now marengo-can.service
if [[ -f "${INSTALL_ROOT}/bin/marengo-gateway" ]]; then
  systemctl enable marengo-gateway.service
  systemctl restart marengo-gateway.service
fi
if [[ -f "${INSTALL_ROOT}/bin/marengo-log-cli" ]]; then
  systemctl enable --now marengo-log-maintenance.timer
fi
if [[ -f "${INSTALL_ROOT}/bin/marengo-pi" ]]; then
  systemctl enable marengo-pi.service
  systemctl restart marengo-pi.service
fi

echo "Done. CAN (can0/can1) should be UP — verify: ip -br link show type can"
if [[ -x "${INSTALL_ROOT}/bin/motor-repl" ]] && [[ -x "${INSTALL_ROOT}/scripts/homing-preflight.sh" ]]; then
  BENCH_CFG="${INSTALL_ROOT}/config"
  if [[ -f /etc/marengo/env ]] && grep -q '^MARENGO_CONFIG_DIR=' /etc/marengo/env 2>/dev/null; then
    BENCH_CFG="$(grep '^MARENGO_CONFIG_DIR=' /etc/marengo/env | tail -1 | cut -d= -f2- | tr -d "\"'")"
    if [[ "$BENCH_CFG" != /* ]]; then
      BENCH_CFG="${INSTALL_ROOT}/${BENCH_CFG}"
    fi
  fi
  echo ""
  MARENGO_ROOT="${INSTALL_ROOT}" MARENGO_CONFIG_DIR="${BENCH_CFG}" \
    "${INSTALL_ROOT}/scripts/homing-preflight.sh" || true
fi
echo "Next:"
echo "  1. Edit /etc/marengo/env (MARENGO_ROOT, MARENGO_CONFIG_DIR)"
echo "  2. Consul UI: https://marengo.local:8444 (gateway enabled on boot; accept self-signed cert once)"
echo "  3. Bench motion: run marengo-pi / motor-repl manually (do not enable marengo-pi.service unless you want always-on control)"
echo "  4. Example: MARENGO_CONFIG_DIR=config ${INSTALL_ROOT}/bin/motor-repl status"
echo "  5. Local dev Consul: VITE_CHAPPE_* in consul/.env.local (see consul/.env.example)"
PI_IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
if [[ -n "${PI_IP}" ]] && [[ -f /etc/marengo/env ]]; then
  if ! grep -q '^MARENGO_GATEWAY_TLS_EXTRA_SAN=' /etc/marengo/env 2>/dev/null; then
    echo "MARENGO_GATEWAY_TLS_EXTRA_SAN=${PI_IP}" >> /etc/marengo/env
  fi
fi
