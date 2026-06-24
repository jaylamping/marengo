# proto/marengo/v1/

## Responsibility
Marengo v1 wire messages for robot state, safety, control, gateway, host metrics, and structured logs.

## Design
| Message group | Key types |
|---------------|-----------|
| Telemetry | `RobotState`, `JointState`, `ImuSample`, `Heartbeat` |
| Safety | `SafetyState`, `Fault`, `OperationalMode`, `EnableRequest` |
| Control | `ControlMode`, `MitJointCommand`, `MitCommandBatch` |
| Gateway | `GatewaySubscribe`, `LogEvent` |
| Host | `HostMetrics`, `CpuMetrics`, `MemoryMetrics`, `DiskMetrics`, `ChappeHealth` |
| Homing | `HomingComplete` |

## Flow
Chappe topic → payload bytes decode to specific message type via prost/@bufbuild

## Integration
- Single file `marengo.proto` — all v1 schemas colocated for review
