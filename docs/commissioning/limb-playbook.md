# Limb commissioning playbook

**Status:** SoT procedure spine (hybrid prose + harness hooks).  
**Map:** [Limb commissioning playbook](https://github.com/jaylamping/marengo/issues/150)  
**Instance (example fill):** `limb = right_arm`  
**Artifact shape:** this markdown spine owns order + exit criteria; named harness/analyzer checks plug in where noted.

Read [docs/safety.md](../safety.md) before any enable or elevated pose. Motion tools require explicit confirm; support the arm on first enable at elevated poses.

---

## Parameters (per limb)

Values below are **starting defaults for this instance** — edit the tables (or override for a single run) if a gate needs a safer / stronger / better-coupled pose or step. Chapter gates reference these names; they do not hard-code magnitudes in the criterion text.

Joint vector order everywhere:  
`[right_shoulder_roll, right_shoulder_pitch, right_upper_arm_yaw, right_elbow_pitch, right_lower_arm_yaw]` (rad).

| Param | Purpose | Example (`right_arm`) |
|-------|---------|------------------------|
| `limb` | Anatomical group from `robot.yaml` | `right_arm` |
| `joints[]` | Online / motor-mapped members of `limb` | `right_shoulder_roll`, `right_shoulder_pitch`, `right_upper_arm_yaw`, `right_elbow_pitch`, `right_lower_arm_yaw` |
| `taught_envelope` | Soft/hard from Set Limits (live config) + MIT/safety caps | DOF1–4 taught 2026-07-22 (~27 mrad soft inset); DOF5 kinematics envelope until re-teach; MIT caps 2.5/2.5/2.0/1.5/1.5 rad/s; pitch `elevated_shoulder_pitch_fall` (0.45 rad/s descent) |
| `commissioning_velocity_baseline` | Manual @ bus voltage, derated — **reference only** for sizing ladder rungs | RS03 **9.4** / RS02 **19.3** / RS00 **14.8** rad/s @ 24 V → pitch·roll 9.4, yaw·elbow 19.3, lower-arm yaw 14.8 |
| `gcomp_poses` | Three-band static poses within taught envelope | See defaults below (full 5-DOF; pitch-banded) |
| `wave_pose` | Elevated multi-joint hold for §4c Wave-pose G-comp (commissioning pose) | See defaults below — **not** tied to the current Consul Wave preset |
| `standard_payload` | Tip-mounted Limb-standard payload | Assembled **0.5–0.8 kg**, tip/distal mount; weigh every attach |
| `torque_only_tau_cmd` | Open-loop step magnitudes / dwells / order | See defaults below (~10% Davout limit per motor class) |

### `gcomp_poses` defaults (`right_arm`)

| Band | `q` (rad) |
|------|-----------|
| arm-down | `[0.15, 0.00, 0.00, 0.00, 0.00]` |
| mid | `[1.20, 1.00, 0.00, 0.35, 0.00]` |
| elevated | `[1.80, 2.50, 0.00, 0.60, 0.00]` |

Stay inside `taught_envelope` soft limits (~0.15 rad margin preferred). Arm-down roll is `0.15` so soft-lower margin meets that preference (soft lo ≈ `-0.023`). Support the arm on first enable at elevated.

### `wave_pose` defaults (`right_arm`)

| Field | Default |
|-------|---------|
| `q` (rad) | `[0.42, 2.75, 0.00, 0.95, 0.00]` |

Clamped from an older Wave raise sketch into taught soft (~0.15 pitch / ~0.06 elbow margin). Distinct from elevated `gcomp_poses` (different roll/elbow). Tunable — edit if the hold needs a safer or better-coupled posture. **Do not** treat the shipped Consul `compound-tests` Wave raise as SoT (poor 2–3 DOF example; left untouched).

### `torque_only_tau_cmd` defaults (`right_arm`)

| Field | Default |
|-------|---------|
| RS03 (pitch, roll) `|τ_cmd|` | **0.50 Nm** (bidirectional ±) |
| RS02 / RS00 (upper yaw, elbow, lower yaw) `|τ_cmd|` | **0.30 Nm** (bidirectional ±) |
| Dwell | **2 s** per polarity |
| Waveform | open-loop **step** (not sine) |
| Pose for steps | **arm-down** `gcomp_poses` |
| Isolation | **One joint at a time.** Before each step: all other joints `τ_cmd = 0` (clear latches — `torque-cmd` sticks until cleared / leave TorqueOnly). After each polarity: return that joint to `0` and settle before the next joint. |
| Joint order | **distal → proximal:** lower_arm_yaw → elbow → upper_arm_yaw → roll → pitch |
| Mid contrast | mid `gcomp_poses`, **`τ_cmd = 0` on all joints**; **continuous physical support required** (pitch ≥ `elevated_shoulder_pitch_fall` / 0.5 rad — do not release) |

Not an industry-canon magnitude — conservative Marengo starting point. Lower `|τ_cmd|` or dwell if motion is too brisk; if sign is unclear, stop and re-run §3 Sign (do not raise open-loop torque to “force” a sign). Keep Davout caps respected.

---

## Preconditions (every chapter)

- [ ] `pi_health` / CAN up / deploy rev matches intended git
- [ ] Homing **Verified** → Joint Ready for all `joints[]` in scope → Limb Ready (or explicit commissioning scope)
- [ ] `fault=0`; operator support rules for elevated enable
- [ ] Harness hook (gap): `preflight_limb` — _TODO_

---

## 1. Reference

**Goal:** firmware zero + calibration → Joint Ready for each joint in `joints[]`.

| Gate | Criterion | Harness |
|------|-----------|---------|
| Set Zero | Durable Hardware Set Zero + sign attestation; not while Active | manual / Consul Hardware |
| Ready facet | reference = Ready for each online joint in `joints[]` | _TODO: ready_audit_ |

---

## 2. Limits confirm

**Goal:** taught envelope is live before motion stress.

| Gate | Criterion | Harness |
|------|-----------|---------|
| Readback | soft/hard match config vs Davout effective (URDF ∩ taught hard) | _TODO: limits_readback_ |
| Near-limit probe | mandatory soft-band approach per joint; **re-teach only on failure** | _TODO: near_limit_probe_ |

**Provisional envelopes** (e.g. DOF5 kinematics-only) must be called out in `taught_envelope` and treated as unverified until a Set Limits pass replaces them.

---

## 3. Sign

**Goal:** direction correct before full τ_g.

| Gate | Criterion | Harness |
|------|-----------|---------|
| Sign pulse | small τ_ff / motion matches URDF sense for each joint | _TODO: sign_pulse_ |

---

## 4. Gravity compensation

Unweighted model. Sign (ch. 3) is a hard prerequisite. Elevated release must not free-fall.

### 4a. Per-joint

Per online joint in `joints[]`: low-risk sign/magnitude honesty under `GravityComp` before limb coupling.

| Gate | Criterion | Harness |
|------|-----------|---------|
| Per-joint G-comp | direction + magnitude credible vs `τ_g`; no runaway | _TODO: per_joint_gcomp_ |

### 4b. Coupled (limb)

All online `joints[]` in `GravityComp`.

| Gate | Criterion | Harness |
|------|-----------|---------|
| Three-band static | at least one pose each in arm-down / mid / elevated (`gcomp_poses`) | _TODO: coupled_gcomp_static_ |
| Residuals | per-joint `\|τ_meas − τ_g\| < 0.20 Nm`; limb RMS residual `< 5%` of peak `\|τ_g\|` for that pose | analyzer _TODO_ |
| Dwell | ≈5–10 s; no joint drift `> 0.05 rad` without operator input | _TODO_ |
| Hand float | neutrally backdrivable through taught envelope; no fault/watchdog/runaway; 2–3 paused residual spot-checks | manual + spot harness |

Campaign shape is static multi-joint poses + float (not scripted multi-joint GravityComp trajectories — those belong to the Position ladder).

### 4c. Wave-pose G-comp (commissioning hold)

**Job:** prove GravityComp at the elevated multi-joint `wave_pose` (pitch/roll/yaw/elbow raise band). This is a **commissioning hold**, not automatic unlock of live Consul Wave.

Prerequisite: §4b coupled G-comp green (or at least elevated-band residuals green for the Wave joint set).

| Gate | Criterion | Harness |
|------|-----------|---------|
| Wave-pose hold | At `wave_pose`, under `GravityComp`, with initial operator support then careful release per [safety.md](../safety.md) | _TODO: wave_pose_gcomp_ |
| Residuals | same bars as §4b at that pose | analyzer _TODO_ |
| No free-fall | elevated release does not runaway / fault / watchdog-trip | manual + logs |

Record: date, git rev, `wave_pose` joint angles, residual summary.

**Live Consul Wave** (`WAVE_POSE_GCOMP_SIGNED`): keep **`false`** until a redesigned Wave gesture exists and is signed against its own raise pose. Passing §4c alone does **not** flip the flag. Do not retarget the current shipped Wave preset as part of this gate.

---

## 5. Position speed ladder

**Speed law (mandatory):** effective rung speed for each joint is

`min(% × commissioning_velocity_baseline, live safety ceiling)`

where **live safety ceiling** is the tightest of current `velocity_max_rad_s` (MIT/Davout), configured trajectory cruise ceilings, and danger-zone clamps (e.g. `elevated_shoulder_pitch_fall`).

- `commissioning_velocity_baseline` sizes the ladder; it is **not** a license to raise safety caps in this chapter.
- **Do not** raise `velocity_max_rad_s` / MIT caps / danger-zone limits as part of the Position ladder. Cap changes are a separate Limits/caps commission.
- If `% × baseline` exceeds the live ceiling, run the rung at the ceiling and record it as **cap-limited** (still must pass trip class).
- **Before each rung:** write `position_trajectory_velocity_rad_s` (and related cruise fields) to the **effective** rung speed, sync config (`pi_sync_bench_config`), confirm readback.

### 5a. Interior ladder

Rungs: **25% / 50% / 75% / 100%** of each joint’s commissioning baseline, then min’d with the live safety ceiling (above).

At each rung:

| Gate | Criterion | Harness |
|------|-----------|---------|
| Per-joint sweeps | substantial interior taught-ROM fraction; others hold | _TODO: ladder_per_joint_ |
| Multi-joint path | coordinated interior trajectory | _TODO: ladder_multi_joint_ |
| Trip class | **interior of soft envelope:** any fault / OutOfLimits / watchdog / danger-zone latch = **fail** | log classifier _TODO_ |

### 5b. Near-limit stress

After the interior ladder:

| Gate | Criterion | Harness |
|------|-----------|---------|
| Soft approach | per-joint toward soft at **25–50%** of baseline (then min’d with live safety ceiling); safe envelope clamp / limit reaction without fault latch = **pass**; fault latch or runaway = **fail**; controlled interior return | _TODO: ladder_near_limit_ |

### 5c. Cross-axis hold (yaw / isolation DOFs)

For joints expected to move with neighbors held (example: `right_upper_arm_yaw` with pitch/roll/elbow held):

| Gate | Criterion | Harness |
|------|-----------|---------|
| ±50 mrad hold | command a ±50 mrad step (or hold band) about the reference; settle inside ±50 mrad of command | _TODO: cross_axis_hold_ |
| Cross-talk | non-commanded joints (esp. shoulder pitch) drift `< 50 mrad` peak during the hold window; review `position-trace-latest.csv` + `candump` | operator + trace |
| Trip class | same as §5a interior | logs |

Harness smoke (`yaw_attached`, etc.) is **not** this gate; smoke `pass_kind=smoke` ≠ commissioning complete.

---

## 6. Impedance

| Gate | Criterion | Harness |
|------|-----------|---------|
| Multi-joint poses | mid + elevated bands within taught envelope | _TODO: impedance_poses_ |
| Push / backdrive / hold | springy/damped; returns without runaway; light dwell ≈5–10 s, drift `< 0.05 rad` / joint | manual + _TODO_ |
| Mode switch | into/out of Impedance; kp ramp / no τ spike | _TODO: mode_switch_clean_ |

---

## 7. TorqueOnly

Un-aliased semantics: `τ_ff = τ_cmd` only (no `τ_g` / friction), hard-zero kp/kd, `q_des = q`, `v_des = 0`. Operator latch via `torque-cmd` / Testing overlay (default 0; cleared on leave). `gravity-off` enters TorqueOnly with `τ_cmd ≡ 0`. Davout caps apply; wrong-sign watchdog remains GravityComp-only.

**Support:** any TorqueOnly hold with shoulder pitch ≥ `elevated_shoulder_pitch_fall` (`0.5` rad) — including mid contrast and elevated band — requires **continuous physical support**. Do not release under zero-FF TorqueOnly.

**Latch hygiene:** `τ_cmd` is sticky per joint. Campaigns must keep **one joint active**; clear / zero all others before each step.

| Gate | Criterion | Harness |
|------|-----------|---------|
| τ_cmd steps | bidirectional open-loop steps at arm-down per `torque_only_tau_cmd` (magnitudes, dwell, **one joint at a time**, clear others, distal→proximal order); correct sign; caps respected; no runaway | _TODO: torque_only_steps_ |
| No-τ_g contrast | mid `gcomp_poses` with **all** `τ_cmd = 0`, **arm supported**: does **not** auto-inject full `τ_g` | _TODO_ |
| Disabled hygiene | clean enter/exit of `Disabled` | _TODO_ |

---

## 8. Payload critical gates

**Job:** safety under load — dynamics honesty **and** envelope/motion stress with tip mass; not a full mode re-commission.

1. Assemble Limb-standard payload (tip/distal, **0.5–0.8 kg** band)
2. Weigh on a scale (**every attach**); record kg + mount note (lever / COM offset)
3. Update the **active** URDF tip mass **and** mount/COM for that fixture (not YAML-only; do not edit archived `assets/urdf/archive/seed-*` unless promoting into the live tree)
4. Sync URDF to Pi with **`pi_sync_bench_urdf`** (`install_to_opt: true`) — **`pi_sync_bench_config` does not push URDF/meshes**
5. Verify gravity against the tip load (`pi_gravity_preview` and/or supported hold) **before** running gates
6. Run critical subset below
7. Detach fixture; restore unweighted URDF/COM; **`pi_sync_bench_urdf` again**; re-verify unweighted gravity before declaring the chapter done (do not leave weighted mass live)

| Gate | Criterion | Harness |
|------|-----------|---------|
| Coupled G-comp | same procedure + bars as ch. 4b (three-band + float + spot-checks) | _TODO: payload_coupled_gcomp_ |
| Position @ 50% | full rung shape at **effective** 50% speed (§5 speed law): per-joint interior sweeps **and** multi-joint path; same trip classification as ch. 5a | _TODO: payload_ladder_50_ |
| Near-limit @ 25% | per-joint soft approach at effective 25% speed; clamp-without-fault = pass; fault latch / runaway = fail | _TODO: payload_near_limit_ |

**Out of payload critical:** Limits confirm, Sign, per-joint G-comp, Wave-pose unlock, Impedance, TorqueOnly, Disabled hygiene, full 25/50/75/100% ladder.

**Fail policy:** any gate fail ⇒ payload chapter fail; re-weigh / re-sync / re-verify gravity and retry allowed; no relaxed pass bars under load.

---

## 9. Limb sign-off

| Gate | Criterion |
|------|-----------|
| Checklist complete | chapters 1–8 green for this `limb` (including §4c Wave-pose if Wave is in scope) |
| Record | date, git rev, effective ladder speeds / cap-limited rungs, payload mass + mount note, Wave unlock, operator |

Harness: _TODO: limb_signoff_bundle_

---

## Clean slate

Fragmented `docs/bench-*.md` procedure/evidence/backlog files were retired by clean-slate cutover when this SoT landed ([Decide how to retire superseded bench suites](https://github.com/jaylamping/marengo/issues/160)). Do not restore them as authority; Git history remains for archaeology only.
