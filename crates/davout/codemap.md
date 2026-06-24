# crates/davout/

## Responsibility
Safety gateway and operational state machine — the **only** crate permitted to send motion commands to `robstride`. Every MIT or legacy command from Berthier, Talleyrand, or REPL tools must pass through Davout's filter pipeline before reaching CAN hardware.

Enforces: joint position envelope (URDF hard/soft limits + velocity-scaled kinetic margin, ADR 0009), kp/kd caps per motor type, tau_ff rate limiting, tau_ff max clamp, wrong-sign watchdog, communication watchdog, feedback velocity limit tripping, danger zone rules from config, E-stop assertion.

## Design

### Operational state machine (`OperationalMode`)
```
Disabled ──[set_homing_complete]──► Ready ──[request_enable(true)]──► Active
   ▲                                                                  │
   └────────────────────[disable_all / E-stop]────────────────────────┘
```
- `Disabled`: no motion possible, firmware may be idle.
- `Ready`: all joints homing-verified, motors not yet enabled.
- `Active`: motors enabled, MIT commands flow.

### Core types
- `Supervisor<B: MotorBus>` — owns the state machine, motor config, limits, homing registry, feedback cache, and the `MotorBus` handle. Constructed from repo config files.
- `ControlMode` — re-exported to `berthier`: `Disabled`, `GravityComp`, `TorqueOnly`, `Impedance`, `Position`.
- `JointCommand` — legacy single-joint command (position + velocity + torque).
- `MitJointCommand` — filtered MIT command for one joint (kp, kd, position, velocity, tau_ff).
- `SpeedCommand` — firmware speed-mode command (bench diagnostics only).
- `DavoutError` — error enum: `NotActive`, `Estop`, `Limit`, `CommWatchdog`, `MotorFault`, `Homing`, etc.

### Filter pipeline (`filter_mit_command_at_tick`)
1. **Comm watchdog check**: fail if no feedback received within `comm_watchdog_ms`.
2. **E-stop check**: fail if hardware E-stop asserted.
3. **Operational mode gate**: fail if not Active.
4. **kp/kd cap**: return error if kp > kp_max or kd > kd_max per motor type defaults.
5. **Position envelope clamp**: `clamp_position_in_envelope` applies velocity-scaled kinetic margin around URDF limits.
6. **Hard limit check**: fail if clamped position exceeds hard bounds.
7. **Danger zone clamp**: apply config-defined danger zone rules (clamp or fault).
8. **Velocity cap**: fail if |velocity| > limit per JointLimitPolicy.
9. **tau_ff clamp**: clamp to `[-tau_ff_max, tau_ff_max]` from both limit policy and motor type defaults.
10. **tau_ff rate limit**: slew tau_ff change at `tau_ff_rate_limit_nm_per_s`.
11. **Wrong-sign watchdog**: detect commanded tau_ff sign opposing gravity hold direction.

### Joint↔motor transform
- `direction` and `gear_ratio` from `motors.yaml`: position_rad *= scale, kp /= scale^2, kd /= scale^2, tau_ff /= scale where scale = direction * gear_ratio.
- inverse transform applied on feedback: motor→joint state.

### Feedback processing
- `drain_feedback` — non-blocking poll (control loop path, budget = 0).
- `refresh_feedback` — blocking poll up to `feedback_poll_budget_us` (REPL / set-zero).
- Each received frame is decoded per motor type, checked for velocity limit exceedance (position-derived velocity corroboration, 3-trip fault), and position limit exceedance.

## Flow
```
Berthier MitJointCommand batch
        │
        ▼
  Supervisor::send_mit_batch
        │
        ├─ per joint: filter_mit_command_at_tick()
        │   ├─ comm watchdog
        │   ├─ estop/mode gate
        │   ├─ kp/kd cap
        │   ├─ position envelope clamp (armee-kinematics)
        │   ├─ danger zones (marengo-config)
        │   ├─ velocity cap
        │   ├─ tau_ff clip + rate limit
        │   └─ wrong-sign watchdog
        ├─ joint→motor transform (direction × gear_ratio)
        ▼
  robstride::mit_control_all_at
```

## Integration
- **Depends on**: `robstride` (MotorBus + CAN frames), `armee-kinematics` (limit envelope, URDF parsing), `marengo-config` (YAML configs), `marengo-homing` (homing registry), `chappe` (telemetry), `armee-proto` (wire types).
- **Called by**: `berthier` (ControlLoop::tick → send_mit_batch), REPL binaries (motor-repl, homing tool).
- **Does not**: compute tau_g, plan trajectories, encode CAN bytes, open SocketCAN.
- **Safety contract**: every external motion path enters through Supervisor; no public method sends raw robstride commands. Documented in `docs/safety.md`.
