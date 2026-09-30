# crates/davout/src/

## Responsibility
`Supervisor<B: MotorBus>` implementation — operational state machine, command filtering, joint↔motor coordinate conversion, and feedback polling.

## Design
- `Supervisor` holds the `MotorBus`, limit policies, homing/enable state, per-address original pose timestamps, invalid-feedback gates, and private persistent fault authority.
- `faults.rs` owns bounded fault records, separate complete/partial vendor domains and availability masks, first/latest receive envelope and incomplete-work evidence, observed warnings, stop attempts and the read-only `SafetySnapshot`. Records survive healthy poses, cache clearing, replay and disable. No qualified recovery/reset is available.
- `ControlMode` enum mirrors proto wire type (gravity, impedance, position, torque).
- `JointCommand` / `MitJointCommand` / `SpeedCommand` — command DTOs before/after filtering.
- `DavoutError` — typed invalid requests/runtime failures, persistent `FaultLatched`, and truthful aggregate `StopDelivery` errors.
- `validate_control_candidate` — read-only combined policy validation for control overlays before live/durable installation.

## Flow
Enable path: `request_enable` → complete bounded drain of old queued status → preflight/drive enable → complete final nonblocking drain → new receive session → `OperationalMode::Active`. Saturated preflush refuses activation; saturated/malformed final flush rolls back through all-address stop.
Shutdown: `disable_all` → all-address zero-speed / neutral-MIT / ordinary-disable attempts → `Disabled` + `StopReport`; failed writes remain uncertainty, not physical acknowledgement.
Per-tick: `admit_and_send_mit` → whole-batch checks/filters → checked joint→motor conversion → every-active-address pose watchdog → `bus.mit_control_all_at`

Only neutral MIT status solicitation is allowed before initial pose, bounded by the communication deadline. Invalid admission emits no frame and restores output history. Expired watchdog, device/feedback hazards and transport uncertainty latch and automatically stop; rejected operator requests stay nonlatching. `check_fault_authority` is the owner mode-entry gate, and `stop_generation` invalidates old controller intent.

Feedback: bounded `recv_feedback_report` → merge motor/Error/first backend failure by raw delivery ordinal → inspect every fault/mode/hard-position/malformed event → choose latest admissible pose per address → once-per-drain derivative policy → latch incomplete completion independently → first-hazard best-effort stop. A timestamp tie cannot hide or reorder a raw hazard; host dequeue times do not justify differentiating every queued pose. Malformed and Remote events never renew pose; partial payloads never become complete fault words, and Error frames remain transport evidence. Idle/Quiet is observed host quiescence, with 64 raw frames/256 attempts per poll; it is not an atomic physical snapshot or a Pi jitter qualification.

## Integration
- Re-exports `MotorBus`, `MemoryBus` from robstride for test injection
- Re-exports `JointHomingState` from marengo-homing
- Called exclusively by Berthier control loop and motor-repl commands
