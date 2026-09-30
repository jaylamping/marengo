# crates/armee-dynamics/src/

## Responsibility
URDF-based gravity model implementation.

## Design
| Module | Role |
|--------|------|
| `lib.rs` | `DynamicsModel` trait, `PureGravityTorque`, `max_gravity_torque_over_range` |
| `urdf_gravity.rs` | `UrdfGravityModel` — COM positions, potential energy gradient |

## Flow
`gravity_torques(q)`: for each actuated joint, numerical ∂P/∂qᵢ where P = -Σ(m·g·COM)

## Integration
- Active public-model tests in `../tests/analytic_gravity.rs` use immutable URDFs
  and independent torque values; `../tests/archived_arm_geometry.rs` validates
  geometry-specific historical mass moments. Stale ignored golden/private-cache
  checks have been retired. Current-master independent physics acceptance remains
  open (CS21/T28).
