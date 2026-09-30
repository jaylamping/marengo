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
- `Supervisor<B: MotorBus>` — owns state/motor policy, homing registry, pose cache, persistent `FaultAuthority`, and the `MotorBus`. Constructed from repo config files.
- `SafetySnapshot` — owned read-only persistent records, raw status/detailed/warning domains, hardware input, stop generation, latest stop and first failed stop. Qualified recovery is unavailable in this slice.
- `StopReport` — every address's zero-speed, neutral-MIT and ordinary-disable attempt, including bounded errors. Accepted writes do not prove physical acknowledgement.
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
7. Send the prepared addressed batch through Robstride's checked API. Admission failure restores output-history scratch state. Rejected numeric/gain/neutral-position requests remain nonlatching; runtime watchdog/sign/danger faults and transport uncertainty latch and automatically stop after any valid prefix. Motion owners separately discard planner/torque intent.

Startup validates the combined robot/motor/control/homing policy. `validate_control_candidate` checks proposed control overlays against the installed companion configuration before installation or persistence; it does not install policy or rebuild limits.

### Joint↔motor transform
- `direction` and `gear_ratio` from `motors.yaml`: position_rad *= scale, kp /= scale^2, kd /= scale^2, tau_ff /= scale where scale = direction * gear_ratio.
- inverse transform applied on feedback: motor→joint state. Direction must be ±1 and gear ratio finite and positive.

### Feedback processing
- `drain_feedback` — non-blocking poll (control loop path, budget = 0).
- `refresh_feedback` — blocking poll up to `feedback_poll_budget_us` (REPL / set-zero).
- Consume Robstride's lossless ordered report, including observations before a terminal transport error. Every configured status is checked for finite raw/transformed fields, original receive time and measured hard-position evidence before chronology can skip it. All peer faults are retained even if an earlier pose is invalid.
- Vendor status flags, drive mode, four detailed-fault bytes and four warning bytes are separate domains. Full detailed word byte order and physical recovery remain unqualified; `JointFeedback.fault` is a compatibility nonzero indication, not a union of vendor bit identities.
- Reserved mode cannot publish admissible pose. A post-enable status from an enabled address must be Run; Reset/Calibration latches and stops. Reset from an unenabled scoped peer remains diagnostic. Mechanical SetZero enable is not a qualified factory-calibration context.
- Empty drains, unknown addresses, fresh peers, and fault-only reports cannot refresh another motor's pose. Fault-only reports retain fault evidence without creating a zero pose. Older/replayed samples cannot replace a newer pose or mutate derivative policy state.
- Invalid feedback latches independent fault authority despite an older valid cache. A newer valid pose may restore diagnostic visibility, but it cannot restore motion permission. Active getters omit absent, invalid, stale and prior-enable pose.
- Inspect every raw frame's fault/mode/position evidence, including timestamp ties, then select the latest admissible pose per address for derivative/cache admission. Position-derived velocity/trips update once per address per drain: host dequeue spacing cannot recover physical acquisition spacing in a queued burst.
- Drain queued status before enable writes and again after enable/run-mode writes, before creating the session marker. Status queued during those writes cannot authorize the new session. CAN status has no command-generation identifier; traffic arriving after the final drain remains uncorrelated, and session freshness cannot identify every delayed physical packet.

### Fault and stop lifecycle
- The first runtime/device/feedback/transport/controller hazard retains its stable ID/cause, attempts all stops, and increments a Supervisor-lifetime stop generation. Later healthy/empty diagnostics do not clear authority or repeat the stop burst. Additional hazard evidence and secondary delivery failures are retained.
- All motion, enable, calibration and SetZero routes consult the latch. `check_fault_authority` provides the same read-only gate to controller mode entry. `latch_control_fault` is a trusted owner hook for actual controller failures, not an operator reset.
- Explicit Disable always attempts all configured addresses and advances stop generation; it never clears faults. Every newly asserted hardware-input edge attempts a stop, even after an existing fault; releasing the boolean does not reset authority. GPIO integration is still absent.
- Checked Ready transition refuses Active; unchecked Ready is a no-op while Active. Calibration enable refuses existing Active motion so live drive/watchdog authority cannot disappear behind a Ready flag.
- The latest stop report and first failed report distinguish transport acceptance from unconfirmed physical stop. No automatic recovery or firmware fault-clear transaction is implemented. See ADR 0020 and the remediation ledger for remaining Pi/protobuf generation/publication, reference and drive-local qualification.

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
- **Intended safety contract**: application motion enters through Supervisor. Public mutable configuration, `bus_mut`, and synthetic feedback methods still permit trusted callers to bypass parts of that boundary; closing those APIs remains CS15. Replay/cache APIs cannot erase the new fault latch, but raw bus access remains a bypass. Persistent Pi publication/command generation, explicit qualified recovery, physical stop confirmation, reference provenance and drive-local readback remain separate work.
