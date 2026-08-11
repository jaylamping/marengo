# Actuator manual velocity baselines at 24 V

**Wayfinder research** for [marengo#162](https://github.com/jaylamping/marengo/issues/162) (map [marengo#150](https://github.com/jaylamping/marengo/issues/150)).

**Upstream:** [Define position speed ladder and trip classification](https://github.com/jaylamping/marengo/issues/153) — Position ladder rungs are 25/50/75/100% of these commissioning baselines after config sync.

**Snapshot date:** 2026-08-11 (branch `research/actuator-velocity-baselines-24v`).

**Primary sources (manufacturer):**

| Model | Manual (RobStride Product_Information) |
|-------|----------------------------------------|
| RS00 | [RS00 User Manual 260713](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS00/RS00User%20Manual260713.pdf) |
| RS02 | [RS02 User Manual 260713](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS02/RS02User%20Manual260713.pdf) |
| RS03 | [RS03 User Manual 260713](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS03/RS03User%20Manual260713.pdf) |

**Secondary (cross-check only):** [Seeed RobStride control](https://wiki.seeedstudio.com/robstride_control/) speed table; [hardware ADR 0002](../../../hardware/docs/decisions/0002-robstride-protocol.md) Seeed-derived MIT ranges; `config/motors.yaml` motor-type map (identity of which joints use which type — **not** velocity truth).

**Scope:** Motor types on `right_arm` today; numbers are per motor type and reusable for other limbs.

---

## Question answered

For each Robstride type on the right arm: what rated/manual max speeds apply, how to adjust for **24 V bench vs 48 V rating**, and what **small additional derate** yields the playbook’s per-joint commissioning velocity baselines.

---

## Right-arm motor types (identity only)

From `config/motors.yaml` (joint → type mapping only; velocity fields ignored as SoT):

| Joint | CAN ID | `motor_type` | `gear_ratio` |
|-------|--------|--------------|--------------|
| `right_shoulder_pitch` | 1 | `rs03` | 1.0 |
| `right_shoulder_roll` | 2 | `rs03` | 1.0 |
| `right_upper_arm_yaw` | 3 | `rs02` | 1.0 |
| `right_elbow_pitch` | 4 | `rs02` | 1.0 |
| `right_lower_arm_yaw` | 5 | `rs00` | 1.0 |

With `gear_ratio = 1.0`, joint rad/s = motor output rad/s.

---

## Manufacturer electrical ratings

All three manuals state the same voltage framing under **§1.2 Standard service condition**:

| Spec | RS00 | RS02 | RS03 | Source |
|------|------|------|------|--------|
| Rated voltage | 48 VDC | 48 VDC | 48 VDC | Manual §1.2 |
| Operating voltage range | 24–60 VDC | 24–60 VDC | 24–60 VDC | Manual §1.2 |
| No-load speed | 315 rpm ±10% | 410 rpm ±10% | 200 rpm ±10% | Manual §1.3 |
| Rated load (CW) | 5 N·m @ 100 rpm | 6 N·m @ 100 rpm | 20 N·m @ 100 rpm | Manual §1.3 (“Tested at 100 rpm”) |
| Peak torque | 14 N·m | 17 N·m | 60 N·m | Manual §1.3 |
| Back-EMF | 9.5 Vrms/kRPM ±10% | 9.6 Vrms/krpm ±10% | 17 Vrms/krpm ±10% | Manual §1.3 |
| T-N curve voltage callout | T-N (48 V) | T-N (36 V) **and** T-N (48 V) | T-N (unlabeled in text; specs rated 48 V) | Manual §1.3 |

### Manual max speed → rad/s (48 V context)

Convert no-load rpm with \(\omega = \mathrm{rpm}\cdot 2\pi/60\):

| Type | No-load (nominal) | \(\omega_{nl,48}\) (rad/s) | Rated-load test speed | \(\omega_{rated}\) (rad/s) |
|------|-------------------|----------------------------|------------------------|----------------------------|
| RS03 | 200 rpm | **20.94** | 100 rpm | 10.47 |
| RS02 | 410 rpm | **42.94** | 100 rpm | 10.47 |
| RS00 | 315 rpm | **32.99** | 100 rpm | 10.47 |

**Commissioning “max” choice:** use **no-load speed** as the manual max-speed ceiling (matches “rated/manual max speeds” in #162). Rated-load 100 rpm is the continuous-duty test point under rated torque — useful context, not the ladder 100% ceiling.

### MIT protocol velocity encoding (same manuals)

MIT Operation Control target/feedback velocity spans (manual §4.1.2 / type-1 frames):

| Type | Manual MIT \(v\) span | ≈ no-load? |
|------|----------------------|------------|
| RS03 | ±20 rad/s | Yes (~200 rpm) |
| RS02 | ±44 rad/s | Yes (~410 rpm) |
| RS00 | ±33 rad/s | Yes (~315 rpm) |

These are **encode ranges**, not a second physical rating. They corroborate no-load as the physical max at rated voltage.

**Repo drift note (not used for baselines):** `crates/robstride` / hardware ADR Seeed table still list RS03/RS00 MIT velocity scales of 50 rad/s; current 260713 manuals say ±20 / ±33. Baselines below follow the PDFs, not the crate table.

---

## 24 V vs 48 V — what the manuals state (and do not)

**Stated:**

- Rated voltage is **48 VDC**; **24 V is the bottom of the allowed operating range**, not a second rated point (all three manuals §1.2).
- Electrical / T-N callouts are at **48 V** (RS02 also publishes a **36 V** T-N figure; no numeric table was extractable from the PDF image).

**Not stated:**

- No formula such as \(\omega \propto V_{bus}\).
- No tabulated no-load or rated-load speed at 24 V.
- No explicit derate percentage for reduced bus voltage.

### Proposed voltage adjustment (evidence-based, not vendor-stated)

For a PMSM / QDD actuator, no-load speed is limited by back-EMF vs available bus voltage. With manuals publishing back-EMF constants and rating all T-N data at 48 V, the conservative first-order model is:

\[
\omega_{nl,24} = \omega_{nl,48} \times \frac{24}{48} = 0.5\,\omega_{nl,48}
\]

| Type | \(\omega_{nl,48}\) | Proposed \(\omega_{nl,24}\) |
|------|--------------------|-----------------------------|
| RS03 | 20.94 | **10.47** |
| RS02 | 42.94 | **21.47** |
| RS00 | 32.99 | **16.49** |

Caveats: real 24 V capability may be slightly below linear scale under load (current/IR drop, inverter headroom). RS02’s 36 V T-N plot implies the vendor expects voltage-dependent T-N shapes, but without numbers we do **not** invent a non-linear fit. The small derate below is meant to absorb that uncertainty plus the published ±10% no-load band.

---

## Small additional derate

**Proposed commissioning derate: 10%** after the 24 V scale.

Rationale:

1. Manuals publish no-load as ±10%; a 10% cut parks the 100% ladder rung near the lower edge of that band after V-scale.
2. Shared bench 24 V rail sag under multi-joint MIT bursts is uncharacterized in vendor docs — keep a small margin so the top rung is not the absolute theoretical ceiling.
3. Matches #153’s “small additional derate” (not a limb-safety redesign). Limb danger zones (e.g. elevated pitch descent) remain orthogonal and still apply.

**Formula (per motor type):**

\[
v_{\mathrm{baseline}} = \omega_{nl,48} \times \frac{24}{48} \times (1 - 0.10)
\]

| Type | \(\omega_{nl,24}\) | After 10% derate | **Proposed baseline (rad/s)** |
|------|--------------------|------------------|------------------------------|
| RS03 | 10.47 | 9.425 | **9.4** |
| RS02 | 21.47 | 19.321 | **19.3** |
| RS00 | 16.49 | 14.844 | **14.8** |

Rounded to 0.1 rad/s for config write.

---

## Proposed right_arm commissioning baselines

Write these into runtime velocity SoT (`control.yaml` actuator-group / trajectory caps as required by the playbook) **before** running Position ladder rungs.

| Joint | Type | Baseline \(v\) (rad/s) | 25% | 50% | 75% | 100% |
|-------|------|------------------------|-----|-----|-----|------|
| `right_shoulder_pitch` | RS03 | **9.4** | 2.35 | 4.70 | 7.05 | 9.4 |
| `right_shoulder_roll` | RS03 | **9.4** | 2.35 | 4.70 | 7.05 | 9.4 |
| `right_upper_arm_yaw` | RS02 | **19.3** | 4.83 | 9.65 | 14.48 | 19.3 |
| `right_elbow_pitch` | RS02 | **19.3** | 4.83 | 9.65 | 14.48 | 19.3 |
| `right_lower_arm_yaw` | RS00 | **14.8** | 3.70 | 7.40 | 11.10 | 14.8 |

### Cross-check vs current YAML (not truth)

[Snapshot ticket #155](https://github.com/jaylamping/marengo/issues/155) recorded actuator-group MIT caps of 2.5 / 2.5 / 2.0 / 1.5 / 1.5 rad/s. Those are **below** even the 25% RS02/RS00 rungs and near the 25% RS03 rung. Per #153 / #162, YAML is **not** the baseline SoT — config should be updated to these manual@24V values before the ladder; the first rung (~2.35 rad/s on shoulders) is intentionally close to today’s shoulder ops.

### Reuse for other limbs

Same per-type baselines apply wherever `gear_ratio = 1.0`. If a future joint uses gearing ≠ 1, scale: \(v_{\mathrm{joint}} = v_{\mathrm{motor}} / |gear\_ratio|\) (Davout joint↔motor ownership unchanged).

---

## Worked example (RS03 shoulder)

1. Manual no-load @ 48 V: 200 rpm → 20.94 rad/s ([RS03 Manual §1.3](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS03/RS03User%20Manual260713.pdf)).
2. 24 V scale (proposed): \(20.94 \times 0.5 = 10.47\) rad/s.
3. 10% derate: \(10.47 \times 0.9 = 9.42\) → **9.4 rad/s** commissioning baseline.
4. Ladder: 2.35 / 4.70 / 7.05 / 9.4 rad/s (#153).

---

## Open follow-ups (out of scope for #162)

- Confirm bench bus is truly ~24 V under load (measure `VBUS` / PSU).
- Align `crates/robstride` MIT velocity scales with 260713 manuals (RS03 ±20, RS00 ±33) — separate engineering ticket.
- Whether playbook should further clamp distal joints for inertia/payload after first ladder failures (HITL / execution tickets, not this research).

---

## Citations (quick index)

1. RobStride RS00/RS02/RS03 User Manuals **260713** — §1.2 voltage, §1.3 no-load / rated-load / back-EMF / T-N, §4.1.2 MIT \(v\) spans. Canonical copies: [RobStride/Product_Information](https://github.com/RobStride/Product_Information).
2. [marengo#153](https://github.com/jaylamping/marengo/issues/153) — ladder = fractions of manual@24V baselines + small derate; config sync before rungs.
3. [hardware/docs/decisions/0002-robstride-protocol.md](../../../hardware/docs/decisions/0002-robstride-protocol.md) — manual links + Seeed secondary table (superseded for RS03/RS00 \(v\) spans by 260713 PDFs).
4. `config/motors.yaml` — joint → `motor_type` identity only.
