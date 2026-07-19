# Position slew audit (`position_slew_rad_s`)

Explore artifact for Phase 2 item 3. **No YAML changes** — documentation only.

## Summary

| Question | Answer |
|----------|--------|
| Is `15.0` a typo for `0.15`? | **Likely yes** — see [Verdict](#verdict). |
| Does `15.0` mean the joint runs at 15 rad/s? | **No.** Berthier clamps planner `v_max` to the rs03 velocity cap (2.0 rad/s). |
| What does `15.0` actually do vs `0.15`? | For small retargets (≤ ~60 mrad), `15.0` effectively disables the small-move cap and allows **2.0 rad/s** (~13× faster than `0.15`). |
| Safe to fix in YAML now? | **No** — require bench evidence on each profile before changing. |

## Config inventory

All `position_slew_rad_s` values in repo configs (2026-06-16):

| File | Joint(s) | `position_slew_rad_s` | `position_slew_max_lead_rad` | Trajectory fields present? |
|------|----------|----------------------:|-----------------------------:|----------------------------|
| `config/control.yaml` | `shoulder_pitch` | **15.0** | 0.5 | No |
| `config/bringup/arm_4dof_left/control.yaml` | `shoulder_pitch` | **15.0** | 0.5 | No |
| `config/bringup/shoulder_pitch_weighted/control.yaml` | `left_shoulder_pitch`, `right_shoulder_pitch` | **15.0** (both) | 0.5 | No |
| `config/bringup/shoulder_pitch_dual/control.yaml` | `left_shoulder_pitch`, `right_shoulder_pitch` | **15.0** (both) | 0.5 | No |
| `config/bringup/shoulder_pitch_left_only/control.yaml` | `left_shoulder_pitch` | **0.15** | 0.10 | Yes (`v=1.0`, `a=6.0`) |
| `config/bringup/arm_3dof_right/control.yaml` | `right_shoulder_pitch` | **0.15** | 0.10 | Yes (`v=2.0`, `a=4.8`) |

**Serde defaults** (`crates/marengo-config/src/lib.rs`): `position_slew_rad_s = 0.25`, `position_trajectory_velocity_rad_s = 0.30`, `position_trajectory_accel_rad_s2 = 0.20`.

**Documented targets:**

| Source | Stated value | Context |
|--------|-------------|---------|
| [ADR 0007](decisions/0007-bench-position-trajectory-control.md) | **0.15** | Small retargets (≤ ~60 mrad); right-only bench table |
| [docs/tuning.md](tuning.md) | **0.25** | Default when field omitted |
| [docs/bench-position-tuning.md](bench-position-tuning.md) | **0.15** | Active right-only small-move cap |
| [docs/rust-patterns.md](rust-patterns.md) §7 | small-move cap | Moves ≤ ~60 mrad use `position_slew_rad_s`; larger use trajectory `v` |

## Code path: how slew is applied

Berthier (`crates/berthier/src/loop.rs`) splits hold-at motion by move distance:

```text
POSITION_SMALL_MOVE_VMAX_RAD = 0.06 rad  (~3.4°)
```

### Small moves (|target − q| ≤ 0.06 rad)

- `advance_position_commands`: `v_max = clamp_v_max(joint, position_slew_rad_s)`
- `clamp_v_max` applies `Supervisor::joint_velocity_cap` (rs03 → **2.0 rad/s** from `control.yaml` motor type / actuator group)
- Trajectory planner integrates `q_ref` at this capped rate
- Also used for: downward-return seed rate, limit-envelope `dq_cmd` estimate, stuck-lead resync seed

**Effective small-move speed:**

| Config value | After velocity cap | Wall time for 0.06 rad move |
|-------------:|-------------------:|----------------------------:|
| 0.15 | 0.15 rad/s | ~0.4 s |
| 15.0 | **2.0 rad/s** (cap wins) | ~0.03 s |

### Large moves (|target − q| > 0.06 rad)

- `v_max = clamp_v_max(joint, position_trajectory_velocity_rad_s)` (default **0.30** if omitted)
- `a_max = position_trajectory_accel_rad_s2` (default **0.20**)
- Profiles with `15.0` slew but **no** trajectory fields therefore use serde defaults for large moves — not the 2.0 rad/s tuned on `arm_3dof_right`

### MIT setpoint layer

- `q_des = clamp(q_traj, q, target, position_slew_max_lead_rad)` each tick
- `15.0` profiles also carry `max_lead = 0.5` vs **0.10** on corrected bench profiles — a second divergence from ADR 0007 tuning

## Physical sanity (rs03 shoulder pitch)

| Quantity | Value |
|----------|------:|
| rs03 `velocity_max_rad_s` (config) | 2.0 rad/s (~115°/s) |
| `15.0` rad/s if uncapped | ~859°/s — **not physically achievable** on rs03 |
| ADR 0007 accepted small-move rate | 0.15 rad/s (~8.6°/s) |
| Typical bench `hold-at 0.1 rad` workflow | tuned at 0.15–0.25 rad/s |

**Conclusion:** A literal 15 rad/s setpoint is not sane for this actuator. The velocity cap prevents wire insanity, but the **intent** of the field (gentle small retargets) is defeated when the value exceeds the cap.

## Git history

| Commit | Change |
|--------|--------|
| `c954510` *Mirror shoulder pitch control tuning across profiles* | Introduced `position_slew_rad_s: 15.0` + `max_lead: 0.5` into `config/control.yaml`, weighted, dual, arm_4dof_left (comment: *"ramp setpoint at this rate (rad/s) instead of stepping instantly"*) |
| `cd86191` *Add actuator velocity guard and MCP fixes* | **right_only:** `15.0 → 0.25`, comment *"keep scripted moves within the bench speed cap"*; added trajectory tuning path |
| `6a74cdd` *refactor(config): control.yaml sole velocity cap source* | **left_only:** `15.0 → 0.15` + trajectory fields |

**Pattern:** Bench-validated profiles (`right_only`, `left_only`) were explicitly corrected away from `15.0`. Generic/mirror profiles were not updated.

## Verdict

**Likely typo (or stale mirror of pre-trajectory tuning), not intentional production tuning.**

Evidence:

 1. **Corrected profiles exist** — `arm_3dof_right` and `shoulder_pitch_left_only` were deliberately changed from `15.0` to `0.15` with bench-oriented comments.
2. **Documentation consensus** — ADR 0007, bench tuning notes, and test fixtures in `marengo-config` all use **0.15** for small moves.
3. **100× magnitude** — `15.0` vs `0.15` matches a classic decimal-place error; no ADR or commit message argues that small retargets should run at cap speed.
4. **Correlated stale fields** — profiles still at `15.0` also retain `max_lead: 0.5` and lack trajectory blocks, consistent with an unmaintained copy from `c954510`.

**Counter-evidence (weak):** Original comment framed `15.0` as "fast ramp instead of instant step" — plausible as a pre-trajectory placeholder, but superseded once ADR 0007 trapezoid planner and velocity caps landed. No bench log shows `15.0` was re-validated after the planner split.

## Recommendation

1. **Do not change YAML without bench evidence.** Wrong small-move cap can cause velocity trips, stick-slip, or harsh retargets on loaded arms.
 2. **Use `config/bringup/arm_3dof_right/control.yaml` as the reference** for shoulder pitch bring-up:
   - `position_slew_rad_s: 0.15`
   - `position_slew_max_lead_rad: 0.10`
   - `position_trajectory_velocity_rad_s: 2.0`, `position_trajectory_accel_rad_s2: 4.8`
3. **Before aligning stale profiles**, run on each target profile:
   - `hold-at 0.1 rad` small push/return (visual + `fault=0`)
   - Scripted sweep if profile is used for large moves (weighted/dual)
   - Compare CAN velocity trace vs `position_slew_rad_s` expectation
4. **If fixing:** treat as a paired update — slew, `max_lead`, and trajectory fields together, mirroring `right_only` / ADR 0007 table.

## Related docs

- [ADR 0007 — bench position trajectory control](decisions/0007-bench-position-trajectory-control.md)
- [ADR 0010 — actuator velocity cap resolution](decisions/0010-actuator-velocity-cap-resolution.md)
- [bench-position-tuning.md](bench-position-tuning.md)
- [rust-patterns.md §7](rust-patterns.md)
