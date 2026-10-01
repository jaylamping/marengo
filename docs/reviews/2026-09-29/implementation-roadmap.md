# Marengo implementation roadmap

Started September 29, 2026 from main `52f12678277a2cae786d8df648f035895789f026`.
Active Windows checkout and local CAD: `J:\code\marengo`. All review/recovery
records remain under `J:\code`; no development checkout is moved into Ubuntu.

This is an implementation plan and progress ledger, not a claim that every
finding is repaired. The original review contained 100 IDs. Independent analytic
tests discovered **CS23**, and motion/stall tests discovered **CS24**, bringing
this plan to **102 IDs**. CS22/G11 and G10/F05
are explicit cross-layer overlaps. Tooling findings also intersect runtime work;
ID counts do not count independent defects.

## How to use the plan

The [machine-readable ledger](implementation-ledger.json) assigns every finding
one primary work package, current disposition, source and verification record.
The [finding index](finding-index.md) preserves the original evidence and full
recommended fixes. Detailed design and acceptance criteria are in the
[control plan](control-implementation-plan.md),
[gateway plan](gateway-implementation-plan.md),
[frontend/tooling plan](frontend-tooling-implementation-plan.md), and
[test-quality audit](test-quality-plan.md).

Status `verified` means the described software repair has direct evidence and
required checks. `partial` means a useful subproblem is repaired but the finding
stays open. `implemented` awaits the required integrated gate and review. `open`
means no implementation is complete. An overlap references
its canonical finding but must still be validated in every affected layer.
Hardware acceptance is tracked separately, never inferred from software tests.

Each repair batch follows this sequence:

1. State the observable invariant and the exact interface that owns it.
2. Reproduce the failure with the actual implementation and an independent
   expected outcome. Use bounded failures and deterministic clocks, not sleeps
   or source-string checks where behavior can be exercised.
3. Repair the smallest complete slice. When ownership is the problem, rewrite
   that module and migrate callers rather than add another flag or wrapper.
4. Run focused tests, inspect the diff, then run the required primary gate.
   Review safety/protocol changes independently. Record the red/green evidence,
   scope limits and any changed physical model output.
5. Update the ledger, retire superseded code/tests, and merge only a verified
   change. Pick the next dependency-ready package. Do not accumulate unrelated
   fixes in a long-lived branch or close issues from documentation alone.

## Repair order and dependency graph

Work packages are delivery slices, not a proposal to create seventeen new crates.
Subdivide a package into reviewable PRs using its per-finding acceptance criteria.
Independent repairs can proceed together with disjoint edit ownership.

```mermaid
flowchart TD
  Tests[WP00: test contracts and gates] --> Safety[WP01/02: motor safety and output]
  Safety --> Stop[WP03: reliable stop lifecycle]
  Safety --> Reference[WP04: reference verification]
  Stop --> Motion[WP05: Pi-owned motion]
  Reference --> Motion
  Stop --> Config[WP06: config and management authority]
  Reference --> Config
  Fresh[WP07/08: bounded truthful telemetry] --> Motion
  Fresh --> Config
  Security[WP10: mutation security] --> Motion
  Security --> Config
  Config --> Deploy[WP11: deploy and SSH contracts]
  Motion --> Learn[WP14: Auto Learn admission]
  Model[WP16: independent dynamics and plant] --> Bench[Separate physical acceptance]
  Motion --> Bench
  Config --> Bench
  Safety --> Bench
```

Logging/storage (WP09), portable builds (WP12), dependencies/TLS/render lifecycle
(WP13), and research repair (WP15) can proceed without waiting for motion design.
All software packages require meaningful tests from WP00; that package is a
continuing quality rule rather than a blocker requiring a giant test rewrite.

| Package | Findings | Change and completion contract | Prerequisites |
|---|---|---|---|
| WP00 Test contracts and gates | T21, T22, T26 | Replace false source-shape audit alarms and invalid scanner flags; execute Python, shell, Compound and advisory checks in the appropriate gate. Classify tests by independent contract and measure compile/install versus execution. Failed scanners must report failure, not success or an invented vulnerability. | None; applies throughout |
| WP01 Feedback, validity and faults | CS01, CS03, CS04, CS12, CS13, CS15 | Davout accepts a validated policy, rejects nonfinite/negative-gain input, expires each active drive from real decoded RX, and latches complete vendor/runtime faults. Empty drains, unknown traffic or another motor cannot refresh a silent drive. Invalid batches produce no motion frames. Fault/reset/enable are explicit. | WP00 |
| WP02 Legal output and limits | CS02, CS10, CS11, CS14, CS16 | Hard cap wins after every filter; limiter state has a defined enable/disable lifecycle. Define and test total PD+FF torque admission, measured-pose limit edits and typed danger responses. Preserve existing ceilings and gravity-support requirements. Verify drive-local limits separately. | WP01 for full admission; cap slice independent |
| WP03 Stop lifecycle | CS07, CS08, CS09, F07, T05, T12 | One owner attempts all disables, reports per-drive failures/uncertainty, and stops before persistence waits. First software E-stop activation reaches the runtime and latches; old CLI/deploy process kill paths cannot create a second CAN owner. Stop helper success requires observed termination. | WP01/02, WP10 |
| WP04 Reference transactions | CS05, CS06, F14, F23 | Bind calibration to hardware/config and current boot; Set Zero awaits postcommand evidence and yields a verified receipt or timeout. Reference changes invalidate taught poses. A queued response cannot be shown as Applied. | WP01, WP03 |
| WP05 MotionSession | CS19, CS20, F01-F04, F11-F13 | Replace browser-owned live playback and one-shot local mode changes with Pi-owned sessions, lease/boot/run generation and acknowledged completion/cancel. Disable invalidates old commands before returning; gain-only updates cannot retarget. Preview execution has its own deterministic completion and cannot switch to live in-place. Wave gates remain closed until physical acceptance. | WP01-04, WP07, WP10 |
| WP06 ConfigAuthority | G04-G06, G08-G09, G21, F16, F22, T02-T03 | One writer owns complete live/durable generations and all waiters. Failed persistence remains degraded; unknown/stale state refuses management. CAS prevents competing promotions; precommit validation plus recoverable receipts distinguish committed from uncommitted failures. Deploy preservation must succeed before replacing/restarting. Local mirror is bounded and optional to Pi durability. | WP01, WP03/04, WP07, WP10 |
| WP07 Transport and truthful state | G01, G10, G20, F05-F06, F08-F10, F15, F17, F20 | Bounded cancelable IPC; producer boot/time/age and authoritative facets; supervised listeners; complete stream cleanup/reconnect; stable WebGL scene lifecycle; severity-aware decode and matching deployment job identity. An open gateway stream cannot imply a fresh Pi or successful unrelated job. | No dependency for resource fixes; identities coordinate WP05/06 |
| WP08 Sensors and diagnostics | CS17-CS18, G18-G19 | Original IMU sample age survives publication/reset; captured BNO085 batches retain report alignment. Parse documented CPU/network/mount fields and return unknown when collection fails. | None |
| WP09 Logs and archives | G02-G03, G13-G17, F18-F19, F24-F25 | Refillable tracing quota, deadlock-free retention, transactional schema/recovery, preserving artifact/date import, bounded worker/file/parse costs, real page/filter semantics, cancellation against wrong-session results and truthful time units. | None; bounded executor coordinated with WP07 |
| WP10 Security of mutations | G07, T01, T04, T06 | Shared fail-closed server auth, runtime credentials, origin validation and bounded request bodies; root-owned helper parents; typed command arguments without shell substitution. Use harmless local injection fixtures and foreign-origin loopback tests. | None; prerequisite for trustworthy clients |
| WP11 Deployment and SSH results | T07-T11, T13-T14 | Propagate structured exit/error/timeout outcomes; validate motion wait duration; build isolated pinned revisions without changing the operator checkout; distinguish source tree from installed runtime and resolve executable paths consistently. Execute scripts against fake SSH/systemd/file fixtures rather than compare strings. | WP03, WP06, WP10 |
| WP12 Host portability | CS22/G11, T15-T16 | Split portable bus/framing from the Unix adapter; explicit unsupported transport if a native backend is absent. Windows/macOS portable tests compile; Linux checks retain real SocketCAN/I2C. Mac shell/hash tools have supported prerequisites. Host source and CAD stay under J on Windows. | None; preserve wire/protocol behavior |
| WP13 Dependencies and lifetimes | G12, F21, T31-T32 | Renew TLS keys/fingerprint coherently; retain stable route tests; patch remaining tool advisories with compatible versions; handle Router major migration deliberately and prove affected behavior. Render lifecycle F15 is owned by WP07. | None; consult SDK instructions for Compound upgrades |
| WP14 Auto Learn | T23-T25 | Require complete finite live joint limits/context, predecessor stage evidence and continuous trajectory velocity bounds. No incomplete model or empty prior can bypass admission. Motion requests use the same runtime owner and fail closed. | WP01, WP05/06/07, WP10 |
| WP15 Research tooling | T17-T20 | Await all asynchronous cache-miss handlers, use the installed arXiv client contract, preserve explicit zero scrape count and real week/month recency. Mock only external network boundaries and check returned content. | None |
| WP16 Model and plant validation | CS21, CS23, CS24, T27-T30 | Immutable analytic gravity fixtures, correct COM point transforms, production URDF/MJCF provenance and independent cross-check; deterministic production-controller plant tests cover delay, dropout, saturation, stiction and cancellation. Reject incomplete pose input and unsupported model/export capabilities; scaffold commands cannot report success. | Analytic checks immediate; integrated plant after WP01/02/05 |

## Rewrite decisions

[ADR 0019](../../decisions/0019-runtime-authority-and-verification.md) records the
target ownership and migration rules. The detailed plans identify actual files
and public interfaces. The likely substantial rewrites are:

- Pi command/overlay lifecycle and the browser compound runner: one MotionSession
  replaces competing timers, implicit enables and transient flags.
- Pi limit persistence plus gateway import/management lifecycle: one
  ConfigAuthority replaces whole-request overwrites, parallel writers and global
  Pending/Degraded booleans.
- Davout admission/output/state internals: a validated private policy and explicit
  receive/fault/stop state replace invariants that rely on a caller's ordering.
- Chappe transport lifetime and bounded fanout, with portable framing independent
  of the Unix adapter; keep the protobuf protocol rather than invent a second bus.

Keep the checked Robstride codec, independent dynamics and useful PositionHold
contracts unless their invariants demand a replacement. Do not rewrite empty
planning/perception scaffolds to satisfy unrelated findings. PositionHold changes
need an independent plant; replaying the planner as measured feedback is not an
acceptance oracle.

For each rewrite, add the new owner at the existing seam, migrate every active
caller, demonstrate the old path is gone, and remove old private/helper tests
after their behavior is covered. One live CAN owner and one durable writer are
cutover conditions, not aspirations left for a later PR.

## Test validity and development cost

The previous strict gate reported 582 passing Rust executions (498 workspace
tests plus 84 repeated fixture tests), 355 frontend and 72 Pi-MCP tests. It
did not catch the identified watchdog/NaN/stop/ownership defects. Counts and
passing snapshots are not acceptance criteria. The test audit supplies exact
keep/replace/consolidate candidates; tests are not deleted just for being small.

The previous cached Docker check spent 9.78 s compiling Rust tests and about
1.40 s summed in Rust test binaries; frontend Vitest took 10.45 s. Dependency
installation, browser module import/DOM setup, bundling and cross-compilation
cost more than most assertions. A fresh native Windows Vitest run passed
355/355 but took about 68.72 s to its final file versus 7.20 s summed assertion
time. This single observation does not establish a disk or operating-system
cause. Benchmark the same environment/cache before claiming a speedup.

Use three verification levels:

1. **Edit loop:** affected public-interface regressions and formatter/type checks.
   Deterministic clocks and event completion replace wall-clock sleeps. Pure
   frontend tests use a Node environment; DOM tests cover actual interaction.
2. **PR gate:** required `just check`, meaningful Python/shell/Compound checks,
   protocol compatibility and Linux vCAN/sim gates when their paths change.
   Preserve compile/build/lint/security checks; do not label their cost as tests.
3. **Model/runtime acceptance:** independent plant and production-model parity,
   then separately supervised physical commissioning under the locked playbook.

Initial targets are a warm focused edit loop under 10 s and stable regression
test bodies under 2 s per ordinary suite. These are planning budgets, not measured
guarantees. Large-file/outage/concurrency tests use bounded inputs/deadlines and
run at the PR/integration tier. Do targeted mutation checks for high-consequence
invariants instead of imposing full-workspace mutation testing on every edit.

## First batch and evidence

Review and delivery: [PR213](https://github.com/jaylamping/marengo/pull/213).

The first batch implements CS02/CS11, G02/G03 and the newly discovered CS23.
CS21 is **partial**: its eight ignored stale golden/private-cache checks are
replaced by four active analytic public-interface tests; current-master
independent model/plant validation remains required.

The full gate also exposed two legacy Berthier tests that assumed
`sign(tau_g) == sign(q)` for a laterally offset archived arm. They are replaced by
one geometry-specific public dynamics test with independently derived mass
moments. Its torque at q=0 is -1.18701 Nm, not zero. This retains meaningful
historical-fixture coverage while removing the misleading general sign rule.

- Torque regressions exercise real Supervisor/MemoryBus wire output in both
  polarities, lowered caps, first activation and disable/re-enable. Original
  failures include 7.4 Nm output under a 5 Nm cap and stale limiter replay.
- Log quota tests publish through the real tracing layer/Bus using a controllable
  monotonic clock. Original code forwards zero events after the first exhausted
  window; the repaired window forwards the next admitted events.
- Retention uses a bounded child process to reproduce the old deadlock, then
  verifies public Store state and artifact outcomes after expiry.
- Analytic two-link gravity tests fail on the original COM-vector calculation:
  expected shoulder holding torque 46.5975 Nm, actual 17.1675 Nm. COMs are now
  transformed as points, including joint-origin translations. Single pendulum,
  coupled links, rotated origin and configured joint order are independent
  oracles. This changes production gravity output; old bench results must not
  serve as acceptance of the corrected calculation.

Raw red/green logs and test-timing JSON are preserved under
`J:\code\marengo-migration-backup-20260929`; portable acceptance descriptions
and test sources live in the repository. The ledger records the successful strict gate and two independent reviews:
**507 workspace Rust tests passed, 1 ignored; 355 frontend and 72 Pi-MCP tests passed**.
Format, clippy, proto/build, cargo deny/audit and aarch64 release checks pass.
The local full check additionally repeated 84 fixture tests and the same ignored
kinematics placeholder. The primary script no longer invokes that redundant
precheck; the complete workspace fixture coverage remains. Required GitHub PR
checks validate this final script before merge. Linux vCAN runs separately with
its transport feature. None of the analytic gravity tests are ignored.

## Second batch: feedback and numeric admission

The [second batch report](batch02-feedback-command-validity.md) records the
CS01/CS03 receive-time and command admission repair, with CS04/CS14/CS15 still
partial, delivered in [PR214](https://github.com/jaylamping/marengo/pull/214).
Real baseline regressions catch empty drains, silent peers, NaN becoming
the RS03 -60 Nm wire endpoint, invalid policy, overlay/gain mutation, nonneutral
startup output, re-enable between ticks, queued enable status and taught ranges
excluding zero. Independent review and the primary gate pass: 547 workspace
Rust tests (one ignored), 355 frontend and 72 Pi MCP tests. Required GitHub
checks remain recorded separately before merge. Cargo deny's yanked-crate
registry coverage warns and remains unknown; T26 records the gap.

Four shallow/unused driver tests are replaced by public raw-wire and failure
matrices. Existing controller replays now supply status for stationary enabled
peers; their meaningful planner/stall assertions remain. This batch does not
complete fault latching, stop confirmation, reference verification or config
generation ownership. Docker's stale runtime socket error recurred and the
preserving recovery passed; M08 retains the unexplained initiating exit and
upstream Windows socket dependency.

## Third batch: persistent fault evidence and truthful stop

The [third batch report](batch03-fault-authority.md) records ordered driver
evidence, Davout's private fault authority, all-address stop outcomes and
Berthier error/intent handling in [PR215](https://github.com/jaylamping/marengo/pull/215). CS08/CS12 are verified software repairs;
CS04/CS13 remain partial. Primary and independent reviews pass: 598 Rust tests
(one ignored), 355 frontend, 72 Pi MCP, strict lint/build and aarch64 release.
Simulation's five tests and minimal engine smoke pass; they do not qualify the
production plant. Docker Desktop's current kernel lacks virtual CAN, so that
gate ran on GitHub Linux: both virtual CAN tests pass, including the new lossless
report test in zero/positive budget modes. All five GitHub checks pass at the
final head `e1f095d`; PR215 merged as `d50d0a6`.

Three driver and fifteen Supervisor regressions fail on the unchanged baseline;
ten controller regressions catch hidden errors, early intent installation and
rearm/replay. Review found and fixed a new false-overspeed fault for valid queued
traffic, preserving its positive public guard alongside hazard tests. Two weak
fault/TLS tests are replaced with raw-domain/order and real chain/key/failure
contracts. The remaining legacy test-quality audit stays open.

Dependency scanning now requires current database/index fetch and treats
incomplete coverage as a failure. The final primary scan has zero index
failures; the newly exposed unmaintained PEM dependency is removed through the
maintained parser and axum-server 0.8. Paste remains M01. T26 still requires its
other behavior suites and production-tool audits. Next is frame-envelope
integrity and bounded fair receive/enable flush work, followed by reference and
recovery transactions; neither host dequeue time nor process reconstruction is
physical recovery evidence.

## Fourth batch: frame integrity and bounded ingress

The [fourth batch report](batch04-bounded-can-ingress.md) starts from PR215's
checked merge `d50d0a6`. [ADR0021](../../decisions/0021-bounded-can-ingress.md)
defines separate receive class/length evidence, a required nonblocking backend
primitive, one finite total work budget, rotating interface fairness and honest
flush completion. Public baseline failures and real Linux SocketCAN regressions
precede candidate qualification. Local primary passes with 630 Rust tests (one
ignored), 355 frontend, 72 Pi-MCP and eight virtual-setup contracts. The affected
Linux feature check passes 373 tests, including five actual converter contracts;
GitHub then passes all 73 driver feature tests with none ignored, including all
six virtual tests. All five jobs pass at implementation head 6d5cedf,
run36684361659, and final evidence head c9d748e, run36685234071. Simulation smoke
and two independent reviews pass. [PR217](https://github.com/jaylamping/marengo/pull/217)
merged as 6ff2ed8 with exactly the checked tree; all five postmerge main jobs pass
in run36685765511. The ledger's batch04 history records the delivery receipt.
CS04, M06 and T26 remain partial; no hardware acceptance is inferred.
Reference/recovery follows the checked merge of this bounded transport slice.

## Fifth batch: historical calibration and current reference

The [fifth batch report](batch05-reference-history-admission.md) starts from the
checked PR217 merge `6ff2ed8`. [ADR0022](../../decisions/0022-calibration-history-and-current-reference.md)
separates inspectable historical rows from current-process readiness and moves
environment selection to Supervisor composition. Four existing-public regression
groups fail before production edits on that exact baseline; the missing-file
control passes and 56 relevant production files remain unchanged.

The original failing tests pass unchanged against a separate frozen candidate
snapshot. Every fresh registry now starts Unhomed, preserves rows/bytes,
distinguishes missing from corrupt/unreadable history, and refuses checked
home/normal Enable from historical data. Unique explicit-path fixtures replace
shared PID paths and parent environment mutation. Local primary passes 639 Rust
tests (one ignored), 355 frontend, 72 Pi-MCP and the script/dependency contracts;
affected Linux strict lint and 338 tests pass, along with simulation smoke.
All five implementation-head jobs pass at 8c3f62e in run 36691835812, including 73
actual driver feature tests with none ignored. Delivery is
[PR218](https://github.com/jaylamping/marengo/pull/218), merged as `c3068c0` with
the exact checked tree. All five final-head jobs pass at `0c17484` in run
36692769875; all five postmerge main jobs pass in run36693487593. The batch05
ledger history records that delivery receipt. CS05 is partial until private grants and
qualified reference/recovery transactions replace the remaining bypasses.
The fresh motor-repl stop path's dependency on constructor success is explicit
CS07/CS09 caller work; this is not a qualified emergency-stop CLI or a live
commissioning release. No robot action or physical acceptance occurs here.

## Sixth batch: private reference admission

The next slice starts from checked merge `c3068c0` under
[ADR0023](../../decisions/0023-private-current-reference-authority.md).
Actual unchanged-public baseline tests expose Unhomed scoped Enable and
NaN/Hall/None history verification. A further scalar matrix executes 22 real
assertion failures plus one finite-history control against 1,414 unchanged
original files; the identical matrix passes after scalar validation.

Private output permission, truthful unqualified reference refusal and closed
virtual fixtures pass final local qualification. Actual old-public grant and
cached/peer-arming reds are preserved; unchanged public/scalar replay and meaningful
transport/private-factory isolation pass. Review discovers and fixes atomic model
restore, with two exact-c306 old-public and two candidate assertion reds repaired.
Final primary passes 689 Rust/1 existing ignored,355 frontend and 72 Pi-tool tests
with fatal main cross-build; affected Linux 388/0 ignored and minimal simulation 5
pass. Standards has zero findings; Spec's one issue is resolved.
[PR219](https://github.com/jaylamping/marengo/pull/219)'s implementation head
`7951f10` passes all five GitHub jobs in run36706934339, including 73 actual
virtual-CAN driver tests, zero ignored. Final evidence-head `792fb894` passes
all five jobs in run36708482153; checked merge `eae4fc3` passes all five
postmerge main jobs in run36709130567, including fatal aarch64 release, with
the identical checked tree. The batch06 ledger history reconciles the external
merge receipt and verified branch backup/cleanup. CS05/CS06/CS07 remain partial;
qualified target-only transactions and installed-owner clients are next.

The independent law probe adds CS24 open: a positive EMA residue resets the stall
fuse after real motion stops. Two receive-invariant mutants are killed, but this
separate control algorithm is unchanged. There are 102 traceable IDs:12 verified,
10 partial,80 open. Hardware and whole-controller model/plant acceptance are
explicit remaining work; no robot operation or Wave/limit change occurs.

## Seventh batch: stop before persistence

The next dependency-ready slice starts from checked `eae4fc3` under
[ADR0024](../../decisions/0024-stop-before-persistence-shutdown.md). It prioritizes
CS09's stop-before-storage lifecycle ahead of the bounded reference engine.
Six actual regressions now have byte-identical immediate repaired replays:
stop-before-storage, retained work after owner exit, publication-before-idle,
closed owner admission, Quit and external owner-flag changes preventing later
dispatch/ticks. New-interface
drain conformance proves truthful timeout and actual worker-unwind outcomes.
Failed/skipped stop and Active intent cleanup qualify independently. Required
primary passes701 Rust/1 existing ignored,355 frontend,72 Pi MCP and fatal aarch64
release; strict affected205/0 and minimal simulation5/0 also pass. CS09's software
slice is verified in [PR221](https://github.com/jaylamping/marengo/pull/221):
all five implementation-head jobs pass at `3939b3d` in run36728845164, including
73 actual virtual-CAN driver tests with none ignored. Final87e6df1/run36730073787
and equal-tree merged1518176/main36731139955 each pass all five jobs, including
the fatal main release cross-build. The branch has verified backup/cleanup.
There are13 verified,10 partial
and79 open findings across102 IDs. The
[batch report](batch07-stop-before-persistence.md) tracks independent parity
extraction, one-at-a-time actual behavioral proof, worker outcomes and required
delivery checks. Bounded target-only reference acquisition/cancel/cleanup follows
checked delivery; it must remain explicitly unusable until qualified durable
commit, with unsupported physical capability refused before any energizing work.

## Eighth batch: historical write consistency

The [eighth batch](batch08-history-write-consistency.md) starts from checked
PR221 merge1518176. An additional CS05 history-consistency failure is reproduced
through the actual public registry: real write failure leaves an uncommitted
row in memory. All1,432 original Git blobs are verified; real successful-write,
retry and reconstruction controls run before the named failing assertion.
The writer stages the record, writes it, then publishes memory/local state.
Both insertion and existing-row replacement probes pass byte-identically after
failing original production. Independent reviews accept the scoped repair;
primary703Rust/1 existing ignored,355 frontend,72 Pi MCP and fatalcrossbuild,
plus strict affected420/0 pass. [PR222](https://github.com/jaylamping/marengo/pull/222)
passes all five jobs at implementation head `d541bd5` in run36736794717,
including 73 actual virtual-CAN driver tests with none ignored and 5 simulation
tests. Final `7ddc243`/run36738071460 and equal-tree merged `04e4ea2`/
main36738856051 each pass all five jobs, including the fatal main release
cross-build. Verified backup and exact branch cleanup are complete; the ledger
history reconciles the external merge receipt.
This does not provide crash-safe storage, a reference journal or current Davout
permission; CS05 stays partial and the102 finding dispositions remain unchanged.

## Ninth batch: measured ascent progress

[Batch09](batch09-measured-stall-progress.md) repairs CS24 from checked04e4ea2.
The geometric high-water watchdog is independent of velocity/planner recovery,
watches only commanded unresolved ascent, preserves installed decoded-grid
metadata through clear, and accumulates validated nominal Duration. All six
actual original law/public-controller failures pass as byte-identical replays;
the original raw crawl already passed and remains positive. Five new-interface
numeric/period cases pass; two isolated production mutants are caught. Primary
715Rust/1existingignored,355frontend,72PiMCP and fatalARMrelease plus strict
affected432/0 pass. Independent reviews reverify all1042 source inputs; one
docs-only guidance correction follows the executed gates, with1041 other inputs
unchanged and all original bindings preserved. [PR223](https://github.com/jaylamping/marengo/pull/223)
implementationeb1359c/run36755369469 passes all five jobs, including5simulation
and73actual driver tests. Final97881d8/run36756367932 and equal-tree merged7a25bbb/main36757300231
also pass all five jobs and fatal main release; verified backup and cleanup complete.
Counts:14verified,10partial,78open.
Bounded target-only reference R2a remains next; physical acceptance stays separate.

## Tenth batch: bounded virtual reference acquisition

[Batch10](batch10-reference-acquisition-core.md) implements ADR0026 R2a from
checked7a25bbb: private target-only acquisition, exact raw-pop correlation,
bounded receive/deadlines, immutable cleanup, controller ownership and mandatory
Pi stop before storage. Seven candidate regressions pass unchanged; the frozen
positive and five selected production mutants qualify the new tests. Strict
affected453/0 and independent Standards/Spec pass. Primary: **736 Rust tests passed, one existing ignored; 355 frontend and 72 Pi MCP passed**, with fatal ARM release, format/lint/proto/build/deny/audit checks.
PR224 implementation18f281c/run36773511504 passes all five jobs, including73 actual virtual-CAN tests with zero ignored. Final8cd720c/run36774797894 and equal-tree merged1682104/main36775757864 also pass all five jobs, including fatal main ARM release. Verified backup and exact branch cleanup are complete. CS05/CS06/CS07 remain partial;
R2b journal/grant and R3 installed clients follow. Counts stay14verified,
10partial,78open; no physical acceptance, limits or Wave change.

## Eleventh batch: retained evidence and model continuity

[Batch11](batch11-reference-evidence-continuity.md) implements ADR0027 R2b0
from fully checked PR224 merge16821043. It retains actual accepted virtual proof,
checked bounded installed-model identity and exact-handle live continuity after
cleanup. Immutable terminals remain unusable. Original-public model-stamp red
passes unchanged; a six-fixture management candidate regression passes unchanged;
three production mutants are caught after real compiled positives. Independent
Standards/Spec, strict affected460/0, primary743Rust/1 existing ignored,
355frontend/72PiMCP/fatalARM and simulation5 pass. PR225 implementation6c10339/
run36784341442 passes all five jobs, including73 actual virtual-CAN tests/0ignored.
PR225 final2b8d19a/run36785717622 and equal-tree mergedbebc678/main36786310183 pass all five jobs, including fatal main ARM release. Verified all-refs backup and exact branch cleanup are complete. Counts stay14verified,10partial,78open across102IDs.
R2b1 recoverable journal/owner-consumed durable receipt, R2b2 current selected
grant and R3 installed clients follow; no physical acceptance or Wave/limit change.

## Twelfth batch: historical artifact preservation

[Batch12](batch12-session-artifact-preservation.md) repairs G13 from checked PR225
merge `bebc678`: sparse sibling/capture preservation, stale candump-statistics
invalidation, unique-session/artifact reporting and explicit clearing. Three
combined original assertion reds replay unchanged green; actual read-only failure,
writable retry, real CLI and one production mapping mutant qualify behavior.
Independent whole Standards/Spec accept all code/tests. Final affected16/0/0,
primary751Rust/1 existing ignored,355frontend/72PiMCP/fatalARM and simulation5 pass
on the unchanged1449-input source. PR226 implementatione885fee/run36807158080
passes all five jobs, including73 actual virtual-CAN cases/0ignored. Final and
equal-tree main delivery remain pending; counts15verified,10partial,77open.
The next G14 chronology proposal is external design only: establish its actual
unchanged-public red on the completed batch12 base before repair. Unknown-ID and
capture-end policy, actual profile/sidecar conventions and deterministic retention
need explicit qualification. R2b1 remains separate engineering design and does not
close reference findings.

## Completion and continuation

The active thread heartbeat **Marengo repair loop** continues every 30 minutes
from this ledger. Each run reconciles current Git state, completes the next
dependency-ready reviewed slice, records test evidence and updates status.
Follow-ups never operate or deploy to the physical robot. Pause/finish the loop
when software work is complete or a specific external decision blocks progress,
and report the remaining acceptance gates.

Software completion requires a disposition for all 102 IDs, all required callers
migrated, meaningful regression coverage and passing required checks. Do not
declare completion merely because P1s are fixed or an existing suite is green.
Retire dormant code only after checking its consumers and preserving required
behavior; a removed capability must report unsupported rather than pretend it
works.

The ledger also tracks architectural and maintenance tasks beyond numbered bugs:
unmaintained `paste`/`rustls-pemfile`, drive-local timeout/limit handshake,
E-stop/Hall input implementation or explicit refusal, wrong-sign frame policy,
unsupported/cyclic/invalid model rejection, bounded realtime work and model/CAD
provenance. These cannot disappear just because T32's exploitable advisories were
patched. Hardware execution and acceptance remain separate recorded requirements.

Physical release remains gated on fresh reference/sign checks, verified installed
drive torque limits/timeouts/fault semantics, physical E-stop/support behavior,
current CAD/URDF/MJCF consistency, and resumed limb-playbook execution. Issue
#170's 50% ladder retry and #176's live raise/elbow Wave smoke remain incomplete.
Do not change `WAVE_POSE_GCOMP_SIGNED` or close physical acceptance issues from
offline tests. Software corrections, especially CS23, require a fresh model and
commissioning assessment.
