# Rust patterns for Marengo

North-star guide for humans and agents. When the same mistake appears twice, add a **BAD / GOOD** pair here.

**Enforcement:** `just check`, `[lints] workspace = true` in each crate, and [AGENTS.md](../AGENTS.md).

**Binaries:** Chappe producers (`marengo-pi`, `marengo-gateway`) call `chappe::tracing_layer::init_subscriber` once at startup (publishes `LogEvent` on `logs/structured`). Scaffolds and CLI bins call `marengo_support::init_tracing()` (stdout/journal only). Both respect `RUST_LOG`. See [logging-taxonomy.md](logging-taxonomy.md).

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

```rust
// BAD — println! for operator-visible runtime logs in marengo-pi
println!("control tick failed: {e}");

// GOOD — tracing + ChappeLogLayer publishes LogEvent on logs/structured
chappe::tracing_layer::init_subscriber(Some(chappe_bus), "marengo-pi");
tracing::warn!(error = %e, "control tick failed");
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

Position hold (`hold-at`) is Berthier's **joint-space motion primitive executor** — one law for every retarget, whether from operator `hold-at`, future Talleyrand joint streams, or Cartesian primitives resolved upstream. Talleyrand owns IK and multi-joint timing; Berthier does not. The law lives in `berthier::position_hold::PositionHold` (lifecycle + `tick`); `ControlLoop` builds `HoldWorld` and sends the MIT batch through Davout.

**MIT feedforward** (`GravityComp` / `Impedance` / `TorqueOnly`) packs Active MIT outside Position: `berthier::mit_feedforward::MitFeedforward`. YAML `joints.*.gravity_comp` / `impedance` are the gain sources; Testing overrides are allowed only in Impedance/Position, cleared on GravityComp/TorqueOnly/Disabled enter, and ignored under those modes. `TorqueOnly` currently aliases GravityComp (`τ_ff = τ_g`).

Control law (ADR 0007 one-pass):

- **Planner:** always trapezoidal `q_ref(t)`, `dq_ref(t)` toward latched target; moves ≤ ~60 mrad cap `v_max` at `position_slew_rad_s`, larger moves use `position_trajectory_velocity_rad_s`.
- **MIT setpoint:** always lead-bounded — `q_des` starts as `q_traj.clamp(q − max_lead, q + max_lead)` each tick (safety bound, not a second controller). Never drop the lead bound when `|q_traj − q| > max_lead` to open-loop-follow `q_traj`/remaining error. While approaching, if measured `q` is only slightly ahead of `q_ref` (within `POSITION_RETURN_RESYNC_RAD`), never command `q_des` behind `q` — MIT stiffness would pull back and cause mid-travel stick-slip. Same mirror rule on descent lag. Overshoot past target clamps `q_des` toward `target`. When `q` outruns `q_ref` by more than `max_lead`, **do not** resync the planner forward — brake via clamp.
- **Return retarget:** downward moves from well above `target` seed planner `dq_ref` at slew rate so friction/damping FF exceeds gravity at high `q`. Single-joint configs use the same retarget path as multi-joint.
- **Breakaway onset:** full `fc` pulse + `max(max_lead, 0.15)` when retarget starts near home outbound (`|q| ≤ POSITION_RETURN_DESCENT_SEED_RAD`) or on return-to-home from high `q`; cut Coulomb assist when measured speed outruns `dq_ref` by more than the velocity deadband. **Outbound lead boost is onset-only** (300 ms) — do not sustain-boost through the low-angle knee (that blocked stuck-lead resync on `hold-at 0.15`). Return/descent sustained boost and home-final pull remain.
- **Return lag / ascent stall:** freeze planner on return-to-home descent (`target ≈ 0`) in the final band (`q ≤ POSITION_RETURN_FREEZE_Q_MAX_RAD`) while stuck lagging (`q − q_traj > POSITION_RETURN_RESYNC_RAD`, hysteresis on filtered `dq`); **also freeze on outbound ascent stall** when `q_traj` is ahead of `q` by more than `POSITION_RETURN_RESYNC_RAD`, arm is at rest, and not settled — **no ascent pull-harder**. Unfreeze when synced within `POSITION_RETURN_RESYNC_RAD` or motion resumes toward target. Ascent-stall hysteresis uses an **ascent-only latch** (not the combined planner-freeze OR with descent/lead-follow). Sustained true ascent-stall freeze (>2 s) faults the tick (disable path); **do not arm that fuse during lead-follow residual finish**, and **reset the accumulator on any `dq` progress toward target** (slow knee crawl must not false-disable). Stuck premature Hold (planner at target, arm short, `|dq| < deadband`) must **not** reopen Cruise at the latched target — prefer freeze/resync. When Hold is latched but measured `q` is still short by more than `POSITION_HOME_SETTLE_RAD` (residual inside the premature band), **lead-follow finish**: keep `q_traj` at the lead-capped pose (often `target` when remaining `< max_lead`), set cruise `dq_traj` to slew, and freeze the open-loop tick so trajectory-velocity friction stays engaged — do not dead-Hold P-creep, and do not restart a full trapezoid (that thrashes). Bidirectional: positive targets finish from below; **home (`target≈0`) finishes from above only** — past-home must not lead-follow back up (that oscillates with pull-through). **Stuck lead:** if `|q − q_traj| > max_lead` at rest, resync `q_traj` to `q` so the reference catches up (does not apply while moving — runaway still brakes via clamp).
- **Descent breakaway:** while stuck descending (`|dq_filt| < deadband`, not yet latched), command `q_des` below measured `q` for MIT pull. Latch clears pull only after a stuck episode then `dq_filt ≤ −deadband×1.25` toward home (cruise alone does not latch).
- **Stiffness:** `tau_p = Kp * (q_des − q)` always.
- **Damping FF:** filtered `dq` (EMA α=0.25); `tau_d = Kd * (dq_ref − dq_filt)` while moving; spike brake cap (−0.04 Nm max) when approaching and overspeed > 0.04 rad/s; else `-Kd * dq_filt` when settled and moving.
- **Friction FF:** two rules only — `traj_vel` (follow `dq_ref`) or `settle` (fade on `target − q`). Post-retarget onset (300 ms): full `fc` breakaway pulse while stuck; after onset, ramp `fc` with `|dq_ref|/deadband` until motion starts.
- **MIT wire:** `velocity_rad_s = dq_ref` when moving or during post-retarget onset while approaching; `0` when stuck at rest after onset; `kd_mit = 0`; damping through torque FF using Davout-sanitized velocity. Outbound moves > ~50 mrad use `max(max_lead, 0.15 rad)` lead cap for the onset window only.
- **Gravity FF:** `tau_g` at measured `q`.
- **Limit envelope (ADR 0009):** `effective_command_bounds(q, dq_cmd)` shrinks commandable interval toward hard limits by `min_rad + k_v|dq| + k_stop·v²/(2a)` on the approached side only; Berthier clamps targets/`q_traj`/`q_des`; Davout clamps MIT commands and faults measured `q` beyond hard + slack. Soft bounds from URDF `safety_controller`.
- **Velocity cap (ADR 0010):** `marengo-config::resolve_joint_velocity_cap` resolves joint → actuator group → motor type from `control.yaml` into `JointLimitPolicy.velocity`. Berthier clamps planner `v_max` to `Supervisor::joint_velocity_cap`; Davout MIT/speed-mode filters use the same cap. Bench YAML and URDF velocity fields do not override it.

Do not fold `max_lead` into the planner accumulator — that freezes the reference when the arm lags.

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

Chappe producers use `init_subscriber`; other bins use `init_tracing`. Structured fields reach Consul via `LogEvent.fields_json` ([ADR 0013](decisions/0013-structured-log-fields.md)).

## 11a. BNO085 I2C (SHTP)

```rust
// BAD — smbus register read or header + payload-only second read
smbus.read_i2c_block_data(addr, 0, 4);
read_header(); read(&mut buf[4..payload_len]);

// GOOD — plain I2C read; peek header, then read full packet_byte_count (Adafruit BNO08x)
read_header(); // data_length == 0 => no packet
read_packet(total_len, &mut buf[..total_len]);
```

`smbus2` / register reads → `EREMOTEIO` on Pi is normal. Use `scripts/pi-i2c-plain-read.py` or `scripts/pi-bno085-shtp-init.py` to verify.

## 12. Further reading

- [Clippy](https://rust-lang.github.io/rust-clippy/master/)
- [decisions/](decisions/)
- [onboarding.md](onboarding.md)
