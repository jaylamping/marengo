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
  export CARGO_TERM_PROGRESS_WHEN="${CARGO_TERM_PROGRESS_WHEN:-always}"
  export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
  export NPM_CONFIG_PROGRESS="${NPM_CONFIG_PROGRESS:-true}"
  export NPM_CONFIG_LOGLEVEL="${NPM_CONFIG_LOGLEVEL:-notice}"
  if [[ "${MARENGO_DEPLOY_VERBOSE:-}" == 1 ]]; then
    export NPM_CONFIG_LOGLEVEL=verbose
    export CARGO_LOG="${CARGO_LOG:-cargo::core::compiler::fingerprint=info}"
  fi
}
