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

## Implementation qualification in progress

At95b1f03, run.sh invokes cargo audit --no-fetch --no-yanked --json --db with an
explicit MARENGO_ADVISORY_DB path (default ~/.cargo/advisory-db). It never refreshes
that snapshot. Snapshot policy requires a known last commit/date no older than7
days and never future; provenance and age are visible. Registry yanked scanning is
explicitly excluded from this optional offline advisory scan; primary dependency
gates retain their full coverage. Operational failures never become vulnerability
findings or Clean. stdout/stderr, exit code and structured result are retained,
using the same selected interpreter/output UTC directory for both reports.

Original full probe bytes replay unchanged green. Owned public runner fixtures
qualify clean, actual vulnerability count, maintenance warnings, stale snapshot,
exit2 scanner error and inconsistent exit1 empty report; emitted command verified.
Full daily-audit unittest discovery15 passes; shell syntax/diff checks pass. Pinned
cargo-audit0.22.2 installation is live under owned local tool directory for real
CLI qualification. Additional unavailable/malformed/missing/future cases, pinned
CLI evidence, independent reviews, required gate and delivery remain pending.
