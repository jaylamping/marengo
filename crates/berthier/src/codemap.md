# crates/berthier/src/

## Responsibility
Implementation modules for the Berthier realtime control loop and legacy single-joint controller.

## Design
| Module | Role |
|--------|------|
| `loop.rs` | `ControlLoop<B>` — main tick, mode dispatch, Chappe publish |
| `friction.rs` | Velocity-based friction feedforward |
| `position_feedforward.rs` | PD torque for position hold |
| `position_profile.rs` | Trapezoidal/s-curve position profiles |
| `position_setpoint.rs` | Target angle management |
| `position_trajectory.rs` | Time-parameterized position paths |
| `position_wave.rs` | Sine-wave bench excitation |
| `position_trace.rs` | Position command logging |
| `lib.rs` | `Controller` facade, `ControlError`, public re-exports |

## Flow
`ControlLoop::tick` (loop.rs):
1. `supervisor.drain_feedback()`
2. `joint_positions()` → `q`
3. `dynamics_model.gravity_torques(&q)` → τ_g
4. Mode branch: gravity-only / impedance / position / torque
5. `supervisor.send_mit_batch(commands)`
6. Optional `publish_robot_state(chappe_bus)`

## Integration
- Imports `davout::{Supervisor, ControlMode, MitJointCommand}`
- Imports `armee_dynamics::DynamicsModel`
- Re-exported by crate root `lib.rs`
