#!/usr/bin/env bash
# Contract: the Pi deploy keeps marengo-log-cli's default robstride-enrichment
# feature (L-marengo-candump-06). Cargo unifies `--features` across the deploy
# package set, so enrichment survives only while (a) it stays a default feature
# of marengo-log-cli and (b) no deploy build passes --no-default-features.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

if ! grep -q 'default = \[.*"robstride-enrichment".*\]' bins/marengo-log-cli/Cargo.toml; then
  echo "error: marengo-log-cli lost its default robstride-enrichment feature" >&2
  exit 1
fi

if grep -n -- '--no-default-features' scripts/deploy-pi.sh scripts/pi-native-build.sh scripts/setup-mac-pi-cross.sh scripts/deploy-pi-docker.sh 2>/dev/null | grep -v '^.*:#'; then
  echo "error: a Pi build disables default features (drops log-cli enrichment)" >&2
  exit 1
fi

echo "deploy log-cli enrichment contract: ok"
