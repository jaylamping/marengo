# config/ — Runtime configuration

YAML configs consumed by `marengo-config` crate. **Master SoT:** root `config/{robot,motors,control,homing}.yaml` plus `assets/urdf/marengo.urdf`.

## STRUCTURE

```
config/
├── control.yaml              # Control law params (kp/kd/slew/trim), bench settings, danger zones
├── motors.yaml               # Motor map: CAN ID, joint name, direction, gear_ratio, motor_type
├── robot.yaml                # URDF path (`assets/urdf/marengo.urdf`), torque/velocity bench caps
├── homing.yaml               # Homing sequence params
├── motors_humanoid.yaml      # 23-joint humanoid motor map template (tests only until M6)
└── robot_humanoid.yaml       # 23-joint humanoid robot template (tests only until M6)
```

Pi durable layout: `/opt/marengo/config/` (same four YAML files). Historical bringup profiles live in git history and `assets/urdf/archive/`.

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Active bench config | Root `config/` (5-DOF right arm) |
| Velocity caps | `control.yaml` → `resolve_joint_velocity_cap` (ADR 0010) |
| Danger zone rules | `control.yaml` (measured `q`/`dq` based) |
| Motor direction/gearing | `motors.yaml` (Davout applies transforms) |
| Torque caps | `robot.yaml` → `robot.bench` + per-joint in `motors.yaml` |
| Homing config | `homing.yaml` |
| Reference journal | Beside `homing.yaml` `calibration_record_path` (runtime, not here). History only; the old `zero_registry.yaml` registry is retired (docs/homing.md) |

## CONVENTIONS

- **Velocity caps resolve only from `control.yaml`** — `motors.yaml`/`robot.yaml`/URDF velocity fields do NOT override (ADR 0010).
- **Config dir resolution:** `MARENGO_CONFIG_DIR`, else `/opt/marengo/config` **if it exists**, else `<repo>/config`. A stray `/opt/marengo/config` on a dev host silently wins.
- Edit locally → `pi_sync_bench_config` (MCP, YAML only) or `pi_sync_bench_urdf` (URDF, ADR 0017), or `scripts/pi-remote.sh` from the cloud.
- Danger zone rules evaluate **measured** `q`/`dq`, not commanded MIT fields.

## ANTI-PATTERNS

- Setting velocity caps in `motors.yaml` or `robot.yaml` — ignored, use `control.yaml`.
- Assuming `SetZero` or a journal row establishes reference — only a process-local physical grant does (ADR 0036).
- Editing Pi configs directly without syncing from repo — drift.
- Using `config/bringup/*` as runtime SoT — retired; use root master tree.
