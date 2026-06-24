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
- Golden tests in `tests/golden_tau_g.rs` (excluded from codemap tracking)
