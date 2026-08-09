#!/usr/bin/env bash
# Shared helpers for cloud-agent Pi access (Tailscale userspace + SSH).
# shellcheck shell=bash
# Source this file; do not execute directly.

: "${MARENGO_PI_HOST:=joey-robot.tail0b414.ts.net}"
: "${MARENGO_PI_USER:=joey}"
: "${MARENGO_PI_ROOT:=/opt/marengo}"
: "${MARENGO_CONFIG_DIR:=/opt/marengo/config/bringup/arm_4dof_right}"
: "${MARENGO_CLOUD_SSH_DIR:=${HOME}/.marengo-cloud-ssh}"
: "${MARENGO_TAILSCALE_STATE_DIR:=${HOME}/.tailscale}"
: "${MARENGO_TAILSCALE_SOCKS:=localhost:1055}"
: "${MARENGO_TAILSCALE_HTTP_PROXY:=localhost:1054}"

cloud_pi_log() {
  printf '==> %s\n' "$*" >&2
}

cloud_pi_warn() {
  printf 'warn: %s\n' "$*" >&2
}

cloud_pi_need_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || {
    echo "error: required command not found: ${cmd}" >&2
    return 127
  }
}

cloud_pi_tailscale_installed() {
  command -v tailscale >/dev/null 2>&1 && command -v tailscaled >/dev/null 2>&1
}

cloud_pi_install_tailscale() {
  if cloud_pi_tailscale_installed; then
    return 0
  fi
  cloud_pi_log "Installing Tailscale"
  # Official installer may need root for apt; Cursor VMs typically have passwordless sudo.
  if [[ "${EUID}" -eq 0 ]]; then
    curl -fsSL https://tailscale.com/install.sh | sh
  else
    curl -fsSL https://tailscale.com/install.sh | sudo sh
  fi
  hash -r 2>/dev/null || true
  if ! cloud_pi_tailscale_installed; then
    echo "error: Tailscale install finished but tailscale/tailscaled not on PATH" >&2
    return 1
  fi
}

cloud_pi_install_packages() {
  local pkgs=(netcat-openbsd rsync curl openssh-client)
  if ! command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
    pkgs+=(gcc-aarch64-linux-gnu)
  fi
  cloud_pi_log "Installing packages: ${pkgs[*]}"
  sudo apt-get update -qq
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "${pkgs[@]}"
}

cloud_pi_ensure_cross_target() {
  if ! rustup target list --installed 2>/dev/null | grep -qx 'aarch64-unknown-linux-gnu'; then
    cloud_pi_log "Adding Rust target aarch64-unknown-linux-gnu"
    rustup target add aarch64-unknown-linux-gnu
  fi
}

cloud_pi_tailscale_proxy_env() {
  export ALL_PROXY="socks5h://${MARENGO_TAILSCALE_SOCKS}/"
  export HTTP_PROXY="http://${MARENGO_TAILSCALE_HTTP_PROXY}/"
  export HTTPS_PROXY="http://${MARENGO_TAILSCALE_HTTP_PROXY}/"
}

cloud_pi_clear_proxy_env() {
  unset ALL_PROXY HTTP_PROXY HTTPS_PROXY
}

cloud_pi_tailscaled_running() {
  pgrep -x tailscaled >/dev/null 2>&1
}

cloud_pi_start_tailscaled() {
  mkdir -p "${MARENGO_TAILSCALE_STATE_DIR}"
  if cloud_pi_tailscaled_running; then
    return 0
  fi
  cloud_pi_log "Starting tailscaled (userspace networking)"
  nohup tailscaled \
    --state="${MARENGO_TAILSCALE_STATE_DIR}/tailscaled.state" \
    --socket="${MARENGO_TAILSCALE_STATE_DIR}/tailscaled.sock" \
    --tun=userspace-networking \
    --outbound-http-proxy-listen="${MARENGO_TAILSCALE_HTTP_PROXY}" \
    --socks5-server="${MARENGO_TAILSCALE_SOCKS}" \
    >/tmp/tailscaled.log 2>&1 &
  local i
  for i in $(seq 1 30); do
    if [[ -S "${MARENGO_TAILSCALE_STATE_DIR}/tailscaled.sock" ]]; then
      return 0
    fi
    sleep 0.2
  done
  cloud_pi_warn "tailscaled socket not ready; see /tmp/tailscaled.log"
  return 1
}

cloud_pi_tailscale_up() {
  local auth_key="${TAILSCALE_AUTH_KEY:-}"
  if [[ -z "${auth_key}" ]]; then
    cloud_pi_warn "TAILSCALE_AUTH_KEY not set — skipping tailscale up"
    return 1
  fi
  cloud_pi_start_tailscaled || return $?
  export TS_SOCKET="${MARENGO_TAILSCALE_STATE_DIR}/tailscaled.sock"
  if tailscale --socket="${TS_SOCKET}" status --json 2>/dev/null | grep -q '"BackendState":"Running"'; then
    cloud_pi_log "Tailscale already connected"
    return 0
  fi
  cloud_pi_log "Connecting Tailscale"
  tailscale --socket="${TS_SOCKET}" up \
    --auth-key="${auth_key}" \
    --accept-routes \
    --hostname="cursor-cloud-$(hostname -s 2>/dev/null || echo agent)"
}

cloud_pi_write_ssh_key() {
  mkdir -p "${MARENGO_CLOUD_SSH_DIR}"
  chmod 700 "${MARENGO_CLOUD_SSH_DIR}"

  local key_path="${MARENGO_CLOUD_SSH_DIR}/id_ed25519_marengo"
  if [[ -f "${key_path}" ]]; then
    return 0
  fi

  if [[ -n "${MARENGO_PI_SSH_PRIVATE_KEY_B64:-}" ]]; then
    printf '%s' "${MARENGO_PI_SSH_PRIVATE_KEY_B64}" | base64 -d >"${key_path}"
  elif [[ -n "${MARENGO_PI_SSH_PRIVATE_KEY:-}" ]]; then
    printf '%s\n' "${MARENGO_PI_SSH_PRIVATE_KEY}" >"${key_path}"
  else
    cloud_pi_warn "No Pi SSH key secret (MARENGO_PI_SSH_PRIVATE_KEY_B64 or MARENGO_PI_SSH_PRIVATE_KEY)"
    return 1
  fi

  chmod 600 "${key_path}"
}

cloud_pi_write_ssh_config() {
  cloud_pi_write_ssh_key || return $?
  local key_path="${MARENGO_CLOUD_SSH_DIR}/id_ed25519_marengo"
  local known_hosts="${MARENGO_CLOUD_SSH_DIR}/known_hosts"
  touch "${known_hosts}"
  chmod 644 "${known_hosts}"

  cat >"${MARENGO_CLOUD_SSH_DIR}/config" <<EOF
Host ${MARENGO_PI_HOST} *.ts.net
  User ${MARENGO_PI_USER}
  IdentityFile ${key_path}
  IdentitiesOnly yes
  StrictHostKeyChecking accept-new
  UserKnownHostsFile ${known_hosts}
  ProxyCommand /bin/nc -X 5 -x ${MARENGO_TAILSCALE_SOCKS} %h %p
EOF
  chmod 644 "${MARENGO_CLOUD_SSH_DIR}/config"
}

cloud_pi_ssh_target() {
  printf '%s@%s' "${MARENGO_PI_USER}" "${MARENGO_PI_HOST}"
}

cloud_pi_ssh_args() {
  printf '%s\n' \
    -F "${MARENGO_CLOUD_SSH_DIR}/config" \
    -o BatchMode=yes \
    -o ConnectTimeout=20
}

cloud_pi_ssh() {
  cloud_pi_write_ssh_config || return $?
  cloud_pi_tailscale_proxy_env
  # shellcheck disable=SC2046
  ssh $(cloud_pi_ssh_args) "$(cloud_pi_ssh_target)" "$@"
}

cloud_pi_gateway_url() {
  printf 'http://%s:8080' "${MARENGO_PI_HOST}"
}

# Load MARENGO_GATEWAY_LOG_TOKEN when unset: Cursor secret → Pi /etc/marengo/env via SSH.
# Never prints the token value. Cached after first attempt (success or empty).
_cloud_pi_log_token_checked=0
cloud_pi_ensure_log_token() {
  if [[ "${_cloud_pi_log_token_checked}" -eq 1 ]]; then
    return 0
  fi
  _cloud_pi_log_token_checked=1
  if [[ -n "${MARENGO_GATEWAY_LOG_TOKEN:-}" ]]; then
    return 0
  fi
  local token=""
  token="$(
    cloud_pi_ssh \
      "grep -E '^MARENGO_GATEWAY_LOG_TOKEN=' /etc/marengo/env 2>/dev/null | head -1 | cut -d= -f2-" \
      2>/dev/null \
      | tr -d '\r\n' || true
  )"
  if [[ -n "${token}" ]]; then
    export MARENGO_GATEWAY_LOG_TOKEN="${token}"
    cloud_pi_log "Loaded gateway log token from Pi /etc/marengo/env (${#token} chars)"
  fi
}

cloud_pi_curl_gateway() {
  local path="$1"
  shift
  cloud_pi_ensure_log_token
  cloud_pi_tailscale_proxy_env
  local args=(-fsS)
  if [[ -n "${MARENGO_GATEWAY_LOG_TOKEN:-}" ]]; then
    args+=(-H "x-marengo-log-token: ${MARENGO_GATEWAY_LOG_TOKEN}")
  fi
  curl "${args[@]}" "$(cloud_pi_gateway_url)${path}" "$@"
}

cloud_pi_remote_preamble() {
  cat <<EOF
set -euo pipefail
if [[ -f /etc/marengo/env ]]; then set -a; source /etc/marengo/env; set +a; fi
if [[ -f "\${HOME}/.cargo/env" ]]; then set -a; source "\${HOME}/.cargo/env"; set +a; fi
export PATH="\${HOME}/.cargo/bin:/usr/local/cargo/bin:\${PATH:-}"
export MARENGO_ROOT='${MARENGO_PI_ROOT}'
export MARENGO_CONFIG_DIR='${MARENGO_CONFIG_DIR}'
export RUST_LOG='robstride=info,davout=info,berthier=info,marengo_pi=info'
cd '${MARENGO_PI_ROOT}'
EOF
}

cloud_pi_verify() {
  local ok=true

  cloud_pi_log "Verifying Tailscale"
  if ! cloud_pi_tailscale_up; then
    ok=false
  elif ! tailscale --socket="${MARENGO_TAILSCALE_STATE_DIR}/tailscaled.sock" status >/dev/null 2>&1; then
    cloud_pi_warn "tailscale status failed"
    ok=false
  fi

  cloud_pi_log "Verifying SSH"
  if ! cloud_pi_ssh 'echo SSH_OK && hostname'; then
    ok=false
  fi

  cloud_pi_log "Verifying gateway /health"
  if ! cloud_pi_curl_gateway /health; then
    cloud_pi_warn "gateway /health unreachable (marengo-gateway may be down)"
    ok=false
  else
    printf '\n'
  fi

  cloud_pi_log "Verifying log API"
  if cloud_pi_curl_gateway '/logs/sessions?limit=3' >/tmp/marengo-sessions.json 2>/dev/null; then
    head -c 400 /tmp/marengo-sessions.json
    printf '\n'
  else
    cloud_pi_ensure_log_token
    if [[ -z "${MARENGO_GATEWAY_LOG_TOKEN:-}" ]]; then
      cloud_pi_warn "GET /logs/sessions failed (no MARENGO_GATEWAY_LOG_TOKEN in env or Pi /etc/marengo/env; gateway may require a token)"
    else
      cloud_pi_warn "GET /logs/sessions failed (token rejected or gateway log store unavailable)"
    fi
    ok=false
  fi

  if [[ "${ok}" == true ]]; then
    cloud_pi_log "cloud-pi: all checks passed"
    return 0
  fi
  cloud_pi_warn "cloud-pi: some checks failed (see messages above)"
  return 1
}
