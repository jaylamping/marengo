# Batch26 — daily cargo audit result classification

T22 remains open. Baseline f831e587b330b6c353456ee5cec7c55d50139603;
branch codex/daily-cargo-audit-status. Frozen public run.sh fixture replaces only
external deterministic audit/scanner processes in an owned temporary checkout.
The original scanner exit2/usage error becomes a dependency-advisory finding,
triggering the intended assertion failure. Whole probe and baseline are bound in
evidence/batch26. No host report or advisory database is modified.

Pending full repair: supported --no-fetch CLI with explicitly prepared snapshot,
separate missing/stale snapshot and unavailable scanner/error/vulnerability/clean
states, both output streams and exit code retained, report confidence/freshness,
real pinned CLI contract and offline classification fixtures. Reviews and primary
gates/delivery pending. No robot, deploy, limits, Wave or automation action.
