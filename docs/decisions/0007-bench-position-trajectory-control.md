# ADR 0007: Bench position trajectory control

**Status:** Accepted (2026-06-13 one-pass: always-trapezoid joint executor, two-rule friction, MIT `dq_ref`)  
**Date:** 2026-05-26

## Update 2026-06-13: one-pass simplification (implemented)

Berthier **Position** mode is now a single joint-space motion primitive executor:

- Always trapezoidal `q_ref` / `dq_ref` (no slew/threshold split).
- `tau_p = Kp * lead`; `tau_d = Kd * (dq_ref − dq)` while moving; MIT `velocity_rad_s = dq_ref`, `kd_mit = 0`.
- Friction: **`traj_vel`** or **`settle`** only (no breakaway latch / mode zoo).
- **Talleyrand** (future) owns Cartesian → joint timing; Berthier executes whatever joint refs it receives. Operator `hold-at` is a bench/debug surface to the same API.

See [rust-patterns.md](../rust-patterns.md) §7.

## Update 2026-06-13: weighted full-range timing (locked)

After **`3f66ea2`** (high-q return freeze skip, stuck-lead resync, return breakaway onset) and trajectory retune on `shoulder_pitch_right_only`:

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `position_trajectory_velocity_rad_s` | **2.0** | rs03 / config velocity cap; ~1.2 s wall-clock 0→90° on weighted arm |
| `position_trajectory_accel_rad_s2` | **4.8** | Symmetric trapezoid ≈1.2 s for π/2 rad at `v=2.0` |
| `position_slew_rad_s` | 0.15 | Small retargets (≤ ~60 mrad) |
| `position_slew_max_lead_rad` | 0.10 | MIT lead clamp |
| `impedance` | kp=8, kd=1.25 | Layer 2 baseline + return damping |
| `friction.fc` | 0.25 | Reduced from 0.35 after jerk trials |

Bench verification (2026-06-13): weighted `hold-at` 0 → 1.570796 → 0 — outbound **~1.2 s**, return **~3 s**, `fault=0`, no `planner_frozen`. Documented in [bench-position-tuning.md](../bench-position-tuning.md).

## Update 2026-06-13: hold-at planner/MIT split (superseded)

Commit `f1d2be6` implements an **interim** joint-impedance fix in Berthier for large `hold-at` retargets (e.g. weighted right shoulder 0→90°). It does **not** replace the trapezoidal trajectory layer proposed below, but removes the worst failure modes (slew freeze, distant-target friction shove, absent ramp damping).

### Industry pattern (reference)

Manipulator hold/ramp control (MIT [Manipulator Control](http://manipulation.csail.mit.edu/force.html), KUKA iiwa joint impedance, Atlas ID stack) separates:

1. **Planner** — rate-limited reference `q_ref(t)` toward the goal; state accumulates even when measured `q` lags.
2. **Controller** — bounded MIT setpoint `q_des` within `max_lead` of measured `q`.
3. **Gravity FF** — `τ_g` at **measured** `q`, not the distant latch target.
4. **Friction / damping FF** — keyed off **tracking error** (`q_des − q` or `q_ref − q`), not error to the final target.

### What broke on the bench (weighted 0→1.57 rad)

| Symptom | Cause |
|---------|--------|
| Velocity trip ~24–34° | `fc=0.5` applied to full target error (1.57 rad) → constant assist shove; measured motion exceeded Davout cap |
| Arm stuck at q≈0 after config-only fix | `max_lead` clamp applied to **stored** slew state → trajectory froze when arm lagged |
| Weak / no breakaway | `fc=0` removed assist; frozen slew could not advance `q_des` |

### Berthier change (`f1d2be6`)

| Layer | Before | After |
|-------|--------|-------|
| Slew state | Clamped each tick in storage | **Trajectory accumulates** at `position_slew_rad_s` |
| MIT `q_des` | Same as stored command | **Per-tick** `clamp(q_traj, q, target, max_lead)` |
| Friction FF | `target − q` | **`q_des − q`** (bounded lead) |
| Damping scale | `target − q` | **`q_traj − q` while ramping**; settle error near goal |

Documented in [rust-patterns.md](../rust-patterns.md) §7 (joint space / hold-at). Right weighted bench tuning: `shoulder_pitch_right_only` — `position_slew_rad_s=0.08`, `max_lead=0.03`, velocity limits 2.0 rad/s, `fc=0.5` restored.

Large scripted sweeps in this ADR still require explicit trajectory velocity/acceleration planning; single-joint `hold-at` to 90° is retestable with the interim fix.

## Context

Right shoulder pitch bench bring-up reached a good small-motion result after the `hold-at 0.1 rad` workflow was tuned and validated. The accepted behavior is captured in commit `f6d55b3`:

- `Position` hold uses target-aware damping in Berthier so software damping settles the final hold without acting like extra moving friction during approach.
- The ramped MIT position command is clamped one-sided against the final target direction so `q_des` does not fall behind measured `q` and brake the joint while the final target is still ahead.
- Position-hold friction feedforward uses final target error, not ramp lead, so breakaway assist does not flip sign when the joint outruns the ramped command.
- The right-only bench tuning uses `kp=12.0`, `kd=1.0`, `position_slew_rad_s=0.20`, `position_slew_max_lead_rad=0.05`, and `friction.fc=0.5`.

That tuning made the small `hold-at 0.1 rad` move visually smooth and repeatable, with clean disable and `fault=0x0000`.

Immediately afterward, a larger operator-limit sweep was attempted from the staged `~/marengo` runtime:

```text
0.0 -> -0.85 -> 0.0 -> 1.57 -> 3.10 -> 0.0 rad
```

The sweep was intentionally inside the hard bench envelope:

- Motor bench envelope: `[-0.9, 3.17] rad`
- Requested negative endpoint: `-0.85 rad`
- Requested positive endpoint: `3.10 rad`
- Bench velocity cap: `0.5 rad/s`
- Bench torque cap: `5.0 Nm`

The sweep did **not** complete. During the first large negative move, Davout disabled the controller around `q=-0.28 rad` because feedback velocity was corroborated by position delta above the `0.5 rad/s` bench cap:

```text
feedback velocity limit exceeded
position_rad=-0.26615315675735474
raw velocity=-1.2146353721618652 rad/s
position_velocity_rad_s=-0.5512751488521395
limit_rad_s=0.5
trips=2

control tick failed:
safety: joint right_shoulder_pitch: feedback |velocity| 1.2146353721618652 > 0.5
```

The drive stayed electrically healthy (`fault=0x0000`) and Davout disabled as designed. The result is a control-design issue, not a motor fault and not a mechanical limit result.

### What this tells us

The `Position` mode tuning that is excellent for short, local moves is not yet a full-range trajectory controller. It behaves like a position hold with:

- a ramped `q_des`,
- bounded `q_des - q` lead,
- proportional spring action through MIT `kp`,
- static friction feedforward,
- target-aware damping feedforward,
- Davout velocity and torque safety gates.

For a small retarget, this is enough. For a large retarget, the combination can inject too much net acceleration before the outer loop has a planned braking phase. In the negative direction, the bench saw position-derived velocity slightly above the safety cap and disabled early.

The safety response was correct. The missing piece is a first-class trajectory layer for large position moves.

## Decision

Marengo will separate **position hold** from **position trajectory moves** on the bench.

`Position` hold remains the compliant local hold/retarget mode for small moves and final settling. Large commanded moves must be handled by a trajectory generator that explicitly respects velocity, acceleration, stopping distance, and endpoint margin before producing MIT setpoints.

### Mode semantics

We will preserve the existing meaning of `ControlMode::Position`:

- It is a hold / local retarget mode.
- It may be used for small `hold-at` moves where the distance is short enough that the current lead-limited ramp cannot exceed bench safety limits.
- It is still filtered by Davout for position, velocity, torque, homing, and watchdog safety.

We will add a trajectory-aware path for larger scripted moves. The implementation can be a new control mode or a structured sub-mode of position hold, but the behavior must be distinguishable in logs and tests.

Recommended names:

- `PositionTrajectory` if represented as a new `ControlMode`.
- `PositionMove` if represented as a Berthier-internal state while protobuf/API work is deferred.

### Trajectory generator

The trajectory layer must generate `q_des` from the commanded start and final target, not from a pure slew of the previous command. It must track at least:

- `q_start`
- `q_target`
- move direction
- current command position
- commanded velocity
- max velocity
- max acceleration
- stopping distance
- terminal tolerance
- whether the move is accelerating, cruising, decelerating, or holding

The first implementation should be a conservative trapezoidal profile:

```text
accelerate toward v_max
  -> cruise if distance remains
  -> decelerate based on stopping distance
  -> hand off to final position hold
```

An S-curve profile can follow later if jerk is visible, but trapezoidal velocity with explicit acceleration and braking is the minimum needed fix.

### Velocity and acceleration limits

Trajectory limits must be lower than Davout's hard safety limits.

For the right shoulder pitch bench:

- Davout velocity limit remains `0.5 rad/s`.
- Initial trajectory `v_max` should be no higher than `0.30 rad/s`.
- Initial acceleration should be conservative enough that position-derived velocity does not overshoot the Davout cap after static friction breakaway.
- A separate `large_move_velocity_rad_s` should be preferred over reusing `position_slew_rad_s`.

Rationale: `position_slew_rad_s` currently means "how fast the hold command chases the target." It is not a sufficient expression of physical motion speed because `kp`, friction feedforward, damping, and load can make the actual joint move faster than the command ramp during breakaway. A trajectory controller must limit the intended joint velocity and shape the braking phase.

### Stopping distance

The trajectory generator must begin deceleration using stopping distance, not only target proximity:

```text
stopping_distance = v_cmd^2 / (2 * a_max)
if remaining_distance <= stopping_distance:
    reduce v_cmd
```

This prevents the controller from relying on safety gates or near-target damping to arrest a large move.

### Feedforward and damping policy

The target-aware damping introduced for the smooth `0.1 rad` hold remains valid, but the trajectory mode needs a trajectory-aware version of the same idea.

Rules:

- Damping should not act like artificial friction while the joint is tracking the commanded trajectory in the correct direction.
- Damping should increase during deceleration, near the final setpoint, and when measured velocity is away from the desired trajectory.
- Damping should be based on velocity error when trajectory velocity exists:

```text
tau_d = kd * (dq_des - dq_measured)
```

not only:

```text
tau_d = -kd * dq_measured
```

This is the key distinction between holding still and tracking a moving setpoint.

### Friction feedforward policy

The current position-hold friction improvements should be retained:

- Use final target error for Coulomb direction in hold mode.
- Fade breakaway assist near the final target.
- Do not derive Coulomb direction from noisy near-zero measured velocity.

For trajectory moves, friction direction should normally follow commanded trajectory velocity (`dq_des`) while the profile is moving. Near the final target, hand off to the existing target-error-based hold friction.

Suggested rule:

```text
if |dq_des| > trajectory_velocity_deadband:
    friction_direction = sign(dq_des)
else:
    friction_direction = sign(q_target - q)
```

This avoids stale target-direction feedforward if a future trajectory has intermediate waypoints or reversals.

### Commanded MIT setpoints

For trajectory moves, Berthier should send:

- `position_rad = q_des`
- `velocity_rad_s = dq_des` if Davout and robstride path support it safely
- `kp` from config
- firmware `kd = 0.0` until the raw firmware velocity estimate is proven stable
- `torque_ff_nm = tau_g + tau_f + tau_d`

If firmware velocity feedforward is not used initially, `dq_des` should still exist internally and in logs for damping and diagnostics.

### Safety boundaries

Davout remains the final safety boundary:

- homing must be `Verified`;
- joint position must stay inside bench position envelope;
- position-derived velocity over limit disables;
- torque feedforward remains capped and rate-limited;
- comm watchdog disables on stale feedback.

The trajectory layer must be conservative enough that Davout velocity trips are exceptional, not normal motion termination.

### API and operator behavior

Bench tooling should distinguish "hold this pose" from "move through a range":

- `hold-at <joint> <rad>` may remain for small local hold retargets.
- A future command such as `move-to <joint> <rad>` or `sweep <joint> ...` should select the trajectory path.
- The MCP limit-sweep tool should use the trajectory path and report per-waypoint success/failure.

Until that exists, do not use large `hold-at` jumps as a full-limit test.

### Logging requirements

Position trajectory diagnostics must be logged at least once per second and on phase changes:

- joint
- `q`
- `dq`
- `q_des`
- `dq_des`
- target
- remaining distance
- trajectory phase (`accelerate`, `cruise`, `decelerate`, `hold`)
- `kp`
- `kd`
- `tau_g`
- `tau_f`
- `tau_d`
- estimated total torque
- velocity limit margin
- position limit margin

Davout velocity trips should include both raw firmware velocity and position-derived velocity, as they do today.

### Test requirements

Default tests must not require hardware.

Unit tests should cover:

- trapezoidal profile reaches target without exceeding configured `v_max`;
- deceleration begins when remaining distance is less than stopping distance;
- trajectory velocity reverses correctly for negative moves;
- `dq_des` goes to zero at final hold;
- damping based on `(dq_des - dq)` reduces resistance while tracking;
- friction direction follows `dq_des` during movement and target error near hold;
- position commands stay inside `position_slew_max_lead_rad` or its trajectory equivalent;
- generated commands never exceed configured bench position limits.

Simulation / replay tests should cover:

- `0 -> -0.85 -> 0 -> 1.57 -> 3.10 -> 0` with conservative velocity/acceleration limits;
- no Davout velocity-limit trip for nominal position-derived velocity;
- clean final disable and `fault=0x0000`;
- no hardware required in default `cargo test`.

Hardware acceptance should be gated and logged:

1. Start from Disabled / homing Verified / fault `0x0000`.
2. Run a short `0 -> 0.1 -> 0` smoke move.
3. Run `0 -> -0.3 -> 0` before the full negative margin.
4. Run `0 -> -0.85 -> 0` only if the shorter move has no velocity trip.
5. Run `0 -> 1.57 -> 0`.
6. Run `0 -> 3.10 -> 0`.
7. Run the full sweep only after each segment passes independently.

Each stage must end Disabled, with `fault=0x0000`, homing still Verified, and logs checked for velocity-limit warnings.

## Consequences

- The smooth `hold-at 0.1 rad` tuning remains accepted for local position hold.
- Large limit sweeps are deferred until trajectory control is implemented.
- Bench tests should not use large instantaneous `hold-at` retargets as a substitute for trajectory moves.
- Berthier gains trajectory state and tests.
- Davout remains the safety authority; this ADR does not weaken velocity, torque, homing, or watchdog checks.
- The right shoulder pitch negative-direction abort is treated as a successful safety catch and a design requirement for the trajectory layer.

## Alternatives considered

### Increase the Davout velocity limit

Rejected. The abort was near the configured bench limit and happened during a large commanded move. Raising the safety limit would hide the missing trajectory control and reduce bench protection.

### Lower `kp`

Rejected as the primary fix. Lower stiffness may reduce acceleration, but it would also degrade the small `0.1 rad` hold that now works well and still would not provide a planned deceleration phase.

### Lower `fc`

Rejected as the primary fix. Reducing friction feedforward may reduce breakaway acceleration, but earlier testing showed insufficient assist caused weak, chunked motion. The issue is not simply "too much friction assist"; it is unplanned large-move dynamics.

### Lower `position_slew_rad_s`

Partially useful but insufficient. A slower `q_des` ramp can reduce demand, but the actual joint can still move faster than the ramp after breakaway because the spring/assist dynamics are not velocity-planned. `position_slew_rad_s` alone is not a physical trajectory guarantee.

### Use firmware damping again

Rejected for now. Firmware damping depends on the drive's raw velocity estimate, which has been noisy on this bench. Berthier-side damping with Davout-sanitized velocity remains the right direction.

### Keep using `Position` mode for all moves

Rejected. It conflates local hold retargets with full-range motion. The full sweep demonstrated that safe range testing needs an explicit trajectory path.

## Open questions

1. Should `PositionTrajectory` become a protobuf/control mode, or should the first implementation be a Berthier-internal state behind a `move-to` command?
2. Should `dq_des` be sent through MIT velocity, or used only internally for damping at first?
3. What initial `v_max` / `a_max` should be used for the right shoulder pitch bench? A conservative starting point is `v_max <= 0.30 rad/s`, with acceleration tuned from replay and short hardware moves.
4. Should bench profiles define separate limits for local holds and large sweeps?
5. Should MCP gain a dedicated `pi_limit_sweep` tool that enforces the staged acceptance sequence?

## References

- [ADR 0004: Control modes and MIT command model](0004-control-modes-and-mit.md)
- [ADR 0006: Homing, zero, and joint reference strategy](0006-homing-zero-reference.md)
- [Safety](../safety.md)
- [Rust patterns: position hold ramping](../rust-patterns.md)
- [URDF Joint XML Spec](http://wiki.ros.org/urdf/XML/joint)
- [Joint Limits & Dynamics Deep Dive](https://leyaa.ai/core-engineering/learn/ros/part-2/ros-joint-limits-and-dynamics/deep)
