#!/usr/bin/env bash
# Pi deploy via dev container (Windows / hosts without aarch64 cross GCC).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PI_HOST="${1:-${MARENGO_PI_HOST:-joey@marengo.local}}"
SSH_DIR="${MARENGO_SSH_DIR:-${USERPROFILE:-${HOME}}/.ssh}"

cd "$ROOT"
exec docker compose --profile deploy run --rm \
  -v "${SSH_DIR}:/root/.ssh:ro" \
  deploy-pi ./scripts/deploy-pi.sh --install "$PI_HOST"
