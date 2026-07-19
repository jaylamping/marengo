# crates/armee-dynamics/

## Responsibility
Rigid-body gravity compensation torques tau_g(q) for the Marengo arm. Pure Rust, no CAN, no safety policy. Used by Berthier in gravity-comp and impedance modes to hold the arm against gravity.

## Design

### Core types
- `DynamicsModel` trait — single method `gravity_torques(&self, q: &[f64]) -> Result<PureGravityTorque, DynamicsError>`. Accepts joint positions in rad (joint order from `robot.yaml`), returns joint-space holding torque in Nm.
- `PureGravityTorque(Vec<f64>)` — newtype enforcing that `gravity_torques()` returns **only** gravity component. Prevents accidental mixing of friction, payload estimation, or velocity coupling into the gravity path. Implements `Deref<Target=Vec<f64>>` and `Index<usize>`.
- `UrdfGravityModel` — concrete implementation built from a URDF file and ordered joint names.
- `DynamicsError` — error enum: `Urdf(err)`, `Config(err)`, `BadGravityResult`.

### Algorithm (virtual-work gradient)
```
tau_g[i] = -dP/dq_i  where P = -sum(m_j * g · COM_j(q))
```
Numerical central difference at q ± DQ_EPS (1e-6) per joint:
1. For each actuated joint i, perturb q_i by ±DQ_EPS.
2. For each perturbed pose, compute every link's center of mass in world frame via URDF kinematic chain forward transform.
3. Compute potential energy: P = sum(mass_j * GRAVITY · com_world_j).
4. tau_g[i] = -(P(q + eps) - P(q - eps)) / (2 * eps).

### Implementation details
- `UrdfGravityModel::from_urdf(path, joint_names)` — loads URDF, precomputes link chain indices (root→leaf per link) to avoid O(n) joint scan per transform.
- `link_com_world(q_map)` — iterates links, computes world-frame COM using `link_transform`.
- `link_transform(link_name, q_map)` — traverses kinematic chain from root, applies joint transforms along the way using URDF poses and joint types (revolute/continuous/prismatic/fixed).
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
- **Test dependencies**: serial_test (for file-scoped URDF fixtures), approx (float comparison).
