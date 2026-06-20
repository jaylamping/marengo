# config/ — Runtime configuration

YAML configs consumed by `marengo-config` crate. Two layers: root defaults + `bringup/<profile>/` overrides.

## STRUCTURE

```
config/
├── control.yaml              # Control law params (kp/kd/slew/trim), bench settings, danger zones
├── motors.yaml               # Motor map: CAN ID, joint name, direction, gear_ratio, motor_type
├── robot.yaml                # URDF path, torque/velocity bench caps
├── homing.yaml               # Homing sequence params
├── network.yaml              # Chappe / CAN network config
├── motors_humanoid.yaml      # Full humanoid motor map (future)
├── robot_humanoid.yaml       # Full humanoid robot config (future)
└── bringup/                  # Bench bringup profiles (override root configs)
    ├── shoulder_pitch_right_only/   # DEFAULT bench profile
    ├── shoulder_pitch_left_only/
    ├── shoulder_pitch_dual/
    ├── shoulder_pitch_weighted/     # Weighted arm test (700g)
    └── arm_4dof_left/               # Full 4-DOF left arm
```

Each bringup profile contains: `control.yaml`, `motors.yaml`, `robot.yaml`, `homing.yaml`.

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Bench default profile | `bringup/shoulder_pitch_right_only/` |
| Velocity caps | `control.yaml` → `resolve_joint_velocity_cap` (ADR 0010) |
| Danger zone rules | `control.yaml` (measured `q`/`dq` based) |
| Motor direction/gearing | `motors.yaml` (Davout applies transforms) |
| Torque caps | `robot.yaml` → `robot.bench` + per-joint in `motors.yaml` |
| Homing config | `homing.yaml` |
| Zero calibration registry | `var/calibration/zero_registry.yaml` (runtime, not here) |

## CONVENTIONS

- **Velocity caps resolve only from `control.yaml`** — `motors.yaml`/`robot.yaml`/URDF velocity fields do NOT override (ADR 0010).
- Profile selection via `MARENGO_CONFIG_DIR` env var (e.g. `config/bringup/shoulder_pitch_right_only`).
- Edit locally → `pi_sync_bench_config` (MCP) or `scripts/pi-remote.sh` to sync to Pi.
- Danger zone rules evaluate **measured** `q`/`dq`, not commanded MIT fields.

## ANTI-PATTERNS

- Setting velocity caps in `motors.yaml` or `robot.yaml` — ignored, use `control.yaml`.
- Assuming `SetZero` alone establishes calibration — `zero_registry.yaml` audit required.
- Editing Pi configs directly without syncing from repo — drift.
