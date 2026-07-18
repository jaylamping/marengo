# crates/davout/src/

## Responsibility
`Supervisor<B: MotorBus>` implementation — operational state machine, command filtering, joint↔motor coordinate conversion, and feedback polling.

## Design
- `Supervisor` struct holds: `MotorBus`, joint name index, limit policies, homing state, enable policy, comm watchdog timers.
- `ControlMode` enum mirrors proto wire type (gravity, impedance, position, torque).
- `JointCommand` / `MitJointCommand` / `SpeedCommand` — command DTOs before/after filtering.
- `DavoutError` — typed faults (comm watchdog, limit violation, wrong-sign, homing).

## Flow
Enable path: `request_enable` → preflight checks → `OperationalMode::Active`
Shutdown: `disable_all` → zero-torque MIT frames → `Disabled`
Per-tick: `filter_mit_command` → `apply_joint_to_motor` → `bus.mit_control_all_at`

## Integration
- Re-exports `MotorBus`, `MemoryBus` from robstride for test injection
- Re-exports `JointHomingState` from marengo-homing
- Called exclusively by Berthier control loop and motor-repl commands
