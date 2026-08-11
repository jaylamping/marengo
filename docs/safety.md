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

## Config hot-reload (limits)

- **Surface:** numerical hard/soft position bounds, torque cap, and `velocity_max_rad_s` only. `device_id`, CAN interface, `direction`, motor type, gearing, and membership/DOF changes require YAML + restart (+ re-home when wiring/zero changes).
- **Active refusal:** Davout refuses `apply_limit_patch` / `rebuild_limits` while operational mode is `ACTIVE`.
- **ACK honesty:** Gateway waits for live `limit_patch` Pending, then `limit_patch_persist` Durable/Failed, before HTTP success. Do not treat Pending alone as repo truth.
- **URDF expand-only:** Bench Set Limits widens in-memory + on-disk URDF hard when taught hard exceeds URDF (Davout hard = URDF ∩ bench). Soft uses ADR 0009 inset in `control.yaml` (not soft≡hard). See [ADR 0017](decisions/0017-bench-set-limits-urdf-expand.md).
- **Persist-degraded ≠ NeedsRestart:** write-behind failure uses a distinct banner/retry. Restart while YAML/URDF is stale would revert live limits.
- **Dual-writer:** Pi owns live apply and YAML/URDF write-behind. Gateway master-tree transactions use CAS `expected_revision` for durable YAML/URDF edits on the single master SoT; there are no inactive bringup profiles. Local git sync is Durable-gated via `marengo-limit-sync` only.
- **Deploy must not clobber Set Limits:** `install-pi.sh` preserves previous Pi motors hard + control soft + expand-only URDF union for overlapping joints after config rsync (Set Limits Apply remains SoT). Opt out with `MARENGO_REPLACE_LIMITS=1` when intentionally shipping git limit changes over taught envelopes.
- Soft bounds clamp into the new hard envelope in the same txn (Inventory Range must show post-clamp values).

See [ADR 0012](decisions/0012-config-db-overrides.md) and [ADR 0017](decisions/0017-bench-set-limits-urdf-expand.md).

## Hardware status poll (type-4 solicit)

While the **Hardware** page is open and operational mode is not `ACTIVE`, Consul POSTs `/command/motor_status_poll` about once every 2.5 s (gateway global rate limit ~0.5/s, burst 2). Gateway → Chappe `robot/motor_status_poll` → Davout `solicit_status_feedback`, which re-TX RobStride **Disable (type-4)** once per loaded motor that does **not** already desire Active Reporting (global `active_reporting_diagnostics` or an unexpired sheet/modal lease). Motors reply with **OperationStatus (type-2)**; the normal 200 Hz drain updates `RobotState`. No-op while `ACTIVE` (MIT status replies own that path). Best-effort per motor on TX failure. HTTP 200 is publish ACK only.

While not `ACTIVE`, Davout omits free-drive feedback older than ~5 s from `joint_feedback` / `RobotState` so Consul Online/Offline tracks recent RX rather than sticky cache membership after the first successful poll.

## Active Reporting leases (type-24)

Consul may hold **Active Reporting leases** (operator UI: Enhanced logging) via gateway → Chappe → Davout while a Hardware joint settings sheet (Set Limits) or Telemetry actuator modal is open — not for the whole Hardware page. Leases never enable type-24 while operational mode is `ACTIVE` (MIT status replies own that path). Bench profiles with `active_reporting_diagnostics: true` already force type-24 when not ACTIVE, so a lease may be a wire no-op until that global flag is off — the lease path still ships for global-off workflows. HTTP 200 is publish ACK only; Consul shows **Enhanced logging**, not confirmed wire reporting. TTL expiry on the Pi is the backstop if release is lost. `client_id` is not an auth boundary (same honesty as set-zero).

While free-drive sensing is desired (sheet/modal lease or global diagnostics flag), Davout **re-asserts** type-24 enable on a ~1 s heartbeat and when a joint’s feedback goes stale (~200 ms with no RX). Motors can drop Active Reporting mid-sweep; without retry, Consul freezes on the last sample and Set Limits Apply would teach a tiny band.

## Known software gaps (see also [position-hold-control-review.md](position-hold-control-review.md))

- **Hardware E-stop wiring:** `Supervisor::set_hardware_estop` exists but Pi GPIO/input is not yet connected at runtime. Treat physical E-stop as authoritative; do not assume software `Disabled` reflects the hardware line until wired.
- **Danger zones:** Rules evaluate **measured** joint `q`/`dq` (not commanded MIT fields). Prefer `clamp_torque` when Berthier sends `kd_mit = 0` and velocity clamps alone cannot slow gravity-driven descent.
- **Limit envelope:** Davout uses `max(|dq_cmd|, |dq_meas|)` for velocity-scaled margins so gravity-driven motion cannot shrink the envelope unexpectedly.
- **Fault latch policy:** Document whether specific faults require explicit operator reset before re-enable (TBD per fault class).

## When in doubt

Disable drives, E-stop, and fix the fault before resuming.
