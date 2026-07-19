# crates/berthier/

## Responsibility
Realtime control loop (outer loop): read joint state, compute gravity compensation torque tau_g(q), compose friction feedforward, assemble impedance or position-hold MIT commands, and publish telemetry. Berthier decides **what** to command each tick but does **not** enforce safety limits, talk to CAN, or encode vendor frames.

Owns `ControlLoop::tick` — the heartbeat of the robot. Also provides a legacy `Controller` facade for single-joint REPL/bring-up use.

## Design

### Core types
- `ControlLoop<B: MotorBus>` — realtime tick facade; holds `Supervisor<B>`, `UrdfGravityModel`, planner state per joint, Chappe bus reference, gain ramp state, and tick-phase timing accumulators.
- `Controller<B: MotorBus>` — lighter facade wrapping `Supervisor<B>` for single-joint commands (REPL / bench).
- `ControlMode` — re-exported from `davout`: `Disabled`, `GravityComp`, `TorqueOnly`, `Impedance`, `Position`.
- `GainRamp` — per-joint kp/kd linear interpolation over ~100 ms on non-Disabled mode transitions.
- `GainOverride` — runtime per-joint gain override from Testing page; clamped to motor-type safety limits.

### Modules (position-hold subsystem, `ControlMode::Position`)
- `position_trajectory` — `JointPositionPlanner`: trapezoidal acceleration/cruise/deceleration/hold planner per joint. Supports `seed_downward_return_if_needed` for gravity-assisted returns toward home.
- `position_feedforward` — `compose_position_hold_feedforward`: tau_g + tau_f (Coulomb + viscous friction) + tau_d (damping based on EMA-filtered velocity).
- `position_setpoint` — Setpoint mapping from planner reference to MIT q_des: clamp to limit envelope, breakaway detection, stuck-pull lead, descent freeze hysteresis, low-angle breakaway logic.
- `position_profile` — Motion profile classifier: `SmallHold` vs `TrajectoryMove` vs `ReturnHome` vs `LimitProbe`. Selects cruise v_max.
- `position_friction` — Two-rule friction model: trajectory-velocity Coulomb + settle fade (ADR 0007). Constants for onset window, deadband, hysteresis.
- `position_trace` — Optional CSV trace file (`MARENGO_POSITION_TRACE` env var) for high-rate position-hold debugging.
- `position_wave` — In-loop triangle wave generator on one joint while others hold (bench diagnostics).
- `mode_isolation` (test-only) — Property tests verifying non-gravity FF components (tau_f, tau_d) are independent of tau_g changes.

## Flow (`ControlLoop::tick`)
1. **Feedback drain**: `Supervisor::drain_feedback()` — non-blocking poll of CAN RX queue (frames buffered from prior tick's transmit).
2. **Read positions**: joint-space q, dq from Davout's `MotorState` map via joint↔motor transform.
3. **Gravity comp**: `dynamics.gravity_torques(&q)` — armee-dynamics virtual-work gradient produces tau_g.
4. **Position advance** (Position mode only): `advance_position_commands` — tick per-joint trapezoidal planners, detect stuck/breakaway, clamp setpoints to limit envelope.
5. **Compose MIT batch**: per joint, depending on mode:
   - `GravityComp` / `TorqueOnly`: tau_ff = tau_g (or zero), kp=kd=0
   - `Impedance`: tau_ff = tau_g + tau_f, kp/kd from config (or gain override)
   - `Position`: full feedforward composition — tau_ff = tau_g + tau_f + tau_d, kp/kd from impedance config + integral term, q_des from clamped planner output
6. **Apply gain ramp**: interpolate kp/kd during transition window.
7. **Send batch**: `Supervisor::send_mit_batch(cmds)` — goes through Davout's filter pipeline.
8. **Publish Chappe telemetry** at reduced rate (e.g. 20 Hz vs 200 Hz loop).

## Integration
- **Depends on**: `davout` (supervisor + bus), `armee-dynamics` (gravity), `armee-kinematics` (limit envelope), `chappe` (telemetry), `marengo-config` (config loading), `armee-proto` (RobotState protobuf types).
- **Called by**: `marengo-pi` binary — drives `ControlLoop` on the Pi's realtime thread.
- **Does not**: open CAN sockets, enforce E-stop, manage joint↔motor transform, load firmware parameters.
