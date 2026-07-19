# Safety

Read this before enabling motors on the bench or robot.

## Principles

- **No silent enable.** Actuators must not energize without an explicit, logged enable sequence.
- **E-stop first.** Hardware E-stop cuts power independently of software. Software must treat E-stop as latched until a documented reset procedure.
- **Homing before motion.** After power-on or fault, run a homing sequence that verifies encoders, limits, and I/O before tracking commands ([Robot Programming Best Practices](https://intelligentintegrators.org/robot-programming-best-practices/)).
- **Davout in the path.** No joint command reaches [robstride](../crates/robstride/) without passing [Davout](../crates/davout/) limit and rate checks.

## Bench mode (development)

- **Command velocity caps:** set in `config/control.yaml` only — per-joint override, `actuator_groups`, or `motor_type_defaults` (see [ADR 0010](decisions/0010-actuator-velocity-cap-resolution.md)). Davout and Berthier enforce the resolved cap at runtime; do not rely on `motors.yaml` `bench.velocity_limit_rad_s`, `robot.yaml` `bench.max_joint_velocity_rad_s`, or URDF joint velocity for command limiting.
- **Torque caps:** keep below production limits via `config/robot.yaml` (`robot.bench`) and per-joint overrides in `config/motors.yaml`; Davout also applies per-`motor_type` `tau_ff` limits and rate limiting from `control.yaml`.
- Use `motor-repl` only with operators at the robot and clear workspace.
- Prefer the virtual CAN test harness (`just vcan`) or simulation (`just sim-check`) before live CAN when developing control logic.

## Homing and zero (see [homing.md](homing.md))

- **Homing before motion.** After power-on or fault, every configured joint must reach **Verified** before supervisor `Ready`.
- **Sensor health first.** When Hall/limit inputs are configured, startup checks sensor wiring and stuck-state before homing search.
- **No blind hunting.** Out-of-range or stale-zero joints may only use constrained recovery (manual reference or sensor homing), not normal gravity/hold/impedance.
- **Calibration audit.** Host registry at `var/calibration/zero_registry.yaml` records who/when/how zero was established; firmware `SetZero` alone is insufficient.

Interim bench (no Hall hardware yet): manual reference + `set-zero` + verification. Target: three Hall sensors per joint (home, min, max).

## Enable / disable sequence (target behavior)

1. Verify E-stop released (hardware).
2. Precheck CAN, faults, sensor health (if configured).
3. Homing → all joints **Verified** → software `Disabled` → `Ready`.
4. Operator command `Enable` → `Active` (motors may track).
5. Fault, homing fault, or E-stop → `Disabled` (ramp down, then disable drives).

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
7. **Measured position guard (ADR 0009):** feedback `q` beyond URDF hard limits + small slack → `Disabled` (independent of command path).
8. **Exit:** `disable_all` on process exit / SIGTERM where the driver supports it.

See [ADR 0004](decisions/0004-control-modes-and-mit.md) and [hardware/docs/decisions/0002-robstride-protocol.md](../hardware/docs/decisions/0002-robstride-protocol.md).

## Bench procedure (gravity compensation)

1. Verify E-stop and clear workspace.
2. `motor-repl home` → `enable` only with arm supported.
3. `gravity-on` — verify backdrivability and no runaway.
4. **Upright pose test:** slowly release support; elbow/upper arm must not free-fall.
5. `gravity-off` / `disable` before leaving the bench.

## Known software gaps (see also [position-hold-control-review.md](position-hold-control-review.md))

- **Hardware E-stop wiring:** `Supervisor::set_hardware_estop` exists but Pi GPIO/input is not yet connected at runtime. Treat physical E-stop as authoritative; do not assume software `Disabled` reflects the hardware line until wired.
- **Danger zones:** Rules evaluate **measured** joint `q`/`dq` (not commanded MIT fields). Prefer `clamp_torque` when Berthier sends `kd_mit = 0` and velocity clamps alone cannot slow gravity-driven descent.
- **Limit envelope:** Davout uses `max(|dq_cmd|, |dq_meas|)` for velocity-scaled margins so gravity-driven motion cannot shrink the envelope unexpectedly.
- **Fault latch policy:** Document whether specific faults require explicit operator reset before re-enable (TBD per fault class).

## When in doubt

Disable drives, E-stop, and fix the fault before resuming.
