# Agent instructions (Marengo)

**Generated:** 2026-06-19 · **Commit:** f77fb · **Branch:** main

Short index — full rules live in linked docs. Subdirectory guides: [crates/](crates/AGENTS.md), [bins/](bins/AGENTS.md), [consul/](consul/AGENTS.md), [config/](config/AGENTS.md), [scripts/](scripts/AGENTS.md), [tools/](tools/AGENTS.md), [.cursor/](.cursor/AGENTS.md).

## STRUCTURE

```
marengo/
├── Cargo.toml          # Armée workspace: 16 crates + 9 bins, forbids unsafe
├── crates/             # Libraries (codenames: Berthier=control, Davout=safety, …)
├── bins/               # Thin runtimes: marengo-pi, marengo-jetson, motor-repl, …
├── proto/              # Protobuf wire types (Chappe) — proto-first API changes
├── consul/             # Vite+React+TS operator UI (separate npm workspace)
├── config/             # robot.yaml, motors.yaml, control.yaml + bringup/ profiles
├── scripts/            # check.sh, deploy-pi.sh, pi-remote.sh, vcan, sim, systemd
├── tools/              # Vendored MCP servers (marengo-pi-mcp, mem0-mcp) + protoc
├── docs/               # architecture, ADRs, rust-patterns, safety, bench logs
├── hardware/           # Electrical, prints, BOM, hardware ADRs (not software)
├── cad/                # SolidWorks tree (local-only); manifests tracked
├── assets/             # URDF + meshes (exported from CAD)
├── models/             # ONNX policies (Git LFS)
├── sim/                # MuJoCo simulation configs
├── docker/             # dev / check / vcan / sim containers
└── .cursor/            # Agent rules, skills, plans, environment.json
```

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Control loop / modes | `crates/berthier/src/loop.rs` | Outer loop, friction FF → Davout |
| Safety supervisor | `crates/davout/src/lib.rs` (`Supervisor` @ L166) | Sole path to robstride; `disable_all`, `request_enable` |
| CAN encode/decode | `crates/robstride/` | MIT frames, no policy |
| Gravity torques | `crates/armee-dynamics/` | `gravity_torques(q)` only |
| Config loaders | `crates/marengo-config/` | `control.yaml`, `motors.yaml`, `robot.yaml` |
| Pi runtime | `bins/marengo-pi/` | Control + CAN + Chappe on Pi |
| Bench motor tool | `bins/motor-repl/` | `status`, `enable`, `jog`, `set-zero`, `gravity-on` |
| Wire types | `proto/` → `crates/armee-proto/` (Rust) + `consul/src/gen/` (TS) | Never hand-edit gen/ |
| Bringup profiles | `config/bringup/<profile>/` | `shoulder_pitch_right_only` is bench default |
| Pi deploy / logs | `scripts/pi-remote.sh` | Cloud fallback when MCP unavailable |
| ADRs | `docs/decisions/` | 14 ADRs (wire types, control modes, homing, …) |

## Before any change

1. Read [docs/rust-patterns.md](docs/rust-patterns.md) (good/bad Rust, control law).
2. Read [docs/safety.md](docs/safety.md) if touching control, CAN, or enable paths.
3. Run **`just check`** or `docker compose run --rm check` before finishing.

## Never

- Hand-edit `consul/src/gen/` (run `cd consul && npm run gen:proto`).
- Add JSON as the Chappe wire format ([ADR 0001](docs/decisions/0001-protobuf-wire-types.md)).
- Use `unwrap()` / `expect()` in `crates/*` library code (clippy `warn`).
- Skip Davout for motor commands — Berthier → Davout → robstride, no shortcuts.
- Berthier opens CAN directly (see rust-patterns §3).
- Edit the plan file in `.cursor/plans/` during implementation tasks unless asked.
- `println!` for runtime logs in `marengo-pi`/`marengo-gateway` — use `tracing` + ChappeLogLayer.
- Reconstruct Robstride arbitration IDs at call sites — use `robstride::encode_*` helpers.

## Always

- Proto-first API changes under `proto/`; regenerate Rust + TS.
- Thin `bins/`, logic in `crates/`.
- Workspace forbids `unsafe` via `[workspace.lints]` unless an ADR says otherwise.
- Call `marengo_support::init_tracing()` in bin `main` (or `chappe::tracing_layer::init_subscriber` for Chappe producers).
- Update `docs/rust-patterns.md` when introducing a new recurring pattern.
- Read each crate's `src/lib.rs` `//!` doc before editing (responsibilities + allowed deps).
- Default `cargo test` must not require hardware.

## CONVENTIONS (deviations from standard)

- **Codename system:** crates use Napoleonic names (Berthier, Davout, Talleyrand, …) — see README table.
- **Joint vs motor space:** Berthier, armee-dynamics, Chappe, Davout limits operate in URDF **joint space**. robstride is **raw motor/CAN space only**. Davout owns `direction`/`gear_ratio` transforms.
- **Control law:** single-pass trapezoidal planner + MIT setpoint clamp — see rust-patterns §7 for the full law (max_lead, breakaway, return lag, limit envelope ADR 0009, velocity cap ADR 0010).
- **Velocity caps:** resolved only from `config/control.yaml` via `marengo-config::resolve_joint_velocity_cap`. `motors.yaml`/`robot.yaml`/URDF velocity fields do **not** override.
- **Logging:** Chappe producers (`marengo-pi`, `marengo-gateway`) → `init_subscriber` (publishes `LogEvent` on `logs/structured`). Other bins → `init_tracing` (stdout/journal).
- **BNO085 I2C:** plain I2C reads, not smbus register reads (`EREMOTEIO` on Pi is normal).

## ANTI-PATTERNS (THIS PROJECT)

- **Upright-pose fall:** never position-hold an elevated arm without GravityComp (`kp=0, kd=0, torque_ff=tau_g`). See safety.md §"Upright-pose incident".
- **Blind enable:** no motor enable without homing Verified + Davout state machine.
- **Danger zones:** rules evaluate **measured** `q`/`dq`, not commanded MIT fields. Prefer `clamp_torque` when `kd_mit = 0`.
- **Limit envelope:** uses `max(|dq_cmd|, |dq_meas|)` so gravity-driven motion cannot shrink it.
- **Folding max_lead into planner:** freezes reference when arm lags — do not.

## Environment

- Preferred: Dev Container or `docker compose` ([onboarding.md](docs/onboarding.md)).
- Native host tooling is best-effort ([dev-setup.md](docs/dev-setup.md)).
- Windows host: default shell is PowerShell — no `&&`/`||` (see `.cursor/rules/windows-shell.mdc`).

## Architecture

- [architecture.md](docs/architecture.md)
- [ai-sdd.md](docs/ai-sdd.md) — Gentle-AI SDD, mem0, feasibility gate, Consul `/memory`
- [roadmap.md](docs/roadmap.md) — full humanoid target; 4-DOF arm is current execution slice, not project scope
- ADRs: [docs/decisions/](docs/decisions/) (14 total: 0001 protobuf wire → 0014 jetson perception)

## Simulation & CAN

- Default tests: no hardware.
- SocketCAN test harness: `just vcan` then `cargo test -p robstride --features socketcan -- --ignored`
- Sim: `just sim-check` ([ADR 0003](docs/decisions/0003-simulation-testing.md))

## COMMANDS

```bash
just check              # CI-parity (container): build + fmt + clippy + test + deny
just check-native       # Host-native: ./scripts/check.sh (cloud VMs)
just vcan               # Virtual CAN bus up
just sim-check          # MuJoCo sim checks
cargo test --workspace  # Default tests (no hardware)
cd consul && npm run gen:proto   # Regenerate TS proto types
cd consul && npm run build       # Consul type-check
```

## Cursor Cloud specific instructions

Cloud agents run natively (no Docker) with pre-installed tooling: Rust 1.88, Node 24, protoc 28.3, cargo-deny 0.16.3, cargo-audit, and the pinned advisory-db.

### First-time VM setup

Fresh cloud VMs do not include Docker or the dev-container toolchain. Run once:

```bash
./scripts/setup-cloud.sh
```

This installs protoc, cargo-deny, cargo-audit, the pinned advisory-db, and runs `./scripts/bootstrap.sh`.

### Running checks

Use `./scripts/check.sh` directly (the `just check-native` target). Docker and `just check` are unavailable in the cloud VM. The cross-build smoke test (aarch64) is skipped; this is non-fatal.

### Key commands

| Task | Command |
|------|---------|
| Full CI-parity check | `./scripts/check.sh` |
| Build workspace | `cargo build --workspace` |
| Lint (fmt) | `cargo fmt --all -- --check` |
| Lint (clippy) | `cargo clippy --workspace --all-targets -- -D warnings` |
| Test | `cargo test --workspace` |
| Consul proto codegen | `cd consul && npm run gen:proto` |
| Consul type-check | `cd consul && npm run build` |

### Gotchas

- `cargo deny check --disable-fetch` requires the pinned advisory-db at `/usr/local/cargo/advisory-db-pinned/github.com-a946fc29ac602819`. `./scripts/setup-cloud.sh` installs and pins it; do not let `cargo audit` auto-update that tree.
- Consul `dev` script is a scaffold (`echo "Vite app scaffold TBD"`); there is no running frontend dev server.
- `motor-repl` uses SocketCAN only; use `just vcan` / the ignored SocketCAN tests for no-hardware bus checks.
- Default `cargo test` requires no hardware. vCAN and simulation tests are optional and need Docker.

### Pi bench access (Tailscale)

Cloud VMs reach the robot via **Tailscale userspace networking** — not `marengo.local` mDNS. `.cursor/environment.json` runs `./scripts/setup-cloud-pi.sh` on boot.

**One-time:** add secrets per [docs/cloud-pi-tailscale.md](docs/cloud-pi-tailscale.md) (`TAILSCALE_AUTH_KEY`, `MARENGO_PI_SSH_PRIVATE_KEY_B64`, `MARENGO_PI_HOST`).

**Pi / logs (no marengo-pi MCP in cloud):** use `./scripts/pi-remote.sh`:

| Task | Command |
|------|---------|
| Verify connectivity | `./scripts/pi-remote.sh verify` |
| Health | `./scripts/pi-remote.sh health` |
| Log triage | `./scripts/pi-remote.sh logs-last-fault` → `logs-tail` → `logs-grep` |
| Archive sessions | `./scripts/pi-remote.sh logs-sessions` |
| CAN wire truth | `./scripts/pi-remote.sh candump-summary` |
| Deploy to Pi | `./scripts/pi-remote.sh deploy --install` |

Do not ask the user to paste Pi logs or run SSH when `pi-remote.sh` can fetch them.

## Learned User Preferences

- Early bench commissioning (roll / multi-DOF bringup): prioritize reliable safe motion over perfect mass or COM tuning when hardware is still changing week to week.
- For a new joint on the same actuator type as an existing bench motor, start from the proven pitch motor settings (including impedance `ki`) and flip `direction` sign rather than inventing fresh tuning.
- Exclude `var/pi-traces/*.log` from commits unless explicitly requested — bench trace artifacts, not source.
- Re–set-zero at mechanical home whenever arm configuration changes (bare motor vs attached arm, bolt-on segments).
- On `arm_2dof_right` wave motion: **pitch** raises the arm (~π–2.8 rad); **roll** oscillates — do not swap those joint roles.

## Learned Workspace Facts

- SolidWorks CAD binaries under `cad/assemblies`, `cad/parts`, `cad/vendor`, and `cad/exports` are gitignored local-only (removed from git tracking in `0a0adc1`); restore from Git LFS history at the commit before removal and hydrate blobs if pointers remain.
- `pi_sync_bench_config` syncs bringup YAML only — it does not deploy `assets/urdf/`; verify gravity preview after COM/URDF edits or copy URDF assets separately.
- Windows Pi Docker deploy from Git Bash fails on `unix:///var/run/docker.sock` even when Docker Desktop works in PowerShell; set `$env:DOCKER_HOST='npipe:////./pipe/dockerDesktopLinuxEngine'` before `deploy-pi-docker.sh`.
- `pi_sync_main` deploys `main`, may switch the local checkout to `main`, and refuses a dirty git tree; feature-branch Pi deploys use `deploy-pi-docker.sh` with the Docker host workaround above and `MARENGO_SKIP_CONSUL=1` when consul WIP breaks the build.
- Native Windows `cargo test` for crates using `chappe` fails on Unix socket APIs; use `just check` (container) for valid Rust test results on Windows.
- Consul actuator assignment and joint limit edits are not wired today — use `config/bringup/<profile>/motors.yaml` and `control.yaml`, then sync to Pi.
- Active 2-DOF right bench profile is `config/bringup/arm_2dof_right/` with roll CAN id 1 and pitch CAN id 2 on `can0`; roll limits 0→π rad (arm down to sky); re–set-zero roll at arm-down mechanical home (q≥0) before enable after physical homing.
- Roll velocity-limit trips on ascent often trace to `actuator_groups.shoulder_roll.velocity_max_rad_s` disagreeing with joint `position_trajectory_velocity_rad_s` — align both before raising traj speed.
- Never call synchronous `PositionTrace::flush()` in the Berthier 200 Hz loop (~70 ms SD fsync on Pi trips the 50 ms comm watchdog); flush on session `Drop` only.
- **Signed-off position-hold baseline** (`ff9d554`): kp 18 / kd 3 / ki 5, fc 0.08, max_lead 0.12, group vel 2.5 — see [docs/bench-2dof-right-smoke.md](docs/bench-2dof-right-smoke.md); mem0 `control/bench/arm-2dof-right-baseline`.
- Roll `position wave` under elevated pitch: use **triangle** setpoints; sine profiles trip Davout feedback velocity with coupled load.
- Parallel `pi_set_zero` on multiple joints can race homing — zero roll then pitch sequentially before enable.
