# Batch15: refuse a rewritten migration marker before commit

This G15 follow-up starts from fully checked main `3345f129f282fc4e2312f64ddc30744613ba1427`
(tree `d9d19dc7`). G15 remains **partial**. The same102 findings remain16 verified,
11 partial and75 open; the other101 finding objects, eight maintenance tasks
and all13 earlier history records are unchanged. Batch14 completed delivery is
reconciled into history from its independently bound external merge receipt.

## Failure and repair

SQLite can report a successful marker UPSERT while an AFTER UPDATE trigger
rewrites its result. The old migration owner commits without readback. An
independently seeded v2 database rewrites attempted3 back to2 and increments a
counter. A second-attempt SQL abort tripwire bounds the actual public open.
Original Store::open commits the first cache removal under marker2/counter1,
then returns the typed tripwire constraint on attempt2. The completed failure
snapshot proves this mismatched transition; no unbounded hang is executed.

The private owner now reads the persisted marker inside the same Immediate
transaction and requires the expected next version before commit. A mismatch
returns an actionable refusal and rolls back the entire step. Existing public
interfaces, per-step ownership, earlier commits, SQL migrations, defaults,
dependencies and busy policy remain. ADR0029's matching-marker contract applies;
the recurring guidance in rust-patterns is clarified.

## Actual behavior evidence

External immutable evidence is under `J:/code/marengo-migration-backup-20260929/batch15`.
The complete frozen test `migration_marker_progress.rs` has SHA256
`dca81574ce5f7ef1f4a8f076b46a91dd01723894bb4ba89691eb144e03088f52`.
Original and candidate whole bytes are identical. The selected existing-public
worker is `rewritten_final_marker_refuses_before_committing_and_retries_after_trigger_removal`.

| Run | Actual result | Test body | Receipt SHA256 |
| --- | --- | --- | --- |
| Original normal v1/v2 control |1pass/0failed/0ignored/2filtered|0.61s|`0e56eaa7f20bfcbf1acaf629dec5fa3518ca94b6749e9572c220d28fe3aec5e9`|
| Original marker progress |0pass/1failed/0ignored/0filtered|0.31s|`a64502a5cf40fb0fbb45edba2a0c9c9e28a521dbbaf422c7930442c604079b95`|
| Unchanged repaired marker progress |1pass/0failed/0ignored/0filtered|0.38s|`2826245dc81e9a4882c0221de5b00c14c5f60422268c6acd77947393bf7cc819`|

The sole actual original assertion is line408, literal marker-progress
refusal/rollback/retry failed, after controls and actual fixture removal/absence.
It reports controls=true, rollback=false and retry=true with ConstraintViolation
1811 and the independent attempt2 tripwire message. Repaired success requires
Message refusal, unchanged complete prior snapshot, trigger removal, real public
retry/insert/reopen/migrate, historical/fresh FTS, integrity and cleanup. Setup,
compiler, deadline, wrapper line85 and absent-control failures cannot qualify.

All1484 checked original Git blobs are independently framed and verified.
Original-progress adds only the whole public probe (1485files); candidate has the
same1485 paths and differs only in the owner and guidance. Actual locked offline
metadata, fresh local Store/Candump compilation, exact executable hash, joined
worker output, waits and empty fixtures bind all three executions. The unchanged
v5 helpers retain their guards and hashes. The initial positive config's line530
was caught statically before invocation; it remains excluded/unexecuted. Selected
v2 binds actualassert527, with an explicit immutable selection addendum. No
existing behavioral oracle or helper guard was weakened. The original red and
identical green directly establish sensitivity; no redundant mutant was run.

## Required gates and review checkpoint

Affected strict formatting/clippy and30 tests pass (0failed/0ignored). Required
primary passes765 Rust/0failed/1existingignored,355 frontend and72 Pi MCP tests,
proto/build/deny/audit and fatal ARM release. All1460 gate inputs remain unchanged
before/after; mutable review tracking is explicitly excluded from this gate map
and included in complete proof snapshots. Gate receipts retain actual tool-cache
temp inventories; they are not represented as empty fixture directories.

Local simulation is not repeated for this store-only guard. Required GitHub sim
and virtual CAN remain delivery checks. Final Standards/Spec and GitHub delivery
are pending at this frozen tracked checkpoint; root must record separate actual
review/CI/backup/merge/cleanup receipts before accepting delivery. This avoids
inferring a future pass or running another full CI solely to update pending text.
The next tracked iteration reconciles actual completed delivery.

## Remaining acceptance and test quality

One new independent behavior test is added. Existing useful retention/query/FTS/
candump/migration tests remain, with no test deletion, new ignore, negative sleep
or speedup claim. Its SQL tripwire terminates the original deterministically;
the wrapper deadline is only an ordinary-suite safeguard.

G15 still needs completed backups and deliberate historic-prefix recovery,
remaining malformed/busy/interruption acceptance and deterministic opener races.
Readback does not bound an independent writer resetting the marker between
transactions or qualify arbitrary corrupt schemas, crash/power-loss/media
behavior. M08's initiating Docker disappearance/permanent socket cause remains
unproved; this batch performs no restart or runtime recovery. Hardware acceptance
is separate, including fresh reference/sign checks, installed-drive limits/
timeouts/E-stop/support/model provenance and issues170/176. No robot connection,
enable, move, flash, deploy, limit increase or Wave sign-off change occurs.
