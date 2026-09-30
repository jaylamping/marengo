# Seventh repair batch: stop before persistence

Checked baseline: PR219 merge `eae4fc358102c1e75bf02684c73b4c4e1aa9e9b1`.
Decision: [ADR0024](../../decisions/0024-stop-before-persistence-shutdown.md).
Active checkout: `J:\code\marengo`. Status: verified software slice in
[PR221](https://github.com/jaylamping/marengo/pull/221); required local and all five
implementation-head checks pass. Final evidence-head87e6df1/run36730073787 and
equal-tree merged1518176/main36731139955 also pass all five jobs, including the
fatal main aarch64 release build. The branch is backed up and cleaned up.
Evidence root: `J:/code/marengo-migration-backup-20260929/batch07`.

## Scope and observable contract

The baseline Pi exit waits for write-behind before attempting motor stop. Its
owner shutdown flag can terminate the writer with a retained request, and stdin
Quit can continue through remote command dispatch and the current control tick.
These source observations require direct behavior evidence before correction.

Graceful shutdown must inhibit intent, attempt all original Davout stop routes
when the existing exit policy requests it, preserve that exact outcome, and only
then wait for storage. Stop errors, skipped stop and unfinished storage remain
separate outcomes. Writer admission/termination must be independent of owner
shutdown so retained accepted work can complete or remain explicitly unfinished.
Matching completion publication must precede queue idle. No physical stop is
inferred from transport acceptance, a Disabled mode or a completed disk write.

This batch keeps master config, limits, model/CAD and Wave policy unchanged.
Full ConfigAuthority/CAS/coalescing receipts, installed-owner priority stop and
client/protobuf cutover, reference transactions, hard realtime scheduling and
physical drive/support/timeout acceptance remain separate work.

## Baseline and test evidence

The exact checked Git archive preserves all **1,427 original blobs**, with an
all-reference recoverable bundle verified before source edits. Independent review
accepted the frozen unfixed extraction of actual exit/worker logic.
Type generalization permits the actual runtime to use recording or closed virtual
transport; it supplies no new permission or physical capability. Test observers
gate actual filesystem/wait/publication boundaries and never replace the writer.

An extraction baseline is reported as parity-extraction behavior, separately
from unchanged-original-binary proof. Missing dependencies, private/new APIs,
fixture setup errors and source tracing do not count as behavioral reds.

The planned vertical probes are: actual all-address stop evidence before a gated
writer wait; accepted second draft surviving owner shutdown; and Quit preventing
later queued command/tick. Each probe must reach its named assertion and replay
unchanged after its repair. Gates release and workers terminate even after a
violated assertion. No multi-second negative sleep is an acceptance oracle.
The first probe has an **executed behavioral red**. Root verified every archived
blob, overlaid only the reviewed unfixed main/worker and unchanged tracer, mounted
that snapshot read-only, cleaned the Pi package and compiled it at the guarded
`/snapshot/bins/marengo-pi` manifest. Cargo exited 101 with 0 passed/1 failed;
the test body took 0.02 s (full run 10.874 s). Its named assertion observed an
empty stop trace, GravityComp intent and no StopReport at WaitEntered. Before
that assertion, cleanup and reachability controls confirmed actual persisted
Kp=37, a matching Durable ActionEvent and all fifteen eventual literal addressed
stop frames. This is reviewed-extraction proof, not unchanged-original-binary
or physical acceptance. Raw log, run receipt and full frozen hash binding:
`phase01-order-red.log`, `phase01-order-red-run.json` and
`phase01-order-red-binding.json` under the evidence root. Tracer SHA256:
`92ea1ad41fdcf321b2b8b068d6e950deb349373909d5ec844950b4cfd851353b`.
The **byte-identical first repaired replay passed**: 1/0 in 0.02 s (full run
10.272 s), same tracer SHA256 and guarded read-only snapshot rebuild. All fifteen
literal addressed stop writes, Disabled intent and the exact retained StopReport
now exist at WaitEntered while the real writer is blocked. The repair calls the
existing Berthier intent lifecycle, then conditional actual Davout disable, then
the original wait. Worker abandonment/early idle and Quit are still intentionally
unfixed pending their own actual regressions. Evidence: `phase01-order-green.log`,
`phase01-order-green-run.json` and `phase01-order-green-binding.json`.

The retained-write probe also has an **executed behavioral red**: 0/1 in 0.02 s
(full run 10.310 s), Cargo 101. With the owner flag false, the same gated real
worker installs Kp=41 and then Kp=43 and publishes both matching Durable actions.
With that flag set only after both requests were accepted, it installs Kp=41,
publishes only the first action and returns with the second abandoned. After
explicit gate release and worker-return cleanup, the named assertion expects
final Kp=43 and observes 41. The second request is not superseded; this proves
that lifecycle defect without claiming general coalescing/CAS receipts. Evidence:
`phase02-retained-red.log`, `phase02-retained-red-run.json` and
`phase02-retained-red-binding.json`. Frozen tracer SHA256:
`cc54749c595497dcfb0e1c659e22157bde3e7df528837fb0c5425795038cc7a3`.
The **byte-identical retained-write replay passed**: 1/0 in 0.02 s (full run
10.020 s). Both owner-flag states now install 41 and then 43 with two matching
actual Durable events. The minimum repair removes the worker's owner-flag
capture/checks; publication-before-idle, final closed-drain API and Quit remain
separate cycles. Evidence: `phase02-retained-green.log`,
`phase02-retained-green-run.json` and `phase02-retained-green-binding.json`.

The completion-publication probe has a qualified **actual red and byte-identical
green**: each 0.01 s test body (10.011/10.380 s full runs). At the real
BeforePublish gate, disk already contained Kp=47 and the subscribed audit topic
was empty. The old worker reported `is_busy=false`; the repair keeps it true
through the actual matching local ActionEvent publication. Both runs released
the gate, observed real worker return and decoded the matching Durable action
before the decisive assertion. Entire tracer file SHA256:
`39145264a3cd790bdd5bdebc6360d43eeb9443796b339086676fb7fe9fae2489`.
Evidence: `phase03-publication-{red,green}.log`, corresponding run/binding JSON,
and `phase03-publication-red-qualification.json`. The launcher's expected-message
paraphrase was corrected by a separate classifier against the unchanged raw
Cargo-101 log; all 1,429 frozen source hashes were rechecked. Local Bus publication
is not a gateway delivery receipt. Closed admission, typed drain and Quit remain
pending.

The replacement closed-admission case has an **actual candidate-before-cutover
red**: Cargo 101, 0/1 in 0.05 s (full run 10.415 s). The no-exit neighbor proves
the valid patch reaches live Pending, matching local Durable and reloaded copied
model/policy. The exit variant first completes actual stop, then wrongly accepts
the same later patch and persists it. After real worker cleanup, the named
closed-queue assertion fails. Only exclusive fixture position bounds expand;
fixture torque is lowered and velocity is unset. Master limits/model are
unchanged. This replaces the old flag-plus-50-ms worker-death recipe. Frozen
overlay test file SHA256:
`9dff657b28e2661f2aa7b239cd9b4b9938840bcadf25a048d0b4704d1301a467`.
Evidence: `phase04-admission-red.log`, corresponding run/binding JSON. Structured
The **byte-identical immediate replay passed**: 1/0 in 0.05 s (full run
10.205 s). Actual owner shutdown completes its typed drain and joins the worker;
the later valid patch receives PersistQueue rejection while the complete public
policy/model and all three durable files remain unchanged. Evidence:
`phase04-admission-green-v2.log` and corresponding run/binding JSON.
The earlier `phase04-admission-green` attempt failed compilation with zero tests
because a frozen probe still used the removed test wait method. Restoring that
test-only compatibility seam enabled the qualified v2 replay; this setup failure
is excluded from regression evidence. Production construction subsequently
removed the obsolete owner flag, with a mechanical test helper migration; this
later source is not claimed to have the earlier whole-file tracer hash.

The actual-loop Quit probe has qualified **behavioral red and unchanged green**:
Cargo 101, 0/1 in 0.03 s (full run 10.571 s), then 1/0 in 0.05 s (10.769 s).
Its actual subsystem neighbor first applies Kp=83, publishes the matching
TuningChangeEvent and sends a non-neutral addressed elbow MIT frame during one
successful Active tick. With Quit queued before entry, the old actual outer loop
wrongly produces the same mutation/tick/frame/event and consumes the later
operator request. The repaired loop returns with no gain mutation, zero ticks,
frames or tuning events, and the byte-identical later request retained.
Snapshots precede cleanup; actual configured stop, typed worker termination and
unchanged nonpersistent disk controls all pass before the decisive assertions.
The neighbor is actual subsystem reachability, not an outer-loop no-Quit run.
Entire tracer SHA256 remains
`abacacce9a1ddece66aab0b025c544b319150e31133630468c757c073598bc5e`.
Evidence: `phase05-quit-{red,green}.log` and corresponding run/binding JSON.
An initial unexecuted fixture consumed its only raw status before Davout's
session marker; a separate literal post-Enable observation corrected setup
before qualification. The repair checks observed shutdown at each dispatch and
tick boundary. It does not preempt an already admitted handler or establish
command-flood priority, physical stop or hard realtime latency.

An independent review identified missing external-flag transition coverage.
The added actual-runtime matrix has **old-control-flow red and whole-file
unchanged green**: 0/1 in 0.07 s (10.999 s full), then 1/0 in 0.04 s (10.805 s).
An ordinary unreferenced recording transport raises the shared owner flag during
a literal admitted stop write. Both critical stdin and Chappe variants execute
and clean up before the decisive assertion. The earlier runtime consumes later
commands and ticks once; the guarded runtime finishes all fifteen admitted stop
attempts, retains later commands and performs zero later ticks. A neighboring
actual runtime reaches both valid Chappe disables and all thirty attempts.
The bounded watchdog must not fire. Red main/overlay are independently verified
retained phase05 archive files, not a recreated test loop or original binary.
Evidence: `phase06-owner-flag-{red,green}.log` and run/binding JSON; entire tracer
SHA256 `8659b0cba8e25d841f3577b1461e786e9d78dd345df929e2ff05d39933d51237`.
Actual OS signal registration/delivery remains unexecuted; these tests exercise
the shared software flag and admitted handlers without physical operations.

## Failure paths and test quality

Final actual Pi qualification passes all **30 tests**, including successful,
failed and unfinished real storage with exact failed-stop evidence retained.
All fifteen original stop attempts precede storage in each case; the one literal
delivery failure remains visible after later Durable, Failed or timeout outcomes.
Explicit no-disable policy preserves generation and sends no stop writes while
clearing intent. Closed virtual Active gain/torque/Wave cases prove reachable
non-neutral output before shutdown and no old intent replay after a later virtual
Enable. These do not sign off physical Wave/support behavior.

New drain API conformance reaches actual zero-budget timeout/closed admission
and a real isolated worker unwind after writing disk. It distinguishes written
disk from unfinished local publication and requires actual joined termination;
no terminal event or failure counter is invented. The generic probe runner's
successful `green`/default parity wording is explicitly superseded by **new API
conformance**, with no baseline-red claim, in `immediate-regression-evidence.json`.

Three weak tests are replaced: a private pending-count bound now observes actual
first/latest writes, matching events and one coalesced draft; privilege-dependent
chmod/sleeps now use valid live admission followed by a real file-parent blocker
and matching Failed outcome; delayed limits polling now decodes the actual
synchronous publication before any tick. The obsolete private counter is removed.
Five earlier regression function bodies remain byte-identical in final source.
Broader complete-generation coalescing/CAS and client delivery stay open.

## Standards

No hard documented violation; one low, nonblocking fixture-scaffolding duplication
judgment. Preserve frozen probe provenance now and consider shared resource helpers
with the later owner/config cutover. Independent review covers all final source;
the qualification author excludes that file, which a second reviewer independently
accepts. Receipt: `standards-review.md`, with separate qualification Standards in
`extraction-parity-review.md` under the evidence root.

## Spec

Accepted for ADR0024/CS09's scoped software contract. The external-flag coverage
gap was detected and resolved by actual unchanged red/green evidence. Both axes
independently bind final hashes; OS signal delivery, physical stop/support,
priority/flood/preemption, client delivery, CAS and reference acquisition remain
explicitly outside this acceptance. Receipt: `extraction-parity-review.md`.

## Qualification and continuation

Required primary passes **701 Rust, zero failed, one existing ignored**, plus
**355 frontend** and **72 Pi MCP** tests, current deny/audit and the fatal
aarch64 release build (101.788 s). Strict all-target Linux checks with socketcan
and linux-i2c pass **205/0** including final30 Pi tests (13.885 s; Pi body0.52 s).
Minimal MuJoCo smoke and sim-harness pass **5/0** (2.590 s); this is not
production-controller plant acceptance. Evidence: `primary-final`,
`affected-linux-final-v3`, `simulation-final` raw logs, run receipts and immutable
per-run source bindings; `local-gate-summary.json` reconciles them. Initial strict
lint failed only the test observer's type complexity, with zero executed tests;
a named test-only alias fixed it. All285 final compile/gate inputs remain frozen.

All five implementation-head jobs pass at `3939b3d` in
[run36728845164](https://github.com/jaylamping/marengo/actions/runs/36728845164):
701 Rust/1 existing ignored,355 frontend,72 Pi MCP,5 minimal simulation and73
actual virtual-CAN driver feature tests with none ignored. All five final-head
and equal-tree postmerge main jobs pass with the same counts; fatal main release
cross-build passes. Receipt: `merge-receipt.json` under this batch's external
evidence root. The102-ID
ledger has13 verified,10 partial and79 open. No other finding is closed.
Hardware acceptance remains unestablished; the repair loop performs no robot
connection, enable, motion, flash or deployment.
