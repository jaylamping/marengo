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

## Current state (2026-10-28)

- `UrdfGravityModel` (`crates/armee-dynamics/src/urdf_gravity.rs`) is still the virtual-work model above. It runs on the master [`assets/urdf/marengo.urdf`](../../assets/urdf/marengo.urdf) (right 5-DOF arm with CAD inertials). `arm_4dof.urdf` no longer exists; older slices live in `assets/urdf/archive/`.
- Construction fails closed (unsupported joint kinds, cycles, non-finite inertials), and non-modelled actuated joints must be held at a stated angle.
- Bench gravity fits (`pi_gravity_calibrate`, `marengo-log-cli gravity-fit`, `crates/armee-dynamics/src/calibration.rs`) check the model against measured torque. The pre-enable gravity gate requires residual < 0.20 Nm.
- Whole-body work (balance, CoM, locomotion at roadmap M8) will need more than gravity torques: full rigid-body dynamics (RNEA/CRBA) and a floating base. That calls for a new ADR revisiting `k`, a pure-Rust RNEA, or a Pinocchio exception.

## Consequences

- New workspace crate `armee-dynamics`.
- Berthier `ControlLoop` depends on `DynamicsModel`, not MuJoCo.
- Golden vectors for 4-DOF may be added in Phase 5 CI.
