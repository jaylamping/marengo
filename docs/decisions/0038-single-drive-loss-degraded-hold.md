# ADR 0038: single-drive loss while Active: degraded hold, then controlled descent

Status: proposed, October 4, 2026. Nothing is implemented. Builds on
[ADR 0036](0036-physical-robstride-reference.md) (per-joint grants, solicited
silence) and WP-I D2 (`docs/reviews/2026-10-03-crate-audit/phase-b/WP-I.md`,
"limp vs hold" trade-off, item 4).

## Context

### Incident, 2026-10-04 (gravity calibration, arm elevated)

Wire evidence from `candump-20261004T050045Z` (capture-relative seconds), all
five right-arm drives on `can0` (`config/motors.yaml`: ids 1-5 =
shoulder_pitch, shoulder_roll, upper_arm_yaw, elbow_pitch, lower_arm_yaw):

- up to 15.0104 s, id 5 (`right_lower_arm_yaw`, RS00, fw 0.0.3.32) answered
  every MIT frame in about 0.2 ms: type-2 id `0x028005FD`, mode bits
  16..23 = `0x80` (Run), torque about 0, 23 °C;
- 15.0104 to 15.111 s, id 5 was silent: no reply to 20 MIT frames, no type-24.
  Ids 1-4 replied (79 frames). No error frames; `rx_over` unchanged. The bus
  was healthy; one drive went away;
- `marengo-pi` logged `physical reference grant revoked joint=right_lower_arm_yaw
  cause="no feedback within comm_watchdog_ms" silence=101ms`, then `control
  tick failed error=safety: homing: current reference was revoked after feedback
  projection`. Every drive was disabled and the arm, at shoulder pitch 0.8 rad,
  fell;
- at 15.748 s (738 ms later) id 5 came back with `0x020005FD` (mode `0x00`,
  Reset) and then type-24 reports. The drive had rebooted, probably after a power
  interruption [INFERENCE: nothing on the wire tells a brownout from a reset].

Torque and velocity bounds played no part. This is the `docs/safety.md`
*Upright-pose incident* class (lines 78-91), caused this time by Marengo's own
fail-closed stop.

### Why everything stops today

1. `Supervisor::reference_binding_valid_with` (`crates/davout/src/lib.rs:951-1015`)
   revokes only the silent joint (`revoke_binding`), as ADR 0036 *UID binding
   and revocation* requires.
2. `poll_feedback` (`lib.rs:1934-1963`) sees that the active set no longer holds
   every grant (`active_references_held`, `lib.rs:2039`), calls `disable_all`
   itself and returns `DavoutError::Homing("…revoked after feedback
   projection")`. `ensure_reference_for_active` (`lib.rs:1026-1035`, doc comment:
   "a physical joint that lost its own grant stops the whole active session")
   and `ensure_reference_binding` (`lib.rs:1040-1043`) do the same.
3. `perform_stop_paced` (`lib.rs:2371-2461`) sends zero speed, neutral MIT
   (kp = kd = τ = 0) and Disable to every address, then clears `active_joints`.
   The healthy shoulder drives lose τ_g on the next frame.
4. Berthier `ControlLoop::tick` (`crates/berthier/src/loop.rs:1215-1229`)
   discards motion intent for every `LoopError::Safety`, and
   `stop_after_tick_error` (`bins/marengo-pi/src/main.rs:452-473`, called at
   `:1903-1904`) runs `disable_all` again.
5. Even if the stop were skipped, the gravity model needs every coupled joint:
   `tick_inner` refuses with `MissingFeedback` (`loop.rs:1297-1311`, pinned by
   `active_gravity_refuses_when_a_coupled_joint_lacks_feedback`, `loop.rs:2038`)
   because `gravity_torques(&q)` (`loop.rs:1327`) takes the full vector. When
   the drive came back, its Reset-mode reply against the strict Run expectation
   (`feedback_consumer.rs:453-475`) would have latched `DriveState` and stopped
   everything anyway.

`DavoutError::Homing` is not recorded by `record_runtime_error`
(`lib.rs:1168-1214`, `_ => return false`), so this stop latched no fault. Only the
revoked grant stops the process from re-enabling the set
(`enable_targets`, `lib.rs:1298-1303`).

## Invariants this ADR must not change

- **Davout is the only motor path.** Berthier and marengo-pi never write the
  bus; Davout alone qualifies a loss, enforces the deadline and sends the stop.
- **Process-local, latched grants** (ADR 0036 *Process lifetime*, `safety.md:33-49`):
  a revoked joint's drive is never commanded again in-process (no MIT, Enable,
  SetZero, parameter write, type-24 On). Disable stays allowed (ADR 0023).
- **Fault latches mean "stop all"**, `StopDelivery` and `Transport` included.
  Degraded mode only replaces the grant-lapse stop, which today is no latch.
- **E-stop and operator authority:** `set_hardware_estop` (`lib.rs:1274-1296`),
  stdin/Chappe `disable`, `stop_on_lag` and shutdown stop everything at once.
- **No change** to `comm_watchdog_ms` (100), danger zones, τ caps, rate limiter,
  Berthier fuses (`HoldTracking`, `AscentStall`, `WaveStall`) or other stop paths.

## Options

**A. Status quo: disable all.** Simple, proven, one stop path. Any single drive
loss drops an elevated arm; with five drives, an RS00 now known to reboot and a
shared `can0` harness, this will recur.

**B. Degraded hold, timed descent, then disable all.** Davout sheds the lost
joint (Disable to that address, out of the active set); healthy joints keep τ_g
and a bounded position law, ramp to rest, then everything is disabled: limp but
low. Most code; only helps for a distal loss (*Feasibility*).

**C. Degraded hold only, until the operator acts.** Rejected as default:
unbounded hold on an incomplete gravity model, a later fault drops the arm at a
worse moment, no safe end state unattended. Allowed only as bounded C′ (hold at
most `degraded_max_s`, then disable) where descent is not admissible.

**D. Re-acquire the rebooted drive in-process.** Rejected: breaks "a revoked
grant stays revoked" (ADR 0036), and a reboot loses the volatile zero
(`Drive::reboot`, `crates/davout/tests/physical_firmware/mod.rs:324-333`).

**E. Longer `comm_watchdog_ms`.** Rejected: the drive was in Reset for 738 ms,
and it delays every real detection (ADR 0036 *Host-caused silence*, "Not adopted").

**F. Mechanical counterbalance or brake on shoulder pitch.** The only answer
for proximal loss; complements B; hardware, outside this ADR.

## Feasibility: distal vs proximal loss

The chain runs pitch → roll → upper_arm_yaw → elbow_pitch → lower_arm_yaw. Once
its drive is in Reset, a lost joint and everything distal to it swing freely.

- **Lost distal joint (lower_arm_yaw).** It rotates the forearm about its own
  axis. Losing it changes the proximal τ_g only through the hand COM's offset
  from that axis. The healthy joints can still hold and lower the arm. **B is
  feasible.**
- **Lost intermediate joint (elbow_pitch, upper_arm_yaw).** The forearm swings
  as a pendulum. The shoulder τ_g changes both dynamically and by a large amount,
  and the model with a frozen `q` is wrong exactly while the arm moves. This is
  feasible only if the joint passes the admission rule below. [INFERENCE] the
  elbow probably fails it with the arm elevated; `gravity-preview` over the
  range decides.
- **Lost proximal joint (shoulder_pitch, shoulder_roll).** The arm falls about
  the lost axis whatever the distal drives do, and a stiff distal hold only
  changes the shape of the fall. **Not degradable: disable all immediately**, as
  today.

Admission is per joint, from configuration, checked offline at config
validation. A new `control.yaml` joint key `on_drive_loss: disable_all | descend`
defaults to `disable_all`. `descend` validates only if, at every grid point of
the lost joint's soft range (the other joints at their limits and at rest):
(1) `max |τ_g(q) − τ_g(q_frozen)|` on every healthy joint stays within its
`tau_ff` cap minus the hold margin, and (2) the full `τ_g` of the healthy joints,
with the lost joint anywhere in its range, stays within their caps. Only
`right_lower_arm_yaw` is expected to qualify on the current bench.

## Decision (proposed): option B, distal joints only

### What qualifies, all conditions at the moment of the lapse

- The session is Active, in `GravityComp` or `Position` hold (wave and ascent
  included). No reference work is busy and the process is not shutting down.
- Exactly one joint has lapsed, its cause is `Lapse::Silent` (`lib.rs:990`), and
  its `on_drive_loss` is `descend`.
- No fault is latched. Every other Active joint holds its grant and has a current
  pose (`pose_is_current`, `lib.rs:2087`).
- No degraded episode has run before in this process.

### What still disables everything at once

Transport latch (`BusError::Send/Driver/ReceiveIncomplete`, rx overflow), E-stop,
`StopDelivery`, any `Device` status flag (on the shed address too, since an
undervoltage flag hints at a shared supply), `DriveState` on a healthy address,
`Limit` or the measured position guard, `DangerZone` with action fault,
`WrongSign`, malformed feedback, a `Controller` latch (dynamics model,
`MissingSetpoint`, `GainShape`), a second drive loss, a lapse of a
`disable_all` joint, an identity change or coordinate-epoch jump (the drive is
alive but untrusted), owed-On lapses, model or policy change, operator disable,
`stop_on_lag`, shutdown, any Berthier fuse trip during the descent, and the
descent deadline.

### Mechanics

- **Davout** (`poll_feedback`, `ensure_reference_for_active`,
  `ensure_reference_binding`): a qualifying lapse calls `shed_lost_joint(joint)`
  instead of `disable_all`. It writes **one type-4 Disable** to that address
  only; the three-frame stop could add three replies to a tick already carrying
  four MIT replies, which overran the mcp251x two-frame buffer before
  (`lib.rs:2246-2256`, 2026-10-04 soak, Transport latched). The joint leaves
  `active_joints`; its address joins a `shed` set that `admit_and_send_mit_with`
  refuses (a wire to it is an error, not a skip). A `DegradedEpisode { joint,
  cause, since, deadline }` goes into `SafetySnapshot`; Davout returns Ok with a
  degraded event, so `stop_after_tick_error` is not reached. Reset-mode frames
  from the shed address are expected and logged as reboot evidence; a Run-mode
  frame or status flags from it latch and stop everything.
- **Deadline in Davout:** `deadline = since + min(distance / v_desc + settle,
  degraded_max_s)`. At the deadline or the first non-qualifying error it calls
  `disable_all` and latches `FaultClass::Communication` for the lost joint
  (restart required; the episode is a visible fault). A Berthier that stops
  ticking or ignores the episode still ends in a stop at the next Davout call.
- **Berthier:** a `DegradedDescent` mode freezes the lost joint's `q` at its
  last admitted value for `gravity_torques` (lifting `MissingFeedback` for that
  joint only) and ramps every healthy joint to the rest pose, time-synchronized,
  with the existing Position hold law (τ_g FF plus configured kp/kd; no new
  gains). Tracking, not pure GravityComp: with kp = 0 the frozen-`q` model error
  drifts the arm. Fuses and danger zones stay armed.
- **marengo-pi / Consul:** publish the episode in `SafetyState`; operator
  `disable` stops everything at once; motion commands are refused meanwhile.

### Descent bounds (proposed config, `control.yaml`)

- `degraded_descent_velocity_rad_s` ≤ the danger-zone clamp of 0.45 rad/s
  (`config/control.yaml:202-210`). Proposed 0.25 rad/s: 0.8 rad of pitch in
  3.2 s.
- `degraded_max_s`, a hard cap of 6 s from the shed. C′ uses the same cap.
- Rest pose: per joint, defaulting to 0 rad, the pose `pi_hold_on` returns to
  before it disables. [INFERENCE] 0 is the low-energy, hanging pose of this bench;
  confirm with `gravity-preview` before relying on it.
- Progress: the existing `HoldTracking` fuse semantics apply. A descent with no
  measured progress trips and stops everything.

### Interaction with the drive-side CanTimeout (600 ≈ 30 ms)

All five drives read `CanTimeout` (0x7028) = 600 (ADR 0037 probe, 2026-10-03,
`docs/safety.md:471-480`); Marengo never writes it and power-cycle persistence
is unverified.

- **Healthy drives:** the shed must fit inside the detecting tick (no blocking
  drain, paced burst or settle wait). A host gap over ~30 ms sends the healthy
  drives to Reset and repeats the fall: a degraded descent tolerates less host
  jitter, not more.
- **Lost drive:** alive but deaf, it reaches Reset ~30 ms after the last frame
  it heard; mute but listening, it gets the Disable, or times out ~30 ms after
  the host stops addressing it [INFERENCE: WP-I D2 item 2, whether other
  addresses' frames reset its timer is undocumented]. Either way it is torque-off
  without Marengo commanding it.
- **Defence in depth only:** a power-cycled drive may read 0. Then a host death
  mid-descent leaves the healthy drives on their last frame (τ_g plus kp toward
  the last setpoint), as a host death does today.
- Detection latency is unchanged: `comm_watchdog_ms` after the first unanswered
  solicit (ADR 0036 *Solicited silence*); healthy drives are commanded every
  tick throughout (ids 1-4 kept replying in the incident).

## Test plan

Firmware emulator (`FirmwareBus`, `crates/davout/tests/physical_firmware`), no
hardware. Extend `Drive::reboot(silent_until)` and `reboot_after_ack`.

1. Distal silence while Active in GravityComp: lower_arm_yaw is shed. Exactly one
   Disable goes to id 5, MIT frames continue to ids 1-4 with no tick gap over one
   period, and no MIT reaches id 5 again.
2. A reboot-like return (Reset-mode frames after 738 ms): logged and tolerated.
   A Run-mode frame or status flags from the shed address stop everything.
3. Proximal silence (shoulder_pitch): `disable_all` at once, unchanged.
4. Second loss during the episode, a transport error, E-stop or operator disable
   during the episode: immediate all-address stop.
5. Deadline: the episode ends with `disable_all` plus a latched `Communication`
   fault, even if the controller stops ticking.
6. Berthier (simulation): `DegradedDescent` ramps the healthy joints to rest
   within `degraded_max_s` and at most `degraded_descent_velocity_rad_s`. The
   frozen-`q` gravity call succeeds and a fuse trip stops everything. Update
   `stop_path_tests.rs` to show that a degraded event is not a tick error.
7. Config validation: `descend` on a joint that fails the τ-bound grid is
   refused.

Bench, arm supported first, then unsupported at low elevation: hold the arm
in GravityComp, cut id 5 power (or unplug its CAN stub), and confirm that the
arm descends to rest and disables, using `pi_candump_summary` and the position
trace (no MIT to id 5 after the Disable, no gap over 30 ms to ids 1-4).
Repeat with shoulder_pitch to confirm the immediate stop.

## Recommendation

Adopt B for distal joints whose `on_drive_loss: descend` passes the offline
τ-bound check (today only `right_lower_arm_yaw`). Davout owns the shed, the
Disable-only stop and the hard deadline. Keep A for proximal and intermediate
joints and for every latched fault. Pursue F (mechanical support) for proximal
loss, and find the cause of the id 5 reboot (supply or harness). B limits how far
the arm falls; it does not fix the drive.
