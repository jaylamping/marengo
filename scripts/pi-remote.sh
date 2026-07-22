#!/usr/bin/env bash
# Cloud-agent Pi CLI — mirrors marengo-pi MCP read-only/admin commands over SSH.
# Use when marengo-pi MCP is unavailable (cloud sessions) but Tailscale + SSH are configured.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=cloud-pi-lib.sh
source "${ROOT}/scripts/cloud-pi-lib.sh"

LOG_DIR="${MARENGO_PI_ROOT}/var/log"

usage() {
  cat <<'EOF'
Usage: pi-remote.sh <command> [args]

Commands (read-only / admin — no motion):
  health                 CAN, deploy rev, gateway, systemd
  logs-tail [lines]      tail bench-latest.log (default 100)
  logs-grep <pattern>    grep -E over bench-latest.log
  logs-last-fault        ERROR/WARN/fault lines from latest bench log
  logs-list [limit]      gateway sessions + hot files (default 15)
  logs-sessions [limit]  GET /logs/sessions via HTTP (default 15)
  logs-structured [n]    GET /logs/structured (default 100)
  candump-summary        summarize candump-latest.log
  journal [unit]         journalctl (default marengo-pi)
  restart-marengo-pi     stop + start marengo-pi.service (reload hard limits)
  stop-marengo-pi        stop/pkill marengo-pi only (no start)
  ssh [--] <cmd...>      raw remote command
  deploy [--install]     cross-build + rsync (requires aarch64-linux-gnu-gcc)
  verify                 connectivity checks

Setup: ./scripts/setup-cloud-pi.sh --verify
Docs:  docs/cloud-pi-tailscale.md
EOF
}

remote_script() {
  cloud_pi_ssh bash -s <<EOF
$(cloud_pi_remote_preamble)
$1
EOF
}

cmd="${1:-}"
shift || true

case "${cmd}" in
  health)
    remote_script "$(cat <<'REMOTE'
echo '=== CAN interfaces ==='
ip -br link show type can || true
echo
echo '=== deploy rev ==='
cat .deploy-rev 2>/dev/null || echo '(no .deploy-rev)'
echo
echo '=== gateway (localhost) ==='
curl -sf http://127.0.0.1:8080/health 2>/dev/null || echo '(gateway /health unreachable)'
echo
echo '=== systemd ==='
systemctl is-active marengo-can marengo-gateway 2>/dev/null || true
echo
echo '=== processes ==='
pgrep -af 'marengo-pi|motor-repl|marengo-gateway' || echo '(none)'
REMOTE
)"
    ;;
  logs-tail)
    lines="${1:-100}"
    remote_script "tail -n ${lines} '${LOG_DIR}/bench-latest.log' 2>&1 || echo '(no bench-latest.log)'"
    ;;
  logs-grep)
    pattern="${1:?usage: pi-remote.sh logs-grep <pattern>}"
    pat="${pattern//\'/\'\\\'\'}"
    remote_script "grep -E '${pat}' '${LOG_DIR}/bench-latest.log' 2>/dev/null || echo '(no matches)'"
    ;;
  logs-last-fault)
    remote_script "grep -E 'ERROR|WARN|fault|watchdog|outside|limit' '${LOG_DIR}/bench-latest.log' 2>/dev/null | tail -n 80 || echo '(no fault lines)'"
    ;;
  logs-list)
    limit="${1:-15}"
    remote_script "$(cat <<REMOTE
gw='http://127.0.0.1:8080'
if curl -sf "\${gw}/logs/sessions?limit=${limit}" >/tmp/m-sessions.json 2>/dev/null; then
  echo '=== gateway sessions ==='
  cat /tmp/m-sessions.json
  echo
fi
du -sh '${LOG_DIR}' 2>/dev/null || true
echo '=== hot files ==='
ls -lt '${LOG_DIR}'/bench-*.log '${LOG_DIR}'/position-trace-*.csv '${LOG_DIR}'/candump-*.log 2>/dev/null | head -n ${limit} || echo '(no bench logs yet)'
REMOTE
)"
    ;;
  logs-sessions)
    limit="${1:-15}"
    cloud_pi_curl_gateway "/logs/sessions?limit=${limit}"
    printf '\n'
    ;;
  logs-structured)
    limit="${1:-100}"
    cloud_pi_curl_gateway "/logs/structured?limit=${limit}"
    printf '\n'
    ;;
  candump-summary)
    remote_script "$(cat <<'REMOTE'
F="${MARENGO_ROOT}/var/log/candump-latest.log"
if ! test -f "$F"; then
  echo "(no candump-latest.log)"
  exit 0
fi
lines=$(wc -l < "$F" | tr -d " ")
bytes=$(wc -c < "$F" | tr -d " ")
echo "file=$F lines=$lines bytes=$bytes"
first=$(grep -m1 -E "^[[:space:]]*\\(" "$F" 2>/dev/null || true)
last=$(grep -E "^[[:space:]]*\\(" "$F" | tail -n 1 || true)
echo "first=$first"
echo "last=$last"
REMOTE
)"
    ;;
  journal)
    unit="${1:-marengo-pi}"
    remote_script "journalctl -u '${unit}' -n 80 --no-pager 2>/dev/null || journalctl -n 80 --no-pager"
    ;;
  restart-marengo-pi)
    remote_script "$(cat <<'REMOTE'
echo '=== before ==='
systemctl is-active marengo-pi.service 2>/dev/null || echo inactive
pgrep -af '/opt/marengo/bin/marengo-pi|/bin/marengo-pi' || echo '(no marengo-pi process)'
echo
echo '=== stop ==='
sudo systemctl stop marengo-pi.service 2>/dev/null || true
sudo pkill -f '/opt/marengo/bin/marengo-pi' 2>/dev/null || true
pkill -f '/opt/marengo/bin/marengo-pi' 2>/dev/null || true
for i in 1 2 3 4 5 6 7 8 9 10; do
  pgrep -f '/opt/marengo/bin/marengo-pi' >/dev/null 2>&1 || break
  sleep 0.2
done
echo
echo '=== start ==='
if systemctl cat marengo-pi.service >/dev/null 2>&1; then
  sudo systemctl start marengo-pi.service
  sleep 1
  systemctl is-active marengo-pi.service || true
else
  echo 'error: marengo-pi.service unit not found — process stopped; start manually' >&2
  exit 1
fi
echo
echo '=== after ==='
systemctl is-active marengo-pi.service 2>/dev/null || echo inactive
pgrep -af '/opt/marengo/bin/marengo-pi|/bin/marengo-pi' || echo '(no marengo-pi process)'
echo
echo 'Hard limits / motors.yaml reload on marengo-pi process start.'
REMOTE
)"
    ;;
  stop-marengo-pi)
    remote_script "$(cat <<'REMOTE'
echo '=== before ==='
systemctl is-active marengo-pi.service 2>/dev/null || echo inactive
pgrep -af '/opt/marengo/bin/marengo-pi|/bin/marengo-pi' || echo '(no marengo-pi process)'
echo
echo '=== stop ==='
sudo systemctl stop marengo-pi.service 2>/dev/null || true
sudo pkill -f '/opt/marengo/bin/marengo-pi' 2>/dev/null || true
pkill -f '/opt/marengo/bin/marengo-pi' 2>/dev/null || true
for i in 1 2 3 4 5 6 7 8 9 10; do
  pgrep -f '/opt/marengo/bin/marengo-pi' >/dev/null 2>&1 || break
  sleep 0.2
done
echo
echo '=== stop-only (not starting systemd unit) ==='
echo
echo '=== after ==='
systemctl is-active marengo-pi.service 2>/dev/null || echo inactive
pgrep -af '/opt/marengo/bin/marengo-pi|/bin/marengo-pi' || echo '(no marengo-pi process)'
REMOTE
)"
    ;;
  ssh)
    if [[ "${1:-}" == "--" ]]; then
      shift
    fi
    if [[ $# -eq 0 ]]; then
      cloud_pi_ssh
    else
      cloud_pi_ssh "$@"
    fi
    ;;
  deploy)
    do_install=false
    if [[ "${1:-}" == "--install" ]]; then
      do_install=true
      shift
    fi
    cloud_pi_write_ssh_config
    cloud_pi_tailscale_proxy_env
    export MARENGO_PI_HOST MARENGO_PI_USER
    export MARENGO_SSH_DIR="${MARENGO_CLOUD_SSH_DIR}"
    host="$(cloud_pi_ssh_target)"
    if [[ "${do_install}" == true ]]; then
      "${ROOT}/scripts/deploy-pi.sh" --install "${host}"
    else
      "${ROOT}/scripts/deploy-pi.sh" "${host}"
    fi
    ;;
  verify)
    "${ROOT}/scripts/setup-cloud-pi.sh" --verify
    ;;
  -h | --help | help | "")
    usage
    [[ -z "${cmd}" ]] && exit 0
    ;;
  *)
    echo "error: unknown command: ${cmd}" >&2
    usage >&2
    exit 2
    ;;
esac
