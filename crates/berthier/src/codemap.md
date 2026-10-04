# crates/berthier/src/

## Responsibility
Implementation modules for the Berthier realtime control loop and legacy single-joint controller.

## Design
| Module | Role |
|--------|------|
| `loop.rs` | `ControlLoop<B>` — shared private initialization for ordinary and concrete closed simulation constructors, checked gain setters, enable-session neutral bootstrap, main tick, mode dispatch, Chappe publish |
| `degraded.rs` | ADR 0038 degraded episode: `admit_drive_loss` (offline τ_g bound → `DriveLossPlan`s), `DegradedEvent`, hold/lower phases driven from `loop.rs` |
| `friction.rs` | Velocity-based friction feedforward |
| `position_feedforward.rs` | PD torque for position hold |
| `position_hold.rs` | `PositionHold`: latched targets, planners, recovery latches, MIT compose, and the two position-hold fuses (below) |
| `position_profile.rs` | Trapezoidal/s-curve position profiles |
| `position_setpoint.rs` | Target angle management; grid-aware home tolerance (`home_target_tolerance`) |
| `position_trajectory.rs` | Time-parameterized position paths |
| `position_wave.rs` | Sine-wave bench excitation |
| `position_trace.rs` | Position command logging |
| `lib.rs` | `Controller` facade, `ControlError`, public re-exports |

## Flow
`ControlLoop::tick` (loop.rs):
1. `supervisor.drain_feedback()`
2. `joint_positions()` → `q`
   Require current-session active-joint feedback; at most two neutral solicitation
   ticks precede `MissingFeedback`. Davout also checks the elapsed deadline.
3. `dynamics_model.gravity_torques(&q)` → τ_g
4. Mode branch: gravity-only / impedance / position / torque
5. `supervisor.send_mit_batch(commands)`
6. Optional `publish_robot_state(chappe_bus)` — failures are counted
   (`telemetry_failures`) and logged, never propagated as tick errors.
   Tick wall-time overruns against the loop period are counted
   (`tick_overruns`) for the 1 Hz diagnostics.

### Position-hold fuses (`position_hold.rs`)
Both use a 2000 ms no-progress budget (`POSITION_ASCENT_STALL_FAULT_MS`) and credit progress only on a
new best encoder level beyond Davout's feedback-grid threshold (ADR 0025). Either trip makes `tick`
return an error; `ControlLoop::tick` latches a Davout control fault and disables.
- **`AscentStall`** (outbound ascent stall): non-home target, `target − q` beyond 0.03 rad in the direction of the target's sign (`outbound_stall_direction`), no new best `q` in that direction. Model-independent.
- **`HoldTracking`** (hold tracking failure): any target, `|q − target|` > 0.03 rad
  (`POSITION_HOLD_TRACKING_BAND_RAD`) while net commanded torque `tau_p + tau_ff` opposes the
  direction to target, no new closest `q`. Covers a home latch that sags under a wrong `τ_g` model.
- **`WaveStall`**: the wave joint is exempt from the two fuses above; while the wave commands ≥ 0.05 rad/s, measured `q` must keep leaving its credited level by 0.02 rad (or the grid threshold) or the 2000 ms budget trips. `start_position_wave` refuses non-finite arguments, ranges outside the soft envelope, and peak speed/acceleration above the joint's velocity cap, `position_trajectory_velocity_rad_s` and `position_trajectory_accel_rad_s2`.
- A retarget never renews a fuse budget. The law's `kp` is the wire `kp` (override > ramp > YAML).
- `LoopError::{MissingSetpoint, HoldLenMismatch, Dynamics, GainShape}` on the tick are controller-invariant breaks: they latch a control fault and discard intent like the fuses.
- Errors carry `HoldFuseTrip` (`q`, `target`, `tau_p`, `tau_ff`, `tau_g` at trip).
- Home classification: a clamped target within two feedback counts of zero
  (`home_target_tolerance` of the joint's progress threshold) is latched as exactly `0.0`
  (raw request kept in `setpoints_raw`, surfaced per-tick as diag `target_raw`).

## Integration
- Imports `davout::{Supervisor, ControlMode, MitJointCommand}`
- Imports `armee_dynamics::DynamicsModel`
- Re-exported by crate root `lib.rs`
- Only `ControlLoop<davout::simulation::SimulationBus>::from_simulation` accepts a virtual initial reference fixture. It shares the ordinary loop implementation; arbitrary buses and ordinary constructors cannot receive that fixture. Test raw observations and finite scripts go through the production receive/admission/output paths.
- `ControlLoop::from_repo_with_physical_reference(root, bus, journal, loop_hz, chappe_hz)` wraps `Supervisor::from_repo_with_physical_reference` (ADR 0036); the journal path comes from `marengo_config::resolve_reference_journal_path`. `tick` calls `supervisor.advance_reference_work()` while `reference_work_pending()`.
