# Batch14: normal Store migration atomicity

G15 is **partial**. Normal schema transitions now have one connection owner and
one SQLite Immediate transaction per DDL/data/FTS step and matching marker.
This batch starts at fully checked PR227 main `4c1800d4ffb6be13a920a77b6f9b4b47395b0848`.
ADR0029 records the interface, transaction and refusal policy before implementation.

Store holds its mutex once through the private owner. Version reads happen after
the writer reservation, and each completed step is committed before the next
version read. A later failure retains the last coherent prior version. Private
helpers never reacquire the Store mutex. Existing bounded five-second SQLite busy
handling remains; no new public API, dependency or retry loop is introduced.

Current v3 opens do not rerun legacy bootstrap DDL or rewrite marker timestamps.
Missing defaults are inserted only if absent; supplied values and timestamps stay.
Future/invalid markers and missing markers on nonempty databases are refused.
The empty classifier excludes SQLite's literal reserved prefix, so valid
`sqliteXneighbor` evidence cannot disappear behind a LIKE wildcard. Public migrate
refuses journal mode OFF on its owned connection before any logical writes.
Open-time WAL/NORMAL setup remains; logical refusal does not promise unchanged
file headers or journal modes. Known version1/fields_json-present state receives
an actionable backed-up-recovery refusal, with no automatic repair.

## Actual regression and sensitivity evidence

Three unchanged whole integration probes contain eight public behavior cases:
independent supported v1/v2 upgrades; marker2/marker3 rollback and real retry;
current timestamp preservation; future version refusal; retained prior v2 commit
and usable FTS after a later marker3 refusal; markerless nonempty refusal; and
public journal-OFF refusal. Literal fixtures do not import production migration
SQL. Supported Store bootstrap is an explicit dependency only in the OFF fixture.
Independent schema/data/FTS/integrity observations, typed trigger refusal, neighbor
preservation, actual reopen/retry and cleanup precede the sole final oracles.
Bounded child watchdogs make a public migration deadlock finite; direct proof
workers execute the unchanged bodies, not an extra nested test copy.

Six original-public assertion reds reached their intended behavior assertions:

| Original execution | Actual result | Whole probe SHA256 | Receipt SHA256 |
| --- | --- | --- | --- |
| original-v2-red-v1 | 0 passed / 1 failed / 0 ignored | `f9c648d275750ec946810bd8c2d4f06427d1522b1c115c4b0c1216b3608d2fd8` | `ce29cae7dee27f2210e2e213e395cfb889adeb9cc11edbcc848d3877692fa5e6` |
| original-v3-red-v1 | 0 passed / 1 failed / 0 ignored | `f9c648d275750ec946810bd8c2d4f06427d1522b1c115c4b0c1216b3608d2fd8` | `9e374f17d92517cbf7f9b558a3552bcd4ab67b1fa64d32a972a2801dc0f01120` |
| original-current-red-v1 | 0 passed / 1 failed / 0 ignored | `5252efdf3c3720743643c7751efde276b7a1f3fb3366557f2f63664315c6a5d7` | `0b875e1d27d15ccd0c7cd8aa1b8c9f5d1821dc0042b1ae58971a46184cf21c9d` |
| original-future-red-v1 | 0 passed / 1 failed / 0 ignored | `5252efdf3c3720743643c7751efde276b7a1f3fb3366557f2f63664315c6a5d7` | `336a2c0215dd207e225770202e55ff79e7992e4e465da59291988edc265b8274` |
| original-nonempty-red-v1 | 0 passed / 1 failed / 0 ignored | `ed005b6de7e340501b625d590af4bdbac9ba009bf95dce308bde02f15d66f427` | `35711540b49cc5eb5634cef304ae1ec925926d14c5479a54082b7c8fc6f47b06` |
| original-disabled-journal-red-v1 | 0 passed / 1 failed / 0 ignored | `ed005b6de7e340501b625d590af4bdbac9ba009bf95dce308bde02f15d66f427` | `de9239c44e550361a674f3b92b78a9ce0e03ca4f71046d799e7d385d765f092b` |

The first four original regressions precede the first production owner edit.
The two additional admission regressions precede the phase02 guard correction.
All six replay unchanged green in the final candidate. The original supported
upgrade positive passed first. Candidate-only per-step conformance then passed,
rejected one exact production replacement that writes final marker3 during the
v1-to-v2 transition instead of marker2, and replayed unchanged green. Its sole
line496 assertion sees a real typed final-marker refusal, collected missing-v2
observations and completed cleanup. It is mutation sensitivity, not an original
regression, missing-API failure or setup/compiler pass.

Complete originals bind all 1,479 baseline blobs plus their sole whole probe:
1,480 files with no source exclusions. The final candidate binds all 1,483 files.
Actual metadata, local recompilation, artifact/executable hashes, direct worker
selection, process completion and empty fixture directories are preserved in
external `J:/code/marengo-migration-backup-20260929/batch14`. Candidate snapshots
and helpers are read-only during execution, with independent targets and no
network or project build cache masking. Reused targets require exact source,
helper and executable cache identities.

- `root-candidate-phase02-freeze.json` SHA256 `703c208f520945ca4a4ee264d07670055f42b4041f6f83846d35a35bd1a9b92f`.
- `root-phase02-execution-review.json` SHA256 `1faac47167f22ec2248555fec64ed392ecabefe6fdec0f832a8cd1d511e14fbd`.
- `root-mutation-phase02-freeze.json` SHA256 `1bfbb1e71d96c40e28ea5642495a385577bdd75806a46814c0a1661d335efbda`.
- `phase02-step-mutant-config.json` SHA256 `f68ce23f56ea63c515eedd456b902311ba604ed5df4ebe94a7ef33b93c5a4836`.
- `phase02-step-replay-config.json` SHA256 `12e3b22a0a3e7b8042b1e3f252a692aaf3b272742d6c5a1622281c94c3e23d34`.
- `standards-review-phase02.md` SHA256 `41ffdaa5c952a2ccf743892bfbf9169071d37996243dbb8fb1ad9ffe57ad9eb5`.

Phase01's six narrow positives are preserved but do not qualify shipment. Spec
found literal-prefix and OFF-admission defects; Standards found a missing crate
dependency paragraph. All three are corrected in phase02. Earlier external probe
proposals and executor-v4 examples remain unexecuted preparation, never inferred
as results. Independent final Spec/Standards and execution/tracking reconciliation
are recorded in the ledger before delivery; no acceptance is inferred from plans.

## Required checks and remaining acceptance

Strict affected tests: **29 passed, zero failed/ignored** in 17 test binaries.
Primary Docker check: **764 Rust passed, zero failed, one existing hardware ignore**
in108 test binaries; **355 frontend and72 Pi MCP passed**, with actual fatal ARM
release and format/lint/proto/build/deny/audit checks. Simulation: **5 passed,
zero failed/ignored**. Tool caches/temp inventories remain recorded; an existing
ignored hardware case is not a pass. No test deletion or build-speed improvement
is inferred. Meaningful prior query/FTS/retention/candump coverage remains.

Final local gate summary SHA256 `b4171b624baa9fe7e0b1c3b5edd045ee1de8ee3e98bf0e2b1d49cc296fd50b3b`.
Source manifest SHA256 `8396613924dd9fe43b24d1d7aee7e93635c5b14eac2d8981ded18cb76614eb52` binds 1,459 inputs.
Only mutable review tracking files are excluded from that gate manifest;
complete proof snapshots include them. Ignored local CAD/generated/build files
are not Git source inputs. Architecture, runtime/build guides and all new tests
are covered. Tracking changes receive separate exact review. GitHub delivery is
pending at this checkpoint; implementation/final/main results must be actual.

G15 stays partial until historic interrupted prefixes have independent
classification, a completed backup and verified recovery, plus the remaining
bootstrap/statement-boundary interruption, malformed-marker, busy refusal and
deterministic multi-connection/first-open journal race acceptance. This batch
proves selected logical rollback cases, not process crash/power-loss or physical
media durability. Follow-up work must preserve evidence and back up before recovery.

All102 finding IDs and older history remain; only G15 moves open to partial:
**16 verified,11 partial,75 open**. Batch13's completed delivery/backup/cleanup
is appended without erasing its earlier checkpoint. M08 Docker cause remains
partial. Reference journal/grant engineering, hardware acceptance, issue170's
ladder retry and issue176's Wave smoke remain separate. No robot operation,
limit increase, deployment or Wave sign-off change occurred.

## Source review and implementation checkpoint

Independent phase02 Standards and Spec find no remaining normal-slice source
blocker. Reports and exact source/execution bindings remain external:

- `standards-review-phase02.md` SHA256 `41ffdaa5c952a2ccf743892bfbf9169071d37996243dbb8fb1ad9ffe57ad9eb5`.
- `standards-review-phase02-binding.json` SHA256 `0b6203060d18dc81a1b19ae1439593aef8d3a3c10146cd839305152339a168f7`.
- `phase02-spec-review.md` SHA256 `9cbc8868bf5bf46f6bbecc5677bb15f867559baaa211127771bab4c8fee81c22`.
- `phase02-spec-review-bindings.json` SHA256 `9812e6bc9672bb7fd584631b84bd2e0ab6c600a5f9ec1c27eb15980da1c16dfc`.

[PR228](https://github.com/jaylamping/marengo/pull/228) starts at implementation
`5b3f56fcdd7f1b65b6ab5204938877dc0f44c5bf`; exact-head implementation CI is pending.
All1,459 committed and live gate inputs match. A commit whitespace check flagged
only ADR0029's intentional two-space Markdown hardbreak; root verified the exact
line and all other paths without modifying any source input or proof guard.

Final tracking review caught a Windows default-decoding change in one old batch06
history string. Root restored every older history object from the explicit UTF-8
baseline Git blob and checked all other findings and maintenance tasks. The
stage01 root semantic comparison is excluded: it had compared equally misdecoded
objects. Original byte copies/hashes and all actual behavior evidence remain
unchanged. This correction is included before final delivery, with no runtime edit.

`tracking-encoding-reconciliation.json` SHA256 `8e09417642cf7e8d744755aa610e152f7c5f0b8f1c6aac2bfccb8d36005075b3` records the exact correction.

## Completed implementation CI

PR228 implementation `5b3f56fcdd7f1b65b6ab5204938877dc0f44c5bf` passes all five jobs
in [run36831085548](https://github.com/jaylamping/marengo/actions/runs/36831085548).
Actual counts are764 Rust/0failed/1existingignored,355 frontend,72 Pi MCP,5 sim
and73 virtual CAN tests with no ignores in the latter two. PR CI has its separate
non-main ARM policy; the qualified local primary has an actual fatal ARM release.

`github-implementation-receipt.json` SHA256 `649cd165c643d0a31bf2adf3615be54da48e202437434a9abc3027797618e445` retains the raw job/log bindings.
Final tracking review, final-head CI, checked-tree merge/main CI and exact-head
branch cleanup remain pending. Their actual completion belongs in the final
external delivery receipt and the next tracked ledger reconciliation.

The independent completed execution audit accepts all17 scoped actual runs: the
original positive, six original assertion reds, eight final candidate positives,
the production marker mutant and unchanged replay. It rehashes complete source
and actual metadata/build/executable/cache/wait/fixture identities, reconstructs
the original Git tree and verifies the sole reversible production delta.

`independent-phase02-execution-review.md` SHA256 `9e23275560a783588c19fd05e7046d8dabe5a1630e586c7530fef0458d3063e6`.

`independent-phase02-execution-review-binding.json` SHA256 `7bec0bc49a7327103e1698af780d8a4c4307cbd3f651bbd5f789249b6788bfed`.

## Completed delivery reconciled by batch15

The preceding final/main-pending statements preserve their frozen checkpoint.
PR228 final `a7fb73d`/run36832672947 and equal-tree main `3345f129`/run36834154060
each passed all five CI jobs:764 Rust/0failed/1existingignored,355 frontend,72 Pi
MCP,5 simulation and73 virtualCAN (zero ignored in simulation/vcan). Main
completed the fatal ARM release build. Tree `d9d19dc7` matches the checked final
head. Verified all-refs bundle and exact leased remote/local branch cleanup
are complete. External batch14 merge-receipt SHA256
`ba61b17932aed774c5dca381cb1b6ef68ca3b7f6f95e482827344de4c34218c7`
and continuation handoff `a4cee98be3c6588219b4bfe599f472cd2dd988109cdb284d58ad3646073c9ec8`
record the completed delivery. G15 remains partial; no hardware acceptance.
