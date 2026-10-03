# ADR 0004: Control modes and MIT command model

**Status:** Accepted  
**Date:** 2025-05-19

## Context

Marengo targets OpenArm-class control: closed-loop CAN with MIT-mode impedance/feedforward commands. The existing stack used scalar position commands and a non-vendor CAN stub. Gravity compensation is required during 4-DOF arm bring-up after an upright-pose collapse incident (see [safety.md](../safety.md)).

## Decision

### Control modes (Berthier + Davout)

| Mode | `kp` / `kd` | `torque_ff` | Davout | Use |
|------|-------------|-------------|--------|-----|
| `Disabled` | — | — | No send | Default at boot |
| `GravityComp` | 0 / 0 | `tau_g(q)` | Rate-limited `tau_ff` | **Required on bench** until impedance is proven |
| `Impedance` | from `config/control.yaml` | optional friction + `tau_g` | Full MIT limits | After G-comp sign test |
| `Position` | non-zero | optional | Position + torque caps | Not for elevated poses until tuned |
| `TorqueOnly` | 0 / 0 | operator / planner | Torque caps only | Diagnostics |

Modes map to protobuf `ControlMode` and Davout `OperationalMode` (motion only when `Active`).

### MIT command semantics

Align with OpenArm `MITParam{kp, kd, q, dq, tau}` and Robstride MIT Mode 0:

- `position` → `p_des` (rad)
- `velocity` → `v_des` (rad/s)
- `kp`, `kd` → stiffness / damping (model-specific maxima in `config/control.yaml`)
- `torque_ff` → `t_ff` (Nm), includes gravity feedforward

Wire encoding is defined in [hardware/docs/decisions/0002-robstride-protocol.md](../../hardware/docs/decisions/0002-robstride-protocol.md) (ADR 0006).

### Bench policy

- **Gravity compensation is not optional** on the hardware bench until impedance tuning is documented and signed off.
- Do not use **position-only** holding for elevated configurations (shoulder up, elbow up) until G-comp is validated.

### Update 2026-10-03: Impedance setpoint tracks measured `q`

Berthier's `Impedance` packs `position = q_measured`, `velocity = 0` every tick (`MitFeedforward::compose`), so the MIT term `kp·(q_des − q)` is zero at compose time: the mode is compliant at the current pose, and `kp` (YAML or Testing override) has almost no restoring effect. `kd`, `tau_g` and friction shape the command. Position stiffness comes from `Position` mode (`hold-on` / `hold-at`). Pinned by `impedance_setpoint_tracks_displaced_q_so_kp_has_no_restoring_term`.

## Consequences

- Proto adds `MitJointCommand`, `MitCommandBatch`, `ControlMode`.
- Berthier `ControlLoop` builds MIT batches; Davout filters `torque_ff` and mode-specific fields.
- `motor-repl` gains `gravity-on` / `gravity-off` for operator testing.

## References

- [OpenArm CAN](https://docs.openarm.dev/software/can/)
- [OpenArm gravity compensation](https://github.com/enactic/openarm_teleop/blob/main/control/gravity_compasation.cpp)
