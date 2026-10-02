#!/usr/bin/env bash
# Offline research-handler replay only: no CAN, service, config or runtime writes.
set -euo pipefail
cd /home/joey/marengo-validation/batch27-20261001
curl -fsSLo uv-aarch64-unknown-linux-gnu.tar.gz \
  https://github.com/astral-sh/uv/releases/download/0.9.27/uv-aarch64-unknown-linux-gnu.tar.gz
curl -fsSLo uv-aarch64-unknown-linux-gnu.tar.gz.sha256 \
  https://github.com/astral-sh/uv/releases/download/0.9.27/uv-aarch64-unknown-linux-gnu.tar.gz.sha256
sha256sum -c uv-aarch64-unknown-linux-gnu.tar.gz.sha256
tar -xzf uv-aarch64-unknown-linux-gnu.tar.gz
tar -xzf batch27-research-source.tar.gz
export UV_PROJECT_ENVIRONMENT="$PWD/environment"
export UV_CACHE_DIR="$PWD/uv-cache"
export UV_PYTHON_DOWNLOADS=never
uv="$PWD/uv-aarch64-unknown-linux-gnu/uv"
project="$PWD/tools/marengo-research-mcp"
module="$project/src/marengo_research_mcp/tools/search.py"
cp "$module" candidate-search.py
trap 'cp candidate-search.py "$module"' EXIT
cp batch27-original-search.py "$module"
set +e
"$uv" run --project "$project" --locked --extra dev pytest \
  "$project/tests/test_cached_search_handlers.py" -q > original-handler-red.txt 2>&1
original_exit=$?
set -e
test "$original_exit" -eq 1
grep -F '12 failed' original-handler-red.txt
cp candidate-search.py "$module"
trap - EXIT
"$uv" run --project "$project" --locked --extra dev pytest \
  "$project/tests/test_cached_search_handlers.py" -q > unchanged-handler-green.txt 2>&1
cat unchanged-handler-green.txt
"$uv" run --project "$project" --locked --extra dev pytest \
  "$project/tests" -q -m 'not integration' > full-offline-green.txt 2>&1
cat full-offline-green.txt
sha256sum "$project/tests/test_cached_search_handlers.py" "$module"
date -u
