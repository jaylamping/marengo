# Repository Guidelines

Marengo is a personal humanoid robot in one repo: CAD manifests, wiring docs, URDF, the Rust runtime (**Armée**), and the operator UI (**Consul**). The current execution slice is a **5-DOF right bench arm** on a Raspberry Pi 5. The full humanoid is the roadmap target (`docs/roadmap.md`).

**Before editing:** read `docs/rust-patterns.md`. For control, CAN, enable or reference code, also read `docs/safety.md`. Navigation starts at `codemap.md`. Per-folder guides: `crates/AGENTS.md`, `bins/AGENTS.md`, `consul/AGENTS.md`, `config/AGENTS.md`, `scripts/AGENTS.md`. Vocabulary (Set Zero vs Home, Joint/Limb/Robot Ready): `CONTEXT.md`.

---

## Project Overview

| Name | Role |
|---|---|
| **Armée** | Rust workspace (`Cargo.toml`): 15 crates + 6 bins |
| **Chappe** | Topic bus (protobuf envelopes) + Unix-socket IPC between `marengo-pi` and `marengo-gateway` |
| **Berthier** | 200 Hz control loop (`ControlLoop::tick`): trajectories, τ_g, hold law, fuses |
| **Davout** | Safety supervisor (`Supervisor`): **sole path to motors**, reference authority |
| **robstride** | Robstride CAN driver: MIT packing, lifecycle/param/identity frames |
| **Consul** | Operator web UI (Vite + React + TS) |

Removed on 2026-10-03 (crate audit), so do not resurrect them: `fouche`, `talleyrand`, `teleop`, `marengo-jetson`, `probe`, `wave-demo`, `chappe/src/transport.rs`, `config/network.yaml`, `homing-preflight.sh`, motor-repl `home/enable/jog/speed`, and MCP `pi_motor_enable`/`pi_jog`. ADR 0014 keeps the Jetson design as documentation only.

---

## Architecture & Data Flow

```
Consul ──HTTP / WebTransport──▶ marengo-gateway (tokio; Chappe IPC *listener*)
                                     ▲  /run/marengo/chappe.sock
                                     │  (client only if MARENGO_CHAPPE_SOCKET is set)
stdin REPL / MCP ──▶ marengo-pi (sync std thread, 200 Hz)
                       └─ Berthier ControlLoop ─▶ Davout Supervisor ─▶ robstride ─▶ SocketCAN can0/can1 ─▶ drives
```

**Fixed motor path: Berthier → Davout → robstride.** Berthier never opens CAN. Only `marengo-pi` and `motor-repl` open SocketCAN. `motor-repl disable` intentionally **bypasses Davout**: it sends one type-4 Disable, then one type-24 Off, per `motors.yaml` address (`bins/motor-repl/src/stop.rs`) so it can stop drives and their active reporting when no owner is running.

| Layer | Owns | Must not |
|---|---|---|
| `berthier` | Joint-space trajectory, τ_g + impedance → MIT batch, ascent/hold/wave fuses | CAN, limits, IK |
| `davout` | Enable/reference sequencing, limit envelope, caps, watchdog, joint↔motor transform | Trajectories, URDF dynamics |
| `robstride` | Wire encode/decode, CAN I/O, receive budget (64 frames / 256 reads per poll) | Policy, safety |
| `armee-dynamics` | `gravity_torques(q)` from URDF; calibration fit | CAN, commands |
| `armee-kinematics` | URDF limits, velocity-scaled envelope (ADR 0009) | Commands |
| `chappe` | Pub/sub + IPC; 7 allowlisted command topics, rejected if >1000 ms old (`ipc.rs`) | Control policy |

**marengo-pi loop** (`bins/marengo-pi/src/main.rs` `run_control_loop`), paced by `sleep(period − elapsed)`:
1. Drain stdin, then the Chappe command topics, testing MIT batches and the actuator overlay.
2. `loop_ctrl.tick()`. On error: `stop_after_tick_error`, which disables all and sets mode Disabled.
3. Pump reference-queue events, then `enable_gate.poll`.
4. At `chappe_state_hz` (25), publish `SafetyState` and limits. Publish a heartbeat at 1 Hz.

**`ControlLoop::tick`** (`crates/berthier/src/loop.rs`):
- If Davout reference work is pending, the tick only advances the reference and publishes state.
- Otherwise: drain feedback → q → τ_g → Position hold or MIT feed-forward → `filter_mit_to_active` → `send_mit_batch` → drain again → publish `RobotState` (Berthier publishes it, not marengo-pi).
- Listed `LoopError`s latch a controller fault and discard motion intent.

**Spaces:** joint space everywhere except robstride. Davout owns `direction`/`gear_ratio`.

**Wire types:** proto-first (ADR 0001). The single schema is `proto/marengo/v1/marengo.proto`.

### Runtime contracts (scripts and MCP depend on these)

- **Reference grants are process-local (ADR 0036).** `home <joint>... sign-tested` acquires physical evidence: stop, UID, SetZero, a type-2 ack, a 0x7019 readback, and a journal commit. Enable must happen **in the same marengo-pi process**. Calibration history, the journal and caller flags never grant. A `motor-repl set-zero` grant dies when that process exits.
- **Motion-owner lease:** `--motion-owner stdin|chappe` or `MARENGO_MOTION_OWNER` (default `chappe`), fixed for the process lifetime. Stop/observe commands are accepted from either source; motion is accepted only from the owner, and refusals publish `motion_refused` on `robot/audit/action`. Ad-hoc ssh sessions must set `MARENGO_MOTION_OWNER=stdin`; MCP tools already export it.
- **stdin grammar** (`parse_command`): `home <joints> sign-tested`, `enable [operator]` (a `force` token is ignored; nothing bypasses the gravity preflight), `disable`, `gravity-on|off`, `torque-cmd`, `impedance-on|off`, `hold-on`, `hold-at [joint] <rad>`, `hold-off`, `wave`, `lower` (degraded hold only), `status`, `quit`. EOF behaves like `quit`.
- **stdout lines scripts wait for:**
  - `reference <j> current pos=…`, `reference <j> failed: …`, `skipped: …`
  - `waiting for enable to complete`, then `enabled (operator=…)`
  - `enable failed: …`, `enable blocked: …`, `enable refused: …` (stderr), `home failed: …`
  - drive loss (ADR 0038): `drive lost <joint>: shed <a,b>; holding <c,d>; auto-lower in <s> s`, `degraded lower started (operator|timeout)`, `degraded lower complete; disabled`, `degraded episode ended (deadline|stop); disabled`
- **Single-drive loss (ADR 0038):** a silent drive whose joint has `on_drive_loss: shed_subtree` (admitted at startup by the offline τ_g bound; master: upper-arm yaw, elbow, lower-arm yaw) sheds itself and every distal joint with one Disable each; the proximal joints hold in Position, then lower to rest at ≤ `lower_velocity_rad_s` on `lower` or after `hold_window_s`, then every drive stops and a Communication fault latches (restart required). Davout enforces a hard deadline. Every other motion command is refused (stderr `… refused: degraded hold after losing <joint>; …` + `motion_refused`); `disable`/`hold-off`/E-stop stop everything at once. Any other loss stops every drive as before.
- **Enable completion gate:** `enabled` prints only after every target is Active, has no pending Enable writes, and has fresh feedback (`ENABLE_COMPLETION_TIMEOUT` = 2 s, after which all drives stop). `hold-on`/`hold-at`/`wave` sent earlier are deferred.
- **Gravity preflight:** stdin/Chappe Enable and Testing Position auto-enable first run the gravity saturation sweep, at most 2 ms per tick (`bins/marengo-pi/src/gravity_preflight.rs`), so `waiting for enable to complete` arrives about 0.2 s after `enable` on the Pi. Commands sent meanwhile, except `disable`/`quit`, wait and then run in order. If the sweep refuses or is voided (stop, new fault, reference work, limit change), the waiting commands are discarded: `enable refused: …` is printed, followed by `discarded N deferred command(s)`.
- **motor-repl** supports only `status | disable | set-zero <joint> [--sign-tested] | protocol-inspect [joint...] | gravity-preview <q × all joints>`. A partial angle vector is refused. `protocol-inspect` is read-only on Disabled drives (ADR 0037).

---

## Key Directories

| Path | Purpose |
|---|---|
| `crates/` | Libraries (control, safety, CAN, config, dynamics, store, deploy, IMU, metrics) |
| `bins/` | Thin runtimes: `marengo-pi`, `marengo-gateway`, `motor-repl`, `marengo-log-cli` (`firmware-timing`, `gravity-fit`), `marengo-limit-sync`, `imu-probe` |
| `proto/` | Protobuf schema (`buf` STANDARD lint, FILE breaking) |
| `consul/` | Operator UI; `src/gen/` is generated and gitignored (except `.checksum`) |
| `config/` | Master `robot/motors/control/homing.yaml`; `*_humanoid.yaml` are 23-joint templates used only in tests |
| `assets/urdf/marengo.urdf` | Kinematic + inertial source of truth (no meshes); `assets/mjcf/` holds the sim models |
| `tools/` | `marengo-pi-mcp` (Pi bench tools), `marengo-research-mcp` (Python/uv), `limit-sync-local`, `compound-auto-learn` |
| `scripts/` | `check.sh`, deploy/install, `pi-remote.sh`, vcan, systemd units (`scripts/systemd/`) |
| `docs/` | `safety.md`, `rust-patterns.md`, ADRs `decisions/0001–0038`, `commissioning/`, `reviews/` |
| `sim/` | MuJoCo smoke (`sim/scripts/smoke_test.py`, `sim/fixtures/minimal.xml`) |
| `cad/`, `hardware/` | Manifests and docs only; SolidWorks binaries are local to the Windows host |
| `var/` | Runtime output; `var/log`, `var/enable-soak`, `var/gravity-calibration` and `var/firmware-captures` are gitignored |

---

## Development Commands

```bash
just check                 # CI parity in Docker (authoritative gate) → scripts/check.sh
just check-native          # same script on the host (needs cargo, node 24, uv, python3, cargo-deny/audit)
just bootstrap             # consul npm ci + gen:proto + cargo build
just --list                # all recipes (vcan, sim-check, deploy-pi*, mcp-build, consul-lock, …)
```

`scripts/check.sh` runs, in order:
- Consul `npm ci`; `buf lint` (plus `buf breaking` on CI PRs)
- Consul `gen:proto`, proto checksum, `build:qualified`, `npm test -- --run`, dist leak check, `npm audit`
- MCP and limit-sync node tests; research MCP pytest (uv, `-m 'not integration'`)
- daily-audit and reference-journal `unittest`
- `.cursor/hooks` JS drift check; println guard
- `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`
- Shell contract tests; `scripts/check-dependencies.sh` (cargo deny + audit)
- aarch64 cross-build smoke (main-only in CI)

It does **not** run `validate-urdf.sh`, because `cargo test` covers URDF.

```bash
# Rust
cargo test -p davout --test physical_reference -- <name> --exact
cargo clippy --workspace --all-targets -- -D warnings
# On macOS, host clippy of marengo-pi/marengo-host-metrics hits Linux-only dead code; use:
cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings
cargo clippy -p marengo-pi --all-targets --target aarch64-unknown-linux-gnu -- -D warnings

# Proto: Rust regenerates in armee-proto/build.rs (needs protoc 28.3). TS:
cd consul && npm run gen:proto      # then commit consul/src/gen/.checksum (scripts/proto-checksum.sh)

# Consul
cd consul && npm test -- --run      # bare `npm test` is vitest watch mode
cd consul && npm run build

# MCP tools (after editing tools/*/src or .cursor/hooks/*.ts), then restart the MCP server
just mcp-build

# SocketCAN (Linux): virtual CAN, then serial ignored tests
just vcan && just check-vcan        # = cargo test --locked -p robstride --features socketcan -- --include-ignored --test-threads=1

# Sim
just sim-check                      # MuJoCo smoke + cargo test -p sim-harness
```

**Pi deploy:** prefer MCP `pi_sync_main`, which cross-builds, installs and polls `.deploy-rev` plus gateway `/health`. Manual paths: `just deploy-pi host=user@host` (macOS needs Bash ≥ 4) or `just deploy-pi-docker` (Windows). Cloud agents use `./scripts/pi-remote.sh deploy --install`.

---

## Code Conventions & Common Patterns

- **Errors:** every library uses `thiserror` enums with `?`, with no `unwrap`/`expect`/`panic` in `crates/`. Bins report with `eprintln!` + exit code (the gateway uses `Box<dyn Error>`). There is **no `anyhow`** in the workspace.
- **Lints** (`Cargo.toml` `[workspace.lints]`): `unsafe_code = forbid`; clippy `unwrap_used`, `expect_used`, `panic` = warn, so `-D warnings` blocks them. Tests opt out with a file-level `#![allow(clippy::expect_used)]` (or `mod tests { #![allow(...)] }`).
- **Fail closed:** invalid limits, NaN, model errors and unknown YAML keys (`#[serde(deny_unknown_fields)]` on all config structs) are refused with an error, never defaulted to 0 or a midpoint.
- **Async:** tokio runtimes exist only in `marengo-gateway` and `marengo-deploy`. `marengo-pi` and CAN are **synchronous** (non-blocking polls; Chappe receivers via `try_recv`). Do not block gateway handlers; use `spawn_blocking` for file/SQL I/O.
- **Realtime loop:** never call `PositionTrace::flush()` or do synchronous persists in the tick. Config persistence goes through the `ConfigPersistQueue` worker (memory is the source of truth plus write-behind, ADR 0012/0024). The tick still allocates today, but don't add more.
- **Tracing:** Chappe producers (`marengo-pi`, `marengo-gateway`) use `chappe::tracing_layer::init_subscriber`; CLIs use `marengo_support::init_tracing()`. No `println!` in `crates/`.
- **CAN frames:** build arbitration IDs only through `robstride::encode_*`. The encoder **refuses** out-of-range MIT values; ZeroSta/AddOffset writes and type-22 saves are refused by design.
- **Velocity caps:** resolve only via `marengo_config::resolve_joint_velocity_cap` (control.yaml joint, then actuator group, then motor-type default; ADR 0010). Bench YAML velocity fields are ignored.
- **Config resolution:** `MARENGO_CONFIG_DIR`, else `/opt/marengo/config` **if it exists**, else `<repo>/config`. A stray `/opt/marengo/config` on a dev host silently wins. `MARENGO_ROOT` defaults to the compile-time repo path.
- **Ephemeral limb narrowing:** `MARENGO_JOINT_SUBSET=a,b` (unknown names fail closed). Never fork master config per profile.
- **Config/URDF writes:** hold `ProfileWriteLock` and write through `write_atomic` / `write_profile_file_atomic` (unique temp + fsync + rename + dir fsync, `crates/marengo-config/src/atomic_file.rs`). Revisions are SHA-256 over the four master YAMLs (`config_revision.rs`). Clients send the revision they read, and an empty or stale revision is refused (CAS). The lock file `config/.marengo-profile.lock` is gitignored.
- **Naming:** Napoleonic codenames for the core (Berthier, Davout, Chappe, Consul) and `marengo-*` for infrastructure. Read each crate's `src/lib.rs` `//!` before editing. Keep bins thin and logic in `crates/`.
- **Tests:** large suites live in `#[cfg(test)] #[path = "x_tests/<topic>.rs"] mod …`. Shared fixtures are reused via `#[path]` (e.g. `davout/tests/support/mod.rs` `TestDirectory`). Never mutate the parent test process env; re-exec a child with env (see `bins/marengo-gateway/src/gateway_access_conformance_test.rs`).

### Safety hard rules (sources: `docs/safety.md`, ADRs)

- Every motor command goes through Davout. Enable requires a live **process-local physical reference grant** (ADR 0036).
- An elevated arm holds in **GravityComp** (`kp=0, kd=0, torque_ff=τ_g`). A HoldTracking/AscentStall trip at home means a model fault: fix the URDF, never raise kp/ki.
- **Transport latch:** any CAN error frame (including an mcp251x RX overflow) latches persistently. Relaxing it needs an ADR.
- **The bench mcp251x holds only 2 RX frames.**
  - Pace TX bursts with `BURST_GROUP_SPACING` = 2 ms (`davout/src/burst.rs`) and stagger Enables to one target per interface per period.
  - Fault, E-stop and shutdown stops are never paced.
- **Write a type-24 Off and read its echo** before any Enable. Only own-TX echoes (`CAN_RAW_RECV_OWN_MSGS`) order wire events.
- **Post-SetZero blackout** (SetZero to an Enabled drive; firmware 0.3.1.42 on ids 1-2, 0.2.3.34 on 3-4, 0.0.3.32 on 5): drives go silent for about 45–61 ms, starting 511–614 ms after a SetZero.
  - No Enable or type-24 write before `POST_SET_ZERO_QUIET` = 800 ms. Type-24 writes are held from 450 ms.
  - `cargo test -p davout --test firmware_profile` guards the margin against `docs/commissioning/firmware/robstride-timing-profile.json`.
- **RS03 MIT velocity scale is ±20 rad/s.** control.yaml values tuned under the old ±50 need bench re-checks. Never change physical tuning (gains, velocities, caps, limits) without bench evidence.
- **E-stop GPIO is not wired;** the physical E-stop is authoritative. `marengo-pi.service` runs `ExecStopPost=-/opt/marengo/bin/motor-repl disable` on every exit. Marengo never writes a drive-side CAN timeout, but PR #254 wrote CanTimeout = 600 (~30 ms) to all five right-arm drives, and `pi_protocol_inspect` read 600 back on 2026-10-03: a host stall over ~30 ms stops the drives drive-side (`docs/safety.md`).

---

## Important Files

| File | Role |
|---|---|
| `bins/marengo-pi/src/main.rs` | Pi runtime, stdin REPL, control loop; see also `motion_owner.rs`, `enable_gate.rs`, `reference_queue.rs`, `limit_persist.rs` |
| `crates/berthier/src/loop.rs` | `ControlLoop::tick`, enable completion, fuses wiring |
| `crates/davout/src/lib.rs` | `Supervisor`; `reference_transaction.rs`, `reference_physical.rs`, `burst.rs`, `active_reporting.rs` |
| `crates/robstride/src/{bus,mit,motor_type,params}.rs` | Wire layer and MIT ranges |
| `crates/marengo-config/src/lib.rs` | Typed config, velocity cap, `profile_txn.rs` writes |
| `bins/marengo-gateway/src/{main,http,state,restart}.rs` | Gateway, auth, management gates |
| `proto/marengo/v1/marengo.proto` | Wire schema |
| `config/{robot,motors,control,homing}.yaml` | Master 5-DOF config (can0 IDs 1–5: pitch, roll, upper-arm yaw, elbow, lower-arm yaw) |
| `scripts/check.sh`, `justfile`, `compose.yaml` | Gate, tasks, containers |
| `scripts/install-pi.sh`, `scripts/systemd/*.service` | Pi install and units |
| `tools/marengo-pi-mcp/src/tools/*.ts` | MCP bench tools (`launch.ts` holds defaults) |
| `docs/reviews/2026-10-03-crate-audit/` | Intent cards, leads, prune register, `phase-b/WP-*.md` open decisions |

---

## Runtime/Tooling Preferences

| Tool | Choice |
|---|---|
| Rust | **1.88.0** (`rust-toolchain.toml`), edition 2021, rustfmt 100 cols. `.tool-versions` mirrors `mise.toml`; `rust-toolchain.toml` pins the channel |
| Node | **24.16** (`.nvmrc`), **npm** with lockfiles, never bun. Regenerate the Consul lock with `just consul-lock` (Linux) |
| Proto | buf from `consul/node_modules` (`@bufbuild/buf`), protoc **28.3** |
| Python | uv (research MCP), `python3 -m unittest` for scripts, pytest for `test_analyze_position_trace.py` |
| Dependencies | cargo-deny 0.20.2, cargo-audit 0.22.2 via `scripts/check-dependencies.sh` |
| Cross target | `aarch64-unknown-linux-gnu` (`aarch64-linux-gnu-gcc`); features `socketcan`, `linux-i2c` are off by default |
| Hosts | ADR 0018: Windows `J:\code\marengo` (software + local CAD), macOS host checkout. No WSL checkout. Docker for the Linux gate |
| Windows shell | PowerShell; no `&&`/`||` (`.cursor/rules/windows-shell.mdc`). chappe IPC is Unix-only, so native Windows cannot build the full workspace |

**Pi layout:**
- `~/marengo` is the deploy staging tree. Always install from it: `sudo -n ~/marengo/scripts/install-pi.sh`.
- `/opt/marengo` is the sealed install (root-owned, not git). Only `config/`, `assets/` and `var/` are writable by `marengo`.
- `/etc/marengo/env` holds the environment; helpers live in `/usr/local/libexec/marengo/`.
- `joey` may run `sudo -n` only for `can-up.sh`, `pi-restart-marengo-pi.sh` and the two `install-pi.sh` paths. Never use bare sudo or `pkill`: `Restart=always` spawns a second CAN owner. Stop the unit with `pi-restart-marengo-pi.sh stop`.

**MCP-first (`.cursor/rules/pi-mcp-first.mdc`):** use `pi_*` tools for Pi actions; never ask the user to run deploys or paste logs.
- After `just mcp-build`, restart the server, because a stale server exposes stale tools.
- Motion tools need `confirm: true`. Weighted profiles, including the default `elbow_attached`, also need `confirm_weighted_motion: true`.
- Reference sessions (`pi_hold_on`, `pi_bench_harness`, `pi_gravity_calibrate`, `pi_joint_calibrate`, `pi_motion_suite`, `pi_enable_soak`) need `set_zero` + `at_mechanical_reference`.
- `pi_motion_suite` runs the single-joint motion suite (long/short moves, reversals, sweeps, gravity-extreme holds, repeats) over several ≤ 300 s sessions and scores every move with `scripts/analyze-position-trace.py --score-bench`.
- A gravity gate (residual < 0.20 Nm) runs before enable.
- `pi_sync_bench_config` syncs YAML only; use `pi_sync_bench_urdf` for the URDF (ADR 0017).
- Never put env in `.cursor/mcp.json`; defaults live in `tools/marengo-pi-mcp/src/launch.ts`.
- Cloud agents (`CURSOR_AGENT=1`) fall back to `scripts/pi-remote.sh` (`docs/cloud-pi-tailscale.md`).

---

## Testing & QA

| Area | Framework | Command |
|---|---|---|
| Rust | `cargo test` (+ one `proptest` in `berthier/src/mode_isolation.rs`) | `cargo test --workspace` / `-p <crate>` |
| Consul | vitest + jsdom + testing-library (`__tests__/` and colocated `*.test.ts`) | `cd consul && npm test -- --run` |
| Pi MCP, limit-sync-local | Node built-in `node --test` | `cd tools/marengo-pi-mcp && npm test` |
| Research MCP | pytest via uv | see `scripts/check.sh` |
| Scripts | `unittest`, `scripts/*.test.sh` | `python3 -m unittest discover -s scripts/daily-audit -p 'test_*.py'` |

**Rust fixtures:**
- `davout::simulation::SimulationBus` + `Supervisor::from_simulation(root, bus, InitialVirtualReference::…)`, mirrored by `ControlLoop::from_simulation*`. A virtual initial reference is a declared start condition, not an acquisition.
- `robstride::bus::MemoryBus` for unit-level frame tests.
- **`FirmwareBus` emulator** (`crates/davout/tests/physical_firmware/`): echoes, reply latency, post-SetZero blackout, a 2-buffer RX FIFO overrun. It is the authority for echo/enable sequencing; SimulationBus never echoes.
- Safety tests **copy** master config + URDF into a `TestDirectory` and pass explicit calibration-record/journal paths. Master `homing.yaml` points at `/opt/marengo/...`, so never let tests write into `config/` or `/opt`. A leaked `MARENGO_JOINT_SUBSET` / `MARENGO_CONFIG_DIR` changes `cargo test` results.

**Hardware-gated:** `#![cfg(all(feature = "socketcan", target_os = "linux"))]` plus `#[ignore]`, run via `just check-vcan`. Default `cargo test` needs no hardware. The `vcan` feature is just an alias for `socketcan`.

**Regression expectation:** each bug fix ships with a test that fails on the baseline (record red → green). Don't write tests that pin wording or wiring.

**Bench verification** complements tests and never replaces them:
- `pi_enable_soak`: 20 fresh-process no-motion cycles. PASS = all clean, no `rx_over_errors` growth, and `marengo-log-cli firmware-timing` `non_neutral_mit == 0`.
- After motion troubleshooting: `pi_candump_summary` + the position trace.
- Limb commissioning: `docs/commissioning/limb-playbook.md`.

**Coverage:** no CI threshold. The ad-hoc `cargo llvm-cov` baseline is in `docs/reviews/2026-10-03-crate-audit/metrics/`.

---

## Agent Workflow Notes

- **Issues:** GitHub Issues on `jaylamping/marengo` via `gh`. Labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix` (`docs/agents/`).
- **Commits:** conventional commits, with no AI attribution.
- **Search:** `Grep` with an empty pattern fails; list files with `Glob`.
- **Parallel agents:** git worktrees (e.g. `../marengo-wt/<name>`) must use absolute worktree paths. Tools default to the main checkout.
