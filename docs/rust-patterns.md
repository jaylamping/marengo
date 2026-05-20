# Rust patterns for Marengo

North-star guide for humans and agents. When the same mistake appears twice, add a **BAD / GOOD** pair here.

**Enforcement:** `just check`, workspace `[lints]`, and [AGENTS.md](../AGENTS.md).

## 1. How to use this doc

- Changing architecture → write an [ADR](decisions/) first.
- Repeated review feedback → add a snippet below in the same PR.
- Generic Rust style → [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).

## 2. Workspace map

| Crate / bin | Owns |
|-------------|------|
| `armee-proto` | Generated protobuf types |
| `armee-kinematics` | URDF / FK / limits |
| `chappe` | IPC bus |
| `berthier` | Control |
| `davout` | Safety |
| `talleyrand` | Planning |
| `fouche` | Vision / LLM (Jetson) |
| `robstride` | CAN / Robstride protocol |
| `sim-harness` | Sim test helpers |
| `bins/*` | Thin `main`, wiring only |

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

## 8. Testing

- Default `cargo test` must not require hardware.
- Use features: `vcan`, `sim`; mark hardware tests `#[ignore]` with a clear message.
- Sim: deterministic seeds for golden states.

## 9. Unsafe

- `#![forbid(unsafe_code)]` on all crates unless an ADR documents an exception.

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
