# crates/davout/src/

## Responsibility
`Supervisor<B: MotorBus>` implementation — operational state machine, command filtering, joint↔motor coordinate conversion, and feedback polling.

## Design
- `Supervisor` holds the `MotorBus`, limit policies, homing/enable state, per-address original pose timestamps, invalid-feedback gates, and private persistent fault authority.
- `reference.rs` owns private current-reference permission bound to the installed owner/backend realm and relevant effective motor/homing/control frame/envelope policy. History and pose are inspection/evidence, never permits. Relevant mutation, rebuild/patch, fault and stop uncertainty revoke permanently; ordinary successful Disable preserves reference and changes motion generation.
- `simulation.rs` owns a closed concrete in-memory transport, finite source-indexed raw/timed/envelope/typed/error scripts, declarative TX rules and attempted-delivery traces. `Supervisor<SimulationBus>::from_simulation` declares INITIAL virtual reference coverage through the shared initializer and normal safety logic. Generic constructors remain Unsupported; no arbitrary transport delegation, realm extraction, replacement or state conversion exists.
- `faults.rs` owns bounded fault records, separate complete/partial vendor domains and availability masks, first/latest receive envelope and incomplete-work evidence, observed warnings, stop attempts and the read-only `SafetySnapshot`. Records survive healthy poses, cache clearing, replay and disable. No qualified recovery/reset is available.
- `ControlMode` enum mirrors proto wire type (gravity, impedance, position, torque).
- `JointCommand` / `MitJointCommand` / `SpeedCommand` — command DTOs before/after filtering.
- `DavoutError` — typed invalid requests/runtime failures, persistent `FaultLatched`, and truthful aggregate `StopDelivery` errors.
- `validate_control_candidate` — read-only combined policy validation for control overlays before live/durable installation.
- `from_repo` / `from_repo_with_calibration_record_path` share one initializer with the closed simulation constructors. Ordinary construction resolves runtime configuration; simulation takes the supplied root's `config/` directly (ADR0031). Composition selects the legacy environment or explicit supplied history path; historical rows remain inspectable but ordinary startup state is Unhomed. History read/parse failures precede diagnostic TX. Mutable registry/unchecked Ready/synthetic pose/generic mutable bus APIs are removed. Legacy SetZero/cached verification/calibration arming explicitly refuse before TX/persistence (ADRs 0022/0023).

## Flow
Enable path: normal/scoped/Active-shortcut private reference gate → complete bounded drain of old queued status → preflight/drive enable → complete final nonblocking drain and repeated authority gate → new receive session → `OperationalMode::Active`. Saturated preflush refuses activation; saturated/malformed final flush rolls back through all-address stop.
Shutdown: `disable_all` → all-address zero-speed / neutral-MIT / ordinary-disable attempts → `Disabled` + `StopReport`; failed writes remain uncertainty, not physical acknowledgement.
Per-tick: `admit_and_send_mit` → whole-batch checks/filters → checked joint→motor conversion → every-active-address pose watchdog → `bus.mit_control_all_at`

Only neutral MIT status solicitation is allowed before initial pose, bounded by the communication deadline. Invalid admission emits no frame and restores output history. Expired watchdog, device/feedback hazards and transport uncertainty latch and automatically stop; rejected operator requests stay nonlatching. `check_fault_authority` is the owner mode-entry gate, and `stop_generation` invalidates old controller intent.

Feedback: observe current reference binding → bounded `recv_feedback_report` → merge motor/Error/first backend failure by raw delivery ordinal → installed address/type/transform lookup → inspect every fault/mode/hard-position/malformed event → choose latest admissible pose per address → once-per-drain derivative policy → latch incomplete completion independently → first-hazard best-effort stop. Active reference mismatch still consumes all evidence and stops the original installed routes once; policy restoration cannot revive permission. A timestamp tie cannot hide or reorder a raw hazard; host dequeue times do not justify differentiating every queued pose. Malformed and Remote events never renew pose; partial payloads never become complete fault words, and Error frames remain transport evidence. Idle/Quiet is observed host quiescence, with 64 raw frames/256 attempts per poll; it is not an atomic physical snapshot or a Pi jitter qualification.

## Integration
- Re-exports `MotorBus`, `MemoryBus` from robstride for test injection
- Re-exports `JointHomingState` from marengo-homing
- Called exclusively by Berthier control loop and motor-repl commands
