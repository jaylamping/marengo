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

Diagnostic replies that share a status communication type must retain header
fault/mode evidence without renewing pose. Robstride firmware replies beginning
`00 C4 56` are version bytes; Davout consumes their headers through the same
ordered hazard path. Never interpret diagnostic bytes as MIT position or use
them as a boot epoch ([ADR 0036](decisions/0036-disabled-drive-protocol-inspection.md)).

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

**Feedback and fault authority** ([ADR 0020](decisions/0020-lossless-feedback-and-fault-authority.md)):

- Consume ordered `FeedbackReport` observations and its terminal error together. Latest-state maps are compatibility views; healthy status cannot erase fault evidence.
- Inspect every raw device/mode/position hazard before pose admission. Infer velocity once per address per drain; consecutive dequeue timestamps do not prove physical acquisition intervals.
- Davout's private fault authority survives healthy feedback, Disable and cache operations. Invalid operator input is a rejected request; an observed runtime hazard latches and attempts all-address stop.
- A stop attempt records every write failure. Software Disabled and accepted CAN writes do not certify a physical stop; ordinary stop payloads do not clear firmware faults.
- Berthier propagates both post-send receive failures, checks the persistent latch before new planner/torque intent, and discards previous intent when Davout's stop generation changes. Explicit recovery and Pi/protobuf publication remain separate migration work.

**Bounded receive work** ([ADR0021](decisions/0021-bounded-can-ingress.md)):

- Preserve actual receive class and length; a remote request or padded short frame is never a measured pose or complete type-21 report.
- Implement the required nonblocking receive primitive explicitly. Share one total frame/read-attempt budget across sources and rounds; truncating the result of an unbounded callback does not bound work.
- Treat observed idle/quiet separately from work/deadline exhaustion. Incomplete drains retain evidence and cannot satisfy either enable flush. Latest-state projections must expose incomplete work rather than silently accept a prefix.

**Graceful owner shutdown** ([ADR0024](decisions/0024-stop-before-persistence-shutdown.md)):

```rust
// BAD — storage can postpone the motor stop attempt
overlay.wait_persist_idle(timeout);
supervisor.disable_all()?;

// GOOD — mandatory acquisition cleanup precedes optional ordinary stop and storage
control.inhibit_motion_for_shutdown();
let reference_cleanup = control.supervisor_mut().cancel_reference_for_shutdown();
let ordinary_stop = if reference_cleanup.is_none() && disable_on_exit {
    Some(control.supervisor_mut().disable_all())
} else { None };
let deadline = Instant::now() + timeout;
overlay.close_persist_admission();
control.supervisor().close_reference_journal_admission();
let persist = overlay.close_persist_and_drain(deadline.saturating_duration_since(Instant::now()));
let journal = control.supervisor_mut().drain_reference_journal_until(deadline);
// Retain reference_cleanup, ordinary_stop and both writer inventories separately.
```

The runtime applies its existing `disable_on_exit` policy explicitly. A live
reference reservation always performs mandatory cleanup first, even when that
policy skips ordinary exit Disable. Reuse its actual report when ordinary stop
is requested rather than duplicating the burst ([ADR0026](decisions/0026-bounded-virtual-reference-acquisition.md)). Intent
inhibition clears retained controller commands; it does not confirm drive stop.
Queue drain follows that stop attempt and must not gate it. Skipped stop, failed
stop and unfinished persistence have distinct outcomes. No physical stop or
support acceptance follows from a software Disabled state or successful writes.
Owner shutdown closes write admission; it must not terminate accepted work.
Keep the writer alive through the actual local completion publication, then
report pending/in-flight work, failures and observed thread termination separately.
A timed-out drain does not cancel filesystem I/O or confirm client delivery.
Dispatch checks the owner flag before later commands and ticks; synchronous
handlers already admitted are not interrupted by that check.

**Finite physical bench profiles** (Hardware commissioning):

Finite physical bench profiles use closed real-bus owners rather than supplied
transports or imported reference rows ([ADR0038](decisions/0038-finite-neutral-physical-bench-owner.md),
[ADR0039](decisions/0039-finite-first-lower-yaw-motion.md)). Validate bounded tuning
values into immutable private-field types before acquisition; record the selected
value in audit and report. Davout checks the complete profile both before and
after ordinary filtering, so an envelope clamp cannot widen a finite test.
Keep neighboring-joint feedback on the canonical velocity policy; additional
profile guards apply to the exercised joint and retain every raw position hazard.

**Scoped commissioning Enable** (Hardware commissioning):

The following describes the existing scoped caller path, whose private grant and
owner cutover remain unfinished. New registries start `Unhomed` and history
loading cannot supply `Verified`; current cached Set Zero is not qualified
reference evidence.

- Resolve targets with `Supervisor::resolve_enable_targets` → `marengo_homing::select_enable_targets` (no scope file → full-master Robot Ready; persisted scope → Verified in-scope only). Never call `set_homing_complete` on Enable or motion re-arm — Verified is Set Zero only.
- Energize with `Supervisor::enable_targets`. While Active, a different joint set returns `ActiveSetChangeRefused` (Disable first). Partial enable failure still `disable_all`.
- Berthier MIT keepalive / GravityComp / Position and MissingFeedback checks must cover only `supervisor.active_joints()` — never all loaded `joint_names` after a scoped Enable.
- `RobotState` omits joints without CAN feedback so Consul Online ≠ mere protobuf membership.

**History and resource binding** ([ADR 0022](decisions/0022-calibration-history-and-current-reference.md)):

```rust
// BAD — turn a prior same-name calibration into this process's permission
joint_states.insert(saved.joint.clone(), JointHomingState::Verified);

// GOOD — retain history for inspection; current reference starts unknown
joint_states.insert(joint.clone(), JointHomingState::Unhomed);
```

- Bind resources explicitly in libraries; read environment overrides once at the composition boundary. Homing constructors take a deterministic path, and Supervisor provides explicit-path construction for callers/tests.
- Read the resource directly. Only `ErrorKind::NotFound` means absent history; propagate other I/O and parse errors. Do not use `is_file` plus `unwrap_or_default` to hide a damaged record.
- Stage historical row changes and write that candidate before publishing memory/local state. A write error must preserve prior rows and flags. Keep one private writer for staged updates and explicit persist; this ordering alone does not provide crash-safe replacement or fsync durability.
- A closed physical neutral bench owner opens real CAN itself, acquires fresh
  guarded home, durably syncs an exclusively created audit, and rechecks live
  home before selecting finite permission. Never import a receipt or invent a
  device epoch. Every stop revokes this one-enable permission, and its public
  output interface refuses nonneutral batches ([ADR0038](decisions/0038-finite-neutral-physical-bench-owner.md)).
- Use independent exclusively created test directories. Exercise environment precedence with child-only variables, avoiding shared process environment mutation and PID-shared filenames.

Position hold (`hold-at`) is Berthier's **joint-space motion primitive executor** — one law for every retarget, whether from operator `hold-at`, future Talleyrand joint streams, or Cartesian primitives resolved upstream. Talleyrand owns IK and multi-joint timing; Berthier does not. The law lives in `berthier::position_hold::PositionHold` (lifecycle + `tick`); `ControlLoop` builds `HoldWorld` and sends the MIT batch through Davout.

**MIT feedforward** (`GravityComp` / `Impedance` / `TorqueOnly`) packs Active MIT outside Position: `berthier::mit_feedforward::MitFeedforward`. YAML `joints.*.gravity_comp` / `impedance` are the gain sources; Testing overrides are allowed only in Impedance/Position, cleared on GravityComp/TorqueOnly/Disabled enter, and ignored under those modes. `TorqueOnly` packs `τ_ff = τ_cmd` (latch in `TorqueCmdLatch` via `ControlLoop::set_torque_cmd`, which enters TorqueOnly; default 0; cleared on leave) with hard-zero kp/kd — not `τ_g`. Operator `gravity-off` calls `enter_torque_only_zero()` so `τ_cmd ≡ 0` even when already in TorqueOnly.

Control law (ADR 0007 one-pass):

- **Planner:** always trapezoidal `q_ref(t)`, `dq_ref(t)` toward latched target; moves ≤ ~60 mrad cap `v_max` at `position_slew_rad_s`, larger moves use `position_trajectory_velocity_rad_s`.
- **MIT setpoint:** always lead-bounded — `q_des` starts as `q_traj.clamp(q − max_lead, q + max_lead)` each tick (safety bound, not a second controller). Never drop the lead bound when `|q_traj − q| > max_lead` to open-loop-follow `q_traj`/remaining error. While approaching, if measured `q` is only slightly ahead of `q_ref` (within `POSITION_RETURN_RESYNC_RAD`), never command `q_des` behind `q` — MIT stiffness would pull back and cause mid-travel stick-slip. Same mirror rule on descent lag. Overshoot past target clamps `q_des` toward `target`. When `q` outruns `q_ref` by more than `max_lead`, **do not** resync the planner forward — brake via clamp.
- **Return retarget:** downward moves from well above `target` seed planner `dq_ref` at slew rate so friction/damping FF exceeds gravity at high `q`. Single-joint configs use the same retarget path as multi-joint.
- **Breakaway onset:** full `fc` pulse + `max(max_lead, 0.15)` when retarget starts near home outbound (`|q| ≤ POSITION_RETURN_DESCENT_SEED_RAD`) or on return-to-home from high `q`; cut Coulomb assist when measured speed outruns `dq_ref` by more than the velocity deadband. **Outbound lead boost is onset-only** (300 ms) — do not sustain-boost through the low-angle knee (that blocked stuck-lead resync on `hold-at 0.15`). Return/descent sustained boost and home-final pull remain.
- **Return lag / planner recovery:** freeze the planner on return-to-home descent in the final band while stuck lagging. Keep ascent recovery, lead bounds, resync, pull-through and lead-follow finish independent of the watchdog; planner resync must never renew its budget. Stuck premature Hold must use the existing bounded finish instead of reopening a full trapezoid. Home finishes from above only. A moving joint beyond the lead bound still brakes through the clamp.
- **Measured ascent watchdog** ([ADR 0025](decisions/0025-measured-ascent-progress.md)): watch only actually commanded joints with a non-home target ahead in the positive joint direction beyond the existing settle band. Retain a credited measured-position high-water mark; only a qualifying new encoder level resets the existing 2-second budget. Velocity signs, retreat, revisiting old peaks and planner recovery/resync do not. Accumulate subthreshold motion against the credited level so valid one-count crawl survives. Settle, a changed target, leaving Position, stop/fault cleanup and the existing Wave exemption end the episode; an identical-target retarget preserves it. Davout supplies the validated decoded joint-grid threshold from frozen installed conversion. Intent cleanup preserves that profile. Standalone continuous-law inputs use only their f64 roundoff floor; never apply that floor to an installed decoded grid. The threshold qualifies software conversion, not physical sensor noise.
- **Nominal control time:** validate and accumulate the same `Duration` used by the planner. Explicit zero-dt composition remains valid; reject negative, nonfinite, overflowing or positive durations that round to zero before mutating law state. Reject a zero-rounded loop period before constructing Supervisor. Avoid integer `1000 / hz` watchdog accounting. Nominal time does not establish physical scheduling or acquisition timing.
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

**SQLite migration ownership** ([ADR0029](decisions/0029-store-migration-ownership.md)):
read the version after acquiring the Immediate write transaction, then commit one
schema transition and its matching marker together. Read back the stored marker
and require the expected next version before commit; a successful SQL write can
be rewritten by a database trigger. Hold the Store connection
guard across the owner call; private transaction helpers must not call public
Store helpers that acquire the same mutex. Preserve current marker and supplied
setting timestamps. Refuse unknown/historically partial state instead of hiding
a failed non-idempotent DDL operation.

**Deliberate historical recovery** ([ADR0030](decisions/0030-deliberate-known-store-recovery.md)):
keep normal open's historical refusal. A separate explicit library operation
recognizes one complete schema and pins a read-only SQLite snapshot. Complete,
verify and publish a WAL-aware standalone backup before repairing a separate
output through the normal migration owner. Require observed backup completion,
finite work, exact typed-row preservation, FTS external-content integrity and
close/reopen verification. Use owned staging identities and fresh no-overwrite
destinations; report cleanup failures and retain completed artifacts on later
errors. File sync is not proof of physical power-loss durability. Test new
recovery as candidate conformance with real failure paths and a meaningful
production mutation; a missing new API is never an original behavioral red.

**Untrusted capture conversion and reading:** use `Duration::try_from_secs_f64`
for parsed/decoded offsets and propagate domain errors rather than unwinding.
Bound a buffered reader before `read_until` allocates an entire physical line;
count decompressed bytes independently of compressed source size. Check floating
integer boundaries exclusively when the maximum integer rounds up in f64.

- Default `cargo test` must not require hardware.
- Use features: `socketcan`, `sim`; `vcan` names belong only to virtual-CAN test harnesses. Mark hardware tests `#[ignore]` with a clear message.
- Sim: deterministic seeds for golden states.

## 9. Unsafe

### Reference authority in safety tests

Historical calibration and scalar verification are inspectable evidence, not
motor permission. Davout owns private current-reference authority at Ready,
every Enable path, receive conversion, public facets and motion output. Ordinary
constructors cannot acquire it from a mutable registry, cached pose or recording
bus. Unqualified reference requests return a typed refusal before arming or
history writes; Disable remains independent of reference permission.

Positive controller/safety tests use the concrete closed
`davout::simulation::SimulationBus` and specialized `from_simulation` factory.
`InitialVirtualReference` declares starting conditions only. Finite raw frames
and transmit rules exercise the shared production receive/admission/stop/output
implementation. The mutable simulation facade exposes data scripts and trace,
with no transport replacement/extraction or arbitrary callback. Observe actual
script trigger counts, literal wire output, typed errors and preserved stop/fault
evidence. Constructor grants do not qualify a reference transaction or hardware.

The specialized virtual owner separately supports bounded acquisition without
a grant. Begin reserves without TX; advance performs one phase and one shared
64-frame/256-read allowance at most. A private SetZero effect binds the actual
decoded reply pop to owner/realm/transaction/device epoch. Cache, timestamps and
typed queue injection cannot qualify it. Inspect the whole ordered hazard stream
before staging evidence, and retain same-call all-address cleanup and reporting
Off results. Deadline equality, cancellation and uncertain delivery remain final.
EvidenceStaged with CommitUnavailable cannot authorize Ready or output; selecting
current permission requires the separate durable consumer below. Installed clients
remain separate work. New APIs use independent
candidate conformance and selected mutants, not missing-method baseline reds.

Retain actual accepted pose/private correlation through cleanup; reconstructing
it from cached pose or a terminal loses the evidence boundary. A bounded private
immutable model snapshot plus checked installation identity binds every success,
including an equal model restore. A live stage diagnostic validates the exact
retained handle against current continuity; restoring an observed policy edit
does not revive it. Keep that projection separate from immutable acquisition
terminals and permission. Serialization size checking before a retained clone
bounds model memory; it is not a durable encoding or physics oracle (ADR0027).

Only explicit unreferenced virtual factories opt into the concrete journal
([ADR0034](decisions/0034-durable-virtual-reference-history.md)). Capture typed policy
at acquisition and retain its exact bits with the immutable model. Encode on the
worker through bounded primitive tags and exhaustive URDF fields; JSON Value or
reloaded files cannot preserve the captured descriptor. Reserve completion credit
before accepting one job per acquisition, including cancelled and unconsumed jobs.
Consume a private matching real completion after one fresh bounded disabled report,
whole-report hazards, original deadline and sticky continuity checks. Keep lifecycle,
current eligibility and actual disk result distinct. Durable SQL readback remains
history, including after cancellation, shutdown or process recovery. Generic/default
constructors perform no journal I/O. Close every writer before sharing one absolute
shutdown deadline; a timed-out or unwound worker must retain all accepted outcomes.

The separately named current-consuming virtual factories preserve every history-only
constructor contract ([ADR0035](decisions/0035-consume-current-virtual-reference.md)).
Only the real owner consumer can select the acquired joint after actual durable
completion and fresh full continuity checks. Keep the grant's private model/device/
job binding independent of stage deadlines and diagnostic cache retention. Successful
ordinary Disable preserves intact reference; new acquisition, observed relevant policy
or model change, reset, fault, uncertain stop and shutdown revoke it. Snapshot booleans
and recovered rows only describe history/current observations; they never install
permission. Generic/physical constructors retain no qualified acquisition capability.

```rust
// BAD — an already-revoked reference generation misses another model install
self.urdf_robot = candidate;
self.reference_authority.revoke();

// GOOD — stage checked model identity before installing any state
let installed_model = self.installed_model.replacement(&self.robot, &candidate)?;
self.reference_authority.revoke();
self.installed_model = installed_model;
self.urdf_robot = candidate;
```

Keep public red-to-green probes byte-identical in separately bound archived
snapshots. API-removal compile denials are isolation conformance, not behavioral
baseline failures. Provide stationary or coherent measured inputs independent
of planner output; a planner echo cannot prove controller tracking. Use fixed-dt
law inputs for progress semantics, and raw controller/wire cases for fault and
stop propagation. [ADR0023](decisions/0023-private-current-reference-authority.md)
records the boundary and its remaining transaction work.

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

## Bounded IPC publication and frame writes

Keep control-thread transport admission nonblocking: use finite topic classes and
item/byte ceilings, try-lock admission, explicit overflow outcomes and counters.
Latest telemetry replaces its own prior slot; ordered audit/log traffic has an
independent finite FIFO. Commands never enter an outage publication backlog.
Bound frame lengths before reader allocation. Bound the entire socket frame write
with a monotonic deadline and update each syscall timeout from the remaining time;
a fresh timeout per partial write does not bound the complete operation. On failure,
shutdown the connection and join its cloned reader before reconnecting. Queue
admission, socket delivery and physical motor stop are distinct evidence.

## 12. Further reading

- [Clippy](https://rust-lang.github.io/rust-clippy/master/)
- [decisions/](decisions/)
- [onboarding.md](onboarding.md)

## Diagnostic observation validity

Protobuf scalar defaults do not prove that collection succeeded. Keep validity
explicit, preserve unknown results on missing/malformed data, and have consumers
check validity before showing percentages, capacities or writable/healthy states.
CPU and disk host metrics follow this pattern.

```rust
// BAD — missing input is reported as a known zero measurement
metric.capacity_known = true;
metric.total_bytes = observed_total.unwrap_or(0);

// GOOD — unavailable input leaves the observation unknown
if let Some(total) = observed_total {
    metric.total_bytes = total;
    metric.capacity_known = true;
}
```

## Shared gateway request access

Resolve trusted credentials/capabilities and browser origins once at binary
composition. Apply the same immutable policy to HTTP/HTTPS before extraction
and to bounded WebTransport subscription admission before creating a receiver.
Do not read an independent credential environment variable inside each handler.
Use typed capabilities, constant-time credential comparisons and generic errors
that do not include credentials. Correct access admission does not replace
attestation, scope, rate, runtime authority or Davout gates (ADR0033).
