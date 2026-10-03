#!/usr/bin/env bash
# Autoresearch harness: CPU cost of the Marengo CAN path (robstride + Davout).
# Gates on robstride/davout tests and on bit-exact wire/decode checksums, then
# prints median ns per control tick from crates/davout/examples/can_tick_bench.rs.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"

EXPECTED_CHECKSUM="CHECKSUM router=d679c416f5ea3e14 davout=dad9bc29a6c64400"

if ! cargo test -q -p robstride -p davout >/tmp/autoresearch-test.log 2>&1; then
  grep -E "FAILED|panicked|error(\[|:)" /tmp/autoresearch-test.log | head -40 >&2
  echo "tests failed" >&2
  exit 1
fi

cargo build -q --release -p davout --example can_tick_bench
out="$(./target/release/examples/can_tick_bench)"
echo "$out"

checksum="$(grep '^CHECKSUM' <<<"$out")"
if [[ "$checksum" != "$EXPECTED_CHECKSUM" ]]; then
  echo "checksum mismatch: got '$checksum', want '$EXPECTED_CHECKSUM'" >&2
  exit 1
fi
