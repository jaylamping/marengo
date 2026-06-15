#!/usr/bin/env bash
# CI-parity checks — run locally via: just check / docker compose run --rm check
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

CI_MODE=false
if [[ "${CI:-}" == "true" ]]; then
  CI_MODE=true
fi

BUF="${ROOT}/consul/node_modules/.bin/buf"
if [[ ! -x "${BUF}" ]]; then
  BUF="$(command -v buf || true)"
fi

echo "==> buf lint"
if [[ -x "${BUF}" ]]; then
  "${BUF}" lint proto
else
  echo "warn: buf not found, skipping lint (run inside dev container)"
fi

echo "==> buf breaking (PR only)"
if [[ -x "${BUF}" ]] && [[ "${CI_MODE}" == true ]] && [[ "${GITHUB_EVENT_NAME:-}" == "pull_request" ]]; then
  BASE_SHA="${GITHUB_BASE_SHA:-}"
  if [[ -z "${BASE_SHA}" ]]; then
    git fetch origin main 2>/dev/null || true
    BASE_SHA="$(git rev-parse origin/main 2>/dev/null || git rev-parse main 2>/dev/null || true)"
  fi
  if [[ -z "${BASE_SHA}" ]]; then
    echo "warn: could not resolve base ref for buf breaking, skipping"
  else
    AGAINST_DIR="$(mktemp -d)"
    # buf's `.git#branch=…` source mis-detects proto deletions in shallow/docker
    # checkouts; materialize the base tree instead.
    git archive "${BASE_SHA}" proto | tar -x -C "${AGAINST_DIR}"
    "${BUF}" breaking proto --against "${AGAINST_DIR}/proto"
    rm -rf "${AGAINST_DIR}"
  fi
fi

echo "==> consul: npm ci, gen:proto, build, audit"
(
  cd consul
  npm ci
  npm run gen:proto
  test -f src/gen/marengo/v1/marengo_pb.ts
  "${ROOT}/scripts/proto-checksum.sh"
  npm run build --ignore-scripts
  if [[ "${CI_MODE}" == true ]]; then
    npm audit --audit-level=high
  else
    npm audit --audit-level=high || echo "warn: npm audit reported issues (review before release)"
  fi
)

echo "==> validate fixtures (URDF + MJCF)"
"${ROOT}/scripts/validate-urdf.sh"

echo "==> cargo fmt"
cargo fmt --all -- --check

echo "==> cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test"
cargo test --workspace

echo "==> cargo deny"
if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check --disable-fetch
else
  echo "warn: cargo-deny not installed, skipping"
fi

echo "==> cargo audit"
if command -v cargo-audit >/dev/null 2>&1; then
  if [[ "${CI_MODE}" == true ]]; then
    cargo audit
  else
    cargo audit || echo "warn: cargo audit reported advisories"
  fi
else
  echo "warn: cargo-audit not installed, skipping"
fi

echo "==> cross-build smoke (aarch64)"
if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
  # shellcheck source=deploy-lib.sh
  source "${ROOT}/scripts/deploy-lib.sh"
  ensure_cargo_in_path
  ensure_pi_cross_target
  if [[ "${CI_MODE}" == true ]] && [[ "${GITHUB_REF:-}" != "refs/heads/main" ]]; then
    echo "skip: aarch64 cross-build (main branch only in CI)"
  elif [[ "${CI_MODE}" == true ]]; then
    cargo build --workspace --release --target aarch64-unknown-linux-gnu -p marengo-pi -p imu-probe --features socketcan,linux-i2c
  else
    cargo build --workspace --release --target aarch64-unknown-linux-gnu -p marengo-pi -p imu-probe --features socketcan,linux-i2c || \
      echo "warn: aarch64 cross-build failed (non-fatal locally)"
  fi
else
  echo "skip: aarch64-linux-gnu-gcc not found"
fi

echo "check: ok"
