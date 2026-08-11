<!--
  PROTOTYPE / THROWAWAY — not the locked SoT playbook.
  Wayfinder: https://github.com/jaylamping/marengo/issues/157
  Map: https://github.com/jaylamping/marengo/issues/150
  Purpose: react to shape (spine, params, gate placeholders, harness hooks)
  before locking path/outline in #158.
-->

# Limb commissioning playbook

**Status:** PROTOTYPE STUB — do not run as procedure yet.  
**Instance (example fill):** `limb = right_arm`  
**Artifact shape:** hybrid — this prose spine owns order + exit criteria; named harness/analyzer checks plug in where noted.

---

## Parameters (per limb)

| Param | Purpose | Example (`right_arm`) |
|-------|---------|------------------------|
| `limb` | Anatomical group from `robot.yaml` | `right_arm` |
| `joints[]` | Online / motor-mapped members | pitch, roll, upper-arm yaw, elbow, lower-arm yaw |
| `taught_envelope` | Soft/hard from Set Limits | DOF1–4 taught 2026-07-22; DOF5 provisional until re-teach |
| `commissioning_velocity_baseline` | Manual @ bus voltage, derated; written to config before Position ladder | RS03 9.4 / RS02 19.3 / RS00 14.8 rad/s @ 24 V (research) |
| `gcomp_poses` | Three-band static poses | TBD — arm-down / mid / elevated joint angles |
| `standard_payload` | Mass + mount for payload chapter | TBD — [Choose standard payload fixture](https://github.com/jaylamping/marengo/issues/159) |
| `torque_only_tau_cmd` | Open-loop step magnitudes | TBD — after [Design un-aliased TorqueOnly semantics](https://github.com/jaylamping/marengo/issues/163) |

---

## Preconditions (every chapter)

- [ ] `pi_health` / CAN up / deploy rev matches
- [ ] Homing **Verified** → Joint Ready for all `joints[]` in scope → Limb Ready (or explicit commissioning scope)
- [ ] `fault=0`; operator support rules for elevated enable
- [ ] Harness hook (gap): `preflight_limb` — _TODO_

---

## 1. Reference

**Goal:** firmware zero + calibration → Joint Ready for each joint in `joints[]`.

| Gate | Criterion | Harness |
|------|-----------|---------|
| Set Zero | Durable Hardware Set Zero + sign attestation; not while Active | manual / Consul Hardware |
| Ready facet | reference = Ready for each online joint | _TODO: ready_audit_ |

---

## 2. Limits confirm

**Goal:** taught envelope is live before motion stress.

| Gate | Criterion | Harness |
|------|-----------|---------|
| Readback | soft/hard match config vs Davout effective | _TODO: limits_readback_ |
| Near-limit probe | mandatory soft-band approach per joint; re-teach only on failure | _TODO: near_limit_probe_ |

---

## 3. Sign

**Goal:** direction correct before full τ_g.

| Gate | Criterion | Harness |
|------|-----------|---------|
| Sign pulse | small τ_ff / motion matches URDF sense | existing sign scripts / suite fragments |

---

## 4. Gravity compensation

### 4a. Per-joint

Absorb existing single-joint sign/magnitude suites (pitch T1 spirit, per-DOF sign).  
Harness: fragmented today — _TODO: unify pointers_.

### 4b. Coupled (limb)

All online `joints[]` in `GravityComp` (unweighted).

| Gate | Criterion | Harness |
|------|-----------|---------|
| Three-band static | arm-down, mid, elevated (`gcomp_poses`) | _TODO: coupled_gcomp_static_ |
| Residuals | per-joint `\|τ_meas−τ_g\| < 0.20 Nm`; limb RMS `< 5%` peak `\|τ_g\|` | analyzer _TODO_ |
| Dwell | ≈5–10 s; drift `< 0.05 rad` / joint | _TODO_ |
| Hand float | neutrally backdrivable; no fault/runaway; 2–3 paused residual spot-checks | manual + spot harness |

---

## 5. Position speed ladder

**Before start:** write `commissioning_velocity_baseline` into config (traj cruise / caps as needed).

### 5a. Interior ladder

Rungs: **25% / 50% / 75% / 100%** of baseline.

At each rung:

| Gate | Criterion | Harness |
|------|-----------|---------|
| Per-joint sweeps | substantial interior taught-ROM fraction; others hold | _TODO: ladder_per_joint_ |
| Multi-joint path | coordinated interior trajectory | _TODO: ladder_multi_joint_ |
| Trip class | interior soft: any fault/OutOfLimits/watchdog/danger latch = **fail** | log classifier _TODO_ |

### 5b. Near-limit stress

| Gate | Criterion | Harness |
|------|-----------|---------|
| Soft approach | per-joint toward soft at 25–50% baseline; safe envelope clamp OK; fault latch = fail | _TODO: ladder_near_limit_ |

---

## 6. Impedance

| Gate | Criterion | Harness |
|------|-----------|---------|
| Multi-joint poses | mid + elevated bands | _TODO: impedance_poses_ |
| Push / backdrive / hold | springy/damped; no runaway; light dwell `< 0.05 rad` | manual + _TODO_ |
| Mode switch | into/out of Impedance; kp ramp / no τ spike (T5 spirit) | _TODO: mode_switch_clean_ |

---

## 7. TorqueOnly

**Blocked on:** un-alias ([Design](https://github.com/jaylamping/marengo/issues/163) → [Implement](https://github.com/jaylamping/marengo/issues/164)).

| Gate | Criterion | Harness |
|------|-----------|---------|
| τ_cmd steps | bidirectional low open-loop at arm-down; sign + caps; no runaway | _TODO: torque_only_steps_ |
| No-τ_g contrast | mid-pose: does **not** auto-inject full τ_g | _TODO_ |
| Disabled hygiene | clean enter/exit (T9 spirit) | _TODO_ |

---

## 8. Payload critical gates

Attach `standard_payload`; repeat **critical** subset (TBD which): at least coupled G-comp residuals + one Position ladder rung + near-limit smoke.

Harness: _TODO: payload_critical_

---

## 9. Limb sign-off

| Gate | Criterion |
|------|-----------|
| Checklist complete | chapters 1–8 green for this `limb` |
| Record | date, git rev, baselines, payload id, operator |

Harness: _TODO: limb_signoff_bundle_ (gap today)

---

## Retired / superseded

Fragmented `docs/bench-*-test-suite.md` suites → retire policy TBD  
([Decide how to retire superseded bench suites](https://github.com/jaylamping/marengo/issues/160)).

---

## Reaction prompts (for #157)

1. Is **one parameterized markdown spine** the right SoT shape (vs split per-chapter files)?
2. Are chapter boundaries / order right?
3. Are **Parameters** + **Harness** columns enough for limb reuse?
4. What feels missing or over-structured?
