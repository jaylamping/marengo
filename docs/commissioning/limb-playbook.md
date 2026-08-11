# Limb commissioning playbook

**Status:** SoT procedure spine (hybrid prose + harness hooks).  
**Map:** [Limb commissioning playbook](https://github.com/jaylamping/marengo/issues/150)  
**Instance (example fill):** `limb = right_arm`  
**Artifact shape:** this markdown spine owns order + exit criteria; named harness/analyzer checks plug in where noted.

Read [docs/safety.md](../safety.md) before any enable or elevated pose. Motion tools require explicit confirm; support the arm on first enable at elevated poses.

---

## Parameters (per limb)

| Param | Purpose | Example (`right_arm`) |
|-------|---------|------------------------|
| `limb` | Anatomical group from `robot.yaml` | `right_arm` |
| `joints[]` | Online / motor-mapped members of `limb` | `right_shoulder_roll`, `right_shoulder_pitch`, `right_upper_arm_yaw`, `right_elbow_pitch`, `right_lower_arm_yaw` |
| `taught_envelope` | Soft/hard from Set Limits (live config) | DOF1–4 taught 2026-07-22 (~27 mrad soft inset); DOF5 kinematics envelope until re-teach |
| `commissioning_velocity_baseline` | Manual @ bus voltage, derated; **write into config before Position ladder** | RS03 **9.4** / RS02 **19.3** / RS00 **14.8** rad/s @ 24 V → pitch·roll 9.4, yaw·elbow 19.3, lower-arm yaw 14.8 |
| `gcomp_poses` | Three-band static poses within taught envelope | TBD — arm-down / mid / elevated joint angles per limb |
| `standard_payload` | Tip-mounted Limb-standard payload | Assembled **0.5–0.8 kg**, tip/distal mount; weigh every attach |
| `torque_only_tau_cmd` | Open-loop step magnitudes / dwells | TBD — fill before TorqueOnly chapter runs |

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

---

## 5. Position speed ladder

**Before start:** write `commissioning_velocity_baseline` into runtime config (trajectory cruise and related velocity caps as needed) so the robot matches this chapter. Baselines are SoT for the ladder — not whatever cruise/caps happen to be in YAML today.

### 5a. Interior ladder

Rungs: **25% / 50% / 75% / 100%** of each joint’s commissioning baseline.

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
| Soft approach | per-joint toward soft at **25–50%** baseline; safe envelope clamp / limit reaction without fault latch = **pass**; fault latch or runaway = **fail**; controlled interior return | _TODO: ladder_near_limit_ |

---

## 6. Impedance

| Gate | Criterion | Harness |
|------|-----------|---------|
| Multi-joint poses | mid + elevated bands within taught envelope | _TODO: impedance_poses_ |
| Push / backdrive / hold | springy/damped; returns without runaway; light dwell ≈5–10 s, drift `< 0.05 rad` / joint | manual + _TODO_ |
| Mode switch | into/out of Impedance; kp ramp / no τ spike | _TODO: mode_switch_clean_ |

---

## 7. TorqueOnly

Un-aliased semantics: `τ_ff = τ_cmd` only (no `τ_g` / friction), hard-zero kp/kd, `q_des = q`, `v_des = 0`. Operator latch via `torque-cmd` / Testing overlay (default 0; cleared on leave). `gravity-off` enters TorqueOnly with `τ_cmd ≡ 0`. Davout caps apply; wrong-sign watchdog remains GravityComp-only. Elevated TorqueOnly only with physical support.

| Gate | Criterion | Harness |
|------|-----------|---------|
| τ_cmd steps | bidirectional low open-loop steps at arm-down using `torque_only_tau_cmd`; correct sign; caps respected; no runaway | _TODO: torque_only_steps_ |
| No-τ_g contrast | mid-pose: does **not** auto-inject full `τ_g` | _TODO_ |
| Disabled hygiene | clean enter/exit of `Disabled` | _TODO_ |

---

## 8. Payload critical gates

**Job:** safety under load — dynamics honesty **and** envelope/motion stress with tip mass; not a full mode re-commission.

1. Assemble Limb-standard payload (tip/distal, **0.5–0.8 kg** band)
2. Weigh on a scale (**every attach**); record kg + mount note
3. Write mass into active URDF/config → sync to Pi
4. Run critical subset below
5. Detach fixture and **restore unweighted model** (do not leave weighted mass live)

| Gate | Criterion | Harness |
|------|-----------|---------|
| Coupled G-comp | same procedure + bars as ch. 4b (three-band + float + spot-checks) | _TODO: payload_coupled_gcomp_ |
| Position @ 50% | full rung shape: per-joint interior sweeps **and** multi-joint path; same trip classification as ch. 5a | _TODO: payload_ladder_50_ |
| Near-limit @ 25% | per-joint soft approach at 25% baseline; clamp-without-fault = pass; fault latch / runaway = fail | _TODO: payload_near_limit_ |

**Out of payload critical:** Limits confirm, Sign, per-joint G-comp, Impedance, TorqueOnly, Disabled hygiene, full 25/50/75/100% ladder.

**Fail policy:** any gate fail ⇒ payload chapter fail; re-weigh / re-sync and retry allowed; no relaxed pass bars under load.

---

## 9. Limb sign-off

| Gate | Criterion |
|------|-----------|
| Checklist complete | chapters 1–8 green for this `limb` |
| Record | date, git rev, commissioning baselines written, payload mass + mount note, operator |

Harness: _TODO: limb_signoff_bundle_

---

## Clean slate

Fragmented `docs/bench-*.md` procedure/evidence/backlog files were retired by clean-slate cutover when this SoT landed ([Decide how to retire superseded bench suites](https://github.com/jaylamping/marengo/issues/160)). Do not restore them as authority; Git history remains for archaeology only.
