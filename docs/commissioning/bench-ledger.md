# Bench ledger

One row per bench tool call (one per session for multi-session suites), appended by the `bench-run` skill. Use it to compare runs across commits and to answer "did this fail before commit X?".

Columns:

- **UTC**: run date and time.
- **Session**: the artifact timestamp (`<TS>`).
- **Tool / session**: e.g. `pi_motion_suite/short_moves`.
- **Profile**: bench profile and payload state.
- **Joint(s)**: swept or referenced joints.
- **Rev**: deployed `.deploy-rev`, with `*` when it differs from local HEAD.
- **Cfg**: 8-character config fingerprint (see `bench-run`), or `-`.
- **Question**: what the run was meant to answer.
- **Verdict**: PASS / FAIL / REFUSED / ABORTED.
- **Key numbers**: the few numbers that answer the question.
- **Δrx_over**: change in can0 `rx_over_errors`.
- **Artifacts**: local directory or Pi path.
- **Follow-up**: diagnosis, issue or next run.

| UTC | Session | Tool / session | Profile | Joint(s) | Rev | Cfg | Question | Verdict | Key numbers | Δrx_over | Artifacts | Follow-up |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2026-10-04 20:24 | 20261004T202431Z | pi_motion_suite/long_moves_1 | arm_attached | right_shoulder_pitch | `3ed5c004` | `40b70dfb` | Does scaled-PD J·a + Stribeck (48b76b64) pass the full-range suite? | FAIL | 12/13; slow wave ±0.992 @0.14 rad/s track 0.043 (>0.03); fast moves +0–9 %, stops ≤ 5 mrad, τ step ≤ 0.032 | 0 | var/motion-suite/20261004T202431Z | Stall at slow-wave turnarounds → c5890e3c |
| 2026-10-04 20:26 | 20261004T202646Z | pi_motion_suite/long_moves_2 | arm_attached | right_shoulder_pitch | `3ed5c004` | `40b70dfb` | Same, 90 % span to +2.53 rad | FAIL | 5/6; slow wave −1.04→+2.53 @0.12 track 0.047, 4.5 s stall at peak; no faults, 0 clamps | 0 | var/motion-suite/20261004T202646Z | Tool crashed after scoring (d918c1c7); suite stopped |
| 2026-10-04 21:17 | 20261004T211711Z | pi_motion_suite/long_moves_1 | arm_attached | right_shoulder_pitch | `8703dcca` | `12529488` | Does the friction error assist (c5890e3c, λ 20) fix slow-wave turnarounds? | PASS | 13/13; slow waves within 0.03 | 0 | var/motion-suite/20261004T211711Z | — |
| 2026-10-04 21:19 | 20261004T211926Z | pi_motion_suite/long_moves_2 | arm_attached | right_shoulder_pitch | `8703dcca` | `12529488` | Same, 90 % span to +2.53 rad | FAIL | 6/6 moves pass; 1 total-torque clamp (one-tick dq spike 0.741 rad/s) | 0 | var/motion-suite/20261004T211926Z | Slow-speed audit |
| 2026-10-04 21:22 | 20261004T212243Z | pi_motion_suite/sweeps_and_reversals | arm_attached | right_shoulder_pitch | `8703dcca` | `12529488` | Full-width sweeps and reversals | FAIL | 14/14 moves pass; 1 clamp (dq spike 0.89 rad/s at q +1.80, near the 5 Nm cap) | 0 | var/motion-suite/20261004T212243Z | Slow-speed audit |
| 2026-10-04 21:26 | 20261004T212654Z | pi_motion_suite/short_moves | arm_attached | right_shoulder_pitch | `8703dcca` | `12529488` | Small steps at extremes and 0 | FAIL | 20/22; stops 12.7, 14.2 mrad on descents to 0 | 0 | var/motion-suite/20261004T212654Z | Assist adds stop overshoot |
| 2026-10-04 21:28 | 20261004T212824Z | pi_motion_suite/gravity_extremes | arm_attached | right_shoulder_pitch | `8703dcca` | `12529488` | Holds at −1.038/+1.55, moves between | FAIL | 2/4; stops 10.5, 10.9 mrad (0→−1.038 was 6.3 without the assist) | 0 | var/motion-suite/20261004T212824Z | Assist adds stop overshoot |
| 2026-10-04 21:29 | 20261004T212907Z | pi_motion_suite/repeatability | arm_attached | right_shoulder_pitch | `8703dcca` | `12529488` | Same move ×5 | FAIL | 5/10; returns to 0 stop 10.7–11.1 mrad; MCP server died after this session | 0 | var/motion-suite/20261004T212907Z | Owner: slow motion "fights itself" |
