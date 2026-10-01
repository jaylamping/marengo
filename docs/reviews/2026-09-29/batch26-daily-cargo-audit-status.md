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

## Real pinned CLI qualification

Installed cargo-audit0.22.2 in an owned local tool root. Real run.sh execution uses
that scanner and an explicitly prepared owned RustSec clone at3461c0d8f85d084552dd999c58d97c7123a9e0fd;
GitHub inventory alone is replaced with an offline empty fixture, and research
appendix is unavailable on that qualification PATH. Scanner accepts all repaired
flags, scans400 locked dependencies against1278 advisories, returns0 with zero
vulnerabilities and one unmaintained paste warning. Wrapper correctly reports
maintenance-warnings, not a vulnerability or Clean. Both streams/result retained.
No maintenance task is closed by this result. CLI --no-fetch JSON omitted database
provenance; fallback requires a clean prepared Git snapshot and records its exact
commit/date/age. Unknown/missing/modified provenance cannot support Clean.

Additional public runner cases now qualify unavailable scanner, malformed output,
unknown provenance, missing database and future timestamp. Full daily-audit15 tests
pass (including11 table-driven scanner cases); original regression unchanged green.
Independent review and primary/delivery gates remain pending. T22 open.

T20 PR239 final-head CI36908251161 passed check/vcan (sim path skipped) and merged
at5c25a8f54b6043b878ac05cc5b2c21a42be0e4bd. Exact merged-main CI36909443242 is running;
T20 verification awaits that result.

## Final scoped review / draft PR240

Standards and Spec final checks at0db21a0 have zero remaining actionable findings.
Review corrections add explicit missing-snapshot scanner-failure coverage, retain
partial scanner streams on timeout, require canonical advisory Git top-level
identity (unrelated parent repository fixture rejects), and preserve completed
scanner streams when subsequent Git provenance inspection times out.15 classified
runner matrix cases plus frozen regression and existing audit tests pass. T20 now
verified after allfive exact merged-main CI36909443242 passed.

PR240 is draft; exact remote head/required Linux qualification/delivery pending.
The optional daily audit suite remains separately qualified natively; primary
Python/shell coverage omissions remain T26. T22 stays open until delivery proofs.
