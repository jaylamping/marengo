# Commissioning records

Procedures, bench evidence and session handoffs for bringing hardware up. Start with [docs/status.md](../status.md) for the current checkpoint.

## Procedures (current)

| Doc | Use |
|-----|-----|
| [limb-playbook.md](limb-playbook.md) | Locked limb commissioning spine: Reference → Limits → Sign → G-comp → Position ladder → Impedance → TorqueOnly → Payload → sign-off. Parameterized per limb; filled for `right_arm` |
| [right-arm-calibration-suite.md](right-arm-calibration-suite.md) | Right-arm gravity/friction calibration phases (in build, 2026-10-04) |
| [../pi-commissioning.md](../pi-commissioning.md) | Pi OS, CAN HAT, IMU, install and first CAN checks |
| [bench-ledger.md](bench-ledger.md) | One row per bench run: question, verdict, key numbers, artifacts. Add a row for every run |

## Reference data

| Path | Contents |
|------|----------|
| [firmware/](firmware/) | Robstride firmware behaviour, MIT ranges, and the timing profile the Davout tests guard against |
| [calibrations/](calibrations/) | Gravity calibration sessions and `applied-gravity.json` (what the runtime currently applies) |
| [right-arm-urdf-from-cad-2026-10-03.md](right-arm-urdf-from-cad-2026-10-03.md) | How the master URDF's right-arm geometry and inertials were derived from CAD |

## History

Session handoffs and one-off findings, newest first. They record what was true at the time; each newer one supersedes the older.

- [handoff-2026-10-04-soak-ready.md](handoff-2026-10-04-soak-ready.md) (superseded by `docs/status.md`)
- [handoff-2026-10-04-liveness-hardening.md](handoff-2026-10-04-liveness-hardening.md) (soak table, RX-overrun causes A/B, open decisions)
- [handoff-2026-10-03-audit-soak.md](handoff-2026-10-03-audit-soak.md)
- [handoff-2026-10-03-right-arm-bench-hold.md](handoff-2026-10-03-right-arm-bench-hold.md)
- [handoff-2026-10-03-right-arm-urdf.md](handoff-2026-10-03-right-arm-urdf.md)
- [2026-10-02-right-arm-pr254-findings.md](2026-10-02-right-arm-pr254-findings.md)

New sessions should update `docs/status.md` instead of writing another handoff, unless a long session needs a detailed record.
