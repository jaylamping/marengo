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
