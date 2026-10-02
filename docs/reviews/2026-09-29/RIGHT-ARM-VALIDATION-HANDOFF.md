<!-- Subsequent owner cancellation: read CANCELLED-SESSION-CHECKPOINT.md first.
The pushed session checkpoint now includes the five earlier dirty-primary edits.
Those changes are unvalidated and undeployed; earlier qualified results apply
to2d0fd40. Closing documentation merge/source sync was stopped. Resume only at
explicit owner request. The recorded handoff below remains historical evidence. -->

# Right-arm validation handoff — October 2, 2026

The current code pass is merged, validated and installed on the personally owned
Marengo Pi. The owner requested a stopping point and will resume in a new
session. The goal remains unfinished and is to be paused after closing delivery;
`marengo-repair-loop` stays PAUSED. No human reply is pending. No physical
movement test has occurred. Prior powered deployment approval is preserved;
every future physical movement still needs a concrete proposal and explicit
operator confirmation after its commissioning checks pass.

## Start here

1. Read this file, [batch38](batch38-receive-diagnostics.md), and the current
   [ledger](implementation-ledger.json). Fetch current main and inspect local
   status before changing any checkout. The historical batch16 `current_batch`
   object is deliberately retained; `active_windows_continuation` describes this
   latest pass. Earlier HANDOFF sections and roadmap checkpoints are history.
2. Explicitly resume the goal in the new session. Verify personal Pi connectivity,
   installed marker, services, fresh RobotState/SafetyState, taught bounds and
   calibration read-only. Existing snapshot timestamps and physical readiness
   describe this session and must be refreshed for an actual movement test.
3. Prioritize the dependencies needed for a small right-arm direction test:
   CAN receive reliability/evidence, physical reference acquisition, owner
   priority stop/recovery, and bounded selected-joint commissioning. Use the
   all-bugs ledger for independent work while a physical observation is pending.

## Exact software and checkout checkpoint

| Item | Recorded identity/location |
|---|---|
| Qualified installed source | `2d0fd4088b0ca0301b2e27b07560b54b40ec61ca` |
| Code delivery | [PR252](https://github.com/jaylamping/marengo/pull/252), merged `ba0fff7206877943fbca0f79b18d59089d5d4a24`; tree equal to source |
| Primary and native Pi validation | 895 Rust passes / one existing ignored / 374 UI passes; required strict auxiliary and ARM gates pass |
| Code source/main CI | [37039819887](https://github.com/jaylamping/marengo/actions/runs/37039819887) / [37043206357](https://github.com/jaylamping/marengo/actions/runs/37043206357), all five actual jobs pass |
| Windows primary | `J:/code/marengo`, mainba0fff at recorded code sync; five preserved local edits below |
| Clean delivery worktree | `J:/code/marengo-worktrees/current-virtual-reference`, `codex/right-arm-validation-handoff` |
| Pi SSH/source | `ssh marengo-ts`, `/home/joey/marengo`, cleanba0fff at 17:59:45Z;198 staged payload files preserved |
| Pi installed runtime | `/opt/marengo`, marker `2d0fd40... 2026-10-02T17:30:02Z` |
| Final handoff sync | After this documentation is merged, primary/Pi source advance to its main revision; installed2d0fd40 remains unchanged because production inputs are identical. Closing receipts are retained at the external paths below. Fetch latest main rather than pinning to the earlier recorded checkout SHA. |

The primary's existing edits change RS00 effort17 to14 in URDF/kinematics,
RS03/shoulder caps5 to9, and global bench cap5 to9. Files:
`assets/urdf/marengo.urdf`, `config/control.yaml`, `config/motors.yaml`,
`config/robot.yaml`, `hardware/docs/kinematics.md`. These were preserved
byte-identically, separately backed up and excluded from deployment. Do not
commit, reset, stash, widen limits or build a release from that dirty checkout
implicitly. All prior CAD, branches, worktrees and backups remain retained.

## What has been completed

The [repair status snapshot](repair-status-snapshot.md) lists every one of the
102 finding dispositions and all eight maintenance tasks with its source record.
The ledger preserves acceptance criteria and evidence; `verified` means the
described software repair is proven, with hardware acceptance tracked separately.

Completed software repairs cover per-motor feedback freshness and finite command
validation, torque caps/slew initialization, observable stop write failures,
stop-before-persistence shutdown, gravity COM translation and stall handling,
bounded telemetry, tracing/Store deadlocks and artifact imports, historical
timestamp/recovery tooling, local gateway access controls, truthful host metrics,
research handlers/date filters, native Windows tooling guidance and dependency
gates. Thirteen findings remain partial rather than closed by a subproblem fix.

Recent movement preparation is cumulative:

- The reference model/codec/journal/transaction/URDF files implement explicit
  reference authority, identity/persistence checks and isolated regression
  tests. Their name describes reference-state software. They do not establish a
  physical homing/reference grant for this arm. The selected current virtual
  permission slice is tested; the physical adapter is unsupported/unqualified.
- [Batch35](batch35-persistent-fault-publication.md) makes persistent owner faults
  visible in actual Pi SafetyState publications and preserves them across later
  healthy ticks. PR249 is delivered and installed in the cumulative release.
- [Batch36](batch36-physical-can-overflow.md) captures an actual controller RX
  overflow and retained fault/latch transition. IRQ/SPI servicing cause remains
  unproved. Restart does not establish physical recovery.
- [Batch37](batch37-reporting-pacing-comparison.md) spaces healthy reporting
  requests by5ms and rotates after failures. Matched quiet300s windows pass on
  both versions; earlier baseline captures and a later candidate fault prevent
  a causal reliability claim.
- [Batch38](batch38-receive-diagnostics.md) exposes the first retained raw receive
  envelope in persistent faults. Five frozen original assertion failures become
  six unchanged candidate passes, with primary/native/CI and independent reviews
  clear. The installed update and preservation checks pass.

## Observed physical state and unresolved blockers

At 17:59:45Z the Pi was Disabled, all five joints Unhomed, zero Faulted and no
software latch. CAN0 RX errors502; Pi/gateway PIDs708792/708603 active with zero
automatic restarts. CAN unit active/exited. Store integrity/schema3 and
HTTPS200, explicitly trusting `/opt/marengo/var/gateway/tls/cert.pem`, pass.
Internal snapshots are `http://127.0.0.1:8080/snapshot/robot/state` and `/safety`;
the installed UI is `https://127.0.0.1:8444/` on the Pi. The private runtime
environment, calibration registry and taught limits remain unchanged.

The owner last reported motor power ON, arm resting at gravity home, with stable
support, clear workspace and physical E-stop in reach. Motor power may remain on
for authorized testing; no redundant power-off approval is needed. Readiness must
be current when proposing an actual test. E-stop wiring/GPIO behavior is not yet
qualified. No setup/deployment confirmation is waiting on the owner.

Fresh-state sample17:38:24Z measured lower yaw at `-4.1322865486rad`, outside its
unchanged effective hard interval `[-0.3942450583, 3.2440848351]rad` and soft
interval `[-0.3672450583, 3.2170848351]rad`. Do not widen limits or treat gravity
resting pose, an old stored row, process restart or cached feedback as reference
proof. Confirm coordinate convention, physical pose, calibration and the actual
device acquisition path before generating a physical current-reference grant.

The installation60s and final passive300s captures have zero error frames,
counter502 unchanged, and no Enable3/SetZero6 or non-neutral MIT. The final
capture includes297 fresh Disabled samples. Before the update, errors grew
487 to494 to502; exact initiating bytes for that interval were not captured.
Earlier actual overflows remain evidence of an intermittent problem. Neither
quiet windows nor software Disabled prove physical stop, reliability or recovery.
The new owner diagnostic has software proof; a new physical error comparison
was unavailable in these bounded windows.

## Next-session path to the first movement

1. Refresh read-only runtime/identity/limits evidence and preserve any initiating
   CAN error plus the corresponding owner fault message. Finish a reproducible
   CAN receive/IRQ/SPI/workload comparison with one variable changed at a time.
   Do not assign causality from average CPU samples, uncaptured counter growth
   or a quiet capture. No more experiments were started at this stopping point.
2. Complete/qualify the physical reference path with real device/firmware
   identity continuity, reset/ack/readback and freshness, durable current grant
   and correct invalidation. Migrate installed owner and clients to the tested
   authority. Software virtual fixtures are not hardware acceptance.
3. Qualify reference-independent owner priority stop, fault recovery and the
   physical stop/support/E-stop procedure. Accepted CAN writes, Disabled state
   and a restarted process do not establish stopped hardware. Preserve latches
   and stopped cleanup on failure; never force Ready or bypass Davout.
4. Commission one selected joint with explicit joint/motor direction, preserved
   effective limits, drive-local timeout/torque handshake, fresh feedback and
   bounded speed/effort/duration. Resolve the PD total-output cap problem where
   that command mode needs it. Narrow command admission so other joints cannot
   energize accidentally. Keep Wave sign-off and learned limits intact.
5. Present the operator with the exact joint, starting pose, intended direction,
   displacement, numeric speed/effort/duration caps, stop/abort method and expected
   observation. Ask before every physical movement and wait for explicit
   confirmation. Acknowledge the result against live telemetry and operator
   observation, then stop and verify the outcome before another proposal.

If the operator does not reply within ten minutes, leave that movement pending
and work on independent software items. Silence never authorizes movement. Work
only on the personally owned repository/Pi; access-control and permission tests
use isolated fixtures. The all-bugs goal continues after explicit resume, but
the first direction test need not wait for unrelated archive/research/UI repairs.

## Recovery evidence and continuation records

- Committed: `docs/reviews/2026-09-29/evidence/batch38/`, lossless frozen
  red/green logs, actual CI receipts, installed/preservation receipts and decoded
  protobuf snapshots. [Artifact bindings](evidence/batch38/artifact-bindings.json)
  distinguish original bytes from declared UTF-8/LF normalization.
- Windows: `J:/code/marengo-migration-backup-20260929/batch38-pi-receive-diagnostics/`.
  Complete source/test executables, primary/native logs, corrected release,
  failed preparation attempts and both verified observation archives are retained.
  Closing documentation delivery is recorded in `stopping-delivery-receipt.json`;
  closing primary/Pi sync receipts are retained alongside it.
- Pi: `/home/joey/marengo-validation/pi-sync-20261002-batch38-receive/` retains
  installed233-file/config/environment/calibration/unit/Git/Store backups and
  the activation evidence. Native qualification lives at
  `/home/joey/marengo-validation/batch38-20261002/stack-2d0fd40`.
  Passive observation is `/home/joey/marengo-validation/batch38-passive-owner-20261002/`.
  Code and final documentation source-sync backups/receipts use the owned
  `batch38-source-main-sync` and `batch38-final-handoff-sync` directories.
- The previous primary five-file backup hash is
  `895ff9b03e117a73940f13597f9cafa7259fd454ffd72647e5fa1bf833dbb6bf`.
  Other archive/file identities are in committed receipts; retain private backup
  contents on their original hosts. Never infer a Mac checkout path from examples.

## Copyable new-session prompt

> Resume the unfinished Marengo repair goal from the latest main
> `docs/reviews/2026-09-29/RIGHT-ARM-VALIDATION-HANDOFF.md` and implementation
> ledger. This is my personally owned humanoid robot and Raspberry Pi. Preserve
> existing local edits, CAD, taught limits, backups and the paused automation.
> The current code pass PR252 is merged and installed; prioritize resolving the
> CAN/reference/priority-stop/commissioning dependencies for a basic right-arm
> direction test, with remaining bugs handled in dependency order. Verify fresh
> device state first. Prompt me before each concrete bounded physical movement
> and wait for my explicit confirmation; if I do not respond within ten minutes,
> leave movement pending and continue work that needs no movement verification.
> Report software proof and observed hardware acceptance separately.
