# Eighth repair batch: historical rows after failed writes

Checked baseline: PR221 merge `1518176efd0561e60d3e23eee31c7bbe8ca84a2e`.
Active Windows checkout and CAD: `J:\code\marengo`.
Evidence: `J:/code/marengo-migration-backup-20260929/batch08`.
Status: two existing-public failures reproduced and repaired; both unchanged
replays pass; independent final reviews and strict affected checks pass; required
primary and all five implementation-head CI jobs pass in
[PR222](https://github.com/jaylamping/marengo/pull/222); final-head/equal-tree
main delivery remains pending.

## Contract and scope

Baseline `HomingRegistry::record_verification` updates its public in-memory calibration
before writing the history resource. An actual write error therefore leaves an
uncommitted row visible, even though the method returns an error. Historical
publication must follow successful writing: preserve the previous record and
local state on failure, and permit a real later retry.

Stage a candidate historical record, write it through the existing filesystem
path, then publish it and perform the existing local state/flag transitions.
This is a small CS05 history-consistency subproblem. CS05 remains partial until
qualified acquisition, durable commit/private permission and installed-owner
clients are complete. Legacy local Verified is not Davout output permission.
In-place disk writing, partial writes, crash recovery and the new reference
journal remain separate work; write completion is not an fsync durability claim.
No hardware, master limits, CAD/model or Wave sign-off changes are included.

## Actual unchanged-production baseline

The independently reviewed existing-public probe compiles against an exact
archive of `1518176`, with all 1,432 original Git blobs verified and unchanged.
Its sole added integration test has SHA256
`3cc9aac2840cfefc75585637496c3ae08494ad0600b42a934e906ba7667c50d1`.
No production extraction or new API is used. Package cleaning and a compiled
manifest guard bind the actual archived crate; child-only temporary paths and
the native protoc tool stay under J:\code.

`original-red-v2.log` shows one intended assertion failure, Cargo 101, zero ignored
and 0.02 s test execution (1.985 s full run). A regular-file parent blocker causes
the actual typed Io error; the saved history is absent and blocker bytes survive,
but public calibration contains a new row. Actual writable neighbor, same-request
retry, independently decoded YAML, reconstruction with fresh Unhomed state and
exclusive cleanup all pass before the decisive assertion. The first launcher
attempt lacked protoc and executed zero tests; its preserved compile/setup
failure is excluded from behavioral evidence.

The second original-public probe freezes SHA256
`a69ae70c9edf585cbd485293a66b250e0f3da36b6ebaa47b47a0fb0f1fff8172`.
It first reaches a real prior successful write, then parks that exclusive
directory and installs a regular-file parent blocker. Typed Io preserves prior
parked/restored bytes and local state/flags, but memory wrongly replaces the
target row. Literal unrelated history, same-request successful retry, actual
YAML/reload and complete cleanup controls pass before the named assertion.
`replacement-red.log` has one failure, zero ignored, 0.02 s body and 2.597 s full;
all 1,432 original files plus the sole added probe are verified unchanged.

Only the existing registry writer changes: clone the record, stage its update,
write through the shared private writer, then publish memory and existing local
state/flag transitions. Public explicit persist uses the same filesystem path
and typed errors. Production registry SHA256:
`e59ed129418635f2b287a9dfb6c19837a2c11810124c8696fad1c6a62a9859b0`.

Both entire original probes pass byte-identically after this repair in separately
compiled frozen snapshots. Insertion: 1/0, 0.01 s body, 2.553 s full
(`candidate-green.log`). Replacement: 1/0, 0.02 s body, 1.352 s full
(`replacement-green.log`). All1,433 files in each snapshot remain unchanged
after execution, with only registry.rs differing from the original production.
The 287-input final compile/gate manifest binds actual source and both tests.

## Standards

No hard documented violation or blocker. One low possible duplication judgment
for manifest/YAML fixture setup is accepted to preserve frozen probe provenance.
The reviewer independently verifies all 287 source/gate identities and the exact
original archive. Receipt: `standards-review.md` under this batch's evidence root.

## Spec

No issue detected against the scoped historical publication contract. Both
original assertion failures and whole-file identical passing replays are
independently qualified, including all real failure/retry/reload/cleanup controls.
Receipt: `spec-review.md`. CS05 remains partial; no disk-crash durability, new
journal, current-reference grant or hardware acceptance is supplied.

## Required qualification

The identical insertion/replacement replays and independent reviews are complete.
Strict all-target Linux lint and affected homing/Davout/Berthier/Pi tests with
socketcan/linux-i2c pass 420/0, zero ignored (17.307 s full). The initial feature
selection omitted its provider package and executed zero tests; qualified v2
includes Pi and the preserved setup failure is excluded from behavioral proof.
Required primary passes 703 Rust/1 existing ignored, 355 frontend, 72 Pi MCP,
current deny/audit and the fatal aarch64 release build (101.223 s). Its immutable
per-run binding matches all 287 reviewed source/gate inputs. Evidence:
`primary-final` and `affected-linux-final-v2` logs/run JSON/source bindings,
reconciled by `local-gate-summary.json`. All five GitHub jobs pass at exact
implementation head `d541bd57a90f97417eda3d83981bcb7f74f54c0b` in
[run36736794717](https://github.com/jaylamping/marengo/actions/runs/36736794717):
703 Rust/1 existing ignored, 355 frontend, 72 Pi MCP, 5 simulation and 73 actual
virtual-CAN driver tests with none ignored. The committed 287-input manifest
matches local gates and both independently reviewed probes. Final documentation
head and equal-tree postmerge main checks remain pending. No new hardware-dependent
ignored test or sleep-based absence oracle is introduced. The known allowlisted paste maintenance warning remains M01.

## Reference continuation

The independently reviewed R2a-core proposal follows this bounded history repair:
target-only closed virtual acquisition/cancel/cleanup, ending EvidenceStaged with
CommitUnavailable and usable_reference=false. It writes no history and grants no
permission; ordinary physical adapters remain Unsupported. R2b supplies the real
recoverable noncoalescing writer and matching private grant installation; R3
supplies installed-owner clients. Those proposals remain unimplemented and their
acceptance tests unexecuted. Physical identity/reset/readback, drive limits/
timeouts, E-stop/support, model/plant and Wave requirements stay external.
