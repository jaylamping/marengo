<p align="center">
  <img src="docs/portraits/marengo.jpg" alt="Jacques-Louis David, Napoleon crossing the Alps on Marengo" width="480"/>
</p>

# Marengo

Returning to the project: read the [September 2026 repository review](docs/reviews/2026-09-29-repository-review.md) for the current architecture, bench checkpoint, outstanding defects, and ordered repair plan.

One repo for a personal humanoid: CAD, wiring, URDF, and the Rust runtime. SolidWorks and the harness docs define joints, frames, and limits. Control, safety, planning, and the operator UI read that same definition. When the robot changes in CAD, software should change with it.

## Naming

| Name | Role |
|------|------|
| Marengo | The robot (mechanical + electrical design, URDF, runtime) |
| Armée | Rust workspace: shared types, kinematics, crates |
| Chappe | Message bus between processes |
| Berthier | Realtime control |
| Davout | Safety supervision |
| Consul | Web frontend |
| Talleyrand, Fouché | Planner and Jetson vision/LLM: not built; the scaffolds were removed (2026-10-03 crate audit), [ADR 0014](docs/decisions/0014-jetson-perception-semantic-motion.md) holds the design |

Supporting crates: `armee-proto` (protobuf codegen), `armee-kinematics`, `robstride` (CAN driver). Wire schemas live in [`proto/`](proto/). More context: [docs/architecture.md](docs/architecture.md), [docs/roadmap.md](docs/roadmap.md) (full humanoid milestones; the arm is the current bench slice), [ADR 0001](docs/decisions/0001-protobuf-wire-types.md).

## Repository layout

```
marengo/
├── Cargo.toml              # Armée workspace root
├── proto/                  # Protobuf wire types
├── cad/                  # SolidWorks tree (local only); manifests tracked for MCP
│   └── manifests/        # MCP conventions + vendor registry (JSON)
├── hardware/             # Electrical, prints, BOM, hardware docs
│   ├── electrical/       # PDB, harness, CAN docs
│   ├── prints/             # STLs + slicer notes
│   ├── bom/                # Master BOM
│   └── docs/               # Kinematics, assembly, hardware ADRs
├── assets/                 # Exported from hardware, used by software
│   ├── urdf/marengo.urdf   # SW → URDF export
│   └── meshes/             # visual/ + collision/
├── crates/                 # Armée libraries (each has a README)
├── bins/                   # Pi runtimes and dev tools
├── consul/                 # Frontend (Vite + React + TS)
├── models/                 # ONNX policies (Git LFS)
├── config/                 # robot.yaml, motors.yaml, control.yaml, homing.yaml
├── docs/                   # Software architecture + ADRs
└── scripts/                # URDF export, deploy helpers
```

ONNX policies use Git LFS when present. See [.gitattributes](.gitattributes). CAD binaries are local-only — [cad/README.md](cad/README.md).

## Software

### Crates (`crates/`)

| Crate | Codename | README |
|-------|----------|--------|
| `armee-proto` | Armée | [crates/armee-proto/README.md](crates/armee-proto/README.md) |
| `armee-kinematics` | Armée | [crates/armee-kinematics/README.md](crates/armee-kinematics/README.md) |
| `chappe` | Chappe | [crates/chappe/README.md](crates/chappe/README.md) |
| `berthier` | Berthier | [crates/berthier/README.md](crates/berthier/README.md) |
| `davout` | Davout | [crates/davout/README.md](crates/davout/README.md) |
| `robstride` | — | [crates/robstride/README.md](crates/robstride/README.md) |

### Binaries (`bins/`)

| Binary | Host | Purpose |
|--------|------|---------|
| `marengo-pi` | Raspberry Pi | Control, CAN, Chappe |
| `motor-repl` | Dev | Interactive motor exercise |

### Frontend

[consul/](consul/) is the operator UI and URDF viewer.

## Hardware workflow

1. Design in `cad/` (assemblies: `marengo.SLDASM`, sub-assemblies per limb).
2. Document limits and frames in [hardware/docs/kinematics.md](hardware/docs/kinematics.md).
3. Export URDF and meshes: `./scripts/export-urdf.sh` → `assets/`.
4. Wire and CAN: [hardware/electrical/wiring/](hardware/electrical/wiring/).

Vendor CAD (Robstride, Moteus, extrusions) lives under `cad/vendor/`.

## Build

Windows development and local CAD live at `J:\code\marengo`; macOS uses a host checkout of the same repository. Docker runs the full Linux workspace checks. Start with [Windows and macOS development](docs/windows-macos-development.md) and [onboarding](docs/onboarding.md).

```bash
docker compose build dev
just check
```

Native host setup is optional ([docs/dev-setup.md](docs/dev-setup.md)). Rust conventions for contributors and agents: [docs/rust-patterns.md](docs/rust-patterns.md), [AGENTS.md](AGENTS.md).

Deploy helper: `scripts/deploy-pi.sh`. systemd units: `scripts/systemd/`.

## CI

GitHub Actions runs `scripts/check.sh` in the dev image, plus optional `sim` and `vcan` jobs ([.github/workflows/ci.yml](.github/workflows/ci.yml)).

## License

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
