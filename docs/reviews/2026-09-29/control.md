# Control safety configuration dynamics and IMU review

Reviewed on September 29, 2026 against main at `4bc77ba605834fdec04b436daa4bec67bca84fbb`. The recovered working tree initially used the older August 8 branch `refactor/candump-deep-module` (`c97aa96`) with local CAD/URDF edits. Findings use one-based lines from the reviewed main snapshot; source links are pinned to that commit. The review used pure tests and simulated hardware boundaries. No robot connection, physical CAN command, deployment, or hardware validation was performed.

All CS findings remain unresolved in this review. CS22 overlaps G11 in the [gateway appendix](gateway.md). See the [finding index](finding-index.md) for consolidated status and the [repository review](../2026-09-29-repository-review.md) for migration and completed cleanup.

P1 means a concrete safety or correctness issue to repair before relying on the relevant hardware workflow. P2 means a reproducible defect or material gap that should be addressed in normal development. Hardware-dependent conclusions are explicitly distinguished from software behavior.

## Architecture and current implementation status

- Current master is a **right 5-DOF bench arm**, including `right_lower_arm_yaw`: [config/robot.yaml:1](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/config/robot.yaml#L1), `:15`. Root `config/` plus `assets/urdf/marengo.urdf` are the current durable robot description. The restored August 8 branch instead describes the earlier 4-DOF bringup architecture; importing that old tree wholesale would lose subsequent commissioning/control work.
- The central direction is sound: URDF/config -> pure dynamics -> Berthier control -> Davout filtering/state machine -> Robstride CAN. Joint/motor sign and gearing conversion are concentrated in Davout. Proto defines wire messages; Chappe carries telemetry and commands; the Pi binary wires execution and persistence.
- Berthier implements actual gravity, impedance, position, and torque-only control. Latest main fixed the earlier `TorqueOnly = GravityComp` alias: torque-only now uses a finite latched `tau_cmd` and hard-zero gains. Position control is a substantial bench-tuned heuristic controller with trapezoid/cosine reference generation, lead limiting, friction, damping, integral trim, and stall recovery. This is mature bench code, not a general humanoid planner.
- Davout has targeted commissioning enable, free-drive sensing leases/TTL, joint/motor transforms, envelopes, and partial-enable rollback. However several safety guarantees fail in the current executable logic, as detailed below.
- Homing implements registry/state labels, manual-reference verification, and pure Hall truth-table logic. GPIO E-stop/Hall I/O, sensor search, and offset execution are not wired to the Pi runtime. Do not interpret the presence of those configuration fields as functioning hardware homing.
- `armee-kinematics` primarily parses limits/envelopes; it is not a general FK/IK solver. `armee-dynamics` numerically differentiates URDF potential energy, with fixed Z-down gravity and a fixed base. It omits nonconfigured joint angles by treating them as zero. This fits a bolted revolute bench arm; floating-base humanoid state, prismatic/mimic motion, inertia/Coriolis/contact dynamics, and torso-orientation gravity are future work.
- IMU acquisition publishes torso quaternion telemetry on a separate thread. It is not integrated into gravity compensation/state estimation. `probe` and `wave-demo` binaries are logging scaffolds. Despite its name, `motor-repl` is a one-command process, not a persistent controller.

## P1 findings

### CS01 — Empty CAN drains keep the communication watchdog alive forever

**Evidence:** [crates/robstride/src/bus.rs:378-381](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/robstride/src/bus.rs#L378) returns `Ok(0)` for an empty nonblocking queue. [crates/davout/src/lib.rs:882](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L882) unconditionally sets `last_recv` on that result. Berthier drains every control tick at [crates/berthier/src/loop.rs:720](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/loop.rs#L720). The watchdog only examines this global timestamp at [crates/davout/src/lib.rs:1025-1039](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1025). Active feedback deliberately has no per-joint TTL (`:386-395`).

**Trigger/consequence:** CAN feedback stops after a cached sample exists. Empty drains continually refresh `last_recv`, cached feedback remains present, and control continues sending using an obsolete pose. Even after fixing empty drains, one responding motor can keep the global timestamp fresh while another motor goes silent.

**Verified reproduction:** Actual latest-main `Supervisor<MemoryBus>`, watchdog=1 ms, wait 5 ms, drain with zero RX, then send command: `count=0`, command accepted. Existing watchdog unit tests sleep then send without executing the normal empty-drain path, so they miss this.

**Fix:** Update freshness only on decoded samples. Track age per active motor address, use the original receive timestamps, require all active joints fresh after a bounded enable bootstrap, and preserve old cached samples solely as diagnostic data. Include joint/address/age in fault telemetry. Test total silence, a single silent motor while peers reply, unknown-only CAN traffic, and re-enable after a long disabled gap.

### CS02 — The torque rate limiter can raise output above the hard torque cap

**Evidence:** [crates/davout/src/lib.rs:1459-1470](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1459) clamps torque before rate limiting. `seed_tau_ff_rate_limiter` seeds from total measured motor torque at `:588-593`, which can exceed the feedforward cap. `rate_limit_tau_ff` then returns `prev + limited_delta` at `:1625-1633`, without a final hard clamp.

**Trigger/consequence:** Mode transition from a position controller producing substantial stiffness torque; measured torque is used as the new feedforward starting point. The filtered outgoing torque remains above the nominal safety cap while it slews down. The same ordering can defeat a newly triggered danger-zone torque clamp.

**Verified reproduction:** Decode an 8 Nm joint-space feedback sample, seed rate limiter, request 0 Nm with configured feedforward cap 5 Nm: actual filtered outgoing command is **7.400121 Nm**.

**Fix:** Apply hard safety caps after every transformation/rate-limit stage and clamp the rate-limiter seed/state into the current legal envelope. Safety caps must take priority over smooth slew. Test transitions from measured torque above the cap and a danger-zone cap smaller than the previous command.

### CS03 — Nonfinite commands pass Davout and NaN torque encodes as maximum negative torque

**Evidence:** [crates/davout/src/lib.rs:1398](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1398), `:1429`, and `:1453` use comparisons that do not reject NaN. No all-fields finite validation exists before sending. [crates/robstride/src/mit.rs:44-52](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/robstride/src/mit.rs#L44) clamps and casts floats to `u16`; NaN reaches the cast and becomes zero. Position, velocity, gains, and torque are encoded by these helpers at `:86-90`. Testing gain handling also only clamps upper bounds ([crates/berthier/src/gain_runtime.rs:297-336](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/gain_runtime.rs#L297)).

**Trigger/consequence:** A bad protobuf command, malformed YAML value, or calculation produces NaN torque or position. A NaN feedforward is accepted by Davout and encoded to the wire as raw 0, the negative end of the motor torque range. On RS03 that field represents **-60 Nm**, bypassing the intended 5 Nm bench feedforward cap. NaN position with nonzero stiffness similarly produces the negative position endpoint.

**Verified reproduction:** Actual latest-main Davout filter accepts NaN torque. An executable using the repository's unmodified encoder functions reports NaN torque raw=0. This proves the software wire value; the actual physical torque depends on the drive's firmware limits.

**Fix:** Reject all nonfinite command and feedback fields at the Davout boundary, reject negative gains, validate numeric config before constructing policy, and make the Robstride encoder fail on invalid input as a final guard. Test NaN/+Inf/-Inf in each command field and arithmetic overflow. A hardware driver must not silently convert invalid control data to a valid extreme command.

### CS04 — Status fault flags are discarded, detailed faults are truncated, and later status clears faults

**Evidence:** [crates/robstride/src/mit.rs:131](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/robstride/src/mit.rs#L131) hardcodes `fault: 0`. Official RS02 manual page 20 defines status-ID fault bits 16..21. [crates/robstride/src/bus.rs:553-554](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/robstride/src/bus.rs#L553) reads only bytes 0..1 of a type-21 fault, while the manual page 22 defines bytes 0..3 and includes bit16 A-phase current overrun. `:477-486` replaces the whole feedback state with a decoded zero-fault status, overwriting a previously received fault report.

**Trigger/consequence:** A drive reports undervoltage, overcurrent, overheating, encoder failure, or uncalibrated status. The main status path says healthy, some detailed faults disappear through truncation, or a subsequent regular status erases a fault before Davout checks it. UI and enable eligibility can therefore report healthy despite documented vendor flags.

**Verified reproduction:** A standard status ID with bit16 undervoltage set decodes `fault=0` using actual source. Type-21 bit16 is visibly lost by the two-byte decoder. This persists on latest main.

**Fix:** Decode status flags and drive operating mode; preserve full `u32` detailed fault and warning fields through proto as appropriate; define a latched normalized fault representation and explicit clear/reset policy. Do not synthesize a q=0 feedback sample from a fault-only frame. Test every documented bit and fault/status order in the same receive burst.

**Primary source:** [Robstride RS02 vendor manual](https://www.robstride.com/assets/product_manual_robStride02-e7f9f7c4.pdf), pages 20 and 22.

### CS05 — A persisted calibration row makes a joint Verified without a current reference check

**Evidence:** [crates/marengo-homing/src/registry.rs:54-57](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-homing/src/registry.rs#L54) promotes records to Verified using joint-name matching alone. It does not validate device/interface, sign attestation, measured position, method, offset, revision, age, encoder power cycle, or current sensor state. All profiles use the common calibration record path. `set_homing_complete` only checks registry readiness ([crates/davout/src/lib.rs:451-461](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L451)).

**Trigger/consequence:** Restart after motor power loss or after changing motor address, direction, gearing, URDF axis, homing method, or firmware zero. A matching historical name authorizes enable despite stale reference. The vendor RS02 manual explicitly describes mechanical zero as lost on power-off; installed firmware behavior must be checked per model/version.

**Verified reproduction:** Persist a same-name row with device99, `sign_test_passed=false`, position7 rad, offset9 rad, method `none`, and obsolete config revision; recreate HomingRegistry: state is **Verified**.

**Fix:** Separate historical calibration from current-boot verification. Fingerprint the relevant motor/config/URDF reference facts, bind records to hardware identity, and require fresh encoder/sensor plausibility and explicit current reference establishment after power loss or a relevant config change. Keep persisted records as an audit trail, not sufficient proof of readiness. Test mismatched identities/revisions and power-cycle recovery.

### CS06 — Set Zero can verify success against feedback from before the command

**Evidence:** [crates/davout/src/lib.rs:551-554](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L551) discards refresh errors/results, then `verify_zero_after_set` reads any cached position at `:485-490`. There is no requirement for a fresh response after SetZero or an acknowledgement generation.

**Trigger/consequence:** Cached old position is near zero but the drive does not respond to SetZero. The request is transmitted, refresh times out/returns no sample, and verification still records success against the old cache. This undermines the zero gate independently of CS05.

**Verified reproduction:** Seed a cached near-zero sample on MemoryBus, provide no new RX, call `calibrate_joint_zero`: succeeds and records verification.

**Fix:** Drain preexisting samples, record command time/generation, require a new sample or parameter acknowledgement after SetZero, verify the returned zero/current drive state, and fail closed on timeout/receive errors. Return an operator-visible failure ACK. Test cached-zero+no-response and delayed stale queued responses.

### CS07 — `motor-repl set-zero` leaves all drives enabled on success and many failures

**Evidence:** [bins/motor-repl/src/main.rs:360-373](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/motor-repl/src/main.rs#L360) calls `request_enable_for_calibration`, which enables the whole loaded motor set; unknown-joint or verification failures then call `process::exit` at `:367-380`. The success path also returns from the one-shot process without disabling. It does not use the cleanup-aware `Supervisor::calibrate_joint_zero` that Pi already uses. No supervisor Drop cleanup exists.

**Trigger/consequence:** Misspell the joint, omit `--sign-tested`, receive no feedback, or successfully zero one joint. The process has already enabled drives, then exits without returning them to a known disabled state. Host watchdog execution stops with the process. The actual drive persistence depends on its firmware timeout, which the runtime does not configure.

**Fix:** Resolve the target and validate attestation before enable; use the existing calibration transaction with guaranteed best-effort cleanup. Use a Result-based main with a shutdown guard instead of exiting deep in command handling. Narrow calibration enable to the target joint where supported. Test success, unknown joint, missing attestation, RX timeout, partial enable, and disk persist failure with a recording/failing bus.

### CS08 — Disable reports success even when every CAN write fails

**Evidence:** [crates/davout/src/lib.rs:1212](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1212), `:1222-1226` suppress every stop/MIT/disable error and `:1228-1239` marks Disabled and returns Ok unconditionally. API error handling and operator logs therefore cannot detect that motor stop writes failed.

**Trigger/consequence:** CAN send buffer exhaustion, interface removal, transport error, or one failed bus. Software reports Disabled while a drive may keep its previous output. Continuing best effort across all motors is correct; silently erasing all failures is not.

**Verified reproduction:** `Supervisor<FailingBus>` where every TX returns an error still makes `disable_all()` return success.

**Fix:** Attempt every stop, collect failures per address, log them, return a typed aggregate result, and expose an uncertain/stop-failed state distinct from confirmed stopped. Add independent drive communication timeout and hardware power cut integration. Test a failed first motor while later motors still get stop attempts and all-TX failure honesty.

### CS09 — Pi shutdown waits up to five seconds before commanding motors to stop

**Evidence:** [bins/marengo-pi/src/main.rs:991-1002](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/main.rs#L991) exits the control loop, waits for persistence to become idle for up to 5 seconds, then calls disable_all.

**Trigger/consequence:** SIGINT/SIGTERM while Active and SD/config write-behind is slow or stuck. Periodic control stops immediately, but drives retain their last MIT/firmware command during the entire persistence wait. The persistence drain has a valid purpose, but its ordering delays the stop request.

**Fix:** Stop motors immediately when shutdown is requested, publish stop outcome, then drain persistence. Do not rely on a host loop to protect a drive after that loop has stopped. Test with an intentionally slow persist worker and verify recorded disable frames precede the wait. Configure and verify a bounded firmware timeout as a separate safety requirement.

### CS10 — The bench torque policy only bounds feedforward, not stiffness/damping torque

**Evidence:** Davout accepts gains up to motor-rated maxima at [crates/davout/src/lib.rs:1398](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1398), clamps position/velocity separately, and clamps only `torque_ff_nm` at `:1459-1462`. No firmware torque limit or current limit is set on enable. `LimitTorque` and `CanTimeout` parameter IDs exist, but searches find no production writes to either.

**Trigger/consequence:** Operator applies a legal Testing stiffness gain and a modest target error. `kp=5000`, error=0.1 rad is accepted, implying 500 Nm stiffness contribution despite a configured bench joint effort cap of 5 Nm. The drive will saturate at its own configured/rated limits, not the software bench cap. High damping can likewise dominate feedforward.

**Verified reproduction:** Actual Davout filter accepts that 500 Nm nominal stiffness term with effort=5 Nm. This is a proved policy gap; physical torque is a hardware-setting-dependent risk, not a measured 500 Nm robot output.

**Fix:** Define explicitly whether bench policy limits total motor/joint torque or feedforward only. If total torque is intended, account for PD terms and feedback when approving gains/setpoints, reserve torque headroom, and configure/read back drive-local torque/current limits before enable. Test aggressive gain overrides and an externally displaced arm. Prefer a verified drive-side limit as the independent backstop.

## P2 findings and material safety gaps

### CS11 — First activation bypasses torque slew; disable retains a stale previous torque

**Evidence:** [crates/davout/src/lib.rs:1625](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1625) uses `unwrap_or(target)` when no prior feedforward exists. `ControlLoop::set_control_mode` only seeds the limiter when both prior and next modes are non-Disabled ([crates/berthier/src/loop.rs:544-550](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/loop.rs#L544)). `disable_all` resets timing but leaves `last_tau_ff` intact.

**Trigger:** Enter a motion mode immediately with enable, without an intervening zero-command keepalive tick, or re-enable after drives were stopped from a nonzero torque. The first output can step directly to a nonzero or stale torque.

**Verified reproduction:** First filtered request=5 Nm, rate=60 Nm/s and fallback dt=.01 s; actual=5 Nm instead of a .6 Nm first step.

**Fix:** Initialize limiter state to known last sent zero at startup/disable, seed deliberately on each enable/mode transition, and cap dt after long gaps. Test direct disabled-to-gravity/position activation, same-tick enable+hold, and a disable gap from nonzero output. This can be combined with CS02's limiter repair, but needs separate regression coverage.

### CS12 — Post-send safety errors are discarded

**Evidence:** [crates/berthier/src/loop.rs:1003](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/loop.rs#L1003), `:1027`, and `:1051` discard `drain_feedback` errors. The receive path performs measured position/velocity guards and can consume a violating sample before returning an error; e.g. [crates/davout/src/lib.rs:875](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L875), `:1004-1014`.

**Trigger/consequence:** A measured hard-limit violation arrives in the post-send drain rather than the first drain. That guard error is swallowed, and the violating sample is not inserted into the motor cache. With unlucky timing or a single last sample, control can continue without acting on the fault. Main marks an out-of-limits facet, but send does not gate on that facet; the fault return still matters.

**Fix:** Propagate safety errors from every drain, latch faults within Davout at ingestion independently of a caller checking Result, and distinguish benign absence of RX from a measured safety violation. Test a custom bus returning normal samples before send and unsafe samples immediately after send.

### CS13 — Runtime faults are transient, despite `software_estop_latched` naming

**Evidence:** [bins/marengo-pi/src/main.rs:1063-1074](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/main.rs#L1063) replaces `active_fault` with None on the next successful disabled tick. Publishing is lower-rate at `:1087-1089`; `publish_safety` derives `software_estop_latched` from this ephemeral option at `:566`.

**Trigger/consequence:** One tick faults and disables, then subsequent disabled ticks succeed. At 200 Hz versus 25 Hz telemetry the fault may never be included in a SafetyState; even if published, it disappears almost immediately. Another Testing target can automatically re-enable the supervisor via `ensure_active_for_motion` ([crates/berthier/src/loop.rs:465-477](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/loop.rs#L465); Pi testing at [bins/marengo-pi/src/main.rs:499-505](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/main.rs#L499)). UI review independently found intervals that continue after Disable, making this compound behavior particularly risky.

**Fix:** Store a durable runtime fault latch with class, time, and recovery requirements. Publish it until explicit acknowledgement/reset and block automatic re-enable for fault classes requiring recovery. Cancel/expire queued motion commands on disable/fault; separate target submission from enable authority. Preserve convenience auto-enable only under an explicit documented nonfaulted policy.

### CS14 — Danger-zone `fault` actions silently do nothing; shipped velocity clamp cannot brake zero-kd gravity mode

**Evidence:** [crates/davout/src/lib.rs:1560-1575](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1560) handles two string actions and ignores everything else; `DavoutError::DangerZone` exists but is never produced. Actual latest-main config contains an elevated descent rule with `action: clamp_velocity` at [config/control.yaml:204-211](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/config/control.yaml#L204). GravityComp packs velocity=0 and gains=0; clipping an already-zero velocity leaves the motor torque unchanged.

**Verified reproduction:** Measured q=1, dq=-.2 crossing a rule with action `fault`; command is accepted. Existing tests only cover known velocity clamp behavior.

**Fix:** Use a typed action enum, reject unsupported actions at load, implement the declared fault action, and define a physically meaningful fall response. A velocity setpoint clamp only creates braking through nonzero damping; reducing gravity-holding torque can make descent worse. Validate a bounded braking/fault strategy against supported-arm simulation and the bench protocol before commissioning it. Test real measured descent in zero-kd GravityComp.

### CS15 — Numeric and identity validation is incomplete before runtime policies are built

**Evidence:** [crates/marengo-config/src/lib.rs:605-634](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-config/src/lib.rs#L605) checks finite GravityComp gains, but most other safety fields only use inequalities or are not checked. `validate_control_against_limits` at `:652` mainly compares trajectory velocity to the cap. `validate_motors_against_robot` at `:1066-1084` rejects unknown joint names and duplicate addresses, but not duplicate joint-to-motor mappings, invalid direction/gearing, or complete one-to-one identity coverage. `validate_joint_gains_against_motor_type` is not part of normal startup validation.

**Verified reproduction:** Latest-main startup validators accept negative impedance kd, NaN desired velocity (which resolves to NaN), negative torque slew rate, and two different motor addresses for the same joint. Negative slew can panic at `.clamp(-max_step, max_step)`; bad damping reverses the intended software torque; duplicate mapping enables an extra drive while `motor_for_joint` only returns the first.

**Fix:** Provide one validated config object used by startup, hot reload, disk transaction, and runtime overrides. Check finite/range invariants for every safety-critical number, positive rates/acceleration/gearing, direction exactly ±1, ordered nondegenerate bounds, valid motor type agreement, unique joint names, and robot/motor/control/homing coverage. Reject unknown schema fields where typos could silently drop safety intent. Test table-driven invalid configs across every numeric field.

### CS16 — Set Limits tests the wrong feedback cache, so current-pose exclusion normally goes unchecked

**Evidence:** [crates/davout/src/limit_envelope.rs:70-85](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/limit_envelope.rs#L70) checks `last_feedback_samples`, the velocity-derivative scratch map. That map is only populated when Active and cleared by disabled polling ([crates/davout/src/lib.rs:900-903](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L900)) and disable_all (`:1233`). Limit patches are only allowed when non-Active.

**Trigger/consequence:** While limp, submit new hard limits that exclude the current physical joint pose. The supposedly protective guard almost always has no sample to inspect, installs the patch, and the next enable may fault or command the arm toward the new envelope. The unit test injects the internal derivative sample directly and therefore exercises an unrealistically populated state.

**Verified reproduction:** Cached current q=1 rad while Disabled; Set Limits to [-.2,.2] succeeds.

**Fix:** Check a fresh joint-feedback sample from the authoritative feedback cache before installing the new policy; fail or mark explicit unverified limits if fresh feedback is unavailable. Update the test to exercise a real Disabled RX/poll path.

### CS17 — IMU telemetry republishes obsolete samples with fresh timestamps indefinitely

**Evidence:** [crates/marengo-imu/src/driver.rs:84-86](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-imu/src/driver.rs#L84) returns cached `last_rotation` even when no packet was read; `:110-113` does not clear it on reset. Pi publisher constructs a new current wall-clock timestamp for each `Some(sample)` at [bins/marengo-pi/src/imu.rs:132-146](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/imu.rs#L132), so disconnection/silent I2C reads look like fresh healthy orientation. Poll errors are logged but do not end the session or invoke its restart/backoff path.

**Verified reproduction:** With one MockI2c packet, first poll and second poll with no new packet both return the same Some quaternion.

**Fix:** Separate `last_sample` from `poll_new_sample`, preserve sensor/host receive time and sequence, clear caches on reset, publish only new samples, expose stale/unavailable health, and restart on persistent read errors or silence. Test one-sample-then-silence, disconnect, reset, and recovery. This presently affects telemetry; it would become a control safety concern if torso orientation is used later.

### CS18 — BNO085 rotation-vector batch splitting uses the wrong report length

**Evidence:** [crates/marengo-imu/src/shtp.rs:84](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-imu/src/shtp.rs#L84) says rotation vector=12 bytes and advances batches by that length. Standard report 0x05 is 14 bytes; the extra two bytes are rotation-vector accuracy, not an optional timestamp (`:122-133` is misleading). Game rotation vector (0x08), a different report, is 12 bytes.

**Verified reproduction:** A standard 14-byte rotation report followed by a 10-byte gyro report is split into one 12-byte slice, and the gyro is dropped. Current tests construct incorrect 12-byte batches and codify the error.

**Fix:** Use the correct report-ID-specific lengths, parse angular accuracy correctly, and support actual documented variants only with an explicit protocol branch. Feed captured standard batches containing timestamps/multiple sensor reports through tests.

**Primary source:** [Adafruit BNO08x driver source](https://github.com/adafruit/Adafruit_CircuitPython_BNO08x/blob/main/adafruit_bno08x/__init__.py), `_AVAIL_SENSOR_REPORTS` specifies 0x05=14 and 0x08=12.

### CS19 — The first Testing hold ignores its requested gains when coming from another mode

**Evidence:** [bins/marengo-pi/src/main.rs:484-506](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/main.rs#L484) applies overrides before entering Position. `GainRuntime::apply` ignores overrides outside Position/Impedance ([crates/berthier/src/gain_runtime.rs:86-107](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/berthier/src/gain_runtime.rs#L86)). Encoded wave handling at [bins/marengo-pi/src/main.rs:455-483](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/main.rs#L455) continues before gain fields are examined at all.

**Trigger/consequence:** Operator starts a low-gain Testing hold from Disabled/GravityComp, expecting submitted kp/kd; the first command runs with YAML gains instead. Repeated later commands behave differently. The frontend reviewer is checking which widgets rely on this ordering.

**Fix:** Validate request and transition mode first, then atomically apply gains+target; return the actually applied gains in the command outcome. Explicitly reject or apply wave gains consistently. Test the same request from every previous control mode.

### CS20 — Several `motor-repl` commands signal effects that never reach the running controller

**Evidence:** [bins/motor-repl/src/main.rs:384-411](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/motor-repl/src/main.rs#L384) updates an in-process mode/torque latch and then the one-shot program exits; it does not tick or send these changes to Pi/Chappe. `jog` uses `Supervisor::send_joint_command`; that legacy path constructs kp=0,kd=0,ff=0 ([crates/davout/src/lib.rs:1072-1080](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1072)) so the purported new position has no servo effect. `status` prints a newly created local supervisor rather than querying the live process.

**Fix:** Make operator commands a client of the single Pi control owner with explicit ACKs, or implement a genuinely persistent CLI controller with bounded lifecycle/cleanup. Remove or label no-op commands until then; make Jog use the real position primitive. Require exclusive bus ownership so two local processes cannot compete for CAN/control. Test command outcomes on a recording bus or IPC fake, including what survives process exit.

### CS21 — Pure dynamics accuracy/reference tests are disabled and currently fail

**Evidence:** [crates/armee-dynamics/tests/golden_tau_g.rs:6-7](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/armee-dynamics/tests/golden_tau_g.rs#L6) explicitly acknowledges stale golden data and all seven tests are ignored. [crates/armee-dynamics/src/urdf_gravity.rs:168](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/armee-dynamics/src/urdf_gravity.rs#L168) also ignores a pure cached-chain test with obsolete link naming. Default tests mostly assert nonzero torque or internal consistency, not an independent physical oracle.

**Verified results:** The ignored chain test fails looking for `right_upper_arm_stub`. Six of seven ignored golden tests fail; q=0 expected≈0 produces -0.14715 Nm, and ±.3 rad differ from the old values by about .14 Nm. The archived COM changed, so these are proved stale tests, not automatically proof the current dynamics algorithm is wrong.

**Fix:** Create immutable independently calculable simple fixtures, unignore pure tests, validate the current master model against MuJoCo/another independent implementation, and separately record measured bench model calibration with revision/hardware identity. Do not make tests pass merely by copying current algorithm output into the expected values.

### CS22 — Core control crates do not compile natively on Windows

**Evidence:** After supplying protoc, native `cargo test -p berthier -p davout -p marengo-homing -p marengo-pi -p motor-repl` fails with seven unguarded `std::os::unix` references in [crates/chappe/src/ipc.rs](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/chappe/src/ipc.rs), including lines 135,155,157,198,201,236. This is a library compile failure, broader than the documentation's claim that only Unix-socket tests fail on Windows.

**Fix:** Put Unix transport implementation and tests behind `cfg(unix)`/a transport feature, provide a cross-platform desktop transport or explicit unsupported adapter, and keep pure Berthier/Davout tests platform-independent. Add Windows/macOS CI for the portable workspace slice. Keep SocketCAN/Linux-I2C platform-specific on the robot; moving developer source to Windows does not require rewriting those physical drivers.

### CS23 — Gravity COM transforms discard joint-origin translations

**Priority: P1. Discovered during implementation on September 29, after the original review.**

`UrdfGravityModel::link_com_world` computes `transform * com_local` with a
`nalgebra::Vector3`. An isometry acting on a vector applies rotation, whereas a
center of mass is a point and also needs translation. Consequently upstream joint
origins do not contribute to each link's world COM or holding torque. The defect
is present in the original reviewed
[gravity implementation](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/armee-dynamics/src/urdf_gravity.rs).

**Reproduction:** An immutable two-link fixture has a 2 kg upper-link COM 0.5 m
below the shoulder, an elbow 1 m below the shoulder, and a 3 kg distal-link COM
0.25 m below the elbow. At shoulder pi/2 and elbow zero, the independent holding
torque is `(2*0.5 + 3*1.25)*9.81 = 46.5975 Nm`. Original code returns
`17.16749999477574 Nm`, losing the distal mass's 1 m shoulder lever arm. Reversed
configured joint order reproduces the same physical error. The prior loose and
ignored tests do not establish this transform invariant.

**Repair:** Transform a `Point3` and use its coordinates for potential energy.
Four active public-interface analytic tests cover pendulum sign/magnitude,
coupled distal loading, rotated joint origin and configured joint order. The two
coupling/order tests fail before repair and pass afterward. Independent mutation
checks additionally require the suite to reject omitted translation, reversed
gravity, ignored mass and omitted joint rotation.

**Physical consequence and remaining gate:** The corrected calculation changes
production gravity torques. No installed drive or physical arm was exercised;
the example establishes a software error, not the amount of error on the current
robot. Revalidate current-master inertials/kinematics against an independent
physics implementation and repeat applicable supported-arm commissioning before
motion. Do not copy old bench acceptance or expected values from the defective
algorithm into new tests. This repair does not finish CS21's production-model
validation or T28's URDF/MJCF consistency work.

## Architectural gaps requiring explicit decisions

1. **Drive-local timeout is not configured/read back.** `ParameterId::CanTimeout` exists ([crates/robstride/src/params.rs:33](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/robstride/src/params.rs#L33)) but runtime never writes it. The host watchdog cannot act during a hang, crash, SIGKILL, or after a one-shot CLI exits. Vendor default can be zero/disabled; actual installed firmware settings were not read. Make a verified per-model timeout and torque-limit handshake part of enable and a hardware commissioning test. Model hold-up/arm support consequences of torque removal separately from preventing runaway.
2. **E-stop/Hall inputs are scaffolds.** `Supervisor::set_hardware_estop` has no Pi GPIO caller; `publish_safety` hardcodes false at [bins/marengo-pi/src/main.rs:565](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/main.rs#L565). Hall config can be parsed but manual verification records `hall_three_sensor` without sensor search/health enforcement ([crates/marengo-homing/src/verify.rs:88-94](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-homing/src/verify.rs#L88)). Fail startup for an unsupported selected homing method instead of implying it is active. Map per-input polarity independently; `ThreeHallInputs::from_config` currently uses the home sensor's `active_high` for all three ([crates/marengo-homing/src/sensor.rs:53-59](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-homing/src/sensor.rs#L53)).
3. **Wrong-sign watchdog is disabled and coordinate assumptions conflict.** Master sets enabled=false ([config/control.yaml:217](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/config/control.yaml#L217)) because global signs do not cover the joints. The check inspects joint-space FF before motor conversion ([crates/davout/src/lib.rs:1500-1505](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L1500)), while config docs/ADR describe a motor-space expected sign. q-sign is also not a general gravity sign oracle for COM offsets/multijoint poses. Correct the coordinate contract, create per-joint/profile commissioning expectations, and validate the policy physically before enabling it. The current disabled state is deliberate, not a newly discovered malfunction.
4. **Safety invariants depend on callers.** Public `enable_targets` asks callers to prefilter eligibility ([crates/davout/src/lib.rs:693-699](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L693)); public unchecked homing helpers, mutable config fields, `homing_registry_mut`, and `bus_mut` permit policy bypass. Separate test-only adapters from production API, make configuration private/validated, and make Davout enforce eligibility itself. A deep safety module should accept intent and own all the checks rather than rely on a particular bin sequence.
5. **Controller regression coverage needs system-level physics tests.** PositionHold is over a thousand lines of coupled breakaway/stall/lead/friction branches; many tests mirror helper formulas and replay the planner as feedback. Add independent plant simulations with stiction, delayed/lost CAN samples, encoder quantization, inertia, saturation, payload/COM error, moving retargets, and operator interruption. Keep unit tests but evaluate behavior/error bounds and motor-safe outcomes, not just an implementation path.
6. **Dynamic model scope must be honest.** Prismatic joints are accepted by kinematics but motion is omitted by `UrdfGravityModel::link_transform` ([crates/armee-dynamics/src/urdf_gravity.rs:93](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/armee-dynamics/src/urdf_gravity.rs#L93)); mimic relationships and floating-base orientation are also omitted. Reject unsupported joint types/model topology on construction until implemented. Cycle/zero-axis/invalid-mass validation would prevent malformed URDF from hanging or yielding nonfinite dynamics. Pure torque newtype has public Vec storage, so it is an intent marker rather than mathematical proof.
7. **Realtime work should be bounded.** The Pi drains unbounded command streams before ticking, formats/publishes logs, allocates many vectors/maps in every tick, and registry persistence writes synchronous YAML during calibration. These are architectural/performance risks rather than measured regressions here. Bound command work per period, make stop priority explicit, measure jitter on the actual Pi under load, and move durable/audit work off the control owner without losing ACK correctness.
8. **Configuration transactions/ACK coalescing have separate findings.** Gateway/storage reviewer confirmed latest-request coalescing can discard a motor-limits persist request and its correlated ACK, and URDF activation races with Pi expansion/rollback. Those should be addressed as one owned serialized durable transaction stream with explicit request outcomes. The [gateway appendix](gateway.md) provides the exact evidence and fixes as G04–G09; these are shared architecture concerns rather than additional independently counted defects.

## Already fixed on latest main / restored-branch differences

- Ordinary partial-enable rollback was missing in the restored branch at [crates/davout/src/lib.rs:625-635](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/davout/src/lib.rs#L625). Latest main routes through `enable_targets_inner` and on any enable/mode error attempts disable-all (`:786-817`), with passing regression coverage. Do not recommend it as an outstanding main defect. CS08 still means rollback's stop result cannot be trusted.
- Restored branch's TorqueOnly is a GravityComp alias. Latest main has an independent finite torque command latch and tests; this alias is fixed.
- Latest main has per-joint drive_active/homing/out-of-limits facets, commissioning scope selection, free-drive stale feedback expiry, and master 5DOF config. Those are absent/stale in the restored source and must be preserved during migration/merges.
- Local CAD/URDF changes have not been declared mechanically correct by this review. Mechanical reconciliation of the recovered exports against the latest master remains required before physical work; file-shape checks do not establish live robot correctness.

## Executed checks and reproducible evidence

All commands are pure native Windows tests or MemoryBus/MockI2c source-linked reproductions. No hardware test was run.

1. `cargo test --locked -p robstride -p armee-dynamics -p armee-kinematics -p marengo-config -p marengo-imu`: **129 passed**, **9 ignored**, no failures. Counts: robstride29, dynamics9, kinematics16, config63, IMU12.
2. With `PROTOC` pointing to the recovered Windows protoc executable, `cargo test --locked -p davout -p marengo-homing`: **90 passed**, no failures/ignores (Davout67, homing23). Across these seven crates: **219 passed, 9 ignored**.
3. Native Berthier/Pi/motor-repl test attempt: **blocked by Chappe library compile errors** (CS22); those tests are unverified here. Full hardware SocketCAN/Linux-I2C tests are unverified.
4. `cargo test --locked -p armee-dynamics -- --ignored --nocapture`: fails the obsolete pure link-chain test. `cargo test --locked -p armee-dynamics --test golden_tau_g -- --ignored --nocapture`: **1 passed, 6 failed**, all stale-golden mismatches (CS21).
5. Isolated source-linked reproductions use the actual reviewed crates with MemoryBus and MockI2c. An intentionally invalid calibration fixture stayed in the isolated evidence directory. These reproduced CS01, CS02, CS03, CS05, CS06, CS08, CS10, CS11, CS14, and CS16. They exercised the real nonblocking receive and command/filter behavior without opening CAN.
6. Additional isolated checks reproduced accepted negative damping, NaN desired velocity, negative slew and duplicate motor mappings (CS15), and cached IMU sample replay (CS17). A protocol reproduction called the actual Robstride library and included the unmodified SHTP module to prove NaN wire encoding, discarded status faults, and the 14-byte rotation plus 10-byte gyro alignment error (CS03, CS04, CS18). The evidence binaries ran successfully; the harnesses and raw outputs are retained locally with the migration recovery records.

## Recommended implementation sequence

1. Repair command validity, per-active-motor freshness, full fault decoding/latching, and torque limiter cap ordering. Add integrated nonblocking-RX tests before hardware use.
2. Repair current-boot calibration/fresh SetZero verification, reliable stop reporting, CLI cleanup, and immediate shutdown stop. Add drive-local timeout/torque readback and fault-recovery authority.
3. Bound total servo torque policy and replace inert danger-zone actions with a validated measured-motion response. Finish E-stop/sensor input integration or explicitly reject unsupported configurations.
4. Repair config validation, live-limits pose guard, request/gain atomicity, operator command ownership/expiry, and all durable transaction/ACK findings from the storage reviewer.
5. Restore independent physics goldens/simulation regressions, fix IMU freshness/framing, and make pure core builds/tests run on Windows and macOS while the robot drivers remain Linux-specific.
6. Refresh codemaps/runbooks around master 5DOF + commissioning scope, consolidate duplicate/stale profile docs, and preserve CAD/export evidence with explicit model revisions.
