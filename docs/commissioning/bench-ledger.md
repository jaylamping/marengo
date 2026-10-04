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
