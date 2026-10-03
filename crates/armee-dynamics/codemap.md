# crates/armee-dynamics/

## Responsibility
Rigid-body gravity compensation torques tau_g(q) for the Marengo arm. Pure Rust, no CAN, no safety policy. Used by Berthier in gravity-comp and impedance modes to hold the arm against gravity.

## Design

### Core types
- `DynamicsModel` trait — `joint_names()` and `gravity_torques(&self, q: &[f64]) -> Result<PureGravityTorque, DynamicsError>`. Accepts joint positions in rad in configured order, returns joint-space holding torque in Nm.
- `PureGravityTorque(Vec<f64>)` — semantic marker for gravity-only output, with public storage. Implements `Deref<Target=[f64]>` and `Index<usize>`; it does not mathematically validate arbitrary constructed values.
- `UrdfGravityModel` — concrete implementation built from a URDF file and ordered joint names.
- `DynamicsError` — `Urdf`, `JointCount`, `UnknownJoint`, and `UnknownLink`.
- `calibration` — bench gravity calibration (mass scale / COM offset fit, identifiability, refusals) used by `marengo-log-cli gravity-fit`.

### Algorithm (virtual-work gradient)
```
tau_g[i] = dP/dq_i  where P = -sum(m_j * g · COM_j(q))
```
Numerical central difference at q ± DQ_EPS (1e-6) per joint:
1. For each actuated joint i, perturb q_i by ±DQ_EPS.
2. For each perturbed pose, compute every link's center of mass in world frame via URDF kinematic chain forward transform.
3. Compute potential energy: P = -sum(mass_j * GRAVITY · com_world_j).
4. tau_g[i] = (P(q + eps) - P(q - eps)) / (2 * eps).

### Implementation details
- `UrdfGravityModel::from_urdf(path, joint_names)` — loads URDF, precomputes link chain indices (root→leaf per link) to avoid O(n) joint scan per transform.
- `link_com_world(q_map)` — transforms each COM as a point, including upstream joint-origin translations; multiplying an isometry by a vector would lose those lever arms (CS23).
- `link_transform(link_name, q_map)` — traverses the root-to-link chain, applies origins and revolute/continuous rotation. Prismatic motion, mimic and floating-base orientation are unsupported; omitted joint angles currently default to zero.
- Gravity vector: `[0, 0, -9.81]` (Z-down, standard URDF convention).
- Uses `nalgebra` for 3D transforms (Isometry3, Rotation3, Translation3).

### Accuracy and safety
- Estimates depend on URDF inertials from CAD export. Cross-check in sim per ADR 0005.
- Wrong tau_g sign is a safety issue — motor accelerates arm in gravity direction instead of holding. Validate with `motor-repl gravity-preview` before bench enable.
- `PureGravityTorque` type prevents accidental inclusion of non-gravity terms.
- Safety: motor-space transform `tau_motor = tau_g / (direction * gear_ratio)` is applied by Davout, not here.

### Helper functions
- `gravity_model_from_urdf(urdf_path, joint_names)` — convenience constructor.
- `max_gravity_torque_over_range(model, joint_index, q_min, q_max, steps)` — samples tau_g across a range to verify gravity comp won't saturate the drive. Clamped to minimum 2 steps (endpoints only).

## Flow
```
Berthier ControlLoop::tick
  → q = read_positions()
  → dynamics.gravity_torques(&q) → tau_g (PureGravityTorque)
  → tau_ff = tau_g + tau_f (friction) + tau_d (damping)
  → MitJointCommand { torque_ff_nm: tau_ff[i], ... }
  → Davout filter pipeline → robstride CAN encode
```

## Integration
- **Depends on**: `armee-kinematics` (load_urdf), `urdf_rs` (URDF parser), `nalgebra` (3D transforms).
- **Called by**: `berthier` (ControlLoop), `motor-repl` (gravity-preview command), tests.
- **Does not**: send commands, read encoders, open files (URDF loaded externally), run a control loop, or know about CAN/protocols.
- **Tests**: `tests/analytic_gravity.rs` exercises the public model with immutable one/two-link URDFs and independent literal torques. The retired eight ignored archived-model/private-chain checks were stale. Production-model parity and plant acceptance remain open (CS21/T27/T28).
