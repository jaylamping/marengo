# ADR 0003: Simulation testing tiers

**Status:** Accepted  
**Date:** 2025-05-19

## Context

Marengo needs automated validation before hardware is available on every machine. URDF export from CAD is not yet in `assets/`.

## Decision

Three tiers:

| Tier | Engine | Runs in `check.sh`? |
|------|--------|---------------------|
| D0 | URDF parse + kinematics (`urdf-rs`, `armee-kinematics`) | Yes |
| D1 | MuJoCo headless (`sim/fixtures/minimal.xml`, Python smoke + `sim-harness`) | No — `check-sim.sh` / CI `sim` job |
| D2 | Isaac Lab (Python) | Out of band; shares `proto/` only |

- CI fixtures live under [`sim/fixtures/`](../../sim/fixtures/) until production URDF exists.
- Production URDF changes require regen/compare of MJCF (`scripts/urdf-to-mjcf.sh` when added).

## Current state (2026-10-28)

- The CAD-derived right 5-DOF arm URDF is the master at [`assets/urdf/marengo.urdf`](../../assets/urdf/marengo.urdf), and the production MJCF [`assets/mjcf/marengo.xml`](../../assets/mjcf/marengo.xml) mirrors it (CAD inertials included). `scripts/check-sim.sh` smoke-tests both the minimal fixture and the production MJCF, and `sim-harness` holds URDF↔MJCF parity tests.
- `scripts/urdf-to-mjcf.sh` exists. A URDF change still needs the MJCF regenerated or updated, and the parity test catches drift.
- D2 (Isaac Lab or another RL/sim stack for locomotion) is still out of band and unstarted; it becomes relevant at roadmap M8.

## Consequences

- PRs are not blocked on GPU sim or Isaac.
- `just sim-check` / compose profile `sim` requires `Dockerfile.sim`.
- Golden physics regressions may use hashed `qpos` in future.

## Alternatives considered

- Gazebo in CI: heavy ROS coupling.
- In-process Rust MuJoCo bindings only: deferred; Python smoke is sufficient for D1 bootstrap.
