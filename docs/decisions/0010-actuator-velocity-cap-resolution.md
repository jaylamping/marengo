# ADR 0010: Actuator velocity cap resolution

**Status:** Accepted  
**Date:** 2026-06-13

## Context

Command velocity limits (rad/s) were split across URDF, `motors.yaml`, `robot.yaml`, and `control.yaml` `motor_type_defaults`. Davout applied `motor_type_defaults.velocity_max_rad_s` at MIT send time while `JointLimitPolicy.velocity` omitted that layer. Berthier planner cruise speed (`position_trajectory_velocity_rad_s`) was configured separately and could drift out of sync (e.g. RS03 shoulder bench at `v=2.0`).

Operators need per-actuator and grouped tuning (shoulder pitch L/R, hips, …) without changing every motor of the same type. Future Consul UI will edit these values live; this ADR establishes the config model and single resolver.

## Decision

### Desired cap (operator intent)

Resolve in priority order:

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

### Effective cap (runtime)

```text
effective_cap = min(
  desired_cap,
  urdf_joint_limit.velocity,
  motors.yaml bench.velocity_limit_rad_s,
  robot.yaml bench.max_joint_velocity_rad_s,
)
```

Implemented in `marengo-config::resolve_joint_velocity_cap`. Davout stores the result in `JointLimitPolicy.velocity` at startup. MIT filtering, firmware speed mode, and feedback velocity guards use that field only.

Berthier clamps planner `v_max` (slew and trajectory) against `Supervisor::joint_velocity_cap()` so commanded `dq_ref` never exceeds Davout.

Load-time validation (`validate_control_against_limits`) rejects `position_trajectory_velocity_rad_s` above effective cap.

### Semantics

| Layer | Role | Editable from Consul (future) |
|-------|------|--------------------------------|
| URDF / `motors.yaml` / `robot.yaml` | Safety ceilings | Read-only display |
| Joint / group / motor-type desired cap | Operator tuning | Yes |
| `position_slew_rad_s`, trajectory v/a | Motion shaping | Yes (separate fields) |

Consul will show **desired** vs **effective** when bench ceilings clamp the operator request.

## Consequences

- Shoulder bring-up profiles declare `actuator_groups.shoulder_pitch` instead of relying solely on global `rs03` defaults.
- `motor_type_defaults.velocity_max_rad_s` remains the fallback for joints without override or group.
- Proto/Chappe tuning RPC and live overlay on Berthier are follow-ups; resolver API is stable for that path.

## References

- [rust-patterns.md](../rust-patterns.md) §7
- [ADR 0004](0004-control-modes-and-mit.md) — MIT velocity field
- [ADR 0007](0007-bench-position-trajectory-control.md) — planner cruise speed
- [ADR 0009](0009-dynamic-position-limit-envelope.md) — position envelope (orthogonal)
