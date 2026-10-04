# ADR 0038: single-drive loss while Active: subtree shed, degraded hold, controlled lower

Status: accepted and implemented, October 4, 2026. Builds on
[ADR 0036](0036-physical-robstride-reference.md) (per-joint grants, solicited
silence) and WP-I D2 (`docs/reviews/2026-10-03-crate-audit/phase-b/WP-I.md`,
"limp vs hold" trade-off, item 4).

## Context

### Incident, 2026-10-04 (gravity calibration, arm elevated)

Wire evidence from `candump-20261004T050045Z` (capture-relative seconds), all
five right-arm drives on `can0` (ids 1-5 = shoulder_pitch, shoulder_roll,
upper_arm_yaw, elbow_pitch, lower_arm_yaw):

- up to 15.0104 s, id 5 (`right_lower_arm_yaw`, RS00, fw 0.0.3.32) answered
  every MIT frame in about 0.2 ms in Run mode;
- 15.0104 to 15.111 s, id 5 was silent (20 unanswered MIT frames, no type-24).
  Ids 1-4 replied (79 frames); no error frames, `rx_over` unchanged;
- `marengo-pi` logged `physical reference grant revoked joint=right_lower_arm_yaw
  cause="no feedback within comm_watchdog_ms" silence=101ms`, then `control
  tick failed ... current reference was revoked after feedback projection`.
  Every drive was disabled and the arm, at shoulder pitch 0.8 rad, fell;
- at 15.748 s id 5 came back in Reset (`0x020005FD`) with type-24 reports: a
  reboot, probably a power interruption on the harness across the elbow
  [INFERENCE: the wire cannot tell a brownout from a reset]. It happened twice.

Before this ADR, Davout revoked only the silent joint's grant (ADR 0036) but
`poll_feedback`, `ensure_reference_for_active` and `ensure_reference_binding`
then stopped every drive, and `stop_after_tick_error` stopped them again.

## Invariants kept

- **Davout is the only motor path.** Davout alone qualifies a loss, sheds, owns
  the deadline and sends every stop.
- **Process-local, latched grants** (ADR 0036): a shed drive is never commanded
  again in this process except Disable (no MIT, Enable, SetZero, parameter
  write, type-24 On or Off). The all-address stop writes only Disable to it.
- **Every fault latch stops everything**, `StopDelivery` and `Transport` included.
- **E-stop and operator authority:** E-stop, `disable`, Chappe `enable(false)`,
  `hold-off`, `stop_on_lag` and shutdown stop every drive at once.
- **No change** to `comm_watchdog_ms`, gains, velocity caps, danger zones, τ
  caps, the rate limiter or the Berthier fuses.

## Options considered

A. Status quo (disable all): one drive loss drops an elevated arm. B. Shed and
hold, then lower to rest and disable (adopted). C. Hold until the operator acts:
rejected, unbounded hold on an incomplete model. D. Re-acquire the rebooted
drive in-process: rejected, breaks "a revoked grant stays revoked" and the zero
is lost. E. Longer `comm_watchdog_ms`: rejected, the drive was down 738 ms. F.
Mechanical counterbalance or brake on shoulder pitch: the only answer for a
proximal loss; hardware, outside this ADR.

## Decision

### Kinematic-subtree shed

When joint J's drive is lost, Davout disables J **and every joint distal to it**
(its subtree in the URDF parent chain, computed by
`UrdfGravityModel::subtree_joints`, never hard-coded). Joints proximal to J keep
holding; joints of other limbs are not in J's subtree and are unaffected. Losing
shoulder pitch sheds the whole arm, which leaves nothing to hold: such a joint is
refused at admission and its loss stops every drive, as before.

### Admission (config, checked at startup)

`control.yaml` joint key `on_drive_loss: disable_all | shed_subtree` (default
`disable_all`, unknown values refused). `shed_subtree` needs a `control.drive_loss`
block (`hold_window_s`, `lower_velocity_rad_s`, `lower_settle_s`, `lower_max_s`,
`tau_margin_nm`, all validated). `marengo-pi` runs
`ControlLoop::admit_drive_loss` at startup and exits if any `shed_subtree` joint
fails; only admitted joints get a `DriveLossPlan` installed in Davout. Without a
plan, every loss stops every drive.

The offline bound (`armee_dynamics::drive_loss::subtree_loss_torque_bounds`):
the shed joints sweep their soft ranges (7 points each) while every holding
joint sits on a grid of its own soft range (4 points plus rest). For each
holding joint h it records `max |τ_g,h|` and `max |Δτ_g,h|`, the largest change
of `τ_g,h` between two subtree poses with the holding joints fixed (the error a
frozen subtree angle hides). Admission requires, on every holding joint,

`max |τ_g| + max |Δτ_g| + tau_margin_nm ≤ min(policy tau_ff_max, motor-type tau_ff_max_nm)`.

On the 2026-10-04 URDF with `tau_margin_nm: 0.5` (caps: RS03 5 Nm, RS02/RS00 3 Nm):

| Lost joint | Worst holding joint: max \|τ_g\| + max \|Δτ_g\| (Nm) | Result |
|---|---|---|
| right_shoulder_pitch | nothing holds | refused |
| right_shoulder_roll | pitch 2.82 + 5.20 | refused |
| right_upper_arm_yaw | pitch 2.82 + 0.47, roll 2.81 + 0.34 | admitted |
| right_elbow_pitch | pitch 2.82 + 0.47, roll 2.81 + 0.33, upper yaw 0.30 + 0.45 | admitted |
| right_lower_arm_yaw | every holding joint Δ ≤ 0.07 | admitted |

The master `config/control.yaml` sets `shed_subtree` on the three admitted
joints. The bound is static: the swing of a freed forearm adds dynamic load the
grid does not capture. If bench evidence shows the healthy joints cannot ride
that out, set the intermediate joints back to `disable_all`.

### What qualifies (Davout, at the moment of the lapse)

The session is Active in GravityComp or Position, every Enable written and
echoed, no reference work, no latched fault, no E-stop, no earlier episode in
this process. Exactly one Active joint lost its grant, for silence
(`revoke_binding_silent`), and it has an installed plan and `shed_subtree`. Every
holding joint keeps its grant and a current pose, and every shed joint has a last
admitted pose (the frozen angle).

### Shed (Davout, inside the detecting call)

`shed_for_drive_loss` writes **one type-4 Disable per subtree address** (no
paced burst, no settle wait: the drive-side CanTimeout of 600 ≈ 30 ms leaves no
room), revokes the subtree's grants, removes it from the Active set and opens a
`DegradedEpisode { lost_joint, shed_joints, holding_joints, frozen_positions,
since, deadline }`. The call returns Ok, so no tick error is raised. A lapse is
judged in the feedback drain, in MIT admission (a batch composed before the shed
drops its shed joints once; later batches naming them fail `InactiveJoint`),
when the pose watchdog fires a moment after the grant check, and when the
controller finds an Active pose stale (`check_active_references`).

Frames from a shed address are never pose. Reset is expected (a stopped or
rebooted drive; the lost drive's return logs `drive reboot detected`). Any other
mode read later than `SHED_REPLY_GRACE` (20 ms) after the shed latches
`DriveState` and stops everything.

### Deadline (Davout)

`deadline = since + hold_window_s + min(distance / lower_velocity_rad_s +
lower_settle_s, lower_max_s)`, where `distance` is the largest `|q − trim|` of
the holding joints. Every drain and MIT admission, and `marengo-pi` once per
loop iteration (`enforce_degraded_deadline`), stop every drive once it has
passed, whatever the controller does.

### Hold and lower (Berthier)

On the episode Berthier freezes: it aborts any wave, retarget or torque command
and latches every joint at its current `q` in Position hold (existing law, τ_g
feed-forward plus configured gains; no new gains), with the shed joints' angles
frozen in `gravity_torques`. The operator may stop (everything off at once) or
send `lower`. After `hold_window_s` with neither, the lower starts by itself:
every holding joint retargets to rest (0 rad plus `position_hold_trim_rad`,
clamped to its envelope) with its planner velocity capped at
`lower_velocity_rad_s` (0.25 rad/s, below the 0.45 rad/s danger-zone clamp). When
every planner has reached rest and every measured `q` is within
`DEGRADED_REST_TOLERANCE_RAD` (0.05 rad), Berthier calls
`complete_degraded_lower`. Fuses and danger zones stay armed. Any other motion
command (`enable`, `home`, hold, wave, torque, gain changes, Testing batches,
set-zero, mode changes) is refused.

### End

Every end (lower complete, deadline, operator stop, a fault, a second loss) is an
all-address stop that latches `FaultClass::Communication` on the lost joint
(`drive lost; degraded episode ended (<end>); restart required`) and records a
`DegradedOutcome`.

### Still stops everything at once

Transport latch, E-stop, operator stop, `StopDelivery`, any Device status flag
(shed addresses included), `DriveState`, Limit, `DangerZone`, `WrongSign`,
malformed feedback, any Controller latch (Berthier fuses included), a second
drive loss during or after an episode, the loss of a joint without an admitted
plan, a loss outside GravityComp/Position or during enable or reference work, an
identity or coordinate-epoch revocation, the deadline, and a non-Reset frame
from a shed drive.

### Visibility (marengo-pi)

- stdout: `drive lost <joint>: shed <a,b>; holding <c,d>; auto-lower in <s> s`,
  `degraded lower started (operator|timeout)`, `degraded lower complete;
  disabled`, `degraded episode ended (deadline|stop); disabled`; refusals print
  `<cmd> refused: degraded hold after losing <joint>; only disable or lower is
  accepted` (stderr).
- `robot/audit/action`: `drive_loss_shed`, `degraded_lower`,
  `degraded_lower_complete`, `degraded_episode_ended`, and `motion_refused` for
  refusals.
- `SafetyState.active_faults`: a `drive_loss_degraded_hold` WARNING entry while
  the episode runs (no proto change); the latched Communication fault after it.
- tracing WARN events for the shed, the lower and the end.

## Drive-side CanTimeout (600 ≈ 30 ms)

The shed fits inside the detecting tick and the holding drives are commanded
every tick throughout. A host stall over ~30 ms still sends the healthy drives
to Reset: a degraded episode tolerates less host jitter, not more. A lost drive
that still listens gets the Disable or times out on its own.

## Tests

- `crates/davout/tests/drive_loss.rs` (firmware emulator): distal shed with one
  Disable and no MIT gap on the holding drives; elbow loss sheds elbow + lower
  yaw; pitch and non-admitted loss stop everything; second loss; Run frame from
  a shed drive; rebooted shed drive in Reset tolerated; deadline with and without
  a ticking controller; operator stop and completion latch; enable refused.
- `crates/berthier/tests/degraded_hold.rs` (`ControlLoop` over the emulator, a
  servo knob following MIT positions): hold, auto-lower at the window with the
  planner at ≤ 0.25 rad/s, completion disables and latches; operator `lower`
  before the window; motion refused; operator stop.
- `crates/berthier/tests/drive_loss_admission.rs`: master config admitted;
  roll, pitch and an oversized margin refused. `marengo-config` validates the
  key and the timing block.

## Bench verification (pending)

Arm supported first, then unsupported at low elevation: hold in GravityComp,
cut id 5 power (or unplug its CAN stub), confirm the hold, the lower and the
disable with `pi_candump_summary` and the position trace (no MIT to id 5 after
its Disable, no gap over 30 ms to ids 1-4). Repeat with shoulder pitch to confirm
the immediate stop. Find the cause of the id 5 reboot (supply or harness).
