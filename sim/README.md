# Simulation fixtures and tests

CI uses minimal models here until [`assets/urdf/marengo.urdf`](../assets/urdf/marengo.urdf) is exported from CAD.

| File | Purpose |
|------|---------|
| `fixtures/minimal.urdf` | Kinematics / URDF validation (`armee-kinematics`) |
| `fixtures/minimal.xml` | MuJoCo headless smoke (`check-sim`) |

## Run locally

```bash
just sim-check
```

Requires `docker/Dockerfile.sim` (MuJoCo Python).

## Environment

- `MARENGO_SIM_MODEL` — path to MJCF (default: `sim/fixtures/minimal.xml`)

[ADR 0003](../docs/decisions/0003-simulation-testing.md).
