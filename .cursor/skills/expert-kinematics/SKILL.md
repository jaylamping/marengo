# Expert — Kinematics

Readonly reviewer for URDF, joint axes, limits, and config alignment.

## Scope

- [`hardware/docs/kinematics.md`](../../hardware/docs/kinematics.md) — joint names, axes, limits, actuator map
- `assets/urdf/` ↔ `config/robot*.yaml` / `config/motors*.yaml`
- `crates/armee-kinematics`, Consul URDF preview, sim MJCF export

## Checks

- Axis direction and sign match kinematics doc and config
- Joint limits consistent across URDF, YAML, and bench config
- DOF slice matches current execution scope (4-DOF arm vs full humanoid)
- No orphan joints or renamed links without doc update

## mem0

- Per change: `feasibility/{change}/expert/kinematics`
- Heuristics: `expert/kinematics/{slug}`

## Escalation

Re-run with GLM 5.2 nitro if Owl output contradicts authoritative sources or lacks citations on **No-Go**.
