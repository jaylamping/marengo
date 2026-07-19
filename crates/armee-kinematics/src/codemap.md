# crates/armee-kinematics/src/

## Responsibility
URDF loading and limit policy implementation.

## Design
| Module | Role |
|--------|------|
| `lib.rs` | `load_urdf`, `joint_limits`, `actuated_joint_names`, `JointLimits` struct |
| `limits.rs` | `JointLimitPolicy`, velocity-scaled envelope (ADR 0009) |

## Integration
- `urdf_rs` crate for parsing; fixtures under `sim/fixtures/`
