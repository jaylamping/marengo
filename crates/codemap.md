# crates/

## Responsibility
Workspace library crates implementing the Marengo humanoid robot's control stack. Each crate is a single-responsibility module in a layered architecture from kinematics/dynamics through safety filtering to hardware CAN transport, with a protobuf message bus for inter-process telemetry.

## Design

### Naming (historical French military figures)
| Crate | Role |
|-------|------|
| `armee-kinematics` | URDF geometry: joint limits, actuated joint indexing, velocity-scaled command envelope (ADR 0009) |
| `armee-dynamics` | Rigid-body gravity compensation: tau_g(q) via virtual-work gradient on URDF inertials |
| `armee-proto` | Generated protobuf types from `proto/` — wire format for Chappe bus |
| `chappe` | In-process and Unix-domain socket pub/sub for telemetry/state (topic-based) |
| `berthier` | Realtime control loop: read joint state, compute tau_g + friction + impedance, batch MIT commands |
| `davout` | Safety gateway: the only crate that sends motion to robstride; state machine, limit filtering, joint↔motor transform |
| `robstride` | Pure CAN transport driver: encode/decode Robstride MIT frames (29-bit extended protocol), no policy |
| `marengo-config` | YAML/URDF config loading: robot.yaml, motors.yaml, control.yaml, homing.yaml |
| `marengo-homing` | Joint homing registry: encoder zero verification, calibration record persistence |
| `marengo-support` | Shared init helpers: `init_tracing()`, workspace lint overrides |
| `marengo-store` | Time-series key-value store for telemetry replay |
| `marengo-imu` | IMU driver and frame publishing |
| `marengo-host-metrics` | Host-level CPU/Mem/Disk metrics for health dashboard |
| `fouche` | Vision crate (in development) |
| `talleyrand` | Planning crate (in development) |
| `sim-harness` | Simulation test harness |

### Dependency direction (strict)
```
armee-kinematics ← armee-dynamics ← berthier → davout → robstride
                                         ↘ chappe (telemetry)
armee-proto (wire types) ← chappe, armee-dynamics, berthier, davout
marengo-config ← berthier, davout, armee-dynamics
```

Berthier owns **what to command**. Davout owns **may it move** (safety filter + operational state machine). Robstride owns **how to encode for hardware**. Armee-dynamics owns **gravity model**. Armee-kinematics owns **geometric limits from URDF**.

### Safety architecture
- Davout is the sole gateway to robstride — no other crate calls `MotorBus`.
- Davout filters every MIT command: position envelope (ADR 0009), kp/kd caps per motor type, tau_ff rate limiting, wrong-sign watchdog, comm watchdog, E-stop.
- Berthier's `ControlLoop::tick` must pass through Davout `Supervisor::send_mit_batch`.
- Robstride has no policy — only bytes on CAN bus and a feedback cache.

## Flow (control-loop tick)
1. Davout `drain_feedback` — non-blocking CAN RX queue poll
2. Berthier reads joint positions via Davout `joint_position_rad`
3. Berthier computes tau_g via armee-dynamics `DynamicsModel::gravity_torques`
4. Berthier composes feedforward: tau_g + optional friction + impedance or position-hold tau_f/tau_d
5. Berthier assembles `MitJointCommand` batch, sends through Davout `send_mit_batch`
6. Davout filters each command (envelope, kp/kd caps, tau_ff rate limit, wrong-sign check), applies joint↔motor transform (direction/gear_ratio), calls robstride `mit_control_all_at`
7. Robstride encodes MIT Mode 0 CAN frames and transmits; decodes feedback on next tick

## Integration
- **Consul (web UI)**: receives `RobotState` via Chappe IPC (Unix socket) from `marengo-pi`
- **marengo-pi**: runs ControlLoop + Chappe bridge on Pi, hosts supervisor state
- **marengo-gateway**: Chappe IPC listener, serves Consul frontend
- **marengo-jetson**: vision/planning (Fouche/Talleyrand) — future
- **Tests**: `MemoryBus` in robstride allows full-stack unit tests without CAN hardware; `SyntheticBus` in Marengo for integration tests
