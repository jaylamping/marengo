#!/usr/bin/env bash
# Fast dev-container / first-open setup (no full clippy/test).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

echo "==> git lfs"
if command -v git-lfs >/dev/null 2>&1 && git rev-parse --git-dir >/dev/null 2>&1; then
  git lfs install --local 2>/dev/null || true
  if [ -n "$(git lfs ls-files 2>/dev/null | head -1 || true)" ]; then
    git lfs pull || echo "warn: git lfs pull failed (missing credentials or no LFS objects)"
  fi
fi

echo "==> consul npm ci + gen:proto"
(cd consul && npm ci && npm run gen:proto)

echo "==> rust workspace build"
cargo build --workspace

echo "bootstrap: ok"
