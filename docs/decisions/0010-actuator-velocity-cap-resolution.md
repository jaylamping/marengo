# ADR 0010: Actuator velocity cap resolution

**Status:** Accepted  
**Date:** 2026-06-13

## Context

Command velocity limits (rad/s) were split across URDF, `motors.yaml`, `robot.yaml`, and `control.yaml` `motor_type_defaults`. Davout applied `motor_type_defaults.velocity_max_rad_s` at MIT send time while `JointLimitPolicy.velocity` omitted that layer. Berthier planner cruise speed (`position_trajectory_velocity_rad_s`) was configured separately and could drift out of sync (e.g. RS03 shoulder bench at `v=2.0`).

Operators need per-actuator and grouped tuning (shoulder pitch L/R, hips, …) without changing every motor of the same type. Future Consul UI will edit these values live; this ADR establishes the config model and single resolver.

## Decision

### Velocity cap (single source of truth)

`control.yaml` is the only source for command velocity caps. Resolve in priority order:

1. `control.joints.<name>.velocity_max_rad_s` (optional per-joint override)
2. `control.actuator_groups.<group>.velocity_max_rad_s` when joint is listed
3. `control.motor_type_defaults.<type>.velocity_max_rad_s`

Define groups in `control.yaml`:

```yaml
actuator_groups:
  shoulder_pitch:
    joints: [left_shoulder_pitch, right_shoulder_pitch]
    velocity_max_rad_s: 2.0
```

Each joint may appear in at most one group. Group joints must exist in `control.joints` and `robot.joints`.

Implemented in `marengo-config::resolve_joint_velocity_cap` (alias for `resolve_desired_joint_velocity_cap`). Davout stores the result in `JointLimitPolicy.velocity` at startup. MIT filtering, firmware speed mode, and feedback velocity guards use that field only.

**Not used for command velocity caps:** `motors.yaml` `bench.velocity_limit_rad_s`, `robot.yaml` `bench.max_joint_velocity_rad_s`, and URDF joint `limit.velocity`. Those fields may remain for commissioning documentation or other subsystems; they must not silently clamp operator velocity intent at runtime.

Berthier clamps planner `v_max` (slew and trajectory) against `Supervisor::joint_velocity_cap()` so commanded `dq_ref` never exceeds Davout.

Load-time validation (`validate_control_against_limits`) rejects `position_trajectory_velocity_rad_s` above the resolved cap.

### Semantics

| Layer | Role | Editable from Consul (future) |
|-------|------|--------------------------------|
| Joint / group / motor-type cap in `control.yaml` | Operator command velocity limit | Yes |
| `position_slew_rad_s`, trajectory v/a | Motion shaping | Yes (separate fields) |
| URDF / `motors.yaml` / `robot.yaml` velocity fields | Not applied to command cap | N/A (orthogonal) |

Position and torque limits still use URDF + bench YAML as before (ADR 0009 envelope, effort min, etc.).

## Consequences

- Shoulder bring-up profiles declare `actuator_groups.shoulder_pitch` instead of relying solely on global `rs03` defaults.
- `motor_type_defaults.velocity_max_rad_s` remains the fallback for joints without override or group.
- Operators set the cap they want in `control.yaml`; no hidden 0.5 rad/s ceiling from stale bench YAML.
- Proto/Chappe tuning overlay on Berthier is live (`marengo-pi` `ActuatorOverlay`); `persist=true` is durable-write escalation within the existing Chappe `robot/actuator/command` trust (no separate auth), and YAML I/O is async off the 200 Hz path.

## References

- [rust-patterns.md](../rust-patterns.md) §7
- [ADR 0004](0004-control-modes-and-mit.md) — MIT velocity field
- [ADR 0007](0007-bench-position-trajectory-control.md) — planner cruise speed
- [ADR 0009](0009-dynamic-position-limit-envelope.md) — position envelope (orthogonal)
