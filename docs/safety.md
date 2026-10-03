# Safety

Read this before enabling motors on the bench or robot.

## Principles

- **No silent enable.** Actuators must not energize without an explicit, logged enable sequence.
- **E-stop first.** Hardware E-stop cuts power independently of software. Software must treat E-stop as latched until a documented reset procedure.
- **Homing before motion.** After power-on or fault, run a homing sequence that verifies encoders, limits, and I/O before tracking commands ([Robot Programming Best Practices](https://intelligentintegrators.org/robot-programming-best-practices/)).
- **Davout in the path.** No joint command reaches [robstride](../crates/robstride/) without passing [Davout](../crates/davout/) limit and rate checks.

## Bench mode (development)

- **Command velocity caps:** set in `config/control.yaml` only — per-joint override, `actuator_groups`, or `motor_type_defaults` (see [ADR 0010](decisions/0010-actuator-velocity-cap-resolution.md)). Davout and Berthier enforce the resolved cap at runtime; do not rely on `motors.yaml` `bench.velocity_limit_rad_s`, `robot.yaml` `bench.max_joint_velocity_rad_s`, or URDF joint velocity for command limiting.
- **Torque caps:** keep below production limits via `config/robot.yaml` (`robot.bench`) and per-joint overrides in `config/motors.yaml`; Davout also applies per-`motor_type` `tau_ff` limits and rate limiting from `control.yaml`.
- Use `motor-repl` only with operators at the robot and clear workspace.
- Prefer the virtual CAN test harness (`just vcan`) or simulation (`just sim-check`) before live CAN when developing control logic.

## Homing and zero (see [homing.md](homing.md))

- **Homing before motion.** After power-on or fault, every configured joint must reach **Verified** before supervisor `Ready`.
- **Sensor health first.** When Hall/limit inputs are configured, startup checks sensor wiring and stuck-state before homing search.
- **No blind hunting.** Out-of-range or stale-zero joints may only use constrained recovery (manual reference or sensor homing), not normal gravity/hold/impedance.
- **Calibration audit.** Host registry at `var/calibration/zero_registry.yaml` records who/when/how zero was established; firmware `SetZero` alone is insufficient.
- **Current reference.** Every fresh registry starts `Unhomed`; saved calibration is history and cannot authorize checked home or normal Enable. Corrupt/unreadable history returns an error without replacing its bytes. See [ADR 0022](decisions/0022-calibration-history-and-current-reference.md).
- **Permission at output.** Davout's private owner-bound reference gates Ready,
  scoped/normal Enable, Active shortcuts and output. Caller-set history flags
  cannot grant it, and initial virtual test fixtures establish no hardware
  readiness. See [ADR0023](decisions/0023-private-current-reference-authority.md).
- **Physical reference grants.** Only the explicit physical owners acquire a grant
  ([ADR 0036](decisions/0036-physical-robstride-reference.md)). Acquisition
  needs a type-2 ack after the target's SetZero and a type-17 `mechPos` readback
  after the ack, both within tolerance. It stops all drives before storage and
  grants only after the journal commit is durable. Grants are per joint,
  accumulate, bind the MCU UID and coordinate epoch, and are local to the owning
  process. A grant is revoked per joint for a UID change, a missing or mismatched
  UID in the type-0 check at Enable (done before any Enable frame), a coordinate
  discontinuity, Calibration drive mode, or no feedback for longer than
  `comm_watchdog_ms` outside reference work. Silence the host itself caused is
  not counted: a type-24 On it held back from the drive's possible post-SetZero
  blackout counts from the quiet's end, and a just-enabled drive counts from its
  Enable echo (see *Host-caused silence* below). A revocation is checked on
  demand but latched: the check that finds a lapse revokes that joint for the
  rest of the process. Fault, E-stop, uncertain stop, shutdown and model/policy
  changes revoke all grants. `zero_sta`/`add_offset` writes and type-22 saves
  are never sent.

Manual reference is the qualified commissioning workflow. Bench qualification is
pending, and the three-Hall workflow is unimplemented. Home and enable in one
`marengo-pi` process. The former separate CLI Set Zero → home → Pi enable
sequence still refuses, because grants do not cross processes. See
[homing.md](homing.md) for the procedure and the stop-path caveat.

## Enable / disable sequence (target behavior)

1. Verify E-stop released (hardware).
2. Precheck CAN, faults, sensor health (if configured).
3. Homing → all joints **Verified** → software `Disabled` → `Ready`.
4. Operator command `Enable` → `Active` (motors may track).
5. Fault, homing fault, or E-stop → `Disabled` (ramp down, then disable drives).

Document actual pin/signal mapping in [hardware/electrical/wiring/](../hardware/electrical/wiring/) as it is finalized.

## Upright-pose incident (4-DOF arm)

During early arm bring-up with the arm elevated (shoulder/elbow up), motion stopped while the arm was unsupported. Without gravity feedforward, the arm fell rapidly into the operator workspace.

**Required mitigations before repeat tests:**

1. **Control mode:** Use **GravityComp** (`kp=0`, `kd=0`, `torque_ff=tau_g`) — not position-only holding in elevated configurations.
2. **Enable sequence:** Arm supported manually or in a fixture for first enable; E-stop reachable before `Enable`.
3. **Sign test:** Per-joint small `torque_ff` pulse; verify direction matches URDF before full `tau_g`.
4. **Caps:** Davout per-`motor_type` `tau_ff` limits (RS02/RS00 lower than RS03/RS04); rate-limit `tau_ff` steps when enabling.
5. **Danger zones:** `config/control.yaml` rules (e.g. elevated shoulder pitch + downward velocity) → clamp or fault.
6. **Comm watchdog:** CAN receive timeout → `Disabled` and logged fault.
7. **Measured position guard (ADR 0009):** feedback `q` beyond URDF hard limits + small slack → `Disabled` (independent of command path).
8. **Exit:** `disable_all` on process exit / SIGTERM where the driver supports it.

See [ADR 0004](decisions/0004-control-modes-and-mit.md) and [hardware/docs/decisions/0002-robstride-protocol.md](../hardware/docs/decisions/0002-robstride-protocol.md).

## Bench procedure (gravity compensation target)

This requires a qualified current reference in the installed owner: `home
<joints...> sign-tested` in the same `marengo-pi` process that will enable. A
fresh CLI `home` cannot create that reference from history.

1. Verify E-stop and clear workspace.
2. Installed owner confirms current-reference Ready; request Enable only with arm supported.
3. `gravity-on` — verify backdrivability and no runaway.
4. **Upright pose test:** slowly release support; elbow/upper arm must not free-fall.
5. `disable` before leaving the bench (`gravity-off` enters TorqueOnly with `τ_cmd≡0` — diagnostic no-FF, not a full disable).

## Config hot-reload (limits)

- **Surface:** numerical hard/soft position bounds, torque cap, and `velocity_max_rad_s` only. `device_id`, CAN interface, `direction`, motor type, gearing, and membership/DOF changes require YAML + restart (+ re-home when wiring/zero changes).
- **Active refusal:** Davout refuses `apply_limit_patch` / `rebuild_limits` while operational mode is `ACTIVE`.
- **ACK honesty:** Gateway waits for live `limit_patch` Pending, then `limit_patch_persist` Durable/Failed, before HTTP success. Do not treat Pending alone as repo truth.
- **URDF expand-only:** Bench Set Limits widens in-memory + on-disk URDF hard when taught hard exceeds URDF (Davout hard = URDF ∩ bench). Soft uses ADR 0009 inset in `control.yaml` (not soft≡hard). See [ADR 0017](decisions/0017-bench-set-limits-urdf-expand.md).
- **Persist-degraded ≠ NeedsRestart:** write-behind failure uses a distinct banner/retry. Restart while YAML/URDF is stale would revert live limits.
- **Dual-writer:** Pi owns live apply and YAML/URDF write-behind. Gateway master-tree transactions use CAS `expected_revision` for durable YAML/URDF edits on the single master SoT; there are no inactive bringup profiles. Local git sync is Durable-gated via `marengo-limit-sync` only.
- **Deploy must not clobber Set Limits:** `install-pi.sh` preserves previous Pi motors hard + control soft + expand-only URDF union for overlapping joints after config rsync (Set Limits Apply remains SoT). Opt out with `MARENGO_REPLACE_LIMITS=1` when intentionally shipping git limit changes over taught envelopes.
- Soft bounds clamp into the new hard envelope in the same txn (Inventory Range must show post-clamp values).

See [ADR 0012](decisions/0012-config-db-overrides.md) and [ADR 0017](decisions/0017-bench-set-limits-urdf-expand.md).

## Hardware status poll (type-4 solicit)

While the **Hardware** page is open and operational mode is not `ACTIVE`, Consul POSTs `/command/motor_status_poll` about once every 2.5 s (gateway global rate limit ~0.5/s, burst 2). Gateway → Chappe `robot/motor_status_poll` → Davout `solicit_status_feedback`, which re-TX RobStride **Disable (type-4)** once per loaded motor that does **not** already desire Active Reporting (global `active_reporting_diagnostics` or an unexpired sheet/modal lease). Motors reply with **OperationStatus (type-2)**; the normal 200 Hz drain updates `RobotState`. No-op while `ACTIVE` (MIT status replies own that path). Best-effort per motor on TX failure. HTTP 200 is publish ACK only.

While not `ACTIVE`, Davout omits free-drive feedback older than ~5 s from `joint_feedback` / `RobotState` so Consul Online/Offline tracks recent RX rather than sticky cache membership after the first successful poll.

## Active Reporting leases (type-24)

Consul may hold **Active Reporting leases** (operator UI: Enhanced logging) via gateway → Chappe → Davout while a Hardware joint settings sheet (Set Limits) or Telemetry actuator modal is open — not for the whole Hardware page. Leases never enable type-24 while operational mode is `ACTIVE` (MIT status replies own that path). Bench profiles with `active_reporting_diagnostics: true` already force type-24 when not ACTIVE, so a lease may be a wire no-op until that global flag is off — the lease path still ships for global-off workflows. HTTP 200 is publish ACK only; Consul shows **Enhanced logging**, not confirmed wire reporting. TTL expiry on the Pi is the backstop if release is lost. `client_id` is not an auth boundary (same honesty as set-zero).

While free-drive sensing is desired (sheet/modal lease or global diagnostics flag), Davout **re-asserts** type-24 enable on a ~1 s heartbeat and when a joint’s feedback goes stale (~200 ms with no RX). Motors can drop Active Reporting mid-sweep; without retry, Consul freezes on the last sample and Set Limits Apply would teach a tiny band.

## Single motion owner (stdin vs Chappe)

`marengo-pi` has two command sources: stdin (MCP scripted bench sessions, a person on SSH) and Chappe (Consul through the gateway). **Exactly one of them owns motion for the life of the process**, claimed at launch with `--motion-owner stdin|chappe` (or `MARENGO_MOTION_OWNER`); the default is `chappe` (the systemd service has no stdin and is steered by Consul). The MCP remote preamble exports `MARENGO_MOTION_OWNER=stdin`, so every MCP-started session owns its own motion and an open Consul tab cannot steer it. There is no runtime hand-over: ownership ends when the process exits.

| Class | Commands | Non-owner |
|---|---|---|
| Stop | stdin `disable`, `quit`, `hold-off`, `impedance-off`; Chappe `enable(false)`; SIGINT/SIGTERM | **always accepted** |
| Observe | stdin `status`; Chappe status poll / Active Reporting lease | accepted |
| Motion | `enable`, `home`, Set Zero, `hold-on/at`, `wave`, `gravity-on/off`, `impedance-on`, `torque-cmd`, Testing `mit_command_batch`, runtime kp/kd tuning | **refused**: logged, printed (stdin) and published as `ActionEvent{action: "motion_refused", accepted: false}` on `robot/audit/action` |

Further rules enforced in `marengo-pi`/Berthier:

* Testing batches obey the same reference-queue gate as Chappe enable: refused whole while the reference queue is busy or Davout holds a reference, before any gain, mode or enable side effect.
* An **operator disable** (stdin `disable`, Chappe `enable(false)`, or the fail-closed stop below) stands until an explicit enable: a later `hold-at`/`wave`/Testing position command is refused with `drives were disabled by an operator` instead of silently re-enabling (`ControlLoop::forbid_implicit_enable`). A process that was never operator-disabled keeps the existing "motion command re-arms" behaviour. Safety (non-operator) disables are unchanged: Davout still refuses re-enable while a fault is latched.
* `robot/enable` is a stop channel. If its broadcast receiver reports `Lagged`, a Disable may have been dropped and cannot be recovered, so the drain **stops every drive, cancels the reference queue, forbids implicit re-enable and discards the queued survivors**, then publishes `ActionEvent{action: "stop_on_lag"}`. Lag is never treated as an empty channel.
* A Testing gain override is refused (`GainOverrideNotApplicable`) outside Impedance/Position instead of returning Ok; a Testing POSITION batch applies its gains after the mode is entered.
* Configuration-plane Chappe commands (Set Limits, config overlay tuning) are not motion and are not gated by the lease; they remain refused while a reference is busy.

## Graceful owner shutdown

The installed Pi exits dispatch when Quit or the shared shutdown flag is observed,
then discards retained controller intent and attempts the configured Davout exit
stop before waiting for persistence. It retains the exact stop Result/StopReport;
an explicitly skipped stop and a failed delivery remain distinct from successful
storage. An already admitted synchronous operation completes before the next
shutdown check. This does not establish command-flood priority or hard preemption.

Closing persistence admission leaves accepted retained/in-flight work on its
independent worker. Bounded drain reports real write/publication failures,
unfinished work and actual observed thread termination. Timeout does not cancel
filesystem I/O. Local publication, Disabled intent and accepted stop writes do
not establish client delivery or physical stop/support acceptance. See
[ADR0024](decisions/0024-stop-before-persistence-shutdown.md) and the
[software evidence](reviews/2026-09-29/batch07-stop-before-persistence.md).

## Position-hold fuses (Berthier)

In `ControlMode::Position`, Berthier trips two fuses on a 2000 ms no-progress budget. Progress
counts only when the encoder reaches a new best level by more than Davout's feedback-grid threshold
([ADR 0025](decisions/0025-measured-ascent-progress.md)). A trip latches a controller fault and
disables. The fault does not clear on its own.

- **Outbound ascent stall** (`AscentStall`): a non-home target sits more than 0.03 rad ahead of `q`
  in the direction of the target's own side of home (above `q` for a positive target, below `q`
  for a negative one) and the encoder makes no new best level in that direction. The direction
  comes from the latched target and measured `q`, never from `τ_g`, so a wrong gravity model
  cannot switch the fuse off, and a joint whose range lies below home is covered.
- **Hold tracking failure** (`HoldTracking`): this fuse covers any target, home included. It trips
  when `|q − target|` is more than 0.03 rad, the net commanded torque `tau_p + tau_ff` (which
  includes model `τ_g`) points away from the target, and `q` makes no new closest approach. This
  is the case where a wrong gravity model pushes a latched hold off target, including an
  exact-zero hold-on. Uncommanded peers are exempt, and so are wave-driven joints, which carry
  the wave-stall fuse instead. The torque it judges uses the `kp` on the wire, including during a
  100 ms gain ramp.
- **Wave stall** (`WaveStall`): while a wave commands at least 0.05 rad/s, measured `q` must
  leave its credited level by 0.02 rad (or the grid threshold) within 2000 ms. A wave is refused at
  start if an argument is non-finite, its range leaves the soft limit envelope, or its peak speed
  or acceleration exceeds the joint's velocity cap, `position_trajectory_velocity_rad_s` or
  `position_trajectory_accel_rad_s2`. The initial step to the wave's first target is not shaped;
  it is bounded by the lead clamp and Davout.
- **Retargets never renew a budget.** Only measured progress, or the commanded condition ending,
  does (a target flipping the outbound direction starts a new episode). A stream of small
  retargets cannot keep a stalled or sagging joint unfused. Residual: alternating up/down
  retargets that each end the commanded condition still restart it.
- **Controller invariants:** a Position tick without a latched setpoint, mismatched joint
  vectors, a gravity-model error or non-parallel gain inputs latch a controller fault and discard
  intent, instead of erroring every tick until a watchdog acts.
- Both errors report `q`, `target`, `tau_p`, `tau_ff` and `tau_g` at the trip.
- A latched target within two feedback counts of zero counts as home and is commanded as exactly
  `0.0`. Home/outbound classification therefore never depends on a single encoder count.
- A tracking trip at home points to a gravity or model fault. Do not raise kp or ki to get past
  it; fix the model first (bench 2026-10-03 pitch trip).

## Known software gaps (see also [position-hold-control-review.md](position-hold-control-review.md))

- **Hardware E-stop wiring:** `Supervisor::set_hardware_estop` exists but Pi GPIO/input is not yet connected at runtime. Treat physical E-stop as authoritative; do not assume software `Disabled` reflects the hardware line until wired.
- **Danger zones:** Rules evaluate **measured** joint `q`/`dq` (not commanded MIT fields). Prefer `clamp_torque` when Berthier sends `kd_mit = 0` and velocity clamps alone cannot slow gravity-driven descent.
- **RS03 MIT velocity scale (fixed 2026-10-03):** RS03 velocity is ±20 rad/s
  on the wire, not ±50 (manual §4.1.2, bench capture
  `cd-20261003T145133Z`). Before the fix commanded `v_des` reached RS03 drives
  at 0.4×, so the drive's `kd` damping target and danger-zone
  `clamp_velocity` limits (e.g. 0.45 rad/s arrived as 0.18) were 0.4× intent,
  and RS03 feedback velocity read 2.5× high. Both now reach the drive at their
  intended value, so RS03 (shoulder pitch/roll) ramps and settles behave
  differently with unchanged config: less drag at the end of ramps and faster
  clamped descents. Re-check with the arm supported. Values tuned under the old
  scale and the RS00 open question:
  [robstride-mit-ranges.md](commissioning/firmware/robstride-mit-ranges.md).
- **Limit envelope:** Davout uses `max(|dq_cmd|, |dq_meas|)` for velocity-scaled margins so gravity-driven motion cannot shrink the envelope unexpectedly.
- **Limit envelope fails closed (2026-10-03 audit, WP-E):** URDF joint limits that are non-finite or not `lower < upper`, and non-finite soft bounds, are a load error (`UrdfError::InvalidLimits`), never a panic or a clamped guess; a joint with no `control.joints` entry has no policy (no built-in margin defaults). When the kinetic margin swallows the whole soft range the envelope collapses to the soft-range point nearest the measured `q` (hold), not the midpoint. Non-finite targets hold `q`; `measured_position_fault(NaN)` is a fault. A hold-at target is exempt from the kinetic margin only at the soft bottom (+5 mrad) or zero-home, not for every target ≤ 0.005 rad. Expand-only Set Limits refuses inconsistent URDF soft bounds instead of resetting them to the full hard range.
- **Gravity model fails closed (2026-10-03 audit, WP-E):** `UrdfGravityModel` refuses to load when an actuated URDF joint is not in `robot.yaml` (it used to be evaluated at q = 0), when a joint name is duplicated or fixed, for prismatic/mimic/floating joints, a zero axis, non-finite inertials or a cyclic chain. Bench slices that model only some joints must state the locked angles (`from_urdf_with_held`). `gravity_torques` and the pre-enable saturation preflight (`check_gravity_range`, shared by `marengo-pi` and `motor-repl`) refuse on a non-finite torque or a model error instead of reading it as 0 Nm. A gravity-calibration fit that does not converge, or whose posterior covariance is not finite and positive, is refused (`FitVerdict::NotConverged` / `IllConditioned`); an unknown σ is `NaN`, never 0.
- **Gravity preflight and feedback (2026-10-03 audit, WP-G):** stdin Enable, Chappe
  `robot/enable`, and Testing Position auto-enable share one fail-closed preflight.
  It requires each modeled joint's motor, live limit policy and measured position,
  sweeps the coupled gravity model over the joint's live command envelope, and
  refuses missing data, model errors, invalid limits or torque saturation. `force`
  does not bypass this gate. Berthier may send MIT only for active joints, but the
  gravity model couples all modeled positions: after neutral enable bootstrap,
  missing feedback for any modeled joint faults before τ_g is evaluated.
- **Fault authority:** Observed runtime hazards persist across later healthy feedback, Disable and cache clearing. Davout attempts every configured stop address and retains failures; send acceptance is not physical stop acknowledgement. Qualified recovery/reset is not implemented. See [ADR 0020](decisions/0020-lossless-feedback-and-fault-authority.md).
- **Receive integrity and work:** Status/detail feedback requires exactly eight Data bytes. Malformed configured feedback, kernel errors and incomplete receive work latch through fault authority. Every poll is limited to 64 raw frames and 256 nonblocking read attempts across all interfaces, including noise and interruptions; both enable flushes require observed quiescence. Host read order/deadlines do not qualify physical acquisition, drive behavior or Pi jitter. See [ADR 0021](decisions/0021-bounded-can-ingress.md).
- **Enable wire order:** A SocketCAN write only queues a frame. On the bench
  Pi a drive's Reset report went on the wire before its queued Enable but was
  read after Davout went Active. Own writes are therefore read back (`CAN_RAW_RECV_OWN_MSGS`).
  Drive traffic popped before an address's Enable echo is never held to Run and
  never becomes session pose. After the echo, Reset/Calibration latches as before.
  A missing echo latches DriveState once `comm_watchdog_ms` has passed since
  activation (or the target's post-SetZero quiet end, below, when later).
  Echoes are never feedback, liveness or replies.
- **Staggered enable and reporting writes:** Every host frame solicits a drive
  reply, and the bench mcp251x holds only two received frames. On 2026-10-03
  Enable + RunMode to five drives plus their replies (about 2.9 received
  frames/ms for 12 ms, 81% bus load) overran it, and five type-24 Ons did the
  same at startup. On SocketCAN, `poll_feedback` writes Enable + RunMode for at
  most one target per interface each control period, and any remainder at once
  half of `comm_watchdog_ms` after activation. Every target is pending from
  activation, so a target not yet written is never held to Run or admitted as
  pose, and only its own echo (not an older one) arms the strict check; the
  missing-echo latch above is unchanged. Type-24 writes (`sync` On, Off,
  retries, refreshes and the Enable gate's Offs below) take one slot per
  interface per control period.
  Own-message echo is not receive load: the driver builds it in software on
  TX completion, outside the controller's receive buffers.
- **Reporting Off before every Enable:** Robstride drives keep type-24
  reporting across host processes. On 2026-10-03 at 14:51:33 all five were
  streaming before `marengo-pi` started. Paced `sync` had applied only the
  first Ons, so the reference baseline Off, which covered only applied
  streams, left right_lower_arm_yaw streaming through its Enable. At 14:55:24
  it failed with `unexpected drive mode Reset for Disabled`. Enable-to-Run reply
  latency is 1.4-5.2 ms against a 10 ms report period (245 measured Enables;
  [behaviour doc](commissioning/firmware/robstride-firmware-behavior.md)), so a
  report the drive built before acting on the Enable can be read after the
  Enable's echo, still in Reset [INFERENCE: no candump of that run; with every
  stream Off first, 245 captured Enables show no Reset frame after the echo].
  The Active stagger had the same exposure: at 14:51:40 the Enables of elbow
  and lower yaw went out while their streams ran. On SocketCAN a target's Enable
  is now written only after its type-24 Off has been read back from the wire
  at least one control period earlier. The Off is written whatever this
  process applied: in the reference baseline for the target, and by the
  Enable stagger for each target. A missing Off echo fails closed. In a
  reference, the phase deadline times out before any Enable. In an Active
  session, DriveState latches at the Enable-echo bound with "type-24 Off not
  observed". The strict post-echo Run check is unchanged. While a target's
  Enable echo is pending, grant liveness counts from activation (or its
  post-SetZero quiet end), because its traffic is withheld from pose and the
  echo bound covers that silence.
- **No Enable inside the post-SetZero blackout:** After receiving a SetZero
  (type 6) every Robstride drive transmits nothing for 45-61 ms, starting
  511-543 ms later (614 ms once, right_elbow_pitch 2026-10-03 15:34), and never
  acts on a frame received in that window (firmware 0.3.1.42, 124 measured
  blackouts on all five right-arm drives;
  [behaviour doc](commissioning/firmware/robstride-firmware-behavior.md)). An
  Enable written there leaves the drive in Reset, and its later Reset report
  latches DriveState. Davout records each address's latest SetZero at its host
  echo (`EchoedCommand::SetZero`; the write time until the echo is read). On
  SocketCAN no Enable, and no gate Off preceding it, goes to that address until
  `POST_SET_ZERO_QUIET` (800 ms: latest measured blackout end 667 ms + 100 ms,
  rounded up to 50 ms; `crates/davout/tests/firmware_profile.rs` holds that
  margin against the committed profile) has passed since. `enable_targets`
  keeps such a target pending, logs "Enable held until the post-SetZero quiet
  elapses" and writes it once the quiet ends; other targets are not delayed.
  That target's stagger catch-up, missing-echo latch, neutral-bootstrap
  watchdog grace and grant liveness count from the later of activation and its
  quiet end. A reference waits in `AwaitReportingOff` before arming an address
  it zeroed less than 800 ms earlier; arming a different joint is unaffected. A
  Reset report after the held Enable's echo still latches DriveState. Type-0
  identity admission retries through the blackout separately
  (`IDENTITY_ADMISSION_RETRY`).
- **Host-caused silence (2026-10-03 soak, rev e6add09):** `pi_enable_soak`
  failed 14 of 20 cycles: 11 `home failed: ... no private current-reference
  permission` for the first joint (0.1-0.25 s after the fifth reference), 2
  `Enable requires full-master Robot Ready`, and 1 grant lost mid-enable. Two
  causes, one rule: liveness must not count silence the host caused.
  1. *Stream restarted into a blackout.* Each reference's baseline writes a
     type-24 Off to every streaming peer, and the commit's reporting sync turns
     it On again about 100 ms later. For the first joint (SetZero 0.5 s before
     the last reference) those two writes straddle its blackout (start 511-614
     ms after SetZero). An Off before it and an On inside it leave the stream
     Off; the stale retry (200 ms) is slower than the 100 ms liveness bound
     (candump `decay-20261003T170858Z`: pitch SetZero 40.029, baseline Offs at
     +0.364, +0.456, +0.554, Ons at +0.414, +0.507, +0.605, blackout +0.533 to
     +0.586, so the last Off or On is dropped depending on the cadence). From
     `POST_SET_ZERO_BLACKOUT_FROM` (450 ms) to `POST_SET_ZERO_QUIET` (800 ms)
     after its SetZero echo, no type-24 On or Off goes to a drive: the reporting
     sync holds its writes, the reference baseline leaves a peer's stream On,
     and an On that was due is written when the quiet ends. That held silence is
     excused until the quiet's end (`count_silence_from`), then counts as usual.
     A stream that is applied On is never excused: a drive that goes silent
     beyond `comm_watchdog_ms` inside the window still loses its grant.
  2. *Enable echo before the first Run reply.* Traffic of a target whose Enable
     echo is pending is not pose, so its last pose is as old as the session
     (the Enable can be held 0.8 s). The echo is read in the writing tick and
     the Run reply (1.4-5.2 ms later) one tick later; liveness counted the old
     pose the moment the echo cleared the pending flag (soak cycle 9: the tick
     failed with "current reference was revoked" at 17:07:22.358, 4 ms after
     roll's Enable was written). Silence now counts from the echo.
- **Paced reference bursts:** the all-address stop (speed zero, neutral MIT,
  Disable per address) answers 15 frames; written back to back at the end of a
  reference it overran the mcp251x once in three runs (17:09:07, `rx_over_errors`
  5 to 6, Transport latched). During reference work the baseline and finishing
  stops start one address group per `BURST_GROUP_SPACING` (2 ms) per interface,
  as do the baseline's type-24 Offs, the Enable-admission type-0 requests
  (`IDENTITY_ADMISSION_SPACING`) and the Hardware-page status solicit. A stop
  caused by a fault, E-stop, cancellation or shutdown is never paced.
  `tests/physical_firmware` models the controller's two receive buffers
  (`RxFifo`); the Transport latch is unchanged.
- **Position arms wait for enable completion:** `enable_targets` may return while
  Enables are held (stagger, post-SetZero quiet), so a joint can lack session pose.
  During the bounded bootstrap Berthier sends neutral solicitations only; it does
  not compute τ_g from the zero placeholder. Every Position-mode arm (`hold-on`,
  `hold-at`, `wave`, Testing-panel setpoints) refuses with "waiting for enable to
  complete" and latches nothing until the supervisor is Active, no Enable is
  unwritten and every Active joint has fresh session pose. After bootstrap,
  missing feedback for any modeled joint refuses the control tick before gravity
  evaluation because τ_g is coupled across the model. `marengo-pi` prints
  `enabled (operator=…)` only after active target feedback arrives. Earlier stdin
  arms are deferred and retried after each tick. An Enable that does not complete
  within 2 s is refused (`enable failed:`) and every drive is stopped; a deferred
  arm is refused (`<cmd> failed:`).
- **Controller receive overflow is persistent (operator recommendation
  open):** an mcp251x RX overflow reaches Davout as a kernel error frame
  (`CAN_ERR_CRTL_RX_OVERFLOW`) and latches Transport. Making an *isolated*
  overflow recoverable while every Active joint's feedback stays fresh within
  `comm_watchdog_ms` was considered and not adopted. *For:* the drives keep
  their own CAN timeout, the lost frame was most likely a periodic status or
  MIT reply that the next tick replaces, and freshness/watchdog checks still
  bound stale pose. *Against:* the dropped drive frame's identity is unknown;
  it can be a fault report or a reply carrying a Reset/fault mode, so the
  strict mode check and fault authority may have missed exactly the evidence
  they exist for; an overflow also shows host receive servicing already fell
  behind. Relaxing it needs a decided policy (which frames may be lost, how a
  possibly lost fault is re-solicited before motion continues) and an ADR.
- **Reference and stop callers:** Private admission closes legacy direct grants
  and cached verification. Physical reference runs inside the owning
  `marengo-pi`/`motor-repl` process (ADR 0036); installed-owner client migration
  (gateway/MCP/proto) remains incomplete. `motor-repl disable` no longer builds
  a Supervisor: it reads only each drive's `can_interface` and `device_id` from
  `motors.yaml` and sends one type-4 Disable (Byte[0]=0) per drive, so a missing
  `control.yaml`/URDF, corrupt calibration history or a down CAN interface
  cannot stop it from reaching the drives it can reach. It prints a per-drive
  outcome and exits 1 if any drive was not reached. A sent frame is queued on the
  bus, not drive-confirmed, and it is **not** a fault clear: no type-4
  Byte[0]=1 frame is ever sent (ADR 0020), so a latched drive fault persists.
  The commands that can leave a drive enabled (`enable`, `jog`, `speed`,
  `speed-stop`, `set-zero`) arm the same stop on SIGTERM/SIGINT/SIGHUP and on any
  error exit, and refuse to start if it cannot be armed. The MCP aborts a
  session whose pre-session disable did not reach every drive. Reference-
  independent stop through the installed owner remains required; use the physical
  E-stop as the independent stop path.
- **No drive-side or independent watchdog:** `ParameterId::CanTimeout` (0x7028)
  is never written or read back and nothing outside the 200 Hz thread watches it,
  so a killed or hung `marengo-pi` leaves each drive on its last MIT frame
  (including τ_g feed-forward). SIGTERM is handled (`finish_owner_shutdown`);
  SIGKILL, panic and a hung loop are not. Undecided, see
  `docs/reviews/2026-10-03-crate-audit/phase-b/WP-I.md`.

## When in doubt

Disable drives, E-stop, and fix the fault before resuming.
