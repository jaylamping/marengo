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

### 2026-05-24 — Left shoulder gravity sign after bare-motor runaway

- **Status:** queued
- **Hypothesis:** Left `direction` or encoder zero wrong; weighted stub on right would validate `tau_g` sign independently
- **Log evidence:** bare `gravity-on` runaway on left at ~−1.76 rad; see bring-up session logs on Pi
- **Suggested run:** profile `weighted_single_arm`, `MARENGO_LOADED_JOINT=right_shoulder_pitch`, angles `[0, 0.3, -0.3]`, backdrivable hold at 0
- **Blocked on:** measure fixture mass for `shoulder_pitch_weighted.urdf` — use [bench-weighted-gravity-sign.md](bench-weighted-gravity-sign.md) + `scripts/bench-set-weighted-mass.sh`
- **Result:** (pending)
