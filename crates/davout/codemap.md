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
- `Active`: motors enabled; servo/FF motion requires current-session pose from every active motor address.

### Core types
- `Supervisor<B: MotorBus>` — owns the state machine, motor config, limits, homing registry, feedback cache, and the `MotorBus` handle. Constructed from repo config files.
- `ControlMode` — re-exported to `berthier`: `Disabled`, `GravityComp`, `TorqueOnly`, `Impedance`, `Position`.
- `JointCommand` — legacy single-joint command (position + velocity + torque).
- `MitJointCommand` — filtered MIT command for one joint (kp, kd, position, velocity, tau_ff).
- `SpeedCommand` — firmware speed-mode command (bench diagnostics only).
- `DavoutError` — typed errors including command/feedback numeric invalidity, `NotActive`, `Estop`, `Limit`, addressed `CommWatchdog`, `MotorFault`, and `Homing`.

### Command admission (`admit_and_send_mit`)
Single-joint, legacy, and batch MIT sends share one admission path. Validate the whole batch before emitting its first frame:

1. Check E-stop, Active membership, configured motor mapping, and repeated joint names.
2. Reject nonfinite position, velocity, gains, and FF; gains must be nonnegative.
3. Filter each joint: kp/kd ceilings, velocity-scaled position envelope, hard bounds, danger-zone clamps, velocity ceiling, and wrong-sign policy.
4. Apply the hard FF ceiling before and after slew. First enable starts from zero, and a delayed tick earns at most 10 ms of slew credit.
5. Transform joint→motor values and reject nonfinite/overflowing `f32` wire fields before sending.
6. Require a valid pose newer than enable and within `comm_watchdog_ms` for every active address. Until the enable deadline, only zero-gain, zero-velocity, zero-FF MIT frames may solicit initial status.
7. Send the prepared addressed batch through Robstride's checked API. Admission failure restores output-history scratch state. Transport errors may still occur after a valid prefix has reached the physical bus; callers own the stop path.

Startup validates the combined robot/motor/control/homing policy. `validate_control_candidate` checks proposed control overlays against the installed companion configuration before installation or persistence; it does not install policy or rebuild limits.

### Joint↔motor transform
- `direction` and `gear_ratio` from `motors.yaml`: position_rad *= scale, kp /= scale^2, kd /= scale^2, tau_ff /= scale where scale = direction * gear_ratio.
- inverse transform applied on feedback: motor→joint state. Direction must be ±1 and gear ratio finite and positive.

### Feedback processing
- `drain_feedback` — non-blocking poll (control loop path, budget = 0).
- `refresh_feedback` — blocking poll up to `feedback_poll_budget_us` (REPL / set-zero).
- Configured status samples are checked for finite raw and transformed fields, original receive time, position-derived velocity, and position limits before cache/freshness publication.
- Empty drains, unknown addresses, fresh peers, and fault-only reports cannot refresh another motor's pose. Fault-only reports retain fault evidence without creating a zero pose. Older/replayed samples cannot replace a newer pose or mutate derivative policy state.
- Invalid feedback blocks subsequent motion despite an older valid cache; only a strictly newer valid pose restores admission. Active getters omit absent, invalid, stale, and prior-enable pose.
- Drain queued status before enable writes and again after enable/run-mode writes, before creating the session marker. Status queued during those writes cannot authorize the new session. CAN status has no command-generation identifier; traffic arriving after the final drain remains uncorrelated, and session freshness cannot identify every delayed physical packet.

## Flow
```
Berthier MitJointCommand batch
        │
        ▼
  Supervisor::send_mit_batch
        │
        ├─ whole batch: numeric/identity/estop/mode admission
        ├─ per joint: filter_mit_command_at_tick()
        │   ├─ kp/kd cap
        │   ├─ position envelope clamp (armee-kinematics)
        │   ├─ danger zones (marengo-config)
        │   ├─ velocity cap
        │   ├─ tau_ff clip + rate limit
        │   └─ wrong-sign watchdog
        ├─ checked joint→motor transform (direction × gear_ratio)
        ├─ every active address: current-session pose watchdog
        ▼
  robstride::mit_control_all_at
```

## Integration
- **Depends on**: `robstride` (MotorBus + CAN frames), `armee-kinematics` (limit envelope, URDF parsing), `marengo-config` (YAML configs), `marengo-homing` (homing registry), `chappe` (telemetry), `armee-proto` (wire types).
- **Called by**: `berthier` (ControlLoop::tick → send_mit_batch), REPL binaries (motor-repl, homing tool).
- **Does not**: compute tau_g, plan trajectories, encode CAN bytes, open SocketCAN.
- **Intended safety contract**: application motion enters through Supervisor. Public mutable configuration, `bus_mut`, and synthetic feedback methods still permit trusted callers to bypass parts of that boundary; closing those APIs remains CS15 in the review ledger. Fault latching and same-tick stop/error handling remain separate remediation work.
