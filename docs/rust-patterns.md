# Rust patterns for Marengo

North-star guide for humans and agents. When the same mistake appears twice, add a **BAD / GOOD** pair here.

**Enforcement:** `just check`, `[lints] workspace = true` in each crate, and [AGENTS.md](../AGENTS.md).

**Binaries:** call `marengo_support::init_tracing()` once at startup (respects `RUST_LOG`).

## 1. How to use this doc

- Changing architecture → write an [ADR](decisions/) first.
- Repeated review feedback → add a snippet below in the same PR.
- Generic Rust style → [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).

## 2. Workspace map

Each library crate has a **detailed crate-root** `//!` doc in `src/lib.rs` (responsibilities, does-not, allowed dependencies). Read that before editing a crate.

| Crate / bin | Owns |
|-------------|------|
| `armee-proto` | Generated protobuf types |
| `armee-kinematics` | URDF parse, joint limits, actuated joint names |
| `armee-dynamics` | `gravity_torques(q)` only |
| `chappe` | IPC pub/sub (protobuf envelopes) |
| `berthier` | Outer loop, modes, friction FF → Davout |
| `davout` | Safety gateway, sole path to robstride |
| `talleyrand` | Planning |
| `fouche` | Vision / LLM (Jetson) |
| `robstride` | MIT CAN encode/decode, no policy |
| `marengo-imu` | BNO085 SHTP/I2C driver, rotation-vector samples |
| `marengo-config` | `config/*.yaml` loaders |
| `sim-harness` | Sim test helpers |
| `bins/*` | Thin `main`, wiring only |
| `bins/imu-probe` | BNO085 I2C quaternion probe (Pi bench) |

## 3. Crate boundaries

```rust
// BAD — Berthier opens CAN directly
socketcan::CanSocket::open("can0")?;

// GOOD — Berthier → Davout → robstride
davout::filter(cmd)?;
robstride::send(cmd)?;
```

## 4. Errors

```rust
// BAD (library)
let angle = state.angle.unwrap();

// GOOD (library)
let angle = state.angle.ok_or(Error::MissingJoint { name: name.to_string() })?;
```

- Libraries: `thiserror` enums, `Result` in public APIs.
- Bins: `anyhow::Result` in `main` is fine; print or log errors for operators.

## 5. Async / Tokio

- Use async at Chappe, network, and CAN boundaries.
- Do not block inside async without `spawn_blocking` or a dedicated thread.
- Document cancellation when spawning long-running tasks.

## 6. Wire types & Chappe

- Change [`proto/`](../proto/) first; regenerate Rust and TypeScript.
- Binary protobuf on the wire ([ADR 0001](decisions/0001-protobuf-wire-types.md)).
- Never hand-edit `consul/src/gen/`.

```rust
// BAD — duplicate IPC struct
#[derive(Serialize)]
struct JointStateJson { name: String, position: f64 }

// GOOD
use armee_proto::JointState;
let bytes = joint.encode_to_vec();
```

## 7. Safety & control

See [safety.md](safety.md). No motor enable without Davout and an explicit state machine.

```rust
// BAD — reconstructing Robstride arbitration IDs at call sites
let id = (18 << 24) | (0x00ff << 8) | device_id;

// GOOD — vendor frame helpers keep communication type and parameter layout together
let (id, data) = robstride::encode_set_run_mode(device_id, robstride::RunMode::Speed);
```

Keep coordinate ownership explicit:

```rust
// BAD — Berthier or robstride applies motor sign/gearing ad hoc
let motor_tau = tau_g / (motor.direction as f64 * motor.gear_ratio);
robstride::send_mit(&mut bus, &cmd)?;

// GOOD — Davout is the joint↔motor boundary
supervisor.send_mit_batch(joint_space_cmds)?;
```

- Berthier, armee-dynamics, Chappe, and Davout safety limits operate in URDF joint space.
- robstride operates in raw motor/CAN space only.
- Davout owns `config/motors.yaml` `direction` / `gear_ratio` transforms in both directions.

Position hold (`hold-at`) ramps the MIT setpoint toward the latched target each tick at `position_slew_rad_s` in `control.yaml` — do not jump `q_des` instantly on retarget.

## 8. Testing

- Default `cargo test` must not require hardware.
- Use features: `socketcan`, `sim`; `vcan` names belong only to virtual-CAN test harnesses. Mark hardware tests `#[ignore]` with a clear message.
- Sim: deterministic seeds for golden states.

## 9. Unsafe

- Workspace lint `unsafe_code = "forbid"` on all crates (see root `cargo.toml` `[workspace.lints]`) unless an ADR documents an exception.

## 10. Dependencies

- Prefer `[workspace.dependencies]` in the root `Cargo.toml`.
- Feature-gate `socketcan`, heavy sim deps.

## 11. Logging

```rust
// BAD (library)
println!("joint={angle}");

// GOOD
tracing::debug!(angle, "commanded joint");
```

## 12. Further reading

- [Clippy](https://rust-lang.github.io/rust-clippy/master/)
- [decisions/](decisions/)
- [onboarding.md](onboarding.md)
