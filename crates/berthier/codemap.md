# crates/berthier/

## Responsibility
Realtime control loop (outer loop): read joint state, compute gravity compensation torque tau_g(q), compose friction feedforward, assemble impedance or position-hold MIT commands, and publish telemetry. Berthier decides **what** to command each tick but does **not** enforce safety limits, talk to CAN, or encode vendor frames.

Owns `ControlLoop::tick` — the heartbeat of the robot. Also provides a legacy `Controller` facade for single-joint REPL/bring-up use.

## Design

### Core types
- `ControlLoop<B: MotorBus>` — realtime tick facade; holds `Supervisor<B>`, `UrdfGravityModel`, `PositionHold`, `GainRuntime`, `TorqueCmdLatch`, Chappe bus reference, and tick-phase timing accumulators.
- Ordinary `ControlLoop::from_repo` receives no current-reference authority. The concrete `ControlLoop<davout::simulation::SimulationBus>::from_simulation` accepts a declared virtual initial condition through Davout's closed in-memory transport. Both factories share private model/config/state initialization and the same tick/admission/stop implementation; there is no unchecked supervisor-injection factory or alternate test policy.
- `GainRuntime` — sticky Testing `GainOverride` map + mode-transition kp/kd ramp + per-tick `resolve_all` → `ResolvedGains` (law_* + wire_*).
- `TorqueCmdLatch` — per-joint latched open-loop `τ_cmd` for TorqueOnly; cleared on leave / `enter_torque_only_zero`.
- `PositionHold` — owns latched targets, trapezoid planners, freeze/breakaway latches, and Position-mode MIT compose (`tick` → `HoldTickOut`).
- `MitFeedforward` — Active MIT packing for GravityComp / Impedance / TorqueOnly from pre-resolved wire gains + τ_ff; TorqueOnly uses latched `τ_cmd` (hard-zero kp/kd).
- `Controller<B: MotorBus>` — lighter facade wrapping `Supervisor<B>` for single-joint commands (REPL / bench).
- `ControlMode` — re-exported from `davout`: `Disabled`, `GravityComp`, `TorqueOnly`, `Impedance`, `Position`.
- `GainOverride` — runtime per-joint gain override from Testing page; public setters reject unknown joints, nonfinite fields and negative gains before mutation, preflight batches, then clamp to motor-type limits; cleared on GravityComp/TorqueOnly/Disabled enter.
- Davout's monotonic stop generation invalidates old planner, Wave and torque intent, including disable/re-enable between ticks. Planner refresh propagates receive errors and checks persistent fault authority before installing intent; new torque requests also check that authority.
- `ControlLoop::inhibit_motion_for_shutdown` discards that same retained intent before the runtime applies its configured Davout exit-stop policy. It performs no drive writes and establishes no physical stop.
- Enable completion gates every Position-mode arm. `enter_position_hold`, `enter_position_hold_at` and `start_position_wave` drain feedback (a receive failure refuses before any Enable), re-arm via `ensure_active_for_motion`, then require `enable_completion()`: Active, no staggered Enable unwritten (`Supervisor::enable_writes_pending`, including targets held for Davout's post-SetZero quiet) and fresh session pose for every Active joint. `set_joint_position_setpoint` requires it too. Otherwise they refuse with `LoopError::EnableIncomplete` ("waiting for enable to complete: …") and latch nothing, never the 0.0 placeholder `read_positions` uses for a joint without pose. A re-enable done by the arm itself always waits for its first session status. `ENABLE_COMPLETION_TIMEOUT` (2 s) is the bound callers wait for it.

### Modules (position-hold subsystem, `ControlMode::Position`)
- `gain_runtime` — `GainRuntime`, ModeGainPolicy (`mode_allows_gain_override`, `target_gains_from_yaml`, `effective_wire_gains`), override clamp, ramp arm/advance, `resolve_all`.
- `torque_cmd` — `TorqueCmdLatch` storage for operator `τ_cmd`; ControlLoop owns enter-zero / leave-clear policy.
- `position_trajectory` — `JointPositionPlanner`: trapezoidal acceleration/cruise/deceleration/hold planner per joint. Supports `seed_downward_return_if_needed` for gravity-assisted returns toward home.
- `position_feedforward` — `compose_position_hold_feedforward`: tau_g + tau_f (Coulomb + viscous friction) + tau_d (damping based on EMA-filtered velocity).
- `position_setpoint` — Setpoint mapping from planner reference to MIT q_des: clamp to limit envelope, breakaway detection, stuck-pull lead, descent freeze hysteresis, low-angle breakaway logic.
- `position_profile` — Cruise `v_max` selection (`position_hold_v_max` / `position_profile_v_max`) and `PlannerEvent` tags.
- `position_hold` — `PositionHold` lifecycle + advance/compose control law for `ControlMode::Position`.
- `position_law` — ADR 0039 scaled-PD law, selected per joint by `control.yaml` `position_law: scaled_pd` (default `legacy`): drive-side PD with constant kd, a reference that changes velocity by at most `a_max·dt` and slows as its lead grows from `e0` to `e1`, friction + `J·a` (URDF inertia) FF on the reference slewed at 6 Nm/s, leaky integral. `position_hold` caps the descending reference at the joint's `clamp_velocity` danger zone (`DescentCap`); `validate_wave_motion` refuses waves that descend faster there.
- `mit_feedforward` — `MitFeedforward::compose` for non-Position Active modes; consumes pre-resolved `wire_kp` / `wire_kd` / `fc`.
- `position_friction` — Two-rule friction model: trajectory-velocity Coulomb + settle fade (ADR 0007). Constants for onset window, deadband, hysteresis.
- `position_trace` — Optional CSV trace file (`MARENGO_POSITION_TRACE` env var) for high-rate position-hold debugging. Rows are written after the Davout send; ADR 0039 columns `law,q_ref,dq_ref,time_scale,tau_i,kd_mit,tau_ff_wire` are appended. `MARENGO_POSITION_TRACE_HZ` decimates; joints in `MARENGO_POSITION_TRACE_FULL_RATE_JOINTS` are traced every tick. Rows are formatted into one reused buffer (no per-row allocation).
- `position_wave` — In-loop triangle wave generator on one joint while others hold (bench diagnostics).
- `mode_isolation` (test-only) — Property tests verifying non-gravity FF components (tau_f, tau_d) are independent of tau_g changes.

### Test contracts
- Positive controller cases use the closed `SimulationBus`, finite raw status input and finite transmit-triggered receive scripts. Post-send hazards assert script trigger counts before their failure result; raw input passes through Davout's ordinary receive, freshness, bounds and fault checks.
- `tests/simulation_admission.rs` checks ordinary construction remains unreferenced even with a simulation transport, and explicit simulation construction admits only declared joints.
- `tests/feedback_bootstrap.rs` covers neutral solicitation, expiry, re-enable session separation and nonzero hard ranges installed before reference declaration in isolated resource trees.
- `tests/feedback_failure_propagation.rs` covers receive errors in every control mode, unsafe post-send pose, failed mode-entry intent, latched fault refusal and stopped intent cancellation.
- `tests/friction_mode_output.rs` checks actual wire torque responds to the impedance friction override while GravityComp ignores it; this replaces a local arithmetic identity property.
- `src/position_hold_tests/law_gates.rs` runs both laws against a stick-slip plant (ADR 0039 τ_ff-step, velocity-overshoot, rest-chatter and stuck-row gates); `position_hold_tests/bench_replay.rs` replays the 2026-10-04 pitch motion suite against a plant fitted to its traces and gates the danger-zone descent cap; `tests/position_law_wire.rs` decodes the MIT frames of a dithering rest hold (constant kd and v_des under `scaled_pd`).
- Small-move slew cases hold the raw encoder stationary and inspect planner/actual MIT output. Stationary controller stall cases use raw receive observations; the progress-reset law case supplies independent measured q/dq and fixed dt directly to production `PositionHold::tick`. These are software admission/output contracts, not plant tracking, reference acquisition, SetZero correlation or physical commissioning proof.

## Flow (`ControlLoop::tick`)
While Davout owns a reference reservation, discard controller intent, advance
that owner once, and publish diagnostic telemetry. There is no competing receive
or motion MIT; terminal failure stays owned rather than triggering a duplicate
runtime fallback stop. Normal flow below resumes only without a reservation.

1. **Feedback drain**: `Supervisor::drain_feedback()` — non-blocking poll of CAN RX queue (frames buffered from prior tick's transmit).
2. **Read positions**: joint-space q, dq from Davout's `MotorState` map via joint↔motor transform.
   A new Davout enable-session marker starts at most two missing-pose ticks of
   neutral MIT status solicitation (zero kp/kd/velocity/feedforward). No gravity
   or PD calculation runs until all active joints have current-session feedback;
   missing feedback afterward returns `MissingFeedback`. Re-enable between ticks
   still starts a new window. Davout independently enforces its receive deadline.
3. **Gravity comp**: `dynamics.gravity_torques(&q)` — armee-dynamics virtual-work gradient produces tau_g.
4. **Resolve gains**: `GainRuntime::resolve_all` — law_* (override or impedance YAML) + wire_* (override > ramp > YAML target).
5. **Position hold** (Position mode only): `PositionHold::tick(HoldWorld)` with law_* params; patch MIT `kp` from `wire_kp` only.
6. **Compose MIT batch** (non-Position modes): `MitFeedforward::compose` — GravityComp `τ_ff=τ_g` with wire gains; TorqueOnly `τ_ff=τ_cmd` (kp/kd=0); Impedance adds friction (`fc` from resolve) + wire gains.
7. **Send batch**: `Supervisor::send_mit_batch(cmds)` — goes through Davout's filter pipeline; post-send feedback errors propagate before gain/tick success. Safety failures discard motion intent; missing-feedback exhaustion and ascent stalls additionally latch a controller fault through Davout and attempt stop.
8. **Publish Chappe telemetry** at reduced rate (e.g. 20 Hz vs 200 Hz loop).

## Integration

The separately named current-consuming virtual journal factories start Unreferenced.
Their normal busy tick consumes one actual bounded report, may select the acquired
joint after durable completion, and returns without old intent or ordinary output.
`reference_grant_tests.rs` exercises the published-completion tick and later explicit
selected output, including a whole-report fault with an unread bounded suffix.

- **Depends on**: `davout` (supervisor + bus), `armee-dynamics` (gravity), `armee-kinematics` (limit envelope), `chappe` (telemetry), `marengo-config` (config loading), `armee-proto` (RobotState protobuf types).
- **Called by**: `marengo-pi` binary — drives `ControlLoop` on the Pi's realtime thread.
- **Does not**: open CAN sockets, enforce E-stop, manage joint↔motor transform, load firmware parameters.
