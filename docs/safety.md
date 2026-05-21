# Safety

Read this before enabling motors on the bench or robot.

## Principles

- **No silent enable.** Actuators must not energize without an explicit, logged enable sequence.
- **E-stop first.** Hardware E-stop cuts power independently of software. Software must treat E-stop as latched until a documented reset procedure.
- **Homing before motion.** After power-on or fault, run a homing sequence that verifies encoders, limits, and I/O before tracking commands ([Robot Programming Best Practices](https://intelligentintegrators.org/robot-programming-best-practices/)).
- **Davout in the path.** No joint command reaches [robstride](../crates/robstride/) without passing [Davout](../crates/davout/) limit and rate checks.

## Bench mode (development)

- Keep joint velocity and torque caps below production limits in `config/robot.yaml` (`robot.bench`) and per-joint overrides in `config/motors.yaml`.
- Use `motor-repl` only with operators at the robot and clear workspace.
- Prefer **vcan** or simulation (`just sim-check`) before live CAN when developing control logic.

## Enable / disable sequence (target behavior)

1. Verify E-stop released (hardware).
2. Software `Disabled` → `Ready` (homing complete, no faults).
3. Operator command `Enable` → `Active` (motors may track).
4. Fault or E-stop → `Disabled` (ramp down, then disable drives).

Document actual pin/signal mapping in [hardware/electrical/wiring/](../hardware/electrical/wiring/) as it is finalized.

## Upright-pose incident (4-DOF arm)

During early arm bring-up with the arm elevated (shoulder/elbow up), motion stopped while the arm was unsupported. Without gravity feedforward, the arm fell rapidly into the operator workspace.

**Required mitigations before repeat tests:**

1. **Control mode:** Use **GravityComp** (`kp=0`, `kd=0`, `torque_ff=tau_g`) — not position-only holding in elevated configurations.
2. **Enable sequence:** Arm supported manually or in a fixture for first enable; E-stop reachable before `Enable`.
3. **Sign test:** Per-joint small `torque_ff` pulse; verify direction matches URDF before full `tau_g`.
4. **Caps:** Davout per-`motor_type` `tau_ff` limits (RS02/RS00 lower than RS03/RS04); rate-limit `tau_ff` steps when enabling.
5. **Danger zones:** `config/control.yaml` rules (e.g. elevated shoulder pitch + downward velocity) → clamp or fault.
6. **Comm watchdog:** CAN receive timeout → `Disabled` and logged fault.
7. **Exit:** `disable_all` on process exit / SIGTERM where the driver supports it.

See [ADR 0004](decisions/0004-control-modes-and-mit.md) and [hardware/docs/decisions/0002-robstride-protocol.md](../hardware/docs/decisions/0002-robstride-protocol.md).

## Bench procedure (gravity compensation)

1. Verify E-stop and clear workspace.
2. `motor-repl home` → `enable` only with arm supported.
3. `gravity-on` — verify backdrivability and no runaway.
4. **Upright pose test:** slowly release support; elbow/upper arm must not free-fall.
5. `gravity-off` / `disable` before leaving the bench.

## When in doubt

Disable drives, E-stop, and fix the fault before resuming.
