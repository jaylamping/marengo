# crates/davout/src/

## Responsibility
`Supervisor<B: MotorBus>` implementation — operational state machine, command filtering, joint↔motor coordinate conversion, and feedback polling.

## Design
- `Supervisor` struct holds: `MotorBus`, limit policies, homing state, enable policy, per-address original pose timestamps, and invalid-feedback gates.
- `ControlMode` enum mirrors proto wire type (gravity, impedance, position, torque).
- `JointCommand` / `MitJointCommand` / `SpeedCommand` — command DTOs before/after filtering.
- `DavoutError` — typed faults (invalid command/feedback, addressed comm watchdog, limit violation, wrong-sign, homing).
- `validate_control_candidate` — read-only combined policy validation for control overlays before live/durable installation.

## Flow
Enable path: `request_enable` → drain old queued status → preflight/drive enable → final nonblocking drain → new receive session → `OperationalMode::Active`
Shutdown: `disable_all` → zero-torque MIT frames → `Disabled`
Per-tick: `admit_and_send_mit` → whole-batch checks/filters → checked joint→motor conversion → every-active-address pose watchdog → `bus.mit_control_all_at`

Only neutral MIT status solicitation is allowed before initial pose, bounded by the communication deadline. Invalid admission emits no frame and restores output history; a transport failure can occur after partial delivery.

## Integration
- Re-exports `MotorBus`, `MemoryBus` from robstride for test injection
- Re-exports `JointHomingState` from marengo-homing
- Called exclusively by Berthier control loop and motor-repl commands
