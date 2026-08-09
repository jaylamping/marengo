# config/ — Runtime configuration

YAML configs consumed by `marengo-config` crate. **Master SoT:** root `config/{robot,motors,control,homing}.yaml` plus `assets/urdf/marengo.urdf`.

## STRUCTURE

```
config/
├── control.yaml              # Control law params (kp/kd/slew/trim), bench settings, danger zones
├── motors.yaml               # Motor map: CAN ID, joint name, direction, gear_ratio, motor_type
├── robot.yaml                # URDF path (`assets/urdf/marengo.urdf`), torque/velocity bench caps
├── homing.yaml               # Homing sequence params
├── network.yaml              # Chappe / CAN network config
├── motors_humanoid.yaml      # Full humanoid motor map (future)
└── robot_humanoid.yaml       # Full humanoid robot config (future)
```

Pi durable layout: `/opt/marengo/config/` (same four YAML files). Historical bringup profiles live in git history and `assets/urdf/archive/`.

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Active bench config | Root `config/` (4-DOF right arm) |
| Velocity caps | `control.yaml` → `resolve_joint_velocity_cap` (ADR 0010) |
| Danger zone rules | `control.yaml` (measured `q`/`dq` based) |
| Motor direction/gearing | `motors.yaml` (Davout applies transforms) |
| Torque caps | `robot.yaml` → `robot.bench` + per-joint in `motors.yaml` |
| Homing config | `homing.yaml` |
| Zero calibration registry | `var/calibration/zero_registry.yaml` (runtime, not here) |

## CONVENTIONS

- **Velocity caps resolve only from `control.yaml`** — `motors.yaml`/`robot.yaml`/URDF velocity fields do NOT override (ADR 0010).
- **`MARENGO_CONFIG_DIR`** defaults to `/opt/marengo/config` on Pi; dev uses `<repo>/config` when unset.
- Edit locally → `pi_sync_bench_config` (MCP) or `scripts/pi-remote.sh` to sync to Pi (Phase 4 retargets to master paths).
- Danger zone rules evaluate **measured** `q`/`dq`, not commanded MIT fields.

## ANTI-PATTERNS

- Setting velocity caps in `motors.yaml` or `robot.yaml` — ignored, use `control.yaml`.
- Assuming `SetZero` alone establishes calibration — `zero_registry.yaml` audit required.
- Editing Pi configs directly without syncing from repo — drift.
- Using `config/bringup/*` as runtime SoT — retired; use root master tree.
