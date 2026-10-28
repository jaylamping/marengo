# marengo-config

Typed loaders and validation for the master [`config/`](../../config/): `robot.yaml`, `motors.yaml`, `control.yaml` and `homing.yaml`. Declarative data; Davout and Berthier enforce what it loads.

- `resolve_config_dir`: `MARENGO_CONFIG_DIR` if set, else `/opt/marengo/config` when that directory exists (so a stray `/opt/marengo/config` on a dev host wins), else `<repo>/config`. `resolve_urdf_path` fails fast when the URDF is missing.
- Cross-file validation (motor joints ⊆ robot joints, control coverage, control soft limits inside motors.yaml hard limits, gains and caps, homing). Limits against the URDF are checked by Davout's `build_limits`, not here.
- Velocity-cap resolution ([ADR 0010](../../docs/decisions/0010-actuator-velocity-cap-resolution.md)) and position-law parameters.
- Bench Set Limits support: limit patches, URDF expand/merge ([ADR 0017](../../docs/decisions/0017-bench-set-limits-urdf-expand.md)) and atomic, locked profile writes. The YAML writers re-serialise through serde, which drops comments.
- Commissioning scope and completeness reports used by the gateway and Consul.

`config/*_humanoid.yaml` are full-body templates used by tests only. Change joint names, motor types or caps here together with the URDF and [`hardware/docs/kinematics.md`](../../hardware/docs/kinematics.md). See [`config/AGENTS.md`](../../config/AGENTS.md).
