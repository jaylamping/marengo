# crates/armee-kinematics/

## Responsibility
URDF kinematic facts for the Marengo arm: joint limits, actuated joint indexing, and velocity-scaled position command envelope (ADR 0009). **No CAN, no control loop, no dynamics torques.** Serves as the geometric source of truth for config validation and runtime limit enforcement.

## Design

### Core types
- `JointLimits` — hard URDF position limits `(lower, upper)` from `<limit>` element; includes methods `contains(pos)` for checking within bounds.
- `JointLimitBounds` — hard limits plus optional `safety_controller` soft bounds from URDF (soft_lower, soft_upper). Methods: `hard_lower()`, `hard_upper()`, `soft_lower()`, `soft_upper()`.
- `JointLimitPolicy` — runtime limit policy built at startup by Davout from URDF + `motors.yaml` + `control.yaml`. Composed of: hard bounds, velocity cap, tau_ff max, LimitMarginConfig, danger zone rules. Used by Davout's filter pipeline and Berthier's position-hold setpoint clamping.
- `LimitMarginConfig` — per-joint margin tuning: `kinetic_margin_rad`, `stop_margin_rad`, `measured_fault_slack_rad`, `linear_margin_rate`.
- `UrdfError` — error enum for file loading, missing joints, parse failures.

### Modules
- `limits` — velocity-scaled position limit envelope (ADR 0009):
  - `limit_margin_rad(dq_cmd, margin)`: kinetic margin = linear_margin_rate * |dq_cmd| + stop_margin.
  - `effective_command_bounds(policy, q, dq_cmd)`: [lo, hi] = [hard_lower + margin, hard_upper - margin]; clamped to soft bounds when present.
  - `clamp_position_in_envelope(policy, q, dq_cmd, position_rad)`: clamp a commanded position into the effective envelope.
  - `clamp_hold_target(policy, q, dq_cmd, requested_rad)`: clamp operator hold-at target; soft bounds first, then envelope at dq_cmd.
  - `measured_position_fault(q, policy)`: true when measured position exceeds hard limits + fault slack.
  - `approach_velocity_cap(policy, q, dq_cmd, v_max)`: scale down cruise v_max when approaching an envelope wall.

### Key functions
- `load_urdf(path)` — parse a URDF file with urdf-rs.
- `joint_limits(robot, name)` — extract hard position limits for a named joint.
- `joint_limit_bounds(robot, name)` — hard limits plus safety_controller soft bounds.
- `joint_entry_count(robot)` — total URDF joints (including fixed/mimic).
- `actuated_joint_names(robot)` — revolute/continuous/prismatic joints in URDF document order.
- `actuated_joint_count(robot)` — count of actuated joints.

### Fixtures module
- `fixtures::arm_4dof_urdf()`, `fixtures::marengo_urdf()`, `fixtures::invalid_urdf()` — checked-in test fixtures under `sim/fixtures/`.
- `fixtures::fixture_path(name)` — resolve fixture path.

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
```

## Integration
- **Depends on**: `urdf_rs` (URDF parser). No other workspace crate dependencies — deliberately leaf-level.
- **Depended upon by**: `armee-dynamics` (load_urdf for gravity model), `davout` (limit policy building + runtime clamping), `berthier` (position-hold setpoint clamping).
- **Does not**: compute tau_g, send commands, open files beyond URDF loading, run a periodic loop.
- **URDF is the geometric source of truth**: keep `hardware/docs/kinematics.md` in sync when joints change.
