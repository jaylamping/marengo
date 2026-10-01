# Test quality and build-cost remediation plan

This plan audits the test strategy against the [review findings](finding-index.md), not against a target test count. The September 29 maintenance baseline is `52f12678277a2cae786d8df648f035895789f026`; measurements below precede the new remediation changes. Passing the existing suites does not establish safe motor operation. Existing tests miss confirmed defects in watchdog freshness, torque caps, stop outcomes, reference validity, persistence, and browser cancellation.

The decision is to retain fast independent behavioral tests, replace tests that assert implementation details or unsafe behavior, consolidate repeated presentation checks, and add a small number of boundary integration tests. We should not rewrite crates merely to reduce test counts. A test earns its place by failing when its stated contract is broken.

The first new analytic two-link test immediately exposed another production defect, **CS23**: gravity COM transformation used `Isometry * Vector3`, which rotates the COM offset but omits the upstream joint translation. The independent fixture expected 46.5975 Nm while the old implementation returned 17.1675 Nm. Root remediation is changing that operation to point transformation and checking ordering/distal lever arms. This is direct evidence that stronger oracles catch material errors that length/nonzero/same-model checks miss; a passing old suite was insufficient.

First-batch integration also retires the two Berthier `sign_cross_crate.rs`
checks whose archived-arm expectations omitted lateral joint offsets and assumed
that torque always has the angle's sign. One active public dynamics test in
`archived_arm_geometry.rs` replaces them with independently derived mass moments
and exact special-angle torques. It is historical-fixture coverage, not a current
robot acceptance test. The four immutable analytic tests additionally reject all
four isolated mutants: dropped translation, reversed gravity, ignored link mass
and omitted joint rotation. Mutation results are preserved in the local recovery
directory under `dynamics-mutation/results.json`.

## What currently costs time

The preceding strict container check is recorded in `J:\code\marengo-migration-backup-20260929\strict-check.log`. It reported 582 passing Rust test executions: **498 workspace tests plus 84 repeated fixture tests**, alongside 355 Consul and 72 Pi MCP tests. The workspace had nine ignored cases; the precheck repeated one of them, producing ten ignored executions. Counts must distinguish tests from duplicate executions and independent safety requirements. The first repair batch removes the redundant fixture invocation from the primary check; standalone `validate-urdf.sh` remains available. The full workspace suite still executes the same kinematics, sim-harness and config tests.

| Stage | Recorded duration | Interpretation |
|---|---:|---|
| Consul `npm ci` | 10 s | Package installation, not test execution. |
| Pi MCP `npm ci` | 14 s | Package installation, not handler assertions. |
| Three small Node-tool installs | 1 s each | Separate package roots repeat install/startup overhead. |
| Consul Vite production bundle | 4.93 s | Bundle phase; this does not include a separately measured TypeScript phase. |
| Consul Vitest | 10.45 s wall | 69 files; 355 tests. Aggregate test-body time 6.77 s. |
| Consul transform / import / environment | 156.12 / 197.12 / 40.57 s aggregate | Parallel worker totals overlap and must **not** be added to the 10.45 s wall time. They identify substantial setup/import work. |
| Pi MCP Node tests | 619 ms wall | 72 cases; reducing these assertions is unlikely to materially accelerate the full gate. |
| Rust clippy build | 13.06 s | Warm compile/check phase, not test execution. |
| Rust test compile/link | 9.78 s | Warm test-profile build. |
| Rust test binaries | 1.40 s summed reported time | Sum of `finished in` values after the workspace test build. Subprocess startup is not separately recorded. |
| Aarch64 release build | 24.29 s | Separate target/profile compilation. |
| Fixture precheck | 0.10 s summed bodies | `validate-urdf.sh` reruns kinematics, sim-harness, and config tests before the workspace suite; redundant subprocess/build work, not a distinct validation contract. |

On September 29 the read-only follow-up ran native Windows Consul tests with a JSON reporter, writing `frontend-test-timings-20260929.json` into that backup directory. All 355 tests passed in 69 files. The report start to the last file completion was **68.72 s**, while summed individual test bodies were **7.20 s**. JSON's 187 total suites includes nested `describe` blocks; it is not 187 source files. This is one native run compared with a prior warm container run. Cache, filesystem, worker scheduling, compilation, and host differences prevent assigning the gap to `J:` alone.

The slowest native test-body groups were Telemetry page 1.084 s (4 cases), Telemetry overview 0.898 s (3), Hardware overview 0.728 s (14), Set Limits panel 0.549 s (8), Inventory shell 0.451 s (11), and restart dialog 0.358 s (5). The Telemetry page intentionally imports a lazy route; deleting useful route behavior to save its import cost would be the wrong repair.

Only **4 of 69 Consul files** declare the Node environment; 37 declare jsdom and 28 inherit the global jsdom default. Twenty-seven `.ts` files without an explicit environment include pure geometry/order, trajectory, calibration, commissioning, facet, query-key, and teach helpers. Check each file's browser dependencies, then run pure suites in Node. Fetch, AbortController, and many mocked network contracts are available on the pinned Node 24 host without jsdom. Store tests that require localStorage need an explicit storage adapter or a retained browser environment.

No coverage or mutation gate is configured. No stable cold-build or incremental-edit benchmark exists. Keep that absence explicit; the logs do not prove how much of a clean CI run is Rust compilation or npm download time.

## Concrete test decisions

The classifications below are deliberately at file or test level. Mixed suites keep their useful assertions. REMOVE means replace the lost behavioral protection first, then remove the obsolete assertion; it is not permission to delete whole suites indiscriminately.

| Existing test or group | Decision | Why and replacement contract |
|---|---|---|
| `crates/berthier/src/mode_isolation.rs::impedance_tau_f_independent_of_tau_g` | **REPLACE, then REMOVE old property** | The property computes `tau_g + tau_f` twice inside the test and checks subtraction algebra; it never calls production impedance composition or a control tick. Exercise public control output with identical pose/config and independently varied dynamics, observing actual gravity and friction contributions across modes. A production mutation that drops gravity or couples friction must fail it. |
| Same file, `position_non_gravity_ff_independent_of_tau_g` | **KEEP and strengthen** | It calls production `compose_position_hold_feedforward` and observes the returned terms. Add public mode-transition/output coverage so helper correctness is connected to actual dispatch. |
| `crates/armee-dynamics/tests/golden_tau_g.rs` and private `urdf_gravity.rs::link_chains_built_correctly` | **REPLACE** | Eight pure cases are ignored; seven failed when explicitly run. Archived model names and hand-maintained bench numbers drifted. Use immutable one- and two-link URDF fixtures with hand-derived gravity equations, public `DynamicsModel`, a declared frame/sign convention and numerical tolerance. Root remediation is implementing this first; production five-joint model/provenance validation remains a separate CS21/T28 acceptance requirement. Never copy the implementation's new output into expected values. |
| Existing production dynamics tests asserting length, nonzero load, or change with pose | **KEEP as smoke, add oracle tests** | Useful input/shape checks, but a wrong sign or scale can still pass. Pair with independent analytic and URDF/MJCF reference checks. |
| `bins/marengo-gateway/src/restart.rs::restart_allows_active_with_stale_heartbeat_via_stub` | **REPLACE** | It explicitly rewards the unsafe G06 behavior. Missing or stale runtime authorization must refuse restart even when the last mode is Active. A temporary local helper must remain uninvoked. Test Pending, Degraded, stale boot, and CAS generation mismatches too. |
| `crates/davout/src/lib.rs` MemoryBus/filter/transform tests | **KEEP; correct gaps** | Tests observe production supervisor output, including wire frames and joint/motor transforms. Add the reproduced empty-drain, silent-peer, nonfinite, over-cap slew, failed disable and fault-latch transitions. Assert no MIT transmit on rejection, not merely an error variant. |
| Same file, `comm_watchdog_unchanged_despite_larger_poll_budget` | **REPLACE timing mechanism** | Sleeps 10 + 45 ms and bypasses the normal repeated feedback-refresh trigger that exposed CS01. An injected monotonic clock must exercise repeated empty drains and exact expiry boundaries without scheduler timing dependence. |
| `crates/robstride` encode/decode unit tests and ignored vCAN routing | **KEEP and strengthen** | Unit tests observe real bytes; vCAN exercises the production OS transport. Add protocol fixtures independent of `encode_*`, full-width fault bits and nonfinite rejection. An encode/decode round trip alone can hide paired encoder/decoder mistakes. Ignoring vCAN in the default portable suite is legitimate; Linux CI must still run it. |
| `consul/src/lib/compound-runner.test.ts`, `teach-record.test.ts`, `teach-transit.test.ts` | **KEEP pure contracts; replace obsolete runner responsibility** | Several cases check real timing, limits, missing joints, fingerprints and materialized poses. They do not exercise live ownership, Disable races, switching dry-run during playback, or reference changes. Move execution ownership to Pi MotionSession; retain preview/math tests and add browser-to-runtime transition contracts. |
| `consul/src/components/dashboard/testing/__tests__/testing-overview.test.tsx` | **KEEP small route controls; REPLACE motion confidence** | Three cases primarily check which controls are present. They do not click Hold Stop or prove immediate E-stop transmission/failure visibility. Add real user events with deferred request completion and fake time; assert one stop at first activation and no post-stop command. |
| `consul/src/components/dashboard/hardware/__tests__/hardware-overview.test.tsx` | **KEEP useful admission/status cases** | Fresh Active blocking, unknown completeness and wire-facet priority are useful. Add stale producer/boot state, delayed activation receipt, newer generation overriding local Range, scene stability and refused Set Zero. Broad mocking of API success must not be the only acceptance evidence. |
| `consul/src/components/dashboard/inventory/__tests__/set-limits-panel.test.tsx` | **KEEP and extend** | It tests input workflow and failed saves. Add queued-zero refusal/timeout/Verified ACK, incomplete fresh pose, and Pi Durable followed by a permanently stalled optional local mirror. |
| `consul/src/lib/persist-joint-limits.test.ts` | **KEEP and extend** | Existing failure and patch timeout cases observe useful contracts. The local mirror success/reject tests omit a promise that never settles; add bounded mirror status without withholding Pi Durable. Replace expectations derived through the same production inset helper with at least one independently calculated bound. |
| `consul/src/hooks/__tests__/use-motor-status-poll.test.ts` | **KEEP** | Observable poll/backoff/unmount behavior with fake timers is valuable. Add in-flight cancellation/stale result coverage where the implementation changes. |
| `consul/src/hooks/__tests__/use-active-reporting-lease.test.ts` | **KEEP but do not confuse with motion lease** | It checks active-reporting acquire/release calls, not exclusive motion ownership. Add renew failure/expiry and StrictMode delayed acquire; use a separate runtime MotionSession lease test. |
| `dashboard-card-shell.test.tsx`, `chart-section-skeleton.test.tsx`, shell constants in `dashboard-layout.test.tsx`, `logs-panel-shell.test.tsx`, `sidebar-panel-shell.test.tsx`, `inventory-panel-shell.test.tsx`, `sim-panel-shells.test.tsx` | **CONSOLIDATE; REMOVE duplicate class assertions** | Repeated `bg-surface-1`, `border-line`, and `pointer-events-auto` checks mirror shared constants and jsdom cannot validate actual hit testing, stacking or canvas composition. Keep one small shared style contract if desired and one browser smoke proving visible actionable controls over the canvas. Preserve input, navigation, loading, disconnected-control and details behavior within these mixed files. |
| `testing-master-defaults.test.ts` and bringup-map assertions in `hardware-commissioning-ia.test.ts` | **CONSOLIDATE duplicated map checks** | Both inspect omitted four-DOF aliases/serialized constants. Keep one data-map contract and real route/read-only navigation checks. The four F21 route failures were legitimately repaired, not evidence that all route tests are useless. |
| `consul/src/components/ui/__tests__/base-ui-lint-gate.test.ts` | **MOVE to lint, then REMOVE Vitest source-string gate** | It scans raw imports/package JSON for an obsolete UI library. Enforce a current architectural import ban in an actual lint rule if the ban still matters; test the lint rule once. It is not a UI behavior test. |
| `tools/marengo-pi-mcp/test/logs.test.ts`, most command assertions in `motion.test.ts`, installed-helper string assertions in `restart-marengo-pi.test.ts` | **REPLACE string-only acceptance; KEEP schema cases** | Regexes can match text in invalid Bash or a branch that never runs. The log-list syntax error T13 survived this coverage. Generate scripts and run `bash -n`, then execute with PATH-confined fake SSH/systemctl/CAN binaries and temporary directories. Assert exit status, argv, output files and ownership outcomes. Never contact Pi or hardware. |
| `logs.test.ts::BENCH_LOG_KEEP_COUNT is 50` | **REMOVE after behavioral retention check** | A constant-equals-literal assertion locks an arbitrary value without proving retention. Create more than the configured retained sessions and verify the oldest are pruned while current fault artifacts remain. |
| `scripts/deploy-job-contract.test.sh` and `crates/marengo-deploy/tests/job_script_contract.rs` | **CONSOLIDATE, REPLACE generated-output proof** | Grep checks and hand-authored JSON fixtures can both stay green while scripts emit invalid/currently divergent output. Run actual enqueue/update scripts against fake process/build/install commands, parse the emitted JSON with production `DeployJob`, verify every phase/result and nonzero failure. Retain a compact enum backward-compatibility test for intentionally supported old ledgers. |
| `scripts/deploy-rev.test.sh`, executable dist/rebuild checks | **KEEP** | These execute actual helpers against temporary Git trees/files and catch stale installed revision or absent artifacts. Add immutable target SHA and failed installation/restart rollback assertions for T02/T11. |
| `scripts/test_preserve_taught_limits.py` | **KEEP and strengthen** | The temporary-files end-to-end case is valuable. Add missing YAML keys, malformed installed config, partial write and installer failure; assert full semantic preservation, failure status and backup retention, not just a returned list of restored names. |
| `scripts/test_analyze_position_trace.py` | **KEEP and gate** | Captured good/bad trace fixtures test real acceptance decisions, including jerk and missing segments. Assert a failing criterion cannot be relabeled pass by metadata/exit-code changes. |
| `scripts/daily-audit/test_audit.py` | **KEEP parser cases; REPLACE path-only success claims** | Regex/helper tests are useful for those helpers, but changing a checksum filename or an ADR path is not evidence that codegen and domain behavior match. Execute regeneration/diff on temporary repositories; include test-only Robstride references and legitimate generated changes. Scanner failure must be Unknown/Failed, never Clean. |
| Research MCP's two test files | **KEEP and add public handler tests** | Six helper/source tests do not invoke the six public cached handlers that fail on cache miss. Use fresh cache and injected async search/HTTP boundaries; exercise cache miss/hit, locked arxiv API shape, explicit scrape=0 and timestamp windows without network. |
| Compound `shared/asserts.test.ts` and `src/server.test.ts` | **KEEP; REPLACE copied schedule arithmetic and add adversarial inputs** | HTTP auth tests run a real local server with only the paid agent call mocked, which is a sound boundary. The schedule golden repeats the formula rather than checking a complete materialized command against independent velocity bounds. Add missing/duplicate joints, nonfinite/range errors, empty predecessor, stage identity and trajectory derivative violations. Use ephemeral ports instead of fixed ports. |
| `crates/sim-harness` string-count tests and `sim/scripts/smoke_test.py` | **REPLACE as production acceptance; retain minimal engine smoke** | Count of `type="hinge"`, existence and 500 arbitrary steps detect packaging problems but do not validate production physics, actuator presence or controller behavior. Parse the production plant, compare name/frame/axis/limit/mass/COM/inertia manifests, apply production Berthier→Davout commands through a simulated bus, and verify watchdog/caps/stops independent of the implementation. |
| `armee-kinematics` ignored placeholder/full-humanoid DOF test | **REPLACE obsolete premise** | Its reason refers to an old placeholder even though the current acceptance target is the five-joint bench arm. Use an explicit current production model manifest; keep full humanoid acceptance as future scope rather than a silently ignored current requirement. |

## High-value acceptance contracts

Every repaired finding gets a test tied to its ID and a short statement of the counterexample it catches. Several findings can share one integration scenario; duplicating the same assertion in three layers is unnecessary.

1. **Motor boundary:** malformed commands never produce a transmit frame; every active motor needs its own fresh sample; reported faults latch; final output always satisfies the configured envelope/cap, even after slew/state seeding; every stop write is attempted and failures remain visible. Use actual Davout/robstride and fake clock/bus, including device/interface collisions and unknown frames.
2. **Motion ownership:** accepted run generation/boot/owner identity controls execution. Disable/E-stop cancels the session before acknowledging; late commands cannot re-enable. Fresh explicit Enable is required. Hold Stop reaches Pi; tuning cannot change target/mode. Preview has a deterministic timeline and emits no live command. UI tests alone cannot enforce this server invariant.
3. **Configuration:** a full generation is validated, persisted and committed by one owner. Inject failure before/after each write/rename/receipt, restart with partial state, reorder requests and ACKs, and race import against taught-limit edits. All callers receive the correct generation outcome; unknown/quiescence failure blocks management operations.
4. **Transport and telemetry:** delayed subscriptions, StrictMode dispose, silent Pi while gateway remains open, reconnect into a new boot, and a blocked reader must leave bounded queues and truthful age/connection state. INFO floods preserve fault/stop events. Fake time avoids real multi-second sleeps.
5. **Operations and historical data:** temporary Store/SQLite plus captured artifacts must survive retention, partial migration, sibling imports, original UTC dates and paged faults after row 500. Temporary shell environments prove stop, failure exit, quoting and preservation without privileged host effects.
6. **Physics/model:** immutable public analytic fixtures catch gravity sign, magnitude and chain/COM transformation mistakes; an independently computed production model manifest ties current CAD export to URDF/MJCF. Real bench acceptance remains operator work; simulated green is not a physical safety sign-off.

Use targeted mutation checks on these boundaries: remove final clamp; refresh freshness on an empty drain; erase a fault; skip stop-write error; authorize stale restart; treat queued as verified; remove cancellation; replace transaction CAS with unconditional write. Each targeted mutant must make a corresponding acceptance test fail. Run a small documented selection, not an expensive full-repo mutation job on every edit. A mutation that never executes is a test/setup gap, not a survivor to waive automatically.

## Proposed gates and measurable budgets

Before changing timing defaults, collect three warm runs and one explicitly cold run on Windows/Docker and GitHub Linux. Record installation/download, codegen, TS check, Vite bundle, Vitest setup/import/assertions, Rust clippy build, test compile/link, test execution and cross-build separately. A normal feature edit should run the affected boundary first; the required merge gate still exercises all contracts.

| Gate | Required work | Proposed budget policy |
|---|---|---|
| Local targeted | Relevant pure contract and one integration boundary; no hardware | Warm tests should return in seconds. Investigate a new sleep or expensive dependency before accepting it. |
| Required portable contracts | Rust workspace tests, Consul Node/DOM projects, Pi/Compound tests, Python behavior and executed shell tests | No production tool excluded merely because it lives outside Cargo. Retain strict audits and codegen checks. Set numeric limits after comparable measurements, not from the single native/container comparison. |
| Required Linux transport | vCAN real transport/routing plus IPC lifetime tests | Separate from portable Windows/macOS tests; bounded test deadlines and clean teardown. |
| Required changed-model acceptance | Production manifest equivalence and controller/safety plant scenarios | Trigger on model/config/control/sim changes. Run engine smoke separately so it cannot stand in for acceptance. |
| Release/build | Full lint/build, advisory policy and aarch64 release artifact | Track build wall time independently; cache pinned dependencies/targets, avoid duplicate precheck suites, keep immutable source SHA. |
| Operator bench | Commissioning playbook and measured hardware reference/caps/stop outcome | Explicit operator session; never part of unattended local test execution. |

Split Consul tests into Node and DOM projects, with a minimal test-only transform configuration instead of production Tailwind/React-compiler work where not needed. Benchmark that separation before keeping it; production build remains a separate gate so tests cannot silently rely on a different application semantics. Consolidate shared shell/style cases to reduce imported UI graphs, preserve real route behavior, and retain a small browser interaction smoke for CSS/canvas hit testing that jsdom cannot establish.

`validate-urdf.sh` should become a real model validator or the duplicate workspace reruns should be removed after the equivalent production contract is required elsewhere. Do not optimize away schema compatibility, clippy, vCAN, or critical failure-path tests to make a headline runtime lower. The initial audit was read-only; implementation progress follows.

## Second batch implementation evidence

The [feedback/command batch](batch02-feedback-command-validity.md) retires the
unused standard-ID compatibility protocol and its two self-roundtrip tests,
plus two private helper/range checks. A public four-model byte fixture and
preflight failure matrices preserve meaningful encoding coverage. Tests prove
valid prefixes cannot transmit when a later field, identity or route is invalid.
Config matrices invoke real loaders/writers; overlay rejection checks live and
disk state; gain batches check existing state remains intact. Startup tests read
raw output and expire after two ticks, including re-enable between ticks and
taught hard ranges excluding zero.

Existing long controller replays were assuming that one motor's status refreshed
stationary peers. They now provide new stationary-peer observations each tick
while retaining all planner/stall acceptance assertions. Independent review
found an enable-write queue race and the taught-range startup gap that existing
tests missed; new public regressions fail before each repair and pass afterward.
The unchanged external queue probe independently confirms the enable repair.
CS04 fault-latch, CS12 discarded-error and CS13 recovery tests still need their
corresponding implementation; a green admission suite does not close them.

Batch03 now supplies full ordered raw domains, a persistent supervisor latch,
honest failed stop and controller propagation regressions. Three existing-driver
API and fifteen Supervisor cases fail the unchanged e830 baseline; ten
controller cases fail before repair and pass the final primary. New-interface
snapshot/report tests are conformance evidence, not baseline compile-failure
proof. A valid queued-pose case passed baseline, failed the intermediate
candidate and passed after repair; it is retained to prevent false safety trips.
Only the truncated private fault-word assertion and same-input TLS hash check
are retired here. Real PEM chain/key/malformed-material tests preserve loader
behavior. Primary has 598 Rust tests, 355 frontend and 72 Pi MCP passing; other
legacy test replacements, persistent Pi publication/recovery and production
plant acceptance remain required. Actual virtual CAN runs on Linux CI because
the local Docker Desktop kernel rejects vcan; it cannot be invented as a local
pass. Real scanner missing-index/database/refresh failures prove the repaired
coverage gate, alongside four fast CLI failure-propagation contracts.

Archived baseline and candidate sources must use separate Cargo target paths.
Reusing identical container paths/mtime-based outputs produced one stale compile
artifact in this batch; affected package artifacts were cleaned and the final
candidate is rebuilt for the strict gate. Compile/setup failures never count as
red regression proof. Exact assertion logs, suite timing and integrated counts
are preserved in the ledger and batch report; no full-workspace mutation job or
hardware tests are added to the edit loop.

## Sixth batch: reference isolation and independent controller inputs

The private admission slice uses a concrete closed finite SimulationBus through
the real Supervisor and controller factories. Initial virtual references are
declared starting conditions, not acquisition or commissioning evidence. Old
positive cap, freshness, fault, failed-stop and bootstrap contracts migrate their
setup while preserving typed failures and actual output assertions. Custom
post-send callbacks become finite transmit rules whose trigger counts and actual
MIT frames must be observed. Generic arbitrary-bus refusal probes retain an
external Arc recording witness so removing mutable bus access cannot invalidate
their byte-identical replay.

An additional scalar matrix executes 22 actual assertion failures and one
finite-history control on exact merged c306 with 1,414 original files unchanged.
The identical23 cases, plus the prior four scalar probes, pass in a separate
source-bound candidate snapshot. Unsupported Hall/None, malformed identities,
nonfinite/reversed bounds, position/offset/tolerance errors neither alter state
nor persist success. These validate history inputs, not physical reference.

Berthier replaces one arithmetic identity property with actual Impedance versus
GravityComp friction wire output. Two planner-echo replays now check independent
stationary raw input, small-move rate/lead bounds and reachable nonzero wire
output. A progress/reset scenario moves to actual PositionHold law inputs at
fixed5ms, removing2.5s of wall pacing; actual controller stationary-fault and
neutral-bootstrap/expiry cases remain. The intermediate Linux run passes 175
cases with zero ignored, about 0.68s total assertion runtime. Final integrated
qualification and exact-head delivery are recorded by the batch report/ledger.

Test migration independently discovered **CS24**: real encoder motion followed
by500 stationary samples does not trip the existing ascent fuse, because a
positive EMA tail continually counts as progress. An exact-c306 unchanged-law
probe executes one real failing assertion, zero ignored, with 51 unrelated cases
filtered. Correcting invented initial-pose jumps in stationary fixtures does not
repair this separate gap. CS24 remains open with its probe, source hashes and
required measured-progress/noise/plant regressions. No filter, fuse duration,
torque/velocity limit, physical operation or Wave sign-off changes in this slice.

Final qualification supersedes the intermediate run: primary 689 Rust/1 existing
ignored,355 frontend,72 Pi-tool cases; affected Linux 388/0 ignored; minimal sim 5.
Model restore review adds five independent contracts, including two old-public
exact-c306 assertion reds and two candidate reds/control repaired in byte-identical
frozen replay. Davout final 159 cases preserve 133 after 3 obsolete API cases retire
and add 26. Native 2+14 and Linux 2+4 meaningful type-isolation controls/denials pass;
the guessed never-existing owner-pairing name is excluded from required counts.
Two targeted receive-invariant mutants fail actual assertions. All proof kinds,
setup failures, source hashes and command/assertion timing remain separately
recorded; the ledger/report track exact-head delivery.

## Seventh batch: real shutdown and storage contracts

[Batch07](batch07-stop-before-persistence.md) supplies six actual assertion-red
and byte-identical immediate repaired replays. Each archive proves source
identity and compiles the actual extracted/candidate owner and worker. Extraction
and retained pre-cutover code are explicitly separate from unchanged original
binary execution. Five original regression function bodies also remain identical
in final source. Compile/setup/lint failures and classifier corrections are
excluded from behavioral evidence; new drain API timeout/unwind cases are
conformance, not missing-API reds.

The private pending-count bound, chmod-plus-sleep write failure and delayed limits
snapshot poll are replaced with actual first/latest disk writes and matching
events, valid live admission followed by a regular-file parent blocker, and
immediate actual snapshot decoding. The obsolete getter is removed. Failed-stop
matrices preserve all fifteen attempts and the exact initiating report across
real Durable, Failed and TimedOut storage. Skipped policy and reachable closed
virtual Active gain/torque/Wave cleanup remain distinct from physical acceptance.
An independent review caught a test gap: Quit alone could miss removed external
flag checks. The additional actual-runtime matrix raises the shared flag during
a literal admitted stop, retains later commands and prevents later ticks; the
same probe fails retained old dispatch and passes guarded dispatch.

Final strict affected checks pass205 tests, including30 Pi tests in0.52 s body
time; full gate passes701 Rust/1 existing ignored,355 frontend,72 Pi MCP and the
fatal aarch64 release build in101.788 s. Minimal simulation passes5/0. No new
ignored tests or multi-second negative-sleep oracle is introduced. These warm
results separate test execution from compile/install work; they do not establish
a cold-build improvement or physical stop, OS signal delivery or plant behavior.

## Eighth batch: real failed historical writes

Two original-public tests fail unchanged1518176 before the staged-record repair,
then pass with entire test bytes preserved. They distinguish failed insertion
from replacing existing target history, preserving unrelated literal rows and
local state/flags. Real typed filesystem errors, successful-write/retry/reload
controls and cleanup execute before each decisive memory assertion. Regular-file
parent blockers and exclusive guarded directory moves avoid permissions/sleep
assumptions. Missing protoc is recorded as a zero-test setup failure.

Bodies take0.01–0.02 s in native replay; compilation is recorded separately.
No ignored tests or hardware dependencies are introduced. These tests qualify
memory publication after write success; partial writes, crash durability,
reference journaling and current drive permission remain separate acceptance.

## Twelfth batch: session preservation and real refusal

[Batch12](batch12-session-artifact-preservation.md) adds eight public behavior
groups: three whole original-public regressions, three new API conformance groups
and two actual CLI groups. Three combined original assertion reds replay green
without editing their complete tests. Missing siblings arrive later on disk,
literal files are independently decoded, capture times are seeded explicitly,
neighbors/shared files survive, and reopen plus actual cleanup precede the final
oracle. No isolated count/end/statistics-only red is claimed.

Read-only refusal uses actual SQLite query-only state/readback and typed errors,
followed by writable retry. Import refusal is its first write; CLI failure is
database open or argument rejection. The clear probe passes compiled candidate
and rejects the sole Bench-to-Trace production mapping mutant after cleanup.
A missing new API is never classified as an original regression.

Useful retention/query/FTS/candump coverage remains. Final affected16/0/0 and
primary751 Rust/0 failed/1 existing ignored,355 frontend/72 Pi MCP plus fatal ARM
release pass. Full independent Spec and Standards accept every source/test file.
No hardware dependencies, new ignores, negative sleeps or cold-build speedup
claims are introduced. Simulation5/0/0 and implementation PR226/run36807158080
all-five-job receipts are recorded in the ledger. Final6f5ae48/run36808420033 and
equal-tree main94d3cb4/run36808943360 complete all five jobs, including fatal ARM
release, with verified backup and exact branch cleanup.

## Thirteenth batch: independent capture chronology

Four whole existing-public groups reproduce actual original assertion reds and
replay unchanged green: literal UTC/leap/epoch starts, cross-kind invalid-date
refusal, supported profile/legacy artifact eligibility and buckets, and unknown
ends with authoritative finalization. Each uses real SQLite/files/gzip/pages,
reopen, independent neighbors and cleanup before its sole final oracle. Actual
complete source snapshots and local-package recompilation bind execution.

The three final test-only lint allowances change no runtime behavior; fresh whole
final-file refusal/compatibility proofs replace prior-version identity claims.
Candidate cutoff conformance passes, kills one combined `<` to `<=` event/session
SQL mutant at its boundary assertion and replays unchanged green. It uses literal
UTC before/equal/after rows and real archived files, FTS, overflow refusal, repeat
and reopen. Missing new APIs are never original regressions. Earlier missing-pipe,
zero-test lint and out-of-order output attempts remain unqualified.

Strict affected21/0/0, primary756 Rust/1 existing ignored plus355 frontend/72 Pi MCP
and fatal ARM release, observed timezone2/0/0 and simulation5/0/0 pass on the exact
1,455-input source. Independent Standards/Spec and final lint reconciliations
accept the code. The final seven-run independent execution audit accepts actual
proofs. PR227 implementation cabfe945/run36818692724 passes all five jobs, including
73 virtual CAN tests with zero ignored. Final1d3d85b/run36820454955 and equal-tree
main4c1800d/run36821026729 complete all five jobs and verified backup/branch cleanup.
Useful retention liveness,
structured-query/FTS and candump tests stay; no test removal or speedup is justified
by the five new behavior groups alone. No new ignores, hardware dependencies,
negative sleeps, maintenance-clock expectations or late-I/O rollback claims.

## Fourteenth batch: independent SQLite migration behavior

Three whole integration probes cover eight real database behavior cases. Literal
v1/v2/current/future fixtures do not import production migration SQL. Six actual
original-public assertion reds replay unchanged green: marker2 and marker3
refusal after DDL, current metadata preservation, future schema refusal,
unversioned nonempty admission and public migrate with journal mode OFF.
Typed SQLite trigger refusal, independent schema/data/FTS/integrity observations,
real retry/reopen, neighbor preservation and actual cleanup precede each oracle.
The OFF fixture uses supported Store bootstrap as a stated fixture dependency;
it proves refusal before logical writes, not successful rollback with journals off.

Candidate-only per-step conformance establishes a committed, usable v2 after a
later marker3 refusal. It catches one exact production mutation that skips marker2
and replays unchanged green. No missing API, setup failure or compiler error is
counted as a regression. Complete 1,480-file originals and the 1,483-file final
candidate bind metadata, actual local recompilation, executable hashes, direct
workers, completed waits and empty fixture directories. Bounded child watchdogs
keep public migration deadlocks finite; no hardware dependency or new ignore.

Strict affected29/0/0, primary764 Rust/1 existing ignored,355 frontend/72 Pi MCP
and fatal ARM release, plus simulation5/0/0 pass on 1,459 unchanged gate inputs.
Meaningful existing retention/query/FTS/candump tests remain. No test deletion or
build speedup is inferred. Historic recovery, full interruption/refusal/race
coverage and power-loss durability remain outside this normal-upgrade slice.
