# Ultrawork Notepad — Resume gravity-comp bench testing (T1 finish + T2-T10)

Started: 2026-06-21 (new session continuing from `ses_11c5c7e65ffetcesiurAqoi6JP`)

## Plan (exhaustive, atomic)
1. ~~Revert velocity limits in control.yaml/motors.yaml~~ — DONE 2026-06-21
2. ~~Sync config to Pi (pi_sync_bench_config)~~ — DONE 2026-06-21
3. ~~User reattaches 700g load and positions arm at mechanical home~~ — DONE 2026-06-21
4. ~~Re-zero at mechanical home (pi_set_zero)~~ — DONE 2026-06-21 (pos=0.0000 rad verified)
5. T4: Friction ID sweep at 0.785 rad — IN PROGRESS
6. T5: Mode-switch transient — pending
7. T6: Wrong-sign watchdog — pending
8. T7: Negative-retarget ladder — pending
9. T8: Recovery protocol — pending
10. T9: Disable-drop — pending
11. T10: Full re-run + verdict update — pending

## T4 detailed sub-plan
- T4 uses target 1.484 rad (not 0.785 — handoff was wrong). Sweep speeds: 0.1, 0.5, 1.0 rad/s. Directions: + (home→1.484), − (jog to 1.484 then hold-at 0).
- For each speed: edit control.yaml `position_trajectory_velocity_rad_s`, sync config, run move(s), capture trace, run `analyze-position-trace.py --gate layer2 --tau-ff-rate-limit 60` (note: gate is calibrated for 0.1 rad approach, so also extract segment metrics manually: jerk_rms, tau_ff_max_slew, tau_f flips).
- After all 6 traces: restore `position_trajectory_velocity_rad_s: 1.45`, sync config.
- Pass thresholds per doc: fc in [0.075,0.225], fv ≥ 0, jerk_rms < 800 rad/s², tau_ff slew < 120 Nm/s, tau_f flips ≤ 2 per segment, no fault.

## Now (single step in progress)
T4 speed 0.1 rad/s positive direction: edit control.yaml trajectory velocity to 0.1, sync to Pi, then `pi_hold_on position_rad=1.484`. Operator must support arm during first elevated move.

## Scenarios (the contract)
- S1 (T1 finish, happy path): `pi_hold_on position_rad=0.0/1.484/2.0` each → arm moves to target within ±0.05 rad, settles, returns home clean, `pi_logs_last_fault` clean. Real-surface artifact: bench-latest.log line `target=<rad>`, position-trace-latest.csv col 3 (q) shows convergence, fault=0x0000.
- S2 (T1 RMS, edge): tau_meas RMS vs tau_g < 0.124 Nm at every dwell. Real-surface artifact: `pi_read_file /opt/marengo/var/log/position-trace-latest.csv tail=2000` shows cols 18+22 near-equal in steady-state window. Compute RMS in notepad.
- S3 (T7 negative ladder): -0.3 → -0.5 → -0.7 → -0.85, peak |dq| < 2.0 rad/s at every rung, no velocity trip. Real-surface artifact: position-trace-latest.csv col 4 (dq) bounded.
- Regression: previously-passed holds at 0.3, 0.785, π/2 still produce convergent q on the deployed rev. Not re-running them unless user wants; artifact is the prior session's logged CSV.

## Now (single step in progress)
Running T1 hold at 2.0 rad (pi_hold_on position_rad=2.0, profile=weighted_single_arm, timeout_sec=15, return_home_sec=6). This is the 5th and FINAL T1 positive-angle hold. Arm is at home (~0 rad), motor Disabled, clean state.

## User decision
Picked "Follow protocol strictly": T1 = 5 positive angles [0.0, 0.3, 0.785, 1.484, 2.0], then T2-T10. The previous operator's -0.3/-0.5 belong to T7, not T1.

## Todo (remaining, ordered)
- [ ] Get scope answer from user
- [ ] T1 hold at 0.0 rad (pi_hold_on position_rad=0.0)
- [ ] T1 hold at 1.484 rad (pi_hold_on position_rad=1.484)
- [ ] T1 hold at 2.0 rad (pi_hold_on position_rad=2.0)
- [ ] T1 RMS analysis on all 5 positive-angle traces
- [ ] Update docs/bench-weighted-700g-results.md T1 row
- [ ] T2: gravity-on vs gravity-off A/B
- [ ] T3: payload robustness (bare/700g/900g) — requires physical payload swap by user
- [ ] T4: friction ID sweep + Layer 2 gate
- [ ] T5: mode-switch transient
- [ ] T6: wrong-sign watchdog (deliberate fault)
- [ ] T7: negative-retarget ladder (-0.3 → -0.5 → -0.7 → -0.85)
- [ ] T8: recovery protocol (deliberate velocity fault)
- [ ] T9: disable-drop behavior
- [ ] T10: full re-run + verdict update + mem0 save

## Findings (non-obvious facts with file:line refs)
- `docs/bench-gravity-comp-test-suite.md:361-446` — T4 friction sweep target is 1.484 rad (not 0.785 as handoff said); speeds 0.1/0.5/1.0 rad/s; requires editing `position_trajectory_velocity_rad_s` per speed.
- T4 v=0.1 rad/s run 20260621T085658Z FAILED to move the arm: q stayed at ~0.020 rad for 38 s while target=1.484, phase=Cruise, dq_traj=0.1. tau_ff_cmd≈0.2 Nm, tau_meas≈0.45 Nm — not enough to break static stiction with 700g load from home.
- Verification hold at 0.3 rad with v=1.45 rad/s (session 20260621T085852Z) moved successfully: q reached 0.323 rad, returned home clean, fault=0x0000. Confirms arm/load/zero are good; issue is specifically v=0.1 insufficient to break stiction.
- `pi_logs_last_fault` shows Davout ignored raw velocity spikes up to ~3.5 rad/s during return-home (threshold 2.7, limit 2.5) — velocity-limit tuning still marginal but no fault.
- Position-trace column index: col 4=q, col 5=dq, col 7=q_traj, col 8=dq_traj, col 14=phase, col 17=tau_g, col 18=tau_f, col 21=tau_ff_cmd, col 22=tau_meas.

## Now (single step in progress)
T4 blocked: 0.1 rad/s sweep cannot break stiction. Need user decision on whether to skip 0.1 and continue with 0.5/1.0 rad/s, or adjust tuning first.

## Learnings (patterns / pitfalls for next turn)
- `pi_hold_on` REQUIRES `position_rad` explicitly or it latches current pose — previous session burned 10+ attempts on this.
- Hold logs are 40-100 KB — always use `pi_read_file` with `tail=2000` on `position-trace-latest.csv` for analysis, not the full hold log.
- π/2 hold overshoots 0.063 rad — trapezoidal decel tuning, NOT a bug; note for T4.
- Per `pi-mcp-first.mdc`: use MCP tools for all Pi actions; only ask user for physical actions MCP cannot do (support arm, E-stop, power cycle).
- Per `gentle-ai-persona.mdc`: ask at most one question then STOP and wait.