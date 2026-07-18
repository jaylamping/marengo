# Position-hold control review (2026-06)

Holistic review of Marengo joint position hold (`ControlMode::Position`) against standard
robotics control layering. Records bench evidence, intended architecture, and safety
invariants for the weighted right-shoulder-pitch Layer 2 gate (`0 → 0.1 → 0` rad).

## Bench evidence (weighted ~700 g, deploy revs e58cab4–b3bcea8)

| Observation | Detail |
|-------------|--------|
| Safety at CAN/Davout | `fault=0x0000`; candump ~109 Hz; no bus loss |
| Layer 2 analyzer | **FAIL** on `jerk_rms` (1300–2000 vs gate 800); `tau_ff` slew improved (~55–65 Nm/s vs 60 gate) |
| Operator report | Visible **stutter** on `hold-at 0.1` approach and return |
| Trace signature | `q_des`/`tau_p` jumps on return through home; friction assist toggles 0↔0.25 Nm; planner resync/latch churn after overshoot — not CAN faults |

## Root cause: profile selection bug

Berthier classified “small” vs “trajectory” moves with a hardcoded `0.06` rad cap
(`POSITION_SMALL_MOVE_VMAX_RAD`) while `marengo-config` exposes
`position_trajectory_threshold_rad` (default **0.15** rad).

A **0.1 rad** Layer 2 move therefore used the fast trajectory profile
(`position_trajectory_velocity_rad_s = 2.0`) instead of the small-move slew
(`position_slew_rad_s = 0.15`). That mismatch drives aggressive planner speed,
lead saturation, friction mode churn, and jerk failures.

The right-only bringup config had `position_trajectory_threshold_rad: 0`, which
forced all moves onto the trajectory path regardless of distance.

## Intended control layers

```text
profile     = classify(target, q, config)           # position_profile
q_ref,dq_ref = planner.tick(profile)                # position_trajectory
q_des,dq_des = setpoint_mapper.map(...)             # position_setpoint
tau_ff       = gravity(q) + friction + damping      # position_feedforward
cmd          = Davout.filter(q_des, dq_des, tau_ff) # final safety gate
```

| Layer | Owner | Responsibility |
|-------|-------|------------------|
| Reference generation | Berthier planner | Smooth `q_ref`, `dq_ref` toward target under profile speed/accel |
| Setpoint mapping | Berthier setpoint | Map planner output to MIT `q_des`/`dq_des` with bounded lead, overshoot, envelope |
| Feedforward | Berthier FF | `tau_g + tau_f + tau_d`; continuous across planner phase boundaries |
| Safety filtering | Davout | Sole path to `robstride`; limits, torque caps, rate limits, homing gate, watchdog |

Berthier must **not** bypass Davout. Davout must **not** implement motion planning.

## Setpoint invariants

- `q_des` monotonic toward target except for explicit, bounded overshoot handling.
- `q_des` must not jump between target and measured `q` without a logged planner event
  (`reset`, `latch`, `freeze_enter`, `resync`, …).
- Return-home helpers (`freeze`, descent assist) are profile-gated — they must not affect
  ordinary small `hold-at` moves.
- Lower-limit and cross-home moves use dedicated profiles, not gravity-return logic.

## Feedforward invariants

- Gravity FF at measured `q` (sound).
- Damping: `tau_d = Kd * (dq_ref - dq_filtered)` while moving; single settle branch at rest.
- Coulomb friction follows `sign(dq_ref)` while moving, `sign(target - q)` in hold/settle.
- `kd_mit = 0` until firmware velocity quality is validated separately.

## Davout safety invariants (must remain)

- Sole motor command path: Berthier → Davout → robstride.
- Homing gate before Active.
- Measured-position hard-limit fault with slack.
- Torque caps, command clamps, comm watchdog, disable-on-error.
- `tau_ff` rate limiting before motor send.

## Known safety gaps (tracked)

### Hardware E-stop

`Supervisor::set_hardware_estop` exists and disables motion when asserted, but **no Pi GPIO
or bench input wiring** feeds it at runtime. Software state can disagree with the physical
E-stop line.

**Action:** GPIO/input integration or explicit operator acknowledgment before elevated-pose
testing. Until wired, treat E-stop as operator + power-cycle only.

### Danger zones

Previous rules evaluated **commanded** `position_rad` / `velocity_rad_s`. With Berthier
sending `kd_mit = 0`, `clamp_velocity` alone does not create torque authority to slow
gravity-driven descent.

**Action:** Evaluate rules on **measured** `q`/`dq`; support `clamp_torque` to cap
`|torque_ff_nm|` in-zone. Redesign elevated-pitch rules around measured state.

### Limit envelope velocity margin

Dynamic command margins used planner/command velocity only. Gravity-driven descent can
move faster than commanded, shrinking effective margin.

**Action:** Envelope clamp uses `max(|dq_cmd|, |dq_meas|)` for margin computation in Davout.

### Fault latching

Motion faults currently disable motors; policy for explicit operator reset before
re-enable is undocumented.

**Action:** Document in `docs/safety.md`; decide per-fault latch vs auto-clear on next
enable request.

## Validation ladder

1. No-hardware unit tests + analyzer golden fixtures.
2. `just check`.
3. Deploy to Pi; verify `.deploy-rev`.
4. Weighted `0 → 0.1 → 0` with `--require-home-start`.
5. If Layer 2 passes or improves: `0 → -0.3 → 0`.
6. Full positive range only after negative smoke passes.
7. Lower-limit probes last.

**Pass criteria:** `fault=0x0000`; CAN healthy; Layer 2 analyzer pass (or residual fail
attributed to measurement noise); no operator stutter; bounded `q_des`/`tau_p`/`tau_ff`
discontinuities; no planner-event churn near target.

## Related docs

- [bench-90deg-calibrated-roundtrip.md](bench-90deg-calibrated-roundtrip.md) — 2026-06-20
  calibrated 90° (`1.484` rad) weighted round trip, set-zero, return settle gap
- [bench-position-tuning.md](bench-position-tuning.md) — Layer 2 gate thresholds
- [safety.md](safety.md) — motor path and enable rules
- [ADR 0004](decisions/0004-control-modes-and-mit.md) — control modes
- [ADR 0007](decisions/0007-position-hold-trajectory.md) — trapezoid planner
- [ADR 0009](decisions/0009-limit-envelope.md) — velocity-scaled envelope
