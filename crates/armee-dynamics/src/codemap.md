# crates/armee-dynamics/src/

## Responsibility
URDF-based gravity model implementation.

## Design
| Module | Role |
|--------|------|
| `lib.rs` | `DynamicsModel` trait, `PureGravityTorque`, `max_gravity_torque_over_range` |
| `urdf_gravity.rs` | `UrdfGravityModel` — COM positions, potential energy gradient; `point_mass_torques`, `link_inertial`, `with_link_inertial`, `links_downstream_of` for calibration |
| `calibration.rs` | Gravity calibration: MAP fit of per-link mass scale / COM offset along the principal axis to bench holding torques, identifiability and residual refusals, `cancel_friction` |

## Flow
`gravity_torques(q)`: for each actuated joint, numerical ∂P/∂qᵢ where P = -Σ(m·g·COM)

## Integration
- Active public-model tests in `../tests/analytic_gravity.rs` use immutable URDFs
  and independent torque values; `../tests/archived_arm_geometry.rs` validates
  geometry-specific historical mass moments. Stale ignored golden/private-cache
  checks have been retired. Current-master independent physics acceptance remains
  open (CS21/T28).
- `../tests/gravity_calibration.rs` recovers synthetic mass/COM perturbations of the
  live URDF (friction, noise) and checks the ill-conditioned / residual / limit refusals.
- Consumer: `marengo-log-cli gravity-fit` (bench sessions from MCP `pi_gravity_calibrate`).
