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

## When in doubt

Disable drives, E-stop, and fix the fault before resuming.
