# Control implementation and verification plan

This plan follows the [control review](control.md), rechecked against main
`52f12678277a2cae786d8df648f035895789f026` and the remediation working tree on
September 29, 2026. The original CS01–CS22 source findings still apply except for
the repairs described below. Their pinned source links remain the historical
evidence; the implementation ledger records later dispositions. Static
revalidation confirms the listed paths still exist. It does not replace each
package's regression and failure-injection acceptance tests.

The control stack remains joint-space Berthier → Davout → motor-space Robstride.
Rewrite modules where that makes their contracts enforceable; retain this
ownership boundary. The immediate work is software-only. No Pi connection,
enable, motion, deployment, cap increase, Wave unlock, or CAD/model calibration
is part of this implementation session.

## First completed slice: feedforward output safety

CS02 and CS11 are repaired in `crates/davout/src/lib.rs`:

- Feedforward seed, prior state, target, and final output obey the current minimum
  of joint/motor-type limits and any triggered `clamp_torque` danger-zone limit.
  A newly reduced hard limit takes priority over slew continuity.
- An unseeded limiter starts at zero. Successful new enable, disable, and asserted
  hardware E-stop clear old command history and timing. A deliberate mode-change
  measured-torque seed is bounded by the current feedforward policy.
- The existing 10 ms initial-step interval becomes an upper bound on subsequent
  slew intervals too. A delayed tick cannot accumulate a larger torque step. At
  the present 60 Nm/s policy this bound is 0.6 Nm; normal 200 Hz steps use their
  shorter elapsed interval. This is a feedforward bound, not a verified limit on
  total physical drive torque (CS10 remains open).

Five external tests in
[`torque_output_contract.rs`](../../../crates/davout/tests/torque_output_contract.rs)
exercise the actual Supervisor and MemoryBus send/receive pipeline. They manually
construct vendor status bytes and independently read the outgoing torque field
from CAN ID bits 8–23. They do not assert internal map structure or call the
encoder to obtain expected values. Two-sided cases cover both torque polarities;
the comparisons allow 0.002 Nm for existing RS03 wire quantization.

| Contract | Red evidence before its repair | Green acceptance |
|---|---|---|
| Measured torque above cap; cap lowered after seed | -8 Nm seed sent -7.3995 Nm under a 5 Nm cap | All four ±8 Nm / 5 Nm or 0.2 Nm cases stay within cap plus wire quantization |
| First activation | -5 Nm request sent -5.0008 Nm instead of a ≤0.6 Nm step | Both polarities start toward the target within the initial-step bound |
| Disable and re-enable | Previous -4 Nm replayed -3.4004 Nm after a neutral request | Both previous polarities produce neutral outgoing FF after re-enable |
| Triggered danger-zone torque limit | -4 Nm prior state sent -3.3985 Nm under a 0.2 Nm rule | Both polarities immediately obey the smaller rule bound |
| Delayed tick | A 30 ms delay produced a 1.8201 Nm step | Delayed step remains ≤0.6 Nm plus quantization |

Native Windows `cargo test --locked -p davout -p robstride`: 67 existing Davout
tests + 5 new external tests + 29 Robstride tests passed. The new tests took
approximately 0.04 seconds; the existing suites took approximately 0.08 seconds
combined, excluding compilation. Targeted Clippy with `-D warnings` passed.
The parent integration must still run the repository's full gate before merge.
The coordinate-conversion unit test now requests 0.4 Nm instead of 2 Nm so it
tests sign/gearing below the initial slew bound rather than relying on the old
unsafe bypass. No production configuration changed.

This slice does not repair nonfinite numbers, feedback freshness, total PD
torque, firmware stop confirmation, or the effectiveness of a particular
danger-zone strategy in supporting an elevated arm. Those have separate
contracts below.

## Dependency order and module interfaces

Each package should produce a small reviewable change with a demonstrated red
regression, green acceptance, and explicit remaining hardware requirements.
Before changing a major interface, write an ADR describing its state and failure
semantics. Avoid a wholesale Davout rewrite before these invariants are covered
at its external boundary.

| Package | Depends on | Owned modules and interface direction | Findings |
|---|---|---|---|
| C0: validated policy and command data | None; first limiter slice already landed in the working tree | `marengo-config::ValidatedRobotConfig` with private fields; Davout finite joint-command input; Robstride checked motor-command encode returning a typed error | CS02, CS03, CS11, CS15 |
| C1: freshness and fault authority | C0 | Robstride emits addressed, time-stamped `FeedbackEvent` values; Davout owns per-active-address `FeedbackLedger` and persistent `FaultLatch`; send paths require a fresh admissible snapshot | CS01, CS04, CS12, CS13 |
| C2: reference, enable, and stop transactions | C0 + C1 | `ReferenceSession` binds robot/model/motor identity and current-boot validity; `StopOutcome` records every address attempt and uncertainty; enable verifies drive-local limits/timeouts | CS05, CS06, CS07, CS08, CS09, CS10 |
| C3: one motion owner and atomic requests | C1 + C2 | Runtime `MotionSession` accepts typed intent with request ID, generation, bounded lease and outcome; Berthier is its executor; CLI and Consul are clients | CS19, CS20; coordinated F01–F04/F07 and transport/auth work |
| C4: one policy and durable config authority | C0 + C1 + C3 | Prepare a policy revision against fresh Disabled feedback; install an immutable generation; serialize persistence/URDF promotion and correlated outcomes in one owner | CS14, CS16; coordinated G04–G09/G21 and T02/T03 |
| C5: trustworthy sensors and independent physics | C0; production controller scenarios also need C1–C4 | IMU separates new samples from diagnostic cache; dynamics validates supported topology; independent analytical and plant fixtures exercise real production control | CS17, CS18, CS21, CS23, CS24; coordinated T27/T28 |
| C6: desktop portability | Can begin independently; integrate with C1 transport work | Chappe separates platform-neutral framing/pubsub from Unix transport; compile-time selected adapters and portable pure tests; robot hardware backends stay Linux | CS22 = G11 |

C0–C2 are prerequisites for returning to physical commissioning. C3 is a
prerequisite for trusting UI Stop/Disable or exposing autonomous client loops.
C5 analytical fixtures can start immediately and must block an algorithm change
if their independent predictions disagree. Desktop portability should not hold
up pure safety fixes or require another WSL source tree.

## Finding-by-finding implementation contracts

### CS01: per-active-motor freshness (C1, P1, verified software repair)

The [second batch](batch02-feedback-command-validity.md) implements the receive-time
contract below, including status queued during enable writes, bounded neutral
controller startup, same-tick re-enable and taught ranges excluding zero.
Required software checks pass. CAN carries no physical acquisition generation;
installed reporting/timeout behavior remains a separate acceptance requirement.

Remove the global watchdog proof-of-health. Only a valid decoded sample updates
its original receive timestamp, indexed by interface + device ID. Keep cached
pose available for diagnostics, but expose freshness and require every active
address to be fresh for non-neutral motion. A bounded enable bootstrap may send
only the documented zero-gain/zero-FF solicit needed to receive status; it must
not approve stale-feedback motion. Include joint, address, age and missing-frame
reason in the fault. Re-enable starts a new observation generation.

Acceptance uses a scripted real MotorBus and injected clock: total silence
through repeated empty drains; one responding motor with one silent motor;
unknown-only/status-from-wrong-interface traffic; missing first frame; late
frames from the old generation; and recovery with a new complete active set.
After the deadline no non-neutral outgoing frame is recorded. The current sleep
then-send test misses the normal empty-drain path and is insufficient by itself.

### CS02: cap after every output stage (C0, P1, repaired)

The completed slice enforces the FF cap across seed, dynamic cap reduction, slew,
danger-zone caps, and final output. Retain the external wire tests as the contract
while moving the limiter into a small private output-policy module if useful.
Review future transforms for numerical overflow/finite guarantees in CS03/C0.
Do not reinterpret this repair as bounding PD or physical measured torque.

### CS03: invalid numeric input cannot become a valid extreme frame (C0, P1, verified software repair)

The [second batch](batch02-feedback-command-validity.md) checks all numeric
command/feedback fields, transforms and driver firmware write kinds, rejects
unsupported modes, and preflights complete batches before output or gain state
mutation. Required checks pass. Persistent fault authority remains CS12/CS13.

Validate position, velocity, all gains and FF for finiteness before policy math;
gains must also be nonnegative. Validate feedback before freshness/state updates.
Use checked conversions after joint↔motor transforms: a finite `f64` can overflow
to nonfinite `f32`. Make Robstride's production encoder/send boundary return a
typed `InvalidCommand` error instead of casting invalid values to integer zero.
Callers propagate the error into the fault authority; do not silently replace an
invalid motion request with an extreme or inferred command.

Table-driven tests inject NaN/+Inf/-Inf into each field through public command
APIs, negative kp/kd, finite-to-f32 overflow with scaling, invalid feedback and
malformed startup/reload policies. An independent recording bus must see no
motion frame for a rejected command. A small direct driver test proves NaN never
becomes raw torque 0. Fuzz/property sampling supplements the finite table; it
must not replace explicit boundary cases.

### CS04: complete fault decoding and explicit recovery (C1, P1, partial)

Batch02 repairs fault-only pose timestamps. [Batch03](batch03-fault-authority.md)
adds ordered raw fault/status domains and persistent authority. Malformed DLC,
installed firmware qualification and explicit recovery remain open; the local
primary, independent review and actual Linux virtual CAN CI pass.

Retain all four detailed-fault bytes and four warning bytes; a typed word/identity
requires qualified firmware byte order. Decode documented status ID bits and
drive mode separately, and retain the complete type-21 field. A fault
report must not fabricate a zero position sample or count as new pose feedback.
Davout latches decoded faults independently of later healthy status; clear only
through an explicit recovery transaction with fresh evidence.

Feed captured or independently specified bytes for each status fault bit, high
type-21 bits, combined faults, ordinary status after a fault, unknown IDs, and
fault-only traffic into the actual decoder/Supervisor. Assert normalized fault
identity, preserved high bits, unchanged pose freshness on fault-only RX, refusal
of motion, and latch retention until an authorized reset. Protocol fixtures must
come from the vendor format rather than an encode/decode self-roundtrip.

### CS05: calibration history is distinct from current readiness (C2, P1, partial)

R1a ([batch05](batch05-reference-history-admission.md),
[ADR0022](../../decisions/0022-calibration-history-and-current-reference.md))
removes all history-based startup grants, preserves rows/bytes, surfaces typed
non-missing load errors and binds resource paths explicitly. Exact/mismatched
history, corruption and actual Supervisor home/Enable regressions pass unchanged
after failing on merged 6ff. Local required gates pass; exact-head GitHub delivery
checks remain recorded by the batch report/ledger. The following private grant,
device/boot/config/model binding and current-evidence work remains required.

Preserve historical audit rows, but do not mark them Verified on joint name
alone. Bind device identity, interface, motor model, sign, gearing, reference
method, relevant config/URDF revision and a session/boot validity rule. Define how
a motor power-cycle is detected; uncertainty invalidates runtime readiness. A
current reference must be established for the current hardware identity.

Instantiate a real registry/Supervisor with persisted rows differing in each
identity field, failed sign attestation, invalid pose, incompatible model revision
and a new boot generation. Old records remain inspectable while enable emits no
enable frame. Only fresh reference verification changes current readiness.

R1b ([batch06](batch06-private-reference-admission.md),
[ADR0023](../../decisions/0023-private-current-reference-authority.md)) gates all
Ready/Enable/output using private owner-local authority and closes mutable bus,
registry and naked grant paths. Ordinary reference acquisition explicitly refuses
before arming/write; only closed virtual initial conditions are admitted. Relevant
observed policy edits revoke permanently, and rejected model restores are atomic.
Same-type whole-owner replacement and coordinated controller/model installation
remain CS15/CS21; qualified acquisition/device/reset/transaction evidence remains
required. Local final primary and independent reviews pass; exact-head delivery
is tracked by the ledger.

[Batch08](batch08-history-write-consistency.md) repairs a separate historical
publication problem: real failed writes expose uncommitted new/replaced rows in
memory. Two original-public assertion failures precede the staged-record repair;
both identical probes pass afterward, including real retry/reload and preserved
peer history/state/flags. Independent reviews and required primary703Rust/1
existing ignored,355 frontend,72 Pi MCP/fatalcrossbuild and strict affected420/0
pass. [PR222](https://github.com/jaylamping/marengo/pull/222) passes all five
implementation-head jobs at d541bd5/run36736794717, including 73 actual driver
feature tests/0 ignored and 5 simulation tests. Final7ddc243/run36738071460
and equal-tree merged04e4ea2/main36738856051 each pass all five jobs, including
fatal main release cross-build; backup/cleanup are complete. This does not supply crash-safe disk
commit or Davout reference permission; CS05 remains partial.

### CS06: Set Zero verifies a post-command response (C2, P1, partial)

Batch06 removes cached/unqualified success and invalid scalar history writes.
This is truthful refusal, not completed correlated transaction acquisition.

Calibration drains prior queued feedback, issues the zero command with a request
generation/time, and requires a fresh correlated acknowledgement or documented
readback sequence. Timeout, RX failure and absent response are failures. The
transaction always attempts stop before returning and publishes a correlated
Verified/Failed/StopUncertain outcome.

A scripted bus supplies cached near-zero feedback with no new response, delayed
queued pre-command zero, response from another address, explicit nonzero
readback, successful new zero, and RX errors. Only the successful fresh response
records reference verification. The current cached-feedback success test must
become a rejecting regression, not remain a characterization of the defect.

### CS07: one-shot calibration cleanup (C2, P1, partial)

Batch06 routes motor-repl calibration through central preflight, refusing the
unqualified path before arming. Installed-owner cleanup/stop/client work below
remains required; a startup-failing fresh CLI is not an emergency stop.

Resolve the joint and attestations before enable, narrow calibration to the
requested address, and route the CLI to the same calibration owner as the Pi.
Until the client conversion in C3 is available, use Result-based unwinding and an
explicit cleanup guard; remove deep `process::exit` calls from armed paths.
Stop-result uncertainty is reported according to CS08.

Execute the command handler against a recording/failing bus for success,
misspelled joint, missing sign attestation, response timeout, partial enable,
verification and persistence failure. Every armed path attempts stop; validation
failures send no enable frames. Exit codes and user outcomes match the observed
bus/result sequence. Source-string checks do not establish this contract.

### CS08: stop failures remain visible (C2, P1, verified software repair)

Batch03 adds typed failure return, all-address/action stop attempts and a
read-only StopReport retaining first failed delivery alongside the initiating
fault. The baseline all-TX failure now fails honestly. Publication through Pi
and correlated command outcomes accompanies the CS13/C3 owner migration;
physical confirmation and drive-side fail-safe remain separate acceptance.

Attempt zero speed, neutral MIT and disable on every configured/active address,
collect all failures, and return a typed aggregate outcome. Separate software
command inhibition from confirmed drive stoppage: successful transmission is not
physical confirmation. Publish `StopRequested`, `StopUncertain` and documented
confirmation evidence instead of unconditional success. A failed first address
must not prevent attempts on later addresses.

Inject send failures by address and stage, including all-TX failure, and inspect
the complete recording. Outcomes retain each failed attempt and do not report
confirmed stopped. A drive-local timeout and power-cut test are additional
hardware acceptance, because no host code can guarantee stop after SIGKILL.

### CS09: stop precedes persistence drain (C2, P1, verified software repair)

The shutdown owner immediately inhibits new commands and attempts stop, records
the outcome, then waits for durable writes. It may bound that wait and mark
unfinished transactions honestly; it must not postpone motor stop behind SD I/O.

Use a deliberately stalled persistence worker and a deterministic event trace.
Assert stop attempts precede the wait and no later motion runs after shutdown.
Exercise SIGTERM/SIGINT through a software runtime fixture where available;
code-order/source matching alone is insufficient. Hardware timeout behavior
remains a separate commissioned requirement.

[Batch07](batch07-stop-before-persistence.md) now inhibits intent and retains the
configured Davout stop result/report before typed bounded drain. Six unchanged
behavioral replays cover order, retained work, publication lifetime, closed
admission, Quit and the shared owner flag changing during an admitted stop.
Actual storage failures/timeouts preserve the initiating failed-stop report;
no-disable policy reports Skipped. Required primary and strict affected gates
pass. All five implementation-head jobs pass at `3939b3d` in run36728845164,
including 73 actual virtual-CAN driver tests with none ignored;
[PR221](https://github.com/jaylamping/marengo/pull/221)'s final87e6df1/run36730073787
and equal-tree merged1518176/main36731139955 also pass all five jobs, with fatal
main aarch64 release. Actual OS signal registration/delivery and physical
acceptance remain unexecuted.

### CS10: total torque contract and drive-side backstop (C2, P1, open)

Document bench policy as total joint torque or FF-only; the current operator
expectation requires total torque. Davout admission must account for
`kp*(q_des-q) + kd*(dq_des-dq) + FF` using fresh feedback and reserved headroom.
The drive can move after admission, so verified model/firmware-specific torque or
current limits and communication timeout are an independent required backstop.
Enable validates/readbacks those limits; a missing or unsupported handshake
refuses normal enable. Validate torque scaling against sign/gearing.

Simulation covers aggressive gains, external displacement after admission,
feedback delay, gain/target changes and saturated FF. A fake parameter-capable
bus proves limit/timeout writes precede enable and failed/mismatched readback
blocks it. Actual per-model firmware readbacks, torque units and timeout/support
behavior require supported-arm physical commissioning. Do not claim software
tests certify physical torque or safe gravity removal.

### CS11: fresh output lifecycle and bounded timing (C0, P2, repaired)

The completed slice initializes zero, clears output history at enable/disable/
E-stop, preserves deliberately bounded mode seeds and prevents long-gap slew
credit. Retain the external direct-activation, neutral re-enable and delayed-tick
tests. When C1 adds an injectable control clock, replace the single 30 ms sleep
with deterministic virtual time without losing its outgoing-frame assertion.
Reference/fault recovery authorization is C2/C3, not implied by this limiter fix.

### CS12: every feedback-ingestion violation takes effect (C1, P2, verified software repair)

Batch03 propagates both post-send drains and planner-entry errors, checks the
persistent latch before new intent, and cancels retained intent after stop.
Ten public baseline regressions fail before repair and pass the primary gate,
including every ControlMode and actual post-send unsafe pose injection.

Davout latches safety faults as it ingests feedback, before returning. Berthier
propagates all pre-send, post-send and mode-entry refresh errors. Benign empty RX
is distinguishable from an invalid or unsafe sample. A caller ignoring a Result
must still be unable to send further motion through Davout.

A scripted bus returns ordinary feedback before send and an out-of-hard-limit,
overspeed or fault-bearing sample immediately after send. Test each control mode
through real `ControlLoop::tick`; no subsequent motion is emitted, and fault
state survives the next healthy/Disabled tick and a missed telemetry publication.

### CS13: persistent fault state and stop generation (C1/C3, P2, partial)

Batch03 implements private Davout authority and Berthier stop-generation
invalidation. No reset API is supplied. Pi/protobuf publication, other owner
failure classes, boot/device-qualified recovery, process-reconstruction bypass
closure and queued-command session/generation admission remain open.

Move the transient Pi `Option<String>` into Davout's fault authority with class,
first occurrence, evidence and recovery policy. Publish it until explicit reset.
Disable/fault increments the motion generation, cancels/invalidates queued
intent and blocks implicit re-enable. Separate target submission from enable
authority; convenience behavior cannot override a stop/fault barrier.

Use deterministic interleavings: single fault between 25 Hz publishes, next
successful disabled tick, repeated late Testing commands, old lease expiry and
explicit eligible reset. Assertions inspect runtime outcomes and the bus:
failure remains visible and old-generation work cannot energize the drives.

### CS14: meaningful typed danger-zone actions (C4, P2, partial)

Batch02 rejects unsupported actions and absent torque caps. It does not establish
the fall/braking behavior below.

Replace action strings with a validated enum, reject unknown actions and
implement declared Fault semantics. Keep velocity/FF bounds distinct from a
physically valid braking strategy. Zero-kd GravityComp cannot brake through a
velocity setpoint. Choose supported-arm braking/fault behavior with CS10's torque
backstop and CS21's plant model; do not blindly lower gravity support.

Test unknown-action startup rejection and each declared action using measured
state. Independent plant cases include an elevated descending arm under zero
MIT gains, actuator saturation, payload error and stale feedback. Physical
fall-response acceptance requires the commissioning protocol; a helper output
clamp by itself is not proof of descent arrest.

### CS15: one validated config across every writer (C0, P2, partial)

Batch02 implements shared numeric/identity/profile validation at loaders,
startup, profile writers and overlays. Raw mutable config and bus/synthetic
access, schema-key strictness and generation installation remain open below.

Consolidate startup/reload/override/persistence validation into one immutable
validated policy object. Check every safety numeric for finite/range invariants,
nonnegative gains/caps, positive timing/acceleration/gearing, direction exactly
±1, ordered bounds and margins, supported enums, unique joint names/addresses,
motor type agreement and complete robot/motor/control/homing mapping. Reject
unknown fields where a typo loses safety intent. Production APIs cannot mutate
raw policy fields or obtain unrestricted bus/reference bypass handles.

Build table-driven invalid objects and exercise startup plus each update entry
point; all reject the same invalid field before enable, state mutation or disk
commit. Include duplicate joint with different CAN addresses, negative slew,
NaN velocity, zero/negative gear and schema typos. Validate actual repository
fixtures separately from analytical safety fixtures so CAD/config changes do
not silently rewrite test expectations.

### CS16: Set Limits uses fresh Disabled pose (C4, P2, open)

Read the authoritative feedback ledger, not the Active velocity-derivative
scratch map. A policy transaction must ensure every affected fresh pose remains
inside the proposed envelope; absent/stale feedback yields an explicit refusal
or a separately labeled unverified artifact that cannot authorize normal
enable. Freeze the accepted observation/policy generation through activation.

Inject a real Disabled status poll with q=1 rad, propose [-0.2,0.2], and assert
rejection with unchanged runtime/config/model revision. Test silence, stale
sample, valid inclusion, concurrent feedback movement and persistence failure.
Retire the test that injects a private derivative cache to represent Disabled RX.

### CS17: IMU acquisition time remains honest (C5, P2, open)

Separate `poll_new_sample` from `last_sample`; carry sequence and host/sensor
receive times, clear samples on reset and publish only new acquisition evidence.
Health exposes stale/unavailable and starts bounded recovery on sustained
errors/silence. Cached orientation may remain viewable with its original age.

MockI2c emits exactly one packet then silence; assert one publication with the
original timestamp and subsequent stale health. Add disconnect, reset and
recovery cases with virtual time. Do not test only getter equality: test the
driver-to-publisher path so a fresh wall-clock stamp cannot disguise old data.

### CS18: report-ID-specific sensor framing (C5, P2, open)

Use the documented 14-byte standard rotation-vector report and 12-byte game
rotation-vector report as distinct layouts. Decode angular accuracy correctly;
remove the invented optional timestamp explanation. Validate batching and
truncation behavior against externally specified/captured sensor packets.

Golden bytes contain metadata, a standard 14-byte rotation report and a 10-byte
gyro report; both emerge with the expected values and complete boundaries.
Include consecutive rotations, game-vector variant and truncated final report.
Replace the incorrect 12-byte standard fixtures; do not adjust lengths merely
to match the current parser output.

### CS19: gains, mode and target apply atomically (C3, P2, open)

Validate the full typed motion request, transition mode, apply requested gains
and target as one operation, then return the actually applied mode/gains/target.
Wave accepts documented gains or rejects them explicitly. Remove mode-dependent
silent override dropping and overloaded string commands.

The same request starts from Disabled, GravityComp, TorqueOnly, Impedance and
Position; the accepted request yields the same first effective command policy.
Also test rejection with no partial target/mode mutation, legal zero gains,
gain-only updates and unsupported Wave fields through the real runtime adapter.

### CS20: CLI commands address the live control owner (C3, P2, open)

Convert `motor-repl` into an explicit client of the Pi motion/reference/config
owner with bounded correlated outcomes. Status queries live state; mode/torque
commands persist only through the owner; Jog invokes the supported joint-space
position primitive. Until migrated, remove or clearly report inert commands as
unsupported rather than implying a live effect. Enforce exclusive CAN ownership
in the service/runtime, not only documentation.

Run command clients against an IPC fake and the real runtime command dispatcher;
assert exact accepted/rejected outcomes, applied live state and bus frames.
Process exit cannot silently leave a second controller/drive enabled. Exercise
lost replies, owner offline, competing clients and command lease expiry.

### CS21: independent dynamics and controller verification (C5, P2, partial)

Replace stale ignored pure goldens with immutable independently calculable
fixtures. Pure tests run by default. Analytical pendulum/two-link formulas cover
axis sign, translated joint origins, rotated frames, fixed payload and zero-mass
links. A production model check compares the same current URDF and exact joint
vector against an independent dynamics engine, with recorded revision and
tolerances; measured model calibration stays a separate hardware record.

The implementation effort found CS23 while introducing this independent oracle.
Do not regenerate expected values from the algorithm under test. Production
PositionHold tests must use a separate plant with inertia, gravity, friction,
quantization, CAN delay/drop, saturation and supported payload error. Replaying
the planner's own trajectory as perfect feedback can remain a planner unit
check but cannot establish physical controller correctness or bench readiness.

### CS22: portable core tests and platform adapters (C6, P2, open; same as G11)

Move Unix socket types/permission handling behind `cfg(unix)` or a transport
module/feature. Shared framing/pubsub and pure control tests must compile on
Windows/macOS. Select a named pipe/TCP desktop transport deliberately, or return
an explicit typed Unsupported adapter if desktop IPC execution is outside this
slice; never expose an inert success stub. Linux SocketCAN/I2C stay isolated.

CI builds/tests the portable slice on Windows and macOS and the complete robot
runtime in Linux containers. Pure Berthier/Davout tests must run natively without
WSL. Add adapter/framing limit/disconnect tests; cross-platform compile success
alone does not certify an operational desktop transport.

### CS23: translated COM lever arms (C5, P1, newly discovered)

The independent two-link oracle identified `Isometry * Vector3` discarding joint
origin translations in gravity COM calculations. Treat COM as a point and use
`transform_point(Point3)` so downstream masses retain their parent lever arms.
The parent agent owns this repair and its regression evidence. Analytic expected
shoulder compensation for its fixture is 46.5975 Nm; the old code returned
17.1675 Nm. Apply the same independent fixture family across translated and
rotated fixed attachments and unequal joint angles. This defect changes computed
production gravity; software correction alone does not authorize recommissioning
the robot or copying stale tuning into the corrected dynamics model.

### CS24: measured progress after motion stops (C5, P1, verified software repair)

[Batch09](batch09-measured-stall-progress.md) and ADR0025 implement a separate
measured-position high-water watchdog on actually commanded unresolved ascent.
The existing2-second budget and ceilings remain. Frozen installed decoded-grid
metadata bounds software conversion; nominal Duration preserves submillisecond
time and rejects invalid periods before mutation. Six actual original law/public
controller failures have byte-identical repaired replays. Crawl preservation,
five numeric conformance cases and two killed production mutants independently
cover scope, jitter/reversal/held data, slow movement, settle, cleanup and time.
Actual controller stall preserves all15 stop attempts and persistent refusal.
Primary715/0/1, frontend355, PiMCP72, fatalARMrelease and strict affected432/0
pass; independent Standards/Spec accept the exact source with one docs-only
post-gate guidance correction. [PR223](https://github.com/jaylamping/marengo/pull/223)
implementationeb1359c/run36755369469 passes all five jobs, including5simulation
and73actual driver tests. Final-head/main delivery checks and cleanup remain pending.
This is separate from physical noise/plant/drive/timing commissioning, existing
dropout freshness and unfinished reference/config/model ownership.

## Cross-cutting completion gates

Before declaring control remediation complete, prove no public production path
can bypass validated policy, fault/reference state or the single motion owner.
Remove or restrict unrestricted mutable configuration, homing and bus handles;
retain test injection through clearly named test adapters. Integrate E-stop GPIO
and per-input Hall polarity/search/health where selected, or explicitly reject
unsupported selected methods. Preserve their present scaffold status until the
hardware paths are actually wired and tested. Wrong-sign policy needs a precise
joint/motor-space and per-pose contract; its current deliberate disabled setting
must not be flipped merely to satisfy a software test.

Measure command-drain work and control-loop jitter under load using a bounded
priority dispatcher. Stop/fault is processed before bulk commands; persistence,
audit and sensor recovery run outside the realtime owner. Required evidence is
behavior under queue flood and stalled I/O, followed by actual Pi timing before
hardware commissioning. Microbenchmarks and passing fixture-count smoke tests
do not substitute for runtime deadlines.

For each acceptance test, retain the named failure it detects, a test of the
unfixed failure when feasible, expected external effect, runtime cost and any
hardware limitation. A test that only checks enum defaults, helper mirrors,
source text, or ideal planner replay may be consolidated once a stronger
behavior test covers its contract. Remove obsolete tests because their assertion
is misleading or duplicated, not simply because the suite has many tests.
