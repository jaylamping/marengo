<p align="center">
  <img src="docs/portraits/marengo.jpg" alt="Jacques-Louis David, Napoleon crossing the Alps on Marengo" width="480"/>
</p>

# Marengo

Returning to the project: start with [docs/status.md](docs/status.md) for where things stand (bench checkpoint, open issues, open decisions, next steps), then [docs/roadmap.md](docs/roadmap.md).

One repo for a personal, full-size walking humanoid in the Unitree G1 class (1524 mm, 23 DOF, Robstride actuators): CAD manifests, wiring, URDF, the Rust runtime, and the operator UI. The live hardware today is the right 5-DOF arm on the bench, the deliberate first slice of the full body. SolidWorks and the harness docs define joints, frames, and limits. Control, safety, planning, and the operator UI read that same definition. When the robot changes in CAD, software should change with it.

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

Infrastructure crates and bins use `marengo-*` names.

Wire schemas live in [`proto/`](proto/). More context: [docs/architecture.md](docs/architecture.md), [docs/roadmap.md](docs/roadmap.md) (full humanoid milestones), [ADRs](docs/decisions/README.md), and [AGENTS.md](AGENTS.md) (detailed contributor/agent guide).

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
│   ├── urdf/marengo.urdf   # Master URDF (right arm today, from CAD)
│   ├── mjcf/marengo.xml    # MuJoCo model mirroring the URDF
│   └── meshes/             # visual/ + collision/ (not exported yet)
├── crates/                 # Armée libraries (each has a README)
├── bins/                   # Pi runtimes and dev tools
├── consul/                 # Frontend (Vite + React + TS)
├── tools/                  # Pi bench MCP, research MCP, limit sync
├── sim/                    # MuJoCo smoke tests
├── models/                 # ONNX policies (Git LFS; none yet)
├── config/                 # robot.yaml, motors.yaml, control.yaml, homing.yaml
├── docs/                   # Status, roadmap, architecture, safety, ADRs, commissioning, reviews
└── scripts/                # URDF export, deploy helpers
```

ONNX policies use Git LFS when present. See [.gitattributes](.gitattributes). CAD binaries are local-only — [cad/README.md](cad/README.md).

## Software

### Crates (`crates/`)

| Crate | Role |
|-------|------|
| `berthier` | 200 Hz control loop: trajectories, τ_g, position law, fuses |
| `davout` | Safety supervisor and the only path to the motors: reference/enable sequencing, limits, caps, watchdog, joint↔motor transform |
| `robstride` | Robstride CAN driver (MIT, lifecycle, parameters) |
| `chappe` | Topic bus (protobuf envelopes) and Unix-socket IPC |
| `armee-proto` | Generated protobuf types |
| `armee-kinematics` | URDF limits and velocity-scaled envelope |
| `armee-dynamics` | Gravity torques from URDF; calibration fits |
| `marengo-config` | Typed YAML config, atomic profile writes |
| `marengo-store` | SQLite log/session store |
| `marengo-candump` | candump parse, summary, Robstride enrichment |
| `marengo-deploy` | Pi self-update and deployed-version logic |
| `marengo-host-metrics` | Linux host metrics for Chappe |
| `marengo-imu` | BNO085 IMU driver (SHTP over I2C) |
| `marengo-support` | Shared tracing init |
| `sim-harness` | URDF↔MJCF parity and sim test helpers |

Each crate's boundary is documented at the top of its `src/lib.rs`; READMEs exist for the core crates.

### Binaries (`bins/`)

| Binary | Host | Purpose |
|--------|------|---------|
| `marengo-pi` | Raspberry Pi | Control loop, CAN, Chappe, stdin REPL |
| `marengo-gateway` | Raspberry Pi | HTTP + WebTransport for Consul, log store |
| `motor-repl` | Pi (bench) | `status`, `disable`, `set-zero`, `protocol-inspect`, `gravity-preview` |
| `marengo-log-cli` | Pi / dev | Log archive, candump, `firmware-timing`, `gravity-fit` |
| `marengo-limit-sync` | Dev | Apply a Durable Set Limits patch to a local checkout |
| `imu-probe` | Pi | BNO085 quaternion probe |

### Frontend

[consul/](consul/) is the operator UI: live telemetry, the Hardware page (master URDF/YAML, Set Limits, Set Zero, readiness), testing panels and a URDF viewer.

### Tools

[`tools/marengo-pi-mcp`](tools/marengo-pi-mcp/README.md) drives the bench from an editor (deploy, logs, candump, calibration, motion suite, enable soak). `tools/marengo-research-mcp` is a research helper.

## Hardware workflow

1. Design in SolidWorks under the local `cad/` tree on the Windows PC (assemblies: `marengo.SLDASM`, sub-assemblies per limb).
2. Document limits and frames in [hardware/docs/kinematics.md](hardware/docs/kinematics.md).
3. Export URDF and meshes: `./scripts/export-urdf.sh` → `assets/`.
4. Wire and CAN: [hardware/electrical/wiring/](hardware/electrical/wiring/).

Vendor CAD (Robstride, extrusions) lives under the local `cad/vendor/`; [cad/manifests/vendor-assets.json](cad/manifests/vendor-assets.json) tracks it.

## Build

Windows development and local CAD live at `J:\code\marengo`; macOS uses a host checkout of the same repository. Docker runs the full Linux workspace checks. Start with [Windows and macOS development](docs/windows-macos-development.md) and [onboarding](docs/onboarding.md).

```bash
docker compose build dev
just check
```

Native host setup is optional ([docs/dev-setup.md](docs/dev-setup.md)). Rust conventions for contributors and agents: [docs/rust-patterns.md](docs/rust-patterns.md), [AGENTS.md](AGENTS.md).

Deploy: MCP `pi_sync_main` (preferred), or `scripts/deploy-pi.sh` / `just deploy-pi-docker`. systemd units: `scripts/systemd/`.

## CI

GitHub Actions runs `scripts/check.sh` in the dev image, plus `sim`, `vcan` and `build-image` jobs ([.github/workflows/ci.yml](.github/workflows/ci.yml)).

## License

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
