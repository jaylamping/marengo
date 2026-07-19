# Bench test backlog

Running list of **weighted / physical** bench tests to run when home. Newest first.

Agent: append here when investigation suggests a load/model test and the user is not available. Mark `done` with log paths after `pi_bench_harness` (`weighted_single_arm`).

## Template (copy for new items)

```markdown
### YYYY-MM-DD — Short title

- **Status:** queued | done | wont-fix
- **Hypothesis:** what weighted test would validate
- **Log evidence:** bench log path, grep hits, or commit/PR link
- **Suggested run:** profile `weighted_single_arm`, loaded joint, angles, expected pass criteria
- **Blocked on:** fixture mass/COM measurement, encoder zero, etc.
- **Result:** (fill after run)
```

---

### 2026-07-19 — elbow pitch commissioning (E0–E7)

- **Status:** queued
- **Hypothesis:** `right_elbow_pitch` (RS02 id 4) sign/direction, soft limits, and GravityComp sign are correct before Wave raise uses elbow
- **Log evidence:** `pi_set_zero` enable saw id 4 in Davout; no feedback; candump lacks id-4 stream
- **Suggested run:** suite in [bench-elbow-test-suite.md](bench-elbow-test-suite.md); `pi_bench_harness` profile `elbow_attached`, `config_dir: arm_4dof_right`
- **Blocked on:** RS02 id 4 not answering on `can0` (Motor Studio ID / power / wiring); then operator set-zero at straight arm
- **Result:** (pending) — software profile + harness ready; E1 hardware presence FAIL so far

### 2026-07-19 — CAD refresh arm_4dof_right.urdf

- **Status:** queued
- **Hypothesis:** After CAD freeze, replace provisional masses/COM/link lengths so τ_g magnitude matches hardware
- **Log evidence:** —
- **Suggested run:** export URDF → sync → gravity suite magnitude checks
- **Blocked on:** CAD designs finalized
- **Result:** (pending)

### 2026-07-19 — left arm joint rename mirror

- **Status:** queued
- **Hypothesis:** Mirror right vocabulary: `left_elbow`→`left_elbow_pitch`, `left_wrist`→`left_lower_arm_yaw`
- **Log evidence:** —
- **Suggested run:** humanoid + left bring-up rename wave
- **Blocked on:** right 4-DOF bring-up complete
- **Result:** (pending)

### 2026-07-19 — yaw commissioning (Y0–Y4)

- **Status:** queued
- **Hypothesis:** `right_upper_arm_yaw` (RS02 id 3) sign/direction and hold are correct before Wave raise + teach overlays use yaw
- **Log evidence:** —
- **Suggested run:** suite in [bench-yaw-test-suite.md](bench-yaw-test-suite.md); `pi_bench_harness` profile `yaw_attached`, `config_dir: arm_4dof_right`, `skip_set_zero: true` after Y2 Verified
- **Blocked on:** operator at bench; mechanical yaw zero for Y2; must run on `arm_4dof_right` (no skip from 3-DOF)
- **Result:** (pending) — gate Y3–Y4 + candump before shipping yaw on non-Wave presets

---

### 2026-06-22 — arm_3dof_right motion smoke (D1–D3)

- **Status:** done
- **Hypothesis:** shared position-hold tuning lets pitch and roll move cleanly with matched impedance and group velocity caps
- **Log evidence:** Pi `/opt/marengo/var/log/bench-20260622T002026Z.log` (+ position-trace, candump same stamp); deploy `ff9d554`
- **Suggested run:** `pi_bench_harness` profile `arm_2dof_smoke`, `config_dir: arm_3dof_right`, `skip_set_zero: true` after Verified zeros
- **Blocked on:** —
- **Result:** PASS all steps (pitch hold 0.3 rad, roll hold 0.785 rad, cross-talk). Operator: best movement since bench start. Baseline in [bench-2dof-right-smoke.md](bench-2dof-right-smoke.md).

---

### 2026-05-24 — Left shoulder gravity sign after bare-motor runaway

- **Status:** queued
- **Hypothesis:** Left `direction` or encoder zero wrong; weighted stub on right would validate `tau_g` sign independently
- **Log evidence:** bare `gravity-on` runaway on left at ~−1.76 rad; see bring-up session logs on Pi
- **Suggested run:** profile `weighted_single_arm`, `MARENGO_LOADED_JOINT=right_shoulder_pitch`, angles `[0, 0.3, -0.3]`, backdrivable hold at 0
- **Blocked on:** measure fixture mass for `shoulder_pitch_weighted.urdf` — use [bench-weighted-gravity-sign.md](bench-weighted-gravity-sign.md) + `scripts/bench-set-weighted-mass.sh`
- **Result:** (pending)
