# crates/ — Armée libraries

16 Rust crates, each with a `//!` crate-root doc declaring responsibilities, does-nots, and allowed deps. **Read `src/lib.rs` before editing any crate.**

## Codename map

| Crate | Codename | Owns |
|-------|----------|------|
| `armee-proto` | Armée | Generated protobuf types (from `proto/`) |
| `armee-kinematics` | Armée | URDF parse, joint limits, actuated joint names |
| `armee-dynamics` | Armée | `gravity_torques(q)` only — no policy |
| `chappe` | Chappe | IPC pub/sub (protobuf envelopes) |
| `berthier` | Berthier | Outer control loop, modes, friction FF → Davout |
| `davout` | Davout | Safety gateway; **sole path to robstride** |
| `talleyrand` | Talleyrand | Motion planning (IK + multi-joint timing) |
| `fouche` | Fouché | Vision / LLM (Jetson-side) |
| `robstride` | — | MIT CAN encode/decode, no policy |
| `marengo-config` | — | `config/*.yaml` loaders |
| `marengo-homing` | — | Homing state machine, zero registry |
| `marengo-imu` | — | BNO085 SHTP/I2C driver, rotation-vector samples |
| `marengo-support` | — | `init_tracing()`, repo-root resolution, shared utils |
| `marengo-host-metrics` | — | Host-level metrics (CPU, temp, etc.) |
| `marengo-store` | — | Bench session / log archive SQL store |
| `sim-harness` | — | Sim test helpers |

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Control loop tick | `berthier/src/loop.rs` (`ControlLoop`, `tick`) |
| Safety supervisor | `davout/src/lib.rs:166` (`Supervisor` struct) |
| `disable_all` / `request_enable` | `davout/src/lib.rs` |
| MIT CAN frame encode/decode | `robstride/src/` (`encode_*`, `decode_*` helpers) |
| Gravity compensation | `armee-dynamics/src/` (`gravity_torques`) |
| Velocity cap resolution | `marengo-config` (`resolve_joint_velocity_cap`) |
| Chappe wire publish | `chappe/src/` + `chappe::tracing_layer` |
| Homing state | `marengo-homing/src/` + `davout` `HomingRegistry` |

## Boundaries (CRITICAL)

- **Berthier → Davout → robstride.** No shortcuts. Berthier never opens CAN.
- **Joint vs motor space:** Berthier, armee-dynamics, Chappe, Davout limits = URDF joint space. robstride = raw motor/CAN space only.
- **Davout owns** `direction` / `gear_ratio` transforms in both directions.
- **Talleyrand** owns IK and multi-joint timing. Berthier does not.

## CONVENTIONS

- `thiserror` enums + `Result` in public APIs. No `unwrap`/`expect` (clippy `warn`).
- `[workspace.dependencies]` in root `Cargo.toml`; feature-gate `socketcan`, heavy sim deps.
- `unsafe_code = "forbid"` workspace-wide unless an ADR documents an exception.
- Feature gates: `socketcan`, `sim`, `linux-i2c`. Mark hardware tests `#[ignore]`.

## ANTI-PATTERNS

- Reconstructing Robstride arbitration IDs at call sites → use `robstride::encode_*`.
- Berthier applying motor sign/gearing ad hoc → Davout is the joint↔motor boundary.
- `println!` in library code → `tracing::debug!` / `tracing::info!`.
- Duplicate IPC structs instead of `armee_proto` types.
