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
