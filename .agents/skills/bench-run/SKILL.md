---
name: bench-run
description: Run every Marengo bench session the same way - frame the question, pre-flight the Pi and CAN, get the operator's physical go-ahead, run, collect a fixed evidence bundle and log it in the bench ledger. Use before and after any control test suite, calibration suite, enable soak, hold or harness run on the arm (pi_motion_suite, pi_gravity_calibrate, pi_joint_calibrate, pi_enable_soak, pi_bench_harness, pi_hold_on, pi_marengo_pi_script), including casual asks like "run the suite", "calibrate the pitch", "soak it" or "try it on the arm".
---

# Bench run

A bench run is an experiment: one question, one variable changed since the last run, the same evidence collected every time, and a ledger row that lets any later run be compared with it. A run that cannot be compared with its neighbours is wasted arm time.

## 1. Frame the question

Write one sentence naming what this run should answer and the result that would answer it, for example "Does the Stribeck feed-forward from `48b76b64` keep slow pitch moves within 0.03 rad?". Pick the tool that answers it:

| Question | Tool | Verdict comes from |
|---|---|---|
| Do reference and enable work reliably, with no motion? | `pi_enable_soak` | `soak-summary.txt`: all cycles clean, can0 `rx_over_errors` unchanged, firmware-timing `non_neutral_mit == 0` |
| Does one joint track, stop, hold and repeat to the bar? | `pi_motion_suite` (first with `dry_run: true`) | `score.txt`, the `=== ADR 0039 bench score` block, its `verdict:` line |
| Is the gravity model right for pitch or elbow? | `pi_gravity_calibrate` | residuals in `bench-session.txt` plus the `gravity-fit` proposal (never auto-applied, ADR 0017) |
| Gravity and friction of any right-arm joint? | `pi_joint_calibrate` | same, plus a `control.yaml` friction proposal |
| Profile matrix or smoke check | `pi_bench_harness` | its PASS/FAIL lines |
| One hold | `pi_hold_on` | session log and trace |

Size the run to the question: arm time is test time too. Use `dry_run` to settle plan questions, run only the motion-suite `sessions` that bear on the question, pick the poses and joints that matter, and keep the full matrix for sign-off. Exception: a soak's cycle count is its statistic, so a shorter soak answers a weaker question; say so if you shorten one.

Done when the question, tool, parameters and pass bar are written down, and the one variable that differs from the previous comparable run is named.

## 2. Pre-flight

These are read-only; run them yourself without asking:

1. `git rev-parse --short HEAD` and `git status --short`: the code under test.
2. `pi_health`: `.deploy-rev` must equal the code under test. Otherwise deploy with `pi_sync_main`, or record the mismatch as deliberate. Locally edited YAML needs `pi_sync_bench_config`; a URDF change needs `pi_sync_bench_urdf` (ADR 0017: only after durable Set Limits, or after pulling the Pi URDF).
3. `pi_can_status`: record can0 `rx_over_errors` and `rx_errors` and the controller state (expect ERROR-ACTIVE, bus-off 0). A count that keeps rising across runs is a finding in itself.
4. `pi_logs_last_fault` on the previous session, so the run starts from a known state.
5. `pi_motion_suite` only: `dry_run: true` returns the plan, usable window and guard results with no motion.

Done when every item has a recorded value.

## 3. Operator handshake

With `set_zero: true`, every enable-requiring tool runs SetZero at the current pose: wherever the arm hangs becomes zero for every referenced joint, and every later target is measured from it. A run started away from the mechanical reference moves the arm relative to a wrong zero. Before the call, tell the operator:

- the pose required: hanging limp at the mechanical reference, for every joint the session references;
- support: an elevated arm must be supported, because any stop drops a held arm;
- payload: on or off, matching the profile (`arm_attached`, `elbow_attached`, …); weighted profiles also need `confirm_weighted_motion: true`;
- E-stop within reach, workspace clear, hands off for the run;
- what the arm will do and for roughly how long.

Then wait for an explicit go for this run. `set_zero` and `at_mechanical_reference` are the operator's attestation, so pass them only after the operator has given it. OpenCode will also prompt for the motion tool itself.

## 4. Run

Make one call and let it finish. The tools take CAN ownership, stop `marengo-pi.service` and restore it afterwards.

## 5. Evidence bundle

Collect the same set after every run, pass or fail.

Local artifacts: `pi_motion_suite`, `pi_gravity_calibrate` and `pi_joint_calibrate` write `var/motion-suite/<TS>/` or `var/gravity-calibration/<TS>/` (`plan.json`, `bench-session.txt`, `position-trace.csv`, `config/`, `pi-marengo.urdf`, and `score.txt` for the suite). `pi_enable_soak` writes `var/enable-soak/<TS>/` (`soak-summary.txt`, `bench-session.log`, `candump.log`, `firmware-timing.json`). The other tools leave everything on the Pi: the session log's last line is JSON naming `/opt/marengo/var/log/bench-<TS>.log`, `position-trace-<TS>.csv` and `candump-<TS>.log`. Read those with `pi_read_file` (use `tail` on big files) or copy them as in `can-timeline`.

1. **Verdict**: the tool's own verdict lines.
2. **Faults**: grep the session log for `ERROR|WARN|fault|latch|refused|failed|revoked|clamped`, or use `pi_logs_last_fault`.
3. **CAN health**: the session log's `can errors after marengo-pi` and `can kernel delta` lines, and Δ`rx_over_errors` against pre-flight.
4. **Wire**: `pi_candump_summary`.
5. **Motion runs**: the score block. In `score.txt`, the per-segment legacy warnings printed above the ADR 0039 block misfire on every-tick traces (jerk around 1e10 rad/s² and slew around 6e5 Nm/s appeared on passing 2026-10-04 runs); the verdict is the ADR 0039 block.
6. **Config fingerprint**, when the run directory has the copies: `cat <dir>/config/*.yaml <dir>/pi-marengo.urdf | shasum -a 256 | cut -c1-8` (hash the content only; `shasum` on the files would mix in their paths).

Done when every item is recorded or marked n/a.

## 6. Ledger

Append a row to `docs/commissioning/bench-ledger.md` in the format of its header: one row per tool call, and one row per session for a multi-session suite.

## 7. Route

- Every verdict PASS and nothing unexpected in steps 2–3: report and continue the plan.
- Any FAIL, refusal, latch, unexplained log line, Δ`rx_over_errors` > 0 or surprising number: load `fault-signatures` and triage before the next run. A rerun that passes does not erase a failure. The failure is data, and how often it recurs is a measurement.
- Questions about motion quality, including a PASS with thin margins: load `trace-forensics`.
