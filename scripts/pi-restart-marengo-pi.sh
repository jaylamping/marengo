#!/usr/bin/env bash
# Stop or restart marengo-pi.service so Davout reloads motors.yaml hard limits.
# Usage: pi-restart-marengo-pi.sh restart|stop
# Canonical body for marengo-pi MCP (pi_restart_marengo_pi) and pi-remote.sh.
set -euo pipefail

mode="${1:-restart}"
case "${mode}" in
  restart | stop) ;;
  *)
    echo "usage: $0 restart|stop" >&2
    exit 2
    ;;
esac

echo "=== before ==="
systemctl is-active marengo-pi.service 2>/dev/null || echo inactive
pgrep -af '/opt/marengo/bin/marengo-pi|/bin/marengo-pi' || echo '(no marengo-pi process)'
echo
echo "=== stop ==="
sudo systemctl stop marengo-pi.service 2>/dev/null || true
sudo pkill -f '/opt/marengo/bin/marengo-pi' 2>/dev/null || true
pkill -f '/opt/marengo/bin/marengo-pi' 2>/dev/null || true
for _ in 1 2 3 4 5 6 7 8 9 10; do
  pgrep -f '/opt/marengo/bin/marengo-pi' >/dev/null 2>&1 || break
  sleep 0.2
done

if [[ "${mode}" == "restart" ]]; then
  echo
  echo "=== start ==="
  if ! systemctl cat marengo-pi.service >/dev/null 2>&1; then
    echo "error: marengo-pi.service unit not found — process stopped; start manually" >&2
    exit 1
  fi
  sudo systemctl start marengo-pi.service
  sleep 1
  if ! systemctl is-active --quiet marengo-pi.service; then
    echo "error: marengo-pi.service failed to become active" >&2
    systemctl status marengo-pi.service --no-pager -l 2>/dev/null || true
    exit 1
  fi
  systemctl is-active marengo-pi.service
else
  echo
  echo "=== stop-only (not starting systemd unit) ==="
fi

echo
echo "=== after ==="
systemctl is-active marengo-pi.service 2>/dev/null || echo inactive
pgrep -af '/opt/marengo/bin/marengo-pi|/bin/marengo-pi' || echo '(no marengo-pi process)'
if [[ "${mode}" == "restart" ]]; then
  echo
  echo "Hard limits / motors.yaml reload on marengo-pi process start."
fi
