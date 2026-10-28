# crates/armee-kinematics/

## Responsibility
URDF kinematic facts for the Marengo arm: joint limits, actuated joint indexing, expand-only URDF hard widening for Set Limits, and the velocity-scaled position command envelope (ADR 0009). **No CAN, no control loop, no dynamics torques.** Serves as the geometric source of truth for config validation and runtime limit enforcement.

## Design

### Core types
- `JointLimits` — URDF `<limit>` of a named actuated joint: `lower`, `upper`, `velocity`, `effort` (plain fields, no methods).
- `JointLimitBounds` — `hard_lower`, `hard_upper`, `soft_lower`, `soft_upper` fields. Built by `from_hard_and_soft`, which fails closed (`LimitBoundsError`) on non-finite or inverted hard limits or non-finite soft limits and clamps soft into hard.
- `JointLimitPolicy` — runtime limit policy built at startup by Davout `build_limits` from URDF + `motors.yaml` + `control.yaml`: `bounds`, `margin`, `velocity`, `effort`, `tau_ff_max`, with `hard_lower()`/`hard_upper()`/`soft_lower()`/`soft_upper()` accessors. Danger zones live in Davout, not here.
- `LimitMarginConfig` — per-joint margin tuning: `min_rad`, `k_v_s`, `k_stop`, `velocity_deadband_rad_s`, `measured_fault_slack_rad`, `decel_rad_s2`. No `Default`; the defaults live in marengo-config.
- `UrdfError` — `Read` (file load, missing or non-actuated joint) and `InvalidLimits`.

### Modules
- `limits` — velocity-scaled position limit envelope (ADR 0009):
  - `limit_margin_rad(dq_cmd, margin)`: `min_rad + k_v_s·v + k_stop·v²/(2·decel)`, with `v = 0` inside the velocity deadband.
  - `effective_command_bounds(policy, q, dq_cmd)`: hard bounds pulled in by the margin on the side the joint moves toward, then intersected with soft; collapses to the soft point nearest `q` when the margin eats the range.
  - `clamp_position_in_envelope(policy, q, dq_cmd, position_rad)`: clamp a commanded position into the effective envelope (never returns NaN).
  - `clamp_hold_target(policy, q, dq_cmd, requested_rad)`: clamp operator hold-at target; soft bounds first, then envelope at `dq_cmd`.
  - `measured_position_fault(q, policy)`: true when measured position exceeds hard limits + fault slack (fails closed on NaN).
  - `approach_velocity_cap(policy, q, dq_cmd, v_max)`: scale down cruise `v_max` approaching a **hard** wall minus margin (it does not taper toward a soft wall; review CFG-13).
- `expand` — `expand_urdf_joint_hard`: widen an in-memory URDF `<limit>` to cover a taught hard envelope, never shrink (ADR 0017).

### Key functions
- `load_urdf(path)` — parse a URDF file with urdf-rs.
- `joint_limits(robot, name)` — `<limit>` of a named revolute/continuous/prismatic joint.
- `joint_limit_bounds(robot, name)` — hard limits plus safety_controller soft bounds (URDF soft is a fallback; Davout overwrites it from `control.yaml`). Errors (`UrdfError::InvalidLimits`) on non-finite/inverted hard or non-finite soft limits.
- `actuated_joint_names(robot)` — revolute/continuous/prismatic joints in URDF document order.
- `actuated_joint_count(robot)` — count of actuated joints.

### Fixtures module (`test-support` feature or `cfg(test)`)
- `fixtures::minimal_urdf()`, `minimal_mjcf()` — `sim/fixtures/`.
- `fixtures::production_urdf()`, `production_mjcf()` — `assets/urdf/marengo.urdf`, `assets/mjcf/marengo.xml`.
- `fixtures::arm_4dof_urdf()`, `arm_4dof_right_urdf()` — archived seeds under `assets/urdf/archive/`.

## Flow
```
Startup (Supervisor::from_repo):
  → armee-kinematics::load_urdf(path)
  → armee-kinematics::joint_limit_bounds → JointLimitBounds
  → build_limits → JointLimitPolicy per joint
  → stored in Supervisor.limits

Per-tick (Davout filter and Berthier):
  → clamp_position_in_envelope(policy, q, dq_cmd, position_rad)
  → approach_velocity_cap(policy, q, dq_cmd, v_max)
  → clamp_hold_target(policy, q, dq_cmd, requested_rad)
  → measured_position_fault(q, policy) → fault in Davout check_feedback_position

Set Limits (Davout apply_limit_patch, marengo-config urdf_expand):
  → expand_urdf_joint_hard(robot, joint, lower, upper)
```

## Integration
- **Depends on**: `urdf_rs`, `thiserror`. No workspace crate in `[dependencies]`; `marengo-config` is a dev-dependency (tests read the master config).
- **Depended upon by**: `armee-dynamics` (load_urdf for gravity model), `davout` (limit policy building + runtime clamping), `berthier` (position-hold setpoint clamping), `marengo-config` (URDF expand/merge), `marengo-pi`, `sim-harness`.
- **Does not**: compute tau_g, send commands, open files beyond URDF loading, run a periodic loop.
- **URDF is the geometric source of truth**: keep `hardware/docs/kinematics.md` in sync when joints change.
