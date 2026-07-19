# Repository Guidelines

Practical guide for AI assistants working in the Marengo repository.

**Before editing:** read [`docs/rust-patterns.md`](docs/rust-patterns.md); for control/CAN/enable paths also read [`docs/safety.md`](docs/safety.md). For navigation, start at [`codemap.md`](codemap.md). Deeper per-folder guides: [`crates/AGENTS.md`](crates/AGENTS.md), [`bins/AGENTS.md`](bins/AGENTS.md), [`consul/AGENTS.md`](consul/AGENTS.md), [`config/AGENTS.md`](config/AGENTS.md), [`scripts/AGENTS.md`](scripts/AGENTS.md).

---

## Project Overview

Marengo is a **personal humanoid robot** in one repo: CAD, wiring, URDF, and the Rust runtime. SolidWorks and harness docs define joints, frames, and limits; control, safety, planning, and the operator UI consume that same definition.

| Name | Role |
|------|------|
| **Marengo** | The robot (mechanical + electrical + software) |
| **Armée** | Rust workspace (`Cargo.toml`) — 16 crates + 9 bins |
| **Chappe** | Inter-process message bus (binary protobuf) |
| **Berthier** | Realtime control loop |
| **Davout** | Safety supervisor — **sole path to motors** |
| **Talleyrand** | Motion planning (scaffold) |
| **Fouché** | Jetson vision/LLM (scaffold) |
| **Consul** | Operator web UI (Vite + React + TS) |

Current execution slice is a **2-DOF right bench arm** (`config/bringup/arm_2dof_right/`); full humanoid is the long-term target ([`docs/roadmap.md`](docs/roadmap.md)).

---

## Architecture & Data Flow

### Control stack (fixed motor path)

```
Consul (React) ←HTTP/WebTransport→ marengo-gateway ←IPC→ Chappe ← marengo-pi
                                                              ↓
                                                      Berthier (control)
                                                              ↓
                                                      Davout (safety)
                                                              ↓
                                                      robstride (CAN)
                                                              ↓
                                                      Robstride motors
```

**Berthier → Davout → robstride.** No shortcuts. Berthier never opens CAN.

| Layer | Owns | Must not |
|-------|------|----------|
| `berthier` | Joint-space trajectory, τ_g + impedance → MIT batch | CAN, limits, IK |
| `davout` | Enable FSM, filter, watchdog, send | Trajectories, URDF dynamics |
| `robstride` | MIT encode/decode, CAN I/O | Policy, safety |
| `armee-dynamics` | `gravity_torques(q)` | CAN, commands |
| `chappe` | Topic pub/sub (protobuf envelopes) | Control policy |

### Control tick (200 Hz on Pi)

1. Davout drains CAN feedback → joint positions (joint space)
2. Berthier computes τ_g, friction/impedance, planner output
3. MIT batch → Davout filters (limits, caps, danger zones) → robstride CAN
4. `marengo-pi` publishes `RobotState` on Chappe → gateway → Consul

### Coordinate spaces

- **Joint space:** Berthier, armee-dynamics, Chappe, Davout limits (URDF)
- **Motor space:** robstride only — Davout owns `direction` / `gear_ratio` transforms

### Wire types

Proto-first ([ADR 0001](docs/decisions/0001-protobuf-wire-types.md)): edit `proto/` → regenerate Rust (`armee-proto`) and TS (`consul/src/gen/`). Never hand-edit generated code.

---

## Key Directories

| Directory | Purpose |
|-----------|---------|
| `crates/` | Armée libraries — control, safety, CAN, config, Chappe, dynamics |
| `bins/` | Thin runtimes: `marengo-pi`, `marengo-gateway`, `motor-repl`, probes |
| `proto/` | Protobuf wire schemas (`marengo.v1`) |
| `consul/` | Operator UI — telemetry, enable, URDF viewer |
| `config/` | `robot.yaml`, `motors.yaml`, `control.yaml`, `homing.yaml` + `bringup/` profiles |
| `assets/urdf/` | URDF + meshes exported from CAD |
| `scripts/` | CI (`check.sh`), deploy, vcan, Pi remote, URDF validation |
| `docs/` | Architecture, safety, ADRs (`docs/decisions/`), bench runbooks |
| `docker/` | Dev, check, sim, vcan container images |
| `tools/` | Vendored MCP servers (`marengo-pi-mcp`, `marengo-research-mcp`) |
| `hardware/`, `cad/` | Electrical, prints, BOM, SolidWorks (CAD binaries local-only) |

---

## Development Commands

### Primary gate (run before finishing)

```bash
just check              # CI-parity in Docker: lint + fmt + clippy + test + deny + audit
just check-native       # Host-native: ./scripts/check.sh (cloud VMs)
```

### Build & test

```bash
cargo build --workspace
cargo test --workspace                    # No hardware required
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

### Consul (TypeScript)

```bash
cd consul && npm ci && npm run gen:proto && npm run build
cd consul && npm test                     # vitest
```

### Simulation & virtual CAN

```bash
just sim-check            # MuJoCo smoke + sim-harness tests
just vcan                 # Bring up vcan0/vcan1
just check-vcan           # robstride SocketCAN integration tests
cargo test -p robstride --features socketcan -- --ignored
```

### Pi deploy

```bash
just deploy-pi host=joey@marengo.local           # macOS cross-build + rsync
just deploy-pi-docker host=joey@marengo.local    # Windows / no native aarch64 GCC
MARENGO_SKIP_CONSUL=1 just deploy-pi-docker-binaries host=...  # binaries only
```

### Cloud / Pi bench (no MCP)

```bash
./scripts/setup-cloud.sh              # First-time cloud VM
./scripts/pi-remote.sh verify
./scripts/pi-remote.sh health
./scripts/pi-remote.sh logs-last-fault
./scripts/pi-remote.sh deploy --install
```

See [`docs/cloud-pi-tailscale.md`](docs/cloud-pi-tailscale.md) for Tailscale secrets.

---

## Code Conventions & Common Patterns

### Crate boundaries

```rust
// BAD — Berthier opens CAN
socketcan::CanSocket::open("can0")?;

// GOOD — Berthier → Davout → robstride
davout::filter(cmd)?;
robstride::send(cmd)?;
```

### Error handling

- **Libraries:** `thiserror` enums, `Result` in public APIs — no `unwrap()` / `expect()` (clippy `warn`)
- **Bins:** `anyhow::Result` in `main` is fine

### Async (Tokio)

- Async at Chappe, network, and CAN boundaries
- Do not block inside async without `spawn_blocking` or a dedicated thread

### Logging

| Binary type | Init |
|-------------|------|
| Chappe producers (`marengo-pi`, `marengo-gateway`) | `chappe::tracing_layer::init_subscriber` |
| CLI / scaffolds | `marengo_support::init_tracing()` |

No `println!` in `crates/` library code or Chappe producers — use `tracing`.

### Control law

Single-pass trapezoidal planner + MIT setpoint clamp ([`docs/rust-patterns.md`](docs/rust-patterns.md) §7). Velocity caps resolve only from `config/control.yaml` via `marengo-config::resolve_joint_velocity_cap`.

### Naming

- Crates use Napoleonic codenames (Berthier, Davout, Talleyrand, …)
- Read each crate's `src/lib.rs` `//!` doc before editing
- Thin `bins/`, logic in `crates/`

### Hard rules (never)

- Hand-edit `consul/src/gen/` — run `cd consul && npm run gen:proto`
- Add JSON as Chappe wire format
- Skip Davout for motor commands
- Position-hold an elevated arm without GravityComp (`kp=0, kd=0, torque_ff=τ_g`)
- Enable motors without homing Verified + Davout state machine
- Call synchronous `PositionTrace::flush()` in the Berthier 200 Hz loop (SD fsync trips watchdog)
- Reconstruct Robstride arbitration IDs at call sites — use `robstride::encode_*` helpers

### Hard rules (always)

- Proto-first API changes under `proto/`; regenerate Rust + TS
- Workspace forbids `unsafe` unless an ADR says otherwise
- Default `cargo test` must not require hardware
- Update `docs/rust-patterns.md` when introducing a recurring pattern

---

## Important Files

| File | Role |
|------|------|
| `Cargo.toml` | Workspace root — members, lints, shared deps |
| `rust-toolchain.toml` | Rust 1.88.0 + rustfmt + clippy |
| `justfile` | Developer task runner |
| `compose.yaml` | Docker dev/check/sim/vcan services |
| `scripts/check.sh` | CI-parity gate script |
| `proto/marengo/v1/marengo.proto` | Wire schema source of truth |
| `config/bringup/arm_2dof_right/` | Active 2-DOF bench profile |
| `assets/urdf/marengo.urdf` | Kinematic source of truth |
| `bins/marengo-pi/src/main.rs` | Pi control loop entry |
| `bins/marengo-gateway/src/main.rs` | HTTP/WebTransport gateway |
| `bins/motor-repl/src/main.rs` | Bench motor CLI |
| `crates/berthier/src/loop.rs` | `ControlLoop::tick` |
| `crates/davout/src/lib.rs` | `Supervisor` safety gateway |
| `deny.toml` | cargo-deny license/advisory policy |
| `.pre-commit-config.yaml` | fmt + buf lint hooks |

**Deploy layout on Pi:** `/opt/marengo` (binaries + config), `/etc/marengo/env`, systemd units in `scripts/systemd/`.

---

## Runtime/Tooling Preferences

| Tool | Version / choice |
|------|------------------|
| Rust | **1.88** (pinned in `rust-toolchain.toml`) |
| Node | **24.x** for Consul (`consul/package.json` engines) |
| Package manager | **npm** (not Bun) for Consul and MCP tools |
| Task runner | **just** (`just --list`) |
| Proto tooling | **buf** + **protoc** 28.x |
| Dev environment | **Container-first** — `docker compose` + `just check` ([`docs/onboarding.md`](docs/onboarding.md)) |
| Native host | Best-effort ([`docs/dev-setup.md`](docs/dev-setup.md)) |
| Windows shell | PowerShell default — no `&&`/`||` (see `.cursor/rules/windows-shell.mdc`) |
| Windows Rust tests | `chappe` Unix-socket tests fail natively — use `just check` in container |
| Windows Pi Docker | Set `$env:DOCKER_HOST='npipe:////./pipe/dockerDesktopLinuxEngine'` before deploy scripts |
| Formatting | rustfmt: 100 cols, Unix newlines (`rustfmt.toml`) |
| Lint | clippy `-D warnings`; buf STANDARD lint on proto |
| Cross-compile | `aarch64-unknown-linux-gnu` via Docker or `aarch64-linux-gnu-gcc` |

**Cloud VMs:** no Docker — use `./scripts/check.sh` after `./scripts/setup-cloud.sh`. Pi access via Tailscale + `pi-remote.sh`, not mDNS.

**MCP:** rebuild with `just mcp-build`; restart MCP servers after. Motion on Pi requires `confirm: true` on MCP tools.

### Agent tooling (Cursor)

- **Never** call `Grep` / `search` with an empty `pattern` — it fails with `Pattern must not be empty`.
- To **list or discover files**, use `Glob`, `find`, or `Read` — not grep with `""`.
- Repo root is the opened workspace (e.g. `C:\code\marengo` on this machine) — do not invent other paths from usernames or handoff context.

---

## Testing & QA

### Frameworks

| Language | Framework | Scope |
|----------|-----------|-------|
| Rust | `cargo test` | Unit tests (`#[cfg(test)]`) + `crates/*/tests/` integration |
| Rust | `proptest` | Mode isolation in `berthier/src/mode_isolation.rs` |
| TypeScript | `vitest` + testing-library | `consul/src/**/__tests__/` |
| Python | `unittest` / `pytest` | Daily audit, research MCP |
| Shell | assert-based runners | `scripts/*.test.sh` |

### Running tests

```bash
cargo test --workspace                           # Default — no hardware
cargo test -p berthier                         # Single crate
cargo test -p robstride --features socketcan -- --ignored   # Needs vcan
just sim-check                                 # MuJoCo + sim-harness
cd consul && npm test
python3 -m unittest scripts/daily-audit/test_audit.py
```

### CI (`.github/workflows/ci.yml`)

1. **check** — Docker dev image → `scripts/check.sh` (buf lint, consul build, URDF validation, fmt, clippy, test, deny, audit, aarch64 smoke)
2. **sim** (path-filtered) — MuJoCo smoke + `sim-harness`
3. **vcan** — SocketCAN integration on virtual interfaces

No coverage tooling is configured. Expectation: `just check` passes before merge.

### Hardware-gated tests

- Marked `#[ignore]` or require `--features socketcan`
- Use `just vcan` / `just check-vcan` — never required for default `cargo test`
- Physical bench protocols: [`docs/bench-gravity-comp-test-suite.md`](docs/bench-gravity-comp-test-suite.md), [`docs/bench-2dof-right-smoke.md`](docs/bench-2dof-right-smoke.md)

### Bench commissioning notes

- Active profile: `config/bringup/arm_2dof_right/` — roll CAN id 1, pitch CAN id 2 on `can0`
- **Pitch** raises arm (~π–2.8 rad); **roll** oscillates — do not swap roles
- Re–set-zero at mechanical home when arm configuration changes
- `pi_sync_bench_config` syncs YAML only — not `assets/urdf/`; verify gravity after URDF/COM edits
- Signed-off position-hold baseline: kp 18 / kd 3 / ki 5 — see bench smoke doc

---

## Repository Map

Full hierarchical codemap at [`codemap.md`](codemap.md). Read it before starting any task; for deep work, also read that folder's `codemap.md`.
