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

## Consequences

- PRs are not blocked on GPU sim or Isaac.
- `just sim-check` / compose profile `sim` requires `Dockerfile.sim`.
- Golden physics regressions may use hashed `qpos` in future.

## Alternatives considered

- Gazebo in CI: heavy ROS coupling.
- In-process Rust MuJoCo bindings only: deferred; Python smoke is sufficient for D1 bootstrap.
