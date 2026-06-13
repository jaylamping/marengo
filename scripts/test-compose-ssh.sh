#!/usr/bin/env bash
# Diagnostic: verify Docker deploy SSH setup (not run in CI).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=deploy-lib.sh
source "${ROOT}/scripts/deploy-lib.sh"

export MARENGO_DEPLOY_VIA_COMPOSE=1
TARGET="${1:-$(resolve_deploy_pi_host)}"

compose_ssh_preflight "$TARGET"
