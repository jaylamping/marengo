---
name: fault-signatures
description: First-stop triage when the Marengo arm, a drive, CAN or a bench tool does something unexpected - pin the symptom, match its exact log lines against the catalog of known failure signatures, run the confirming check, then route to the deeper skill and grow the catalog. Use whenever hardware is not behaving as expected - a fault or latch, a home/enable refusal, a fuse trip, a drive going limp or silent, a suite FAIL, a gravity-gate refusal, an rx_over_errors jump, odd log lines - before any deep dive.
---

# Fault signatures

Recognition is the cheapest diagnosis. `signatures.md` (next to this file) catalogs how this robot has failed before: the exact text, what it means, what caused it, the check that confirms it, and what to do. Triage starts there, and every solved cause goes back into it.

## 1. Pin the symptom

Write down what was expected, what happened, which joint(s), the session timestamp, the code rev, and the **phase**: startup, reference (`home`), enable, active motion, degraded hold, stop/exit, or a tool gate before any of them. A phased symptom ("`right_elbow_pitch` went limp 2.1 s into `hold-at 0.8`, no fault line") can be matched; a vague one ("it acted weird") cannot.

## 2. Gather the exact text

1. The session log: local `bench-session.txt`/`.log`, or `pi_logs_last_fault`, `pi_logs_grep`, `pi_logs_tail`.
2. `pi_journal` for service events: crashes, restarts, `ExecStopPost`.
3. `marengo-pi` stdout lines (the contract list in `AGENTS.md`).
4. `pi_can_status` counters.

Quote lines verbatim; a paraphrase will not match the catalog.

## 3. Match

Grep `signatures.md` for each distinctive string. A match is a hypothesis until its **Confirm** check agrees. When several lines match, work them in phase order, because the earliest anomaly usually causes the later ones: a Transport latch at startup explains every refusal after it.

## 4. Route

- **Confirmed known cause:** follow its **Action**. If the entry says the cause was fixed and it recurs, treat it as a regression: find the fixing commit and prove the recurrence with `bench-to-test`.
- **No match, or the confirm check disagrees:** classify by layer and go deeper:
  - wire, timing, silence, latches, grant revocations: `can-timeline`;
  - motion quality, fuse trips, model error: `trace-forensics`, and the `control-auditor` agent for the control law itself;
  - mechanical or electrical (harness, power, termination, grounding): name the physical check and ask the operator, since it cannot be done remotely.

A safety mechanism that fires (Transport latch, grant revocation, fuse, cap, pacing) is usually reporting a real hazard. Ask why it fired; relaxing one needs an ADR.

Done when the symptom has a confirmed cause with evidence, or is recorded as unknown with the evidence gathered and the next diagnostic named.

## 5. Grow the catalog

Once a cause is confirmed, whether new or a new variant of a known one, add or update its entry in `signatures.md` in the same format, with the date, session timestamp and evidence. Keep entries short and link the doc, ADR or commit that holds the detail. Then load `bench-to-test` before fixing.
