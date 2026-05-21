<p align="center">
  <img src="docs/portraits/marengo.jpg" alt="Jacques-Louis David, Napoleon crossing the Alps on Marengo" width="480"/>
</p>

# Marengo

Personal humanoid robot: one repo for how it is built and how it runs. Mechanical and electrical design (CAD, kinematics, URDF, wiring) and the Rust runtime share the same model of the machine—hardware truth upstream, control, planning, safety, and operator tooling downstream.

## Naming

| Name | Role |
|------|------|
| **Marengo** | The robot (mechanical + electrical design, URDF, runtime) |
| **Armée** | Rust workspace — shared types, kinematics, crates |
| **Chappe** | Message bus between processes |
| **Berthier** | Realtime control |
| **Davout** | Safety supervision |
| **Talleyrand** | Motion planning |
| **Consul** | Web frontend |
| **Fouché** | Jetson-side vision and LLM |

Supporting crates: `armee-proto` (protobuf codegen), `armee-kinematics`, `robstride` (CAN driver). Wire schemas: [`proto/`](proto/). See [docs/architecture.md](docs/architecture.md), [docs/roadmap.md](docs/roadmap.md) (humanoid milestones; arm is current bench slice), and [ADR 0001](docs/decisions/0001-protobuf-wire-types.md).

## Repository layout

```
marengo/
├── Cargo.toml              # Armée workspace root
├── proto/                  # Wire-type source of truth (protobuf)
├── hardware/               # Physical robot — source of truth
│   ├── cad/                # SolidWorks + vendor STEP (Git LFS)
│   ├── electrical/         # PDB, harness, CAN docs
│   ├── prints/             # STLs + slicer notes
│   ├── bom/                # Master BOM
│   └── docs/               # Kinematics, assembly, hardware ADRs
├── assets/                 # Derived from hardware → consumed by software
│   ├── urdf/marengo.urdf   # SW → URDF export
│   └── meshes/             # visual/ + collision/
├── crates/                 # Armée libraries (each has a README)
├── bins/                   # Pi / Jetson runtimes and dev tools
├── consul/                 # Frontend (Vite + React + TS)
├── models/                 # ONNX policies (Git LFS)
├── config/                 # robot.yaml, motors.yaml, network.yaml
├── docs/                   # Software architecture + ADRs
└── scripts/                # URDF export, deploy helpers
```

Large binaries (CAD, STL, ONNX) are tracked with **Git LFS** — see [.gitattributes](.gitattributes).

## Software

### Crates (`crates/`)

| Crate | Codename | README |
|-------|----------|--------|
| `armee-proto` | Armée | [crates/armee-proto/README.md](crates/armee-proto/README.md) |
| `armee-kinematics` | Armée | [crates/armee-kinematics/README.md](crates/armee-kinematics/README.md) |
| `chappe` | Chappe | [crates/chappe/README.md](crates/chappe/README.md) |
| `berthier` | Berthier | [crates/berthier/README.md](crates/berthier/README.md) |
| `davout` | Davout | [crates/davout/README.md](crates/davout/README.md) |
| `talleyrand` | Talleyrand | [crates/talleyrand/README.md](crates/talleyrand/README.md) |
| `fouche` | Fouché | [crates/fouche/README.md](crates/fouche/README.md) |
| `robstride` | — | [crates/robstride/README.md](crates/robstride/README.md) |

### Binaries (`bins/`)

| Binary | Host | Purpose |
|--------|------|---------|
| `marengo-pi` | Raspberry Pi | Control, CAN, Chappe |
| `marengo-jetson` | Jetson | Planner, Fouché, Chappe |
| `probe` | Dev | Bus / diagnostics |
| `motor-repl` | Dev | Interactive motor exercise |
| `wave-demo` | Dev | Demo trajectories |

### Frontend

[consul/](consul/) — operator UI and URDF visualization.

## Hardware workflow

1. Design in `hardware/cad/` (assemblies: `marengo.SLDASM`, sub-assemblies per limb).
2. Document limits and frames in [hardware/docs/kinematics.md](hardware/docs/kinematics.md).
3. Export URDF and meshes: `./scripts/export-urdf.sh` → `assets/`.
4. Wire and CAN: [hardware/electrical/wiring/](hardware/electrical/wiring/).

Vendor CAD (Robstride, Moteus, extrusions) lives under `hardware/cad/vendor/`.

## Build

**Recommended:** containerized workflow — [docs/onboarding.md](docs/onboarding.md).

```bash
docker compose build dev
just check
```

Native host tooling is optional ([docs/dev-setup.md](docs/dev-setup.md)). Patterns for contributors and agents: [docs/rust-patterns.md](docs/rust-patterns.md), [AGENTS.md](AGENTS.md).

Deploy helpers (stubs): `scripts/deploy-pi.sh`, `scripts/deploy-jetson.sh`. systemd units: `scripts/systemd/`.

## CI

GitHub Actions runs `scripts/check.sh` in the dev image, plus optional `sim` and `vcan` jobs ([.github/workflows/ci.yml](.github/workflows/ci.yml)).

## License

MIT OR Apache-2.0 — see [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
