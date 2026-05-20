# Kinematics

Single source of truth for joint names, axes, limits, and transforms. Values here must match [`assets/urdf/arm_4dof.urdf`](../../assets/urdf/arm_4dof.urdf) (4-DOF bench) or [`assets/urdf/marengo.urdf`](../../assets/urdf/marengo.urdf) (2-DOF CI placeholder).

## 4-DOF bench arm joint table

| Joint | Actuator | Parent → Child | Axis (joint) | Lower (rad) | Upper (rad) | Effort (Nm) | Notes |
|-------|----------|----------------|--------------|-------------|-------------|-------------|-------|
| `shoulder_roll` | RS03 | base → shoulder_roll_link | Z | -1.57 | 1.57 | 60 | High shoulder torque |
| `shoulder_pitch` | RS03 | roll → shoulder_pitch_link | Z (after fixed rpy) | -1.2 | 1.2 | 60 | **Upright hazard** when q > ~0.5 rad |
| `upper_arm_yaw` | RS02 | pitch → upper_arm_link | Z | -1.57 | 1.57 | 17 | |
| `elbow` | RS02 | upper_arm → forearm | Z | 0.0 | 2.5 | 17 | **Upright hazard** — verify G-comp sign |

Masses and inertials in URDF are **estimates** until CAD export; re-run MuJoCo cross-check after export ([ADR 0005](../../docs/decisions/0005-dynamics-library.md)).

## Upright / elevated poses

Documented incident: arm elevated, control stopped, arm fell without gravity feedforward. Before bench tests with elevated shoulder/elbow:

1. Run per-joint `torque_ff` sign test.
2. Use **GravityComp** only until impedance is tuned.
3. Apply `danger_zones` in `config/control.yaml` for shoulder_pitch + downward velocity.

## Frames

- **base_link:** robot mount.
- **forearm_link:** tool frame placeholder (add TCP when gripper is defined).
