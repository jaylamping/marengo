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
