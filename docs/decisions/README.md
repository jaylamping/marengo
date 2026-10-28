# Architecture decision records

One file per decision, numbered in order. An ADR records why a choice was made. Later ADRs supersede or amend earlier ones instead of rewriting them, and short dated "Current state" or "Amendment" notes mark where the code has moved on. Hardware decisions live separately in [`hardware/docs/decisions/`](../../hardware/docs/decisions/).

Status as of 2026-10-28. "Implemented" means the software is in `main`; bench qualification is tracked in [`docs/status.md`](../status.md) and the [bench ledger](../commissioning/bench-ledger.md).

| ADR | Decision | Status |
|-----|----------|--------|
| [0001](0001-protobuf-wire-types.md) | Protocol Buffers for wire types | Accepted |
| [0002](0002-containerized-dev.md) | Containerized development environment | Accepted (host side updated by 0018) |
| [0003](0003-simulation-testing.md) | Simulation testing tiers (D0 / D1 MuJoCo / D2 out of band) | Accepted; D2 not started |
| [0004](0004-control-modes-and-mit.md) | Control modes and MIT command model | Accepted; amended 2026-10-03 (Impedance tracks measured q) |
| [0005](0005-dynamics-library.md) | Pure Rust dynamics for gravity compensation | Accepted; whole-body dynamics needs a new ADR before M8 |
| [0006](0006-homing-zero-reference.md) | Homing, zero, and joint reference strategy | Accepted; verifier/registry workflow retired by 0036 |
| [0007](0007-bench-position-trajectory-control.md) | Bench position trajectory control | Accepted; parts being replaced by 0039/0040 |
| [0008](0008-chappe-webtransport-transport.md) | Chappe operator gateway (HTTP + WebTransport) | Accepted; auth by 0033, Jetson topics removed |
| [0009](0009-dynamic-position-limit-envelope.md) | Dynamic velocity-scaled position limit envelope | Accepted; partly superseded by 0017 |
| [0010](0010-actuator-velocity-cap-resolution.md) | Actuator velocity cap resolution | Accepted; amended 2026-10-03 |
| [0011](0011-log-retention-and-archive.md) | SQLite hybrid log retention and gateway log API | Accepted |
| [0012](0012-config-db-overrides.md) | Runtime config: memory SoT + YAML write-behind | Accepted |
| [0013](0013-structured-log-fields.md) | Structured log fields on `LogEvent` | Accepted |
| [0014](0014-jetson-perception-semantic-motion.md) | Jetson perception + semantic-to-motion pipeline | Accepted design; not implemented (scaffolds removed 2026-10-03) |
| [0015](0015-wrong-sign-watchdog.md) | Config-driven wrong-sign watchdog for GravityComp | Accepted; see its 2026-09-29 limitation |
| [0016](0016-wsl-software-home.md) | WSL ext4 is the software home | **Superseded** by 0018 |
| [0017](0017-bench-set-limits-urdf-expand.md) | Bench Set Limits may expand URDF hard envelopes | Accepted |
| [0018](0018-windows-macos-software-home.md) | Windows and macOS host checkouts | Accepted |
| [0019](0019-runtime-authority-and-verification.md) | Runtime authority and independent verification | Proposed design; delivered in slices by 0020–0038 |
| [0020](0020-lossless-feedback-and-fault-authority.md) | Lossless feedback and persistent fault authority | Implemented (bounded slice) |
| [0021](0021-bounded-can-ingress.md) | Frame integrity and bounded CAN ingress | Implemented |
| [0022](0022-calibration-history-and-current-reference.md) | Calibration history and current reference | Implemented (first slice) |
| [0023](0023-private-current-reference-authority.md) | Private current-reference admission | Implemented |
| [0024](0024-stop-before-persistence-shutdown.md) | Stop before persistence on graceful owner shutdown | Implemented |
| [0025](0025-measured-ascent-progress.md) | Measured encoder progress owns the ascent watchdog | Implemented |
| [0026](0026-bounded-virtual-reference-acquisition.md) | Bounded virtual reference acquisition without a grant | Implemented |
| [0027](0027-retained-reference-evidence-continuity.md) | Retain virtual reference evidence with checked model continuity | Implemented |
| [0028](0028-capture-chronology.md) | Capture chronology for imported log artifacts | Implemented |
| [0029](0029-store-migration-ownership.md) | One owner for each store schema transition | Implemented |
| [0030](0030-deliberate-known-store-recovery.md) | Deliberate recovery of a recognized historical store | Implemented |
| [0031](0031-explicit-simulation-resources.md) | Explicit resources for closed simulation construction | Implemented |
| [0032](0032-local-limits-mirror-resources.md) | Bind the local limits mirror to its checkout | Implemented |
| [0033](0033-gateway-runtime-access.md) | Shared gateway access policy and runtime credentials | Implemented (PR #245) |
| [0034](0034-durable-virtual-reference-history.md) | Durable virtual reference history without motion permission | Implemented |
| [0035](0035-consume-current-virtual-reference.md) | Consume a current durable virtual reference for one joint | Implemented; partly superseded by 0036 |
| [0036](0036-physical-robstride-reference.md) | Qualified physical Robstride reference (process-local grants) | Implemented; bench qualification pending |
| [0037](0037-disabled-drive-protocol-inspection.md) | Read-only protocol inspection of Disabled drives | Implemented |
| [0038](0038-single-drive-loss-degraded-hold.md) | Single-drive loss: subtree shed, degraded hold, controlled lower | Implemented |
| [0039](0039-position-control-simplification.md) | Position control: drive-side PD on a feedback-scaled reference | **Proposed**; phases 0 and 2 implemented, pitch-only bench trial |
| [0040](0040-scaled-pd-slow-speed-law.md) | Scaled-PD slow-speed law (stiffer PD, filtered damping, leaky integral) | **Proposed**; pitch-only bench trial |

## Writing a new ADR

- Take the next number. Title it with the decision, not the problem.
- Start with a `**Status:**` line (Proposed, Accepted, Superseded by NNNN) and a `**Date:**` line.
- Sections: Context, Decision, Consequences, and Alternatives considered when there were real ones.
- When it replaces part of an older ADR, say so in both files and update the table above.
- Anything that relaxes a safety latch, cap or the Berthier → Davout → robstride path needs an ADR first ([`docs/safety.md`](../safety.md)).
