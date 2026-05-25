#!/usr/bin/env bash
# Install Marengo runtime layout on Raspberry Pi (run on the Pi as root).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
INSTALL_ROOT="${MARENGO_INSTALL_ROOT:-/opt/marengo}"
RUN_USER="${MARENGO_USER:-marengo}"

if [[ "$(id -u)" -ne 0 ]]; then
  echo "run as root: sudo $0" >&2
  exit 1
fi

echo "Installing Marengo to ${INSTALL_ROOT} (user ${RUN_USER})"

if ! id "$RUN_USER" &>/dev/null; then
  useradd --system --home "$INSTALL_ROOT" --shell /usr/sbin/nologin "$RUN_USER"
fi
usermod -aG dialout "$RUN_USER" || true

mkdir -p "${INSTALL_ROOT}/bin" "${INSTALL_ROOT}/config" "${INSTALL_ROOT}/assets" "${INSTALL_ROOT}/var/log"
chmod 775 "${INSTALL_ROOT}/var" "${INSTALL_ROOT}/var/log" 2>/dev/null || true
chown root:"${RUN_USER}" "${INSTALL_ROOT}/var" "${INSTALL_ROOT}/var/log" 2>/dev/null || true

PI_BIN="${ROOT}/target/release/marengo-pi"
REPL_BIN="${ROOT}/target/release/motor-repl"
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
if [[ -f "$REPL_BIN" ]]; then
  install -m 755 "$REPL_BIN" "${INSTALL_ROOT}/bin/motor-repl"
fi

rsync -a --delete "${ROOT}/config/" "${INSTALL_ROOT}/config/"
rsync -a "${ROOT}/assets/" "${INSTALL_ROOT}/assets/"

mkdir -p /etc/marengo
if [[ ! -f /etc/marengo/env ]]; then
  install -m 640 "${ROOT}/scripts/env.example" /etc/marengo/env
  chown root:"${RUN_USER}" /etc/marengo/env
fi

install -m 644 "${ROOT}/scripts/systemd/marengo-can.service" /etc/systemd/system/marengo-can.service
install -m 644 "${ROOT}/scripts/systemd/marengo-pi.service" /etc/systemd/system/marengo-pi.service
sed -i "s|WorkingDirectory=.*|WorkingDirectory=${INSTALL_ROOT}|" /etc/systemd/system/marengo-pi.service
sed -i "s|User=.*|User=${RUN_USER}|" /etc/systemd/system/marengo-pi.service
sed -i "s|ExecStart=.*|ExecStart=${INSTALL_ROOT}/bin/marengo-pi|" /etc/systemd/system/marengo-pi.service

chown -R "${RUN_USER}:${RUN_USER}" "${INSTALL_ROOT}"

systemctl daemon-reload
systemctl enable marengo-can.service

echo "Done. Next:"
echo "  1. Edit /etc/marengo/env (MARENGO_ROOT, MARENGO_CONFIG_DIR)"
echo "  2. sudo systemctl start marengo-can.service"
echo "  3. MARENGO_CONFIG_DIR=config/bringup/shoulder_pitch_dual ${INSTALL_ROOT}/bin/motor-repl status"
