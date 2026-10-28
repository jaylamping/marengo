# crates/armee-kinematics/src/

## Responsibility
URDF loading, expand-only URDF hard widening and limit policy implementation.

## Design
| Module | Role |
|--------|------|
| `lib.rs` | `load_urdf`, `joint_limits`, `joint_limit_bounds`, `actuated_joint_names`, `JointLimits` struct, test `fixtures` |
| `limits.rs` | `JointLimitPolicy`, velocity-scaled envelope (ADR 0009) |
| `expand.rs` | `expand_urdf_joint_hard` (ADR 0017) |

## Integration
- `urdf_rs` crate for parsing; fixtures under `sim/fixtures/` and `assets/` (`test-support` feature)
