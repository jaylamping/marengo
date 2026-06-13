#!/usr/bin/env bash
# Shared logging helpers for deploy scripts (source, do not execute).
# shellcheck shell=bash

: "${MARENGO_DEPLOY_START:=$(date +%s)}"

_deploy_elapsed() {
  local now
  now="$(date +%s)"
  printf '%02d:%02d' $(((now - MARENGO_DEPLOY_START) / 60)) $(((now - MARENGO_DEPLOY_START) % 60))
}

log_step() {
  # Always stderr so stdout stays clean for tooling.
  printf '\n==> [%s] %s\n' "$(_deploy_elapsed)" "$*" >&2
}

log_note() {
  printf '    %s\n' "$*" >&2
}

log_warn() {
  printf 'warn [%s] %s\n' "$(_deploy_elapsed)" "$*" >&2
}

deploy_progress_env() {
  # `always` needs a TTY width; Docker deploy uses -T on Windows.
  export CARGO_TERM_PROGRESS_WHEN="${CARGO_TERM_PROGRESS_WHEN:-auto}"
  export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
  export NPM_CONFIG_PROGRESS="${NPM_CONFIG_PROGRESS:-true}"
  export NPM_CONFIG_LOGLEVEL="${NPM_CONFIG_LOGLEVEL:-notice}"
  if [[ "${MARENGO_DEPLOY_VERBOSE:-}" == 1 ]]; then
    export NPM_CONFIG_LOGLEVEL=verbose
    export CARGO_LOG="${CARGO_LOG:-cargo::core::compiler::fingerprint=info}"
  fi
}

# Rust/Cargo may be missing from PATH when scripts run under `bash -lc`, non-login
# shells, or SSH without sourcing ~/.cargo/env (Pi bench builds).
ensure_cargo_in_path() {
  if command -v cargo >/dev/null 2>&1; then
    return 0
  fi
  local d
  for d in /usr/local/cargo/bin "${HOME}/.cargo/bin"; do
    if [[ -x "${d}/cargo" ]]; then
      export PATH="${d}:${PATH}"
      return 0
    fi
  done
  echo "error: cargo not found (PATH=${PATH})" >&2
  return 127
}

ensure_pi_cross_target() {
  ensure_cargo_in_path || return $?
  local target="${MARENGO_PI_TARGET:-aarch64-unknown-linux-gnu}"
  if ! rustup target list --installed 2>/dev/null | grep -qx "${target}"; then
    log_step "rustup target add ${target}"
    rustup target add "${target}"
  fi
}


is_wsl() {
  grep -qiE "microsoft|WSL" /proc/version 2>/dev/null
}

# Host path for existence checks; on WSL maps C:/... to /mnt/c/...
normalize_ssh_dir_path() {
  local dir="${1}"
  dir="${dir//\\//}"
  if is_wsl && [[ "$dir" =~ ^([A-Za-z]):/(.*)$ ]]; then
    local drive="${BASH_REMATCH[1],,}"
    printf "/mnt/%s/%s" "$drive" "${BASH_REMATCH[2]}"
    return
  fi
  printf "%s\n" "$dir"
}

wsl_windows_ssh_dir() {
  is_wsl || return 1
  local user=""
  if command -v cmd.exe >/dev/null 2>&1; then
    user="$(cmd.exe /c "echo %USERNAME%" 2>/dev/null | tr -d "\r\n" | sed "s/^[[:space:]]*//;s/[[:space:]]*$//")"
  fi
  if [[ -n "$user" ]] && [[ -d "/mnt/c/Users/${user}/.ssh" ]]; then
    printf "/mnt/c/Users/%s/.ssh" "$user"
    return 0
  fi
  # Dev fallback: first Windows profile with the Marengo deploy key.
  local d
  for d in /mnt/c/Users/*/.ssh; do
    [[ -f "${d}/id_ed25519_marengo" ]] || continue
    printf '%s\n' "$d"
    return 0
  done
  return 1
}

ssh_dir_has_identity() {
  local dir="$1"
  local k
  for k in id_ed25519_marengo id_ed25519 id_rsa; do
    if [[ -f "${dir}/${k}" ]]; then
      return 0
    fi
  done
  return 1
}

resolve_ssh_dir() {
  local candidate=""
  if [[ -n "${MARENGO_SSH_DIR:-}" ]]; then
    candidate="$(normalize_ssh_dir_path "${MARENGO_SSH_DIR}")"
    if [[ -d "$candidate" ]] && ssh_dir_has_identity "$candidate"; then
      printf "%s\n" "$candidate"
      return
    fi
    if [[ -d "$candidate" ]]; then
      log_warn "MARENGO_SSH_DIR has no deploy key: ${candidate}"
    else
      log_warn "MARENGO_SSH_DIR is not a directory: ${candidate}"
    fi
  fi

  case "$(uname -s 2>/dev/null)" in
    MINGW* | MSYS* | CYGWIN*)
      if [[ -n "${USERPROFILE:-}" ]]; then
        candidate="$(normalize_ssh_dir_path "${USERPROFILE}/.ssh")"
        if [[ -d "$candidate" ]] && ssh_dir_has_identity "$candidate"; then
          printf "%s\n" "$candidate"
          return
        fi
      fi
      ;;
  esac

  if candidate="$(wsl_windows_ssh_dir)" && ssh_dir_has_identity "$candidate"; then
    printf "%s\n" "$candidate"
    return
  fi

  candidate="${HOME}/.ssh"
  if [[ -d "$candidate" ]] && ssh_dir_has_identity "$candidate"; then
    printf "%s\n" "$candidate"
    return
  fi

  # Last resort: return best-effort path for error messages / empty mount checks.
  if [[ -n "${MARENGO_SSH_DIR:-}" ]]; then
    printf "%s\n" "$(normalize_ssh_dir_path "${MARENGO_SSH_DIR}")"
    return
  fi
  if candidate="$(wsl_windows_ssh_dir)"; then
    printf "%s\n" "$candidate"
    return
  fi
  printf "%s\n" "${HOME}/.ssh"
}

docker_ssh_mount_src() {
  local dir
  dir="$(normalize_ssh_dir_path "${1}")"
  dir="${dir//\\//}"
  if [[ "$dir" == /mnt/* ]]; then
    printf "%s\n" "$dir"
    return
  fi
  if [[ "$dir" =~ ^([A-Za-z]):/(.*)$ ]]; then
    local drive="${BASH_REMATCH[1],,}"
    local rest="${BASH_REMATCH[2]}"
    if is_wsl; then
      printf "/mnt/%s/%s" "$drive" "$rest"
      return
    fi
    if [[ "$(uname -s 2>/dev/null)" == MINGW* ]] || [[ "${OSTYPE:-}" == msys* ]]; then
      printf "//%s/%s" "$drive" "$rest"
      return
    fi
    printf "%s:\\%s" "${BASH_REMATCH[1]}" "${rest//\//\\}"
    return
  fi
  case "$(uname -s 2>/dev/null)" in
    MINGW* | MSYS* | CYGWIN*)
      if command -v cygpath >/dev/null 2>&1; then
        cygpath -w "$dir"
        return
      fi
      ;;
  esac
  printf "%s\n" "$dir"
}


# --- Docker deploy SSH (Windows bind-mount .ssh is world-readable; copy to /tmp) ---

COMPOSE_SSH_DIR=""
COMPOSE_SSH_IDENTITY=""
COMPOSE_SSH_KNOWN=""

# user@host from MARENGO_PI_HOST / MARENGO_PI_USER when no CLI arg.
resolve_deploy_pi_host() {
  local host="${MARENGO_PI_HOST:-marengo.local}"
  local user="${MARENGO_PI_USER:-joey}"
  if [[ "$host" == *@* ]]; then
    printf '%s\n' "$host"
  else
    printf '%s@%s\n' "$user" "$host"
  fi
}

compose_ssh_source_dir() {
  local candidate=""

  if [[ -n "${MARENGO_SSH_DIR:-}" ]]; then
    candidate="$(normalize_ssh_dir_path "${MARENGO_SSH_DIR}")"
    if ssh_dir_has_identity "$candidate"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  fi

  # Bind mount is only valid when the mounted dir contains a deploy key.
  if [[ "${MARENGO_DEPLOY_VIA_COMPOSE:-}" == 1 ]] && ssh_dir_has_identity /home/marengo/.ssh; then
    printf '%s\n' "/home/marengo/.ssh"
    return 0
  fi

  candidate="$(resolve_ssh_dir)"
  if ssh_dir_has_identity "$candidate"; then
    printf '%s\n' "$candidate"
    return 0
  fi

  if [[ -d "${HOME}/.ssh" ]] && ssh_dir_has_identity "${HOME}/.ssh"; then
    printf '%s\n' "${HOME}/.ssh"
    return 0
  fi

  return 1
}

compose_ssh_setup() {
  if [[ "${MARENGO_DEPLOY_VIA_COMPOSE:-}" != 1 ]]; then
    return 0
  fi
  if [[ -n "$COMPOSE_SSH_DIR" ]]; then
    return 0
  fi

  local src
  src="$(compose_ssh_source_dir)" || {
    echo "error: no SSH directory for Docker deploy (mount ~/.ssh or set MARENGO_SSH_DIR)" >&2
    return 1
  }

  COMPOSE_SSH_DIR="/tmp/marengo-deploy-ssh"
  rm -rf "$COMPOSE_SSH_DIR"
  mkdir -p "$COMPOSE_SSH_DIR"
  chmod 700 "$COMPOSE_SSH_DIR"
  cp -a "${src}/." "$COMPOSE_SSH_DIR/"

  for f in "$COMPOSE_SSH_DIR"/id_* "$COMPOSE_SSH_DIR"/*.bak; do
    [[ -f "$f" ]] || continue
    if [[ "$f" == *.pub ]]; then
      chmod 644 "$f"
    else
      chmod 600 "$f"
    fi
  done

  if [[ -f "${COMPOSE_SSH_DIR}/config" ]]; then
    sed 's/\r$//' "${COMPOSE_SSH_DIR}/config" \
      | sed "s|~/.ssh|${COMPOSE_SSH_DIR}|g; s|\${HOME}/.ssh|${COMPOSE_SSH_DIR}|g" \
      > "${COMPOSE_SSH_DIR}/config.deploy"
    mv "${COMPOSE_SSH_DIR}/config.deploy" "${COMPOSE_SSH_DIR}/config"
    chmod 644 "${COMPOSE_SSH_DIR}/config"
  fi

  # Marengo bench key first — generic id_ed25519 is often not authorized on the Pi.
  for k in id_ed25519_marengo id_ed25519 id_rsa; do
    if [[ -f "${COMPOSE_SSH_DIR}/${k}" ]]; then
      COMPOSE_SSH_IDENTITY="${COMPOSE_SSH_DIR}/${k}"
      break
    fi
  done

  COMPOSE_SSH_KNOWN="${COMPOSE_SSH_DIR}/known_hosts"
  touch "$COMPOSE_SSH_KNOWN"
  chmod 644 "$COMPOSE_SSH_KNOWN"

  if [[ -z "$COMPOSE_SSH_IDENTITY" ]]; then
    echo "error: no private key in ${src} (expected id_ed25519_marengo)" >&2
    return 1
  fi
}

compose_ssh_opts() {
  compose_ssh_setup || return 1
  local -n _out=$1
  _out=(
    -o BatchMode=yes
    -o IdentitiesOnly=yes
    -o StrictHostKeyChecking=accept-new
    -o ConnectTimeout=15
  )
  if [[ -f "${COMPOSE_SSH_DIR}/config" ]]; then
    _out+=(-F "${COMPOSE_SSH_DIR}/config")
  fi
  if [[ -n "$COMPOSE_SSH_IDENTITY" ]]; then
    _out+=(-i "$COMPOSE_SSH_IDENTITY")
  fi
  if [[ -n "$COMPOSE_SSH_KNOWN" ]]; then
    _out+=(-o UserKnownHostsFile="$COMPOSE_SSH_KNOWN")
  fi
}

compose_ssh_target() {
  local host="$1"
  compose_ssh_setup || return 1
  if [[ -f "${COMPOSE_SSH_DIR}/config" ]] && [[ "$host" == *@* ]]; then
    echo "${host#*@}"
  else
    echo "$host"
  fi
}

compose_ssh() {
  local ssh_opts=()
  local target="$1"
  shift
  compose_ssh_opts ssh_opts || return 1
  target="$(compose_ssh_target "$target")"
  ssh "${ssh_opts[@]}" "$target" "$@"
}

# Verify SSH before rsync (Docker deploy only).
compose_ssh_preflight() {
  local host="$1"
  if [[ "${MARENGO_DEPLOY_VIA_COMPOSE:-}" != 1 ]]; then
    return 0
  fi
  log_step "SSH preflight → $(compose_ssh_target "$host")"
  if compose_ssh "$host" "true"; then
    log_note "SSH preflight ok"
    return 0
  fi
  echo "error: Docker deploy SSH failed (see above)" >&2
  echo "  Host: ${host}" >&2
  echo "  Tips:" >&2
  echo "    - Set MARENGO_PI_HOST to a resolvable name (e.g. Tailscale MagicDNS), not marengo.local" >&2
  echo "    - Ensure ~/.ssh/id_ed25519_marengo is authorized on the Pi" >&2
  echo "    - Host SSH: ssh -o BatchMode=yes ${host} true" >&2
  return 1
}
