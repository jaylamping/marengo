# config/bringup/

## Responsibility
**Bench bringup profiles** — self-contained config directories for specific hardware configurations on the Pi.

## Design
Each profile folder contains a full set: `robot.yaml`, `motors.yaml`, `control.yaml`, `homing.yaml` (and optionally `network.yaml`).

| Profile | Use case |
|---------|----------|
| `arm_2dof_right` | Default right-shoulder bench (MCP default) |
| `arm_4dof_left` | Left 4-DOF arm bring-up |
| `shoulder_pitch_dual` | Dual shoulder pitch motors |
| `shoulder_pitch_left_only` | Left shoulder only |
| `shoulder_pitch_weighted` | Weighted single-arm characterization |

## Flow
`MARENGO_CONFIG_DIR` points at one profile → marengo-pi/motor-repl load that tree exclusively.

## Integration
- Synced to Pi via `scripts/deploy-pi.sh` and MCP `pi_sync_bench_config`
- Installed to `/opt/marengo/config/bringup/` on Pi
