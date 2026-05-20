# ADR 0005: Pure Rust dynamics for gravity compensation

**Status:** Accepted  
**Date:** 2025-05-19

## Context

Gravity feedforward `tau_g(q)` must run on the Pi control loop without ROS, without mandatory GPU sim, and without `unsafe`/FFI unless separately excepted. Pinocchio and MuJoCo bindings were deferred for the realtime path ([ADR 0003](0003-simulation-testing.md) D1 cross-check only).

## Options evaluated

| Option | Pros | Cons |
|--------|------|------|
| **Pinocchio / C++ FFI** | Industry standard accuracy | FFI, build complexity, violates default `unsafe` policy |
| **MuJoCo in-process** | Matches sim | Not pure Rust; heavy on Pi |
| **`k` / `rigid-body-dynamics` crates** | Ready-made RNEA | API churn, URDF import maturity varies |
| **Virtual-work gravity from URDF (`armee-dynamics`)** | No FFI, uses existing `urdf-rs`, testable in CI | Numerical ∂COM/∂q; less accurate than full RNEA for complex trees |

## Decision

Implement **`armee-dynamics`** with:

1. `DynamicsModel` trait: `gravity_torques(&self, q: &[f64]) -> Result<Vec<f64>>`.
2. `UrdfGravityModel`: parse URDF via `urdf-rs`, forward kinematics, gravity torques via **virtual work** (numerical Jacobian of link COM positions w.r.t. actuated joints, `g = [0,0,-9.81]`).
3. **D1 validation:** optional MuJoCo script compares `tau_g` at sampled poses on `arm_4dof.urdf` (not a merge gate).

Selection criteria met: deterministic, `cargo test` on CI, no `unsafe`, works offline.

If accuracy is insufficient after CAD export, revisit `k` or generated Pinocchio with an ADR exception.

## Consequences

- New workspace crate `armee-dynamics`.
- Berthier `ControlLoop` depends on `DynamicsModel`, not MuJoCo.
- Golden vectors for 4-DOF may be added in Phase 5 CI.
