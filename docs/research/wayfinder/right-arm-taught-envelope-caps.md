# Right arm taught envelope and velocity caps

**Wayfinder research** for [marengo#155](https://github.com/jaylamping/marengo/issues/155) (map [marengo#150](https://github.com/jaylamping/marengo/issues/150)).

**Snapshot date:** 2026-08-11 (branch `research/right-arm-taught-envelope-caps`, `origin/main`).

**Primary sources:** `config/motors.yaml`, `config/control.yaml`, `config/robot.yaml`, [ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md), [ADR 0017](../decisions/0017-bench-set-limits-urdf-expand.md).

**Scope:** `right_arm` limb members per `robot.yaml` — five joints on `can0` (pitch 1, roll 2, upper-arm yaw 3, elbow pitch 4, lower-arm yaw 5).

---

## Summary

| Item | Baseline |
|------|----------|
| Taught hard (bench) | `motors.yaml` → `bench.position_lower_rad` / `position_upper_rad` |
| Operator soft | `control.yaml` → `position_soft_lower_rad` / `position_soft_upper_rad` (Set Limits inset per ADR 0009) |
| Davout effective hard | URDF `<limit>` ∩ taught bench hard ([ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md), expand-only URDF widen per [ADR 0017](../decisions/0017-bench-set-limits-urdf-expand.md)) |
| Command velocity ceiling | `control.yaml` actuator groups → `resolve_joint_velocity_cap` (per-joint override absent) |
| Trajectory cruise | `control.yaml` → `position_trajectory_velocity_rad_s` per joint |
| Danger zones | One rule: `elevated_shoulder_pitch_fall` on pitch |
| Provisional DOFs | Elbow pitch (4): taught Set Limits, direction/sign TBD. Lower-arm yaw (5): kinematics envelope only; firmware pending |

Bench envelopes for DOF 1–4 were taught via Set Limits on **2026-07-22** (`motors.yaml` header). DOF 5 remains a provisional kinematics envelope until first measured Set Limits pass.

---

## Per-joint limits and velocity caps

All position values in **radians**. Hard = `motors.yaml` taught bench. Soft = `control.yaml` operator bounds. Gaps are hard − soft on each side (positive = hard is more permissive than soft).

| Joint | CAN ID | Motor | Hard lower | Hard upper | Soft lower | Soft upper | Δ lower | Δ upper | Bench τ cap (Nm) | `motors.yaml` bench v (rad/s)† | Actuator-group v cap (rad/s) | Traj cruise v (rad/s) | Traj accel (rad/s²) | Slew (rad/s) |
|-------|--------|-------|------------|------------|------------|------------|---------|---------|------------------|-------------------------------|------------------------------|----------------------|---------------------|--------------|
| `right_shoulder_pitch` | 1 | rs03 | −0.900474 | 2.925390 | −0.873474 | 2.898390 | 0.027 | 0.027 | 5.0 | 2.0 | **2.5** (`shoulder_pitch`) | 1.25 | 4.5 | 0.15 |
| `right_shoulder_roll` | 2 | rs03 | −0.050000 | 2.579468 | −0.023000 | 2.552468 | 0.027 | 0.027 | 5.0 | 2.0 | **2.5** (`shoulder_roll`) | 0.70 | 4.0 | 0.35 |
| `right_upper_arm_yaw` | 3 | rs02 | −0.749756 | 0.526939 | −0.722756 | 0.499939 | 0.027 | 0.027 | 3.0 | 2.0 | **2.0** (`upper_arm_yaw`) | 1.00 | 3.0 | 0.15 |
| `right_elbow_pitch` | 4 | rs02 | −0.410353 | 1.034702 | −0.383353 | 1.007702 | 0.027 | 0.027 | 3.0 | 1.0 | **1.5** (`elbow`) | 0.80 | 2.5 | 0.12 |
| `right_lower_arm_yaw` | 5 | rs00 | −1.600000 | 1.600000 | −1.550000 | 1.550000 | 0.050 | 0.050 | 3.0 | 1.0 | **1.5** (`lower_arm_yaw`) | 0.80 | 2.5 | 0.12 |

† `motors.yaml` `bench.velocity_limit_rad_s` is **not** the command ceiling; velocity caps resolve only from `control.yaml` ([ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md) margin layer; actuator groups for MIT cap).

### Shared limit-margin parameters (all five joints)

From `control.yaml` → `control.joints.<joint>`:

| Field | Value | Role ([ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md)) |
|-------|-------|-----------------------------------------------------------------------------|
| `position_limit_margin_min_rad` | 0.01 | Rest / crawl margin inside soft |
| `position_limit_margin_k_v_s` | 0.02 | Linear speed term on approach |
| `position_limit_margin_k_stop` | 0.5 | Stopping-distance scale (`decel` = `position_trajectory_accel_rad_s2`) |
| `position_limit_measured_fault_slack_rad` | 0.03 | Measured-`q` fault slack at taught hard (enable/settle jitter) |

### Robot-wide bench caps (`robot.yaml`)

| Field | Value |
|-------|-------|
| `robot.bench.max_joint_velocity_rad_s` | 2.0 |
| `robot.bench.max_joint_torque_nm` | 5.0 |

Per-joint torque ceilings in practice follow `motors.yaml` `bench.torque_limit_nm` (rs03 → 5.0 Nm; rs02/rs00 → 3.0 Nm).

### Motor-type velocity defaults (`control.yaml`, fallback only)

No per-joint `velocity_max_rad_s` overrides exist; groups win. Defaults if a joint were ungrouped: rs03/rs02 → 2.0 rad/s; rs00 → 2.0 rad/s.

---

## Danger zones

| Name | Joint | Condition | Action | Parameter |
|------|-------|-----------|--------|-----------|
| `elevated_shoulder_pitch_fall` | `right_shoulder_pitch` | `q > 0.5` rad **and** `dq < −0.1` rad/s | `clamp_velocity` | `max_velocity_rad_s: 0.45` |

Evaluated on **measured** `q` / `dq` ([ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md) — unchanged by envelope layer). Descent from elevated pitch is capped to 0.45 rad/s (human-paced return vs prior 0.05 rad/s clamp).

No danger-zone rules apply to roll, yaw, elbow, or lower-arm yaw in the current master config.

---

## Provisional / commissioning notes

### DOF 4 — `right_elbow_pitch`

- Bench hard/soft from Set Limits pass (2026-07-22 cohort).
- `motors.yaml` comment: **direction provisional** — verify motor vs URDF +Y with low-gain jog after set-zero.
- `control.yaml`: conservative first-light impedance (kp 12 / kd 1.5); retune after sign/zero verify.
- Lower command velocity ceiling (1.5 rad/s group; 1.0 rad/s in `motors.yaml` bench field).

### DOF 5 — `right_lower_arm_yaw`

- **Not** from measured Set Limits: hard `[−1.6, 1.6]` rad is a **kinematics envelope** (`motors.yaml` header).
- Soft inset is 50 mrad per side (vs ~27 mrad on DOF 1–4) — reflects provisional envelope, not a taught stop.
- `firmware_version: "0.0.0-pending"`; direction provisional (same jog-verify note as elbow).
- Limits confirm for DOF 5 should treat ROM as **unverified** until a bench Set Limits pass replaces the kinematics placeholder.

### DOF 2 — `right_shoulder_roll` (special hard lower)

- Taught hard lower stays at **−0.05 rad** (URDF value), not flush to measured min, so home pose ~−0.01 rad enables ([`motors.yaml`](../../config/motors.yaml) header; [`control.yaml`](../../config/control.yaml) roll comment).
- Soft lower −0.023 rad; hard/soft gap on lower side is still ~27 mrad.

---

## Implications for Limits confirm

Playbook stance from map #150: **verify readback + near-limit probe; re-teach only on failure.**

| Check | What to verify against this snapshot |
|-------|--------------------------------------|
| Hard readback | Davout/Consul reported hard matches `motors.yaml` bench positions (± rounding). Effective hard may be tighter if URDF `<limit>` is inside taught bench ([ADR 0017](../decisions/0017-bench-set-limits-urdf-expand.md)). |
| Soft readback | `control.yaml` soft bounds per joint table above. |
| Near-limit probe | Approach soft limits at low speed; expect velocity-scaled envelope to stop short of soft on fast legs ([ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md)). Fault only if measured `q` exceeds hard ± **30 mrad** slack. |
| DOF 1–4 | Treat as **signed-off taught ROM** unless probe fails → re-teach Set Limits. |
| DOF 5 | Treat as **provisional** — confirm encoder/sign/firmware first; ROM bar is kinematics placeholder until Set Limits. |
| Roll enable margin | Do not re-teach roll lower to measured min without understanding home-enable requirement at ~−0.01 rad. |
| Pitch descent | With `q > 0.5`, downward motion is clamped to 0.45 rad/s — not a limit fault; confirm during near-limit descent tests. |

---

## Implications for position speed ladder

Authoritative **MIT command velocity ceiling** per joint = actuator-group cap in table above. Trajectory cruise speeds are lower and joint-specific.

Suggested ladder rungs (factual baseline from config; playbook may formalize gates):

| Rung | Pitch / roll cap | Upper-arm yaw cap | Elbow / lower-arm yaw cap | Notes |
|------|------------------|-------------------|---------------------------|-------|
| Crawl | ≪ traj cruise | ≪ traj cruise | ≪ traj cruise | Envelope `margin_min` 0.01 rad dominates |
| Traj cruise | 1.25 / 0.70 rad/s | 1.00 rad/s | 0.80 rad/s | `position_trajectory_velocity_rad_s` |
| Group MIT cap | **2.5** rad/s | **2.0** rad/s | **1.5** rad/s | `resolve_joint_velocity_cap` ceiling |
| Near-limit stress | Same caps, probe soft walls | Same | Same | Expect envelope shrink; 30 mrad slack at hard |
| Pitch elevated descent | **0.45** rad/s max when `q>0.5` & falling | — | — | Danger zone, not a ladder cap |

`robot.bench.max_joint_velocity_rad_s: 2.0` is a robot-wide bench metadata cap; shoulder groups at 2.5 rad/s exceed it on paper — ladder should still respect per-joint group caps and trajectory tuning, not `motors.yaml` bench velocity fields.

**Near-limit stress:** Use traj cruise or lower on final approach; full group cap on interior segments. Fast single-leg moves to soft limit may not reach soft in one leg ([ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md)) — that is expected, not a commissioning failure.

---

## Source citations

| Claim | Source |
|-------|--------|
| Taught bench hard positions | [`config/motors.yaml`](../../config/motors.yaml) `motors[].bench.position_*_rad` |
| Operator soft positions | [`config/control.yaml`](../../config/control.yaml) `control.joints.*.position_soft_*_rad` |
| Velocity caps (MIT) | [`config/control.yaml`](../../config/control.yaml) `control.actuator_groups.*.velocity_max_rad_s` |
| Trajectory cruise / accel / slew | [`config/control.yaml`](../../config/control.yaml) per-joint `position_trajectory_*`, `position_slew_*` |
| Danger zone | [`config/control.yaml`](../../config/control.yaml) `control.danger_zones` |
| Limb membership | [`config/robot.yaml`](../../config/robot.yaml) `robot.limbs.right_arm` |
| Hard = URDF ∩ bench; soft inset; measured slack | [ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md) |
| Set Limits 2026-07-22; URDF expand-only | [ADR 0017](../decisions/0017-bench-set-limits-urdf-expand.md), `motors.yaml` header |
