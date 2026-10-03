# Simulation fixtures and tests

`check-sim` smokes the minimal fixture plus the production model
[`assets/mjcf/marengo.xml`](../assets/mjcf/marengo.xml) (mirrors
[`assets/urdf/marengo.urdf`](../assets/urdf/marengo.urdf); body inertials are the
URDF CAD values — never hand-tuned). The Rust side (`sim-harness`) checks
URDF↔MJCF parity: joint names, axes, and range containment in the URDF hard
limits.

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
