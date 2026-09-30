#!/usr/bin/env bash
# Required dependency scans. Optional workspace argument supports focused callers.
set -euo pipefail
ROOT="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
cd "${ROOT}"

echo "==> dependency scanner versions"
cargo deny --version
cargo audit --version

# check logs some crate-fetch failures without returning failure. Require the
# explicit fetch operation to succeed before trusting any cached check result.
echo "==> current advisory DB and registry fetch"
cargo deny --locked fetch db index

echo "==> cargo deny (complete registry coverage required)"
cargo deny --frozen check -D index-failure

echo "==> cargo audit"
# Scanner/transport errors remain failures locally as well as in CI.
cargo audit
