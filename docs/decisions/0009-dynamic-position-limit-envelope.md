# ADR 0009: Dynamic velocity-scaled position limit envelope

**Status:** Accepted (2026-06-13)  
**Date:** 2026-06-13

## Context

Shoulder pitch bench has three limit layers:

| Layer | Source | Example (right pitch) |
|-------|--------|------------------------|
| Hard | URDF `<limit>` ∩ `motors.yaml` bench | `[-0.9, 3.17] rad |
| Soft (operator) | URDF `<safety_controller>` | `[-0.872665, 3.141593] rad |
| Gap at bottom | hard − soft | ~27 mrad |

Full-range weighted `hold-at` sweeps (top → bottom in one leg) overshot soft bottom and tripped Davout hard-limit faults when measured `q` passed `-0.9` rad. Stopping distance at `v=2`, `a=4.8` is ~0.42 rad — far larger than the hard/soft gap.

Berthier `clamp_trajectory_setpoint` could set `q_des = q` on overshoot, commanding past hard limits when the arm coasted under gravity.

## Decision

Implement a **shared velocity-scaled command envelope** in `armee-kinematics` (`limits.rs`), built per joint at Davout startup as `JointLimitPolicy`, and applied in three layers:

1. **Berthier (plan):** clamp `hold-at` targets; clamp planner `q_traj` after each tick; clamp `q_des` (including overshoot branch); approach `v_max` cap near envelope walls.
2. **Davout (command):** clamp MIT `position_rad` into envelope; fault only if still outside hard after clamp (bug guard).
3. **Davout (measured):** fault when feedback `q` exceeds hard ± `position_limit_measured_fault_slack_rad` (default 5 mrad).

### Margin formula

```text
margin = min_rad + k_v_s * |dq_cmd| + k_stop * dq_cmd² / (2 * decel_rad_s2)
```

- `dq_cmd`: planner `dq_traj` or MIT `velocity_rad_s` (intent, not laggy feedback).
- Asymmetric: margin shrinks only the bound being approached (lower when `dq_cmd < 0`, upper when `dq_cmd > 0`).
- Effective interval clipped to `[soft_lower, soft_upper]` (URDF primary; optional YAML override per joint).

### Config (`control.yaml` per joint)

| Field | Default | Role |
|-------|---------|------|
| `position_limit_margin_min_rad` | 0.01 | Rest / crawl margin |
| `position_limit_margin_k_v_s` | 0.02 | Linear speed term |
| `position_limit_margin_k_stop` | 0.5 | Stopping-distance scale |
| `position_limit_measured_fault_slack_rad` | 0.005 | Measured-q fault slack |
| `position_soft_lower_rad` | (optional) | Override when URDF lacks `safety_controller` |
| `position_soft_upper_rad` | (optional) | Same |

`decel_rad_s2` for stopping distance uses `position_trajectory_accel_rad_s2`.

## Consequences

- Fast full-range moves may **not reach operator soft limit in one leg** until speed drops — correct tradeoff vs hard-limit trips.
- Position trace CSV adds `target_raw`, `q_env_lo`, `q_env_hi` columns (ADR 0007 trace schema bump).
- Existing `danger_zones` rules unchanged (semantic upright-pose guards); envelope handles limit approach.
- Talleyrand / Cartesian IK still resolves upstream; Berthier executes clamped joint targets.

## Alternatives considered

- **Static script margins** (`hold-at -0.85`): rejected — breaks when tuning or speed changes.
- **Raise hard limits only**: rejected for CAD/humanoid URDF immutability. **Bench bringup** Set Limits may expand URDF hard expand-only — see [ADR 0017](0017-bench-set-limits-urdf-expand.md).
- **Davout-only clamp**: rejected — planner still targets soft limit; overshoot `q_des` path remains.

## References

- [rust-patterns.md](../rust-patterns.md) §7
- [safety.md](../safety.md)
- [pi-commissioning.md](../pi-commissioning.md)
- [ADR 0007](0007-bench-position-trajectory-control.md) — trajectory layer; limit sweep follow-up
