# Batch12: historical session artifact preservation

Baseline: fully checked PR225 merge `bebc678e7e7319f5847f78e61720d941a9544db5`.
Branch: `codex/session-artifact-preservation`. Finding: G13, independent WP09 slice.
Delivery and finding disposition remain pending the checks recorded below.

Sparse registration previously replaced omitted labels/artifacts with NULL and
overwrote capture end time. Importing three sibling files reported three sessions;
archiving changed a previously finalized end time again. Store now preserves omitted
values and original capture times, replaces supplied references independently, and
counts unique session IDs separately from processed artifacts. A supplied different
candump path also invalidates both old statistics; omitted/same paths retain known
values, and replacement leaves unknown values rather than invented fresh counts.
The existing import
entry point returns unique sessions; the detailed report supplies both counters to
the CLI. Repeated imports count files processed in that call, including existing rows.

`clear_session_artifact` and `session clear-artifact --id ID --artifact KIND`
explicitly clear one selected reference. Files, sibling references and capture
metadata survive; clearing Candump also clears its derived statistics. Absent or
already empty references report no change. SQLite errors propagate to callers.

## Executed regression evidence

Complete original snapshots contain all **1466 Git blobs** at the baseline plus
one unchanged existing-public probe, with Git-blob/SHA256 checks before and after.
Each probe performs actual file/database operations, positive controls and actual
fixture cleanup before the named collected preservation assertion.

| Probe | Exact original result | Unchanged scoped candidate replay |
|---|---|---|
| Sparse registration, all six arrival orders, missing later sibling, duplicate/same-kind replacement, neighboring row/files and reopen | 0 passed / 1 failed; log `f8925b33…`, 24.075 s | 1 / 0; `5b982adc…`, 25.539 s |
| Actual import/repeat/archive/reopen, unique count, gzip/page contents and explicitly seeded capture metadata | 0 / 1; `a8e403de…`, 24.035 s | 1 / 0; `f06b0c02…`, 23.325 s |
| Real two-frame archive to one-frame candump replacement; omitted/same reference controls, shared-file neighbor and authoritative metadata | 0 / 1; `c9c2ba2c…`, 25.364 s | 1 / 0; `041212b7…`, 24.306 s |

These are **three combined regressions**, not six independent arrival-order reds
or isolated count/end/statistics-only failures. The later originals also include
the already proved sparse overwrite. Whole test SHA256s remain `5ce0efe4…`,
`2be89d1d…` and `19e25671…` in final source.
No setup, compilation, missing-API or zero-test attempt qualifies as a behavioral red.

Three new API conformance groups use actual SQLite query-only refusal/readback,
writable retry, shared archived files, per-kind clearing and real reopen. Two real
CLI groups check count output, all clear mappings, duplicate/absent clearing,
invalid-argument refusal and unavailable database failure. They are candidate
conformance; an API absent from the original binary is not an original regression.

The complete clear probe passes on compiled candidate and rejects a single
production mutant mapping Bench clearing to Trace. Its post-cleanup public-row
assertion fails at line305. The actual 1472-file capture and both execution trees
differ only at that production mapping; whole test bytes stay unchanged. Plan:
`9016d6719e0896a3690607040fb484bec1953bb877788979dde8ceba768d58bf`.
Candidate log: `99fe8c43c08a687d6dbda0f204703054763d2f5bc27f3b35bdc4d2d406a6a666`.
Mutant log: `cebcb98b49b97eada68aefdc7953da7333b6d4170911099c181006f946d172c5`.
The frozen unexecuted v1 plan is preserved; v2 corrects its diagnostic path
classifier before execution. That correction is not an executed behavior failure.

The eight new Rust groups exercise independent public behavior and distinct real
failure paths; no new ignores, hardware dependencies or negative sleeps appear.

Strict affected Store/CLI formatting, all-target clippy and tests passed **16 / 0 /
0 ignored**, 16.496 s total. The executed
manifest binds **1449 source/config/build/contract inputs**:
`c617cc43af81e90a933a006d24d99f3049979c1a624551bbd7d9a4695c25543d`.
Existing retention, SQLite query/FTS and literal candump tests remain useful and
are retained. Frozen-probe fixture duplication preserves independent provenance;
no claim of a cold-build speedup is made.

## Review and delivery

Independent Standards accepts the whole change and final delta; full independent
Spec reads all six production/dependency files and all five test files. Neither
final reviewer authored those files. Both report no blocking findings. The earlier
partial Spec report is superseded. All current source/test bindings are rehashed.

| Review receipt | SHA256 |
|---|---|
| `standards-review.md` | `46a2baf6a52448e52e7b8afe460163f75772edcdc3fe19eff573239da00ebec4` |
| `standards-final-review.md` | `d78ef82ba859dc8c62a4b15bafc659bde82ec75b762121dcde42e883e343a076` |
| `spec-final-review.md` | `9f286ab33ca3b36a24bc6c8ed46395214c950c059ad2b480c1912a7dfe8114af` |
| `spec-final-review-bindings.json` | `280b7b7494ae43978b40c629248d4887e3b8d0bda8c5fb1dd7ee50b46b4f2fb2` |

The post-mutation cache reset completed before final checks. Required primary in
fatal CI/main mode passes **751 Rust / 0 failed / 1 existing ignored, 355 frontend,
72 Pi MCP**, ARM release and check completion, in127.854 s. Raw log:
`418e8a0cdc1deea17f27c36c6c2c060b29489a688ada973ee0c90ba210042e37`.
Affected final16/0/0 raw log:
`b0e472ef891f0fb3855de78213f55437162d610d3a4ac3b9118b501b0474a7d4`.
Actual command/PID/wait receipts, before/after source hashes and fixture inventories
are retained. Exclusive behavior fixtures are empty; primary Node compiler/tsx
caches are honestly inventoried and preserved, not erased or counted as passes.

Older primary-v1 actually completed, but its wrapper rejected Node24's `ℹ` summary
prefix. Its untouched unqualified receipt and additive parser qualification remain
historical evidence. Fresh final primary uses the corrected frozen runner and the
final source; the older750-test run does not qualify the later statistics repair.
Final simulation initially failed before any tests because Docker's Linux engine
pipe was absent; that zero-test infrastructure failure is preserved separately.
After independent engine recovery, unchanged-source replay passes **5 / 0 / 0**,
7.214 s, log `4b7ab0a457027edc5b47da968928488b153dbff39837c0cc49f099b7e2d95a6e`.
`final-local-gate-summary.json` collects all three completed, source-bound gates.
Exact implementation/final/equal-tree main GitHub checks, verified all-refs backup
and exact completed-branch cleanup remain pending.

All proof and recovery evidence is under
`J:/code/marengo-migration-backup-20260929/batch12`. The ledger appends completed
batch11 delivery from its exact merge receipt, preserves all ten earlier history
objects semantically, and reconciles every CS/G/F/T ID. The independent audit also
identified and corrected a stale introductory total in the finding index.

## Remaining scope and continuation

G14 timestamp parsing remains open: these tests deliberately seed authoritative
capture times. They establish Store page/path-byte availability, not gateway HTTP
route behavior, atomic rollback after partial import, interrupted archive recovery,
physical filesystem durability or reference permission. Import refusal coverage
fails the first SQLite write; CLI database refusal occurs at open.

The parallel R2b1 resource proof (`373ba002…`) is conditional source-derived design,
not implementation or runtime acceptance. It explicitly accounts for rollback and
implicit statement journals, startup recovery before PRAGMA admission, credit/body
ownership and codec/node limits. Its engineering qualification remains separate.
G14 and other independent packages can progress while that contract is finalized.

No robot operation or physical acceptance occurred. Limits and Wave sign-off stay
unchanged. The repair loop remains active; no human input blocker is identified.
