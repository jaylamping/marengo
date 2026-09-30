# Tenth repair batch: bounded virtual reference acquisition

Baseline: checked PR223 merge `7a25bbbca0fad1ef579e979309acba666d4409da`.
Branch: `codex/reference-acquisition-core`. Software, local CAD and evidence stay
under `J:\code`. Design: [ADR0026](../../decisions/0026-bounded-virtual-reference-acquisition.md).

R2a implements the complete closed virtual acquisition and cleanup contract.
Success stages evidence with `CommitUnavailable` and `usable_reference=false`.
It writes no history and grants no motion permission. CS05/CS06/CS07 stay partial;
durable journal/grant and installed-owner clients follow in separate slices.
Physical/custom backends refuse capability before any transaction arming.

## Owner and execution flow

Davout owns one opaque reservation, owner-issued stamps/handles and eight retained
immutable outcomes. Identical retries return their real retained outcome;
foreign/conflicting/stale/expired identities refuse without transmitting.
Each mutable advance performs one phase and at most one actual receive report,
bounded to 64 raw frames and 256 reads. Incomplete work terminates rather than
receiving a new budget. Finite phase time is capped before arithmetic and expiry
wins a reply at exact equality.

```mermaid
flowchart LR
  Begin[Reserve without TX] --> Stop[All installed stop attempts]
  Stop --> Drain[Applied reporting Off and complete old drain]
  Drain --> Arm[Enable selected virtual target]
  Arm --> Flush[Complete post-arm drain]
  Flush --> Zero[Addressed SetZero once]
  Zero --> Proof[Ordered raw report plus private correlation]
  Proof --> End[Mandatory cleanup and unusable staged evidence]
```

Private owner/realm/transaction/device epoch is joined to the exact actual raw
pop, delivery ordinal, address, CAN ID and receive time after the whole ordered
hazard report is consumed. Cached zero, typed queued reports, recent timestamps
and unrelated replies cannot impersonate it. Peer hazards win over proof.
SetZero attempt invalidates the selected old coordinate/derivative cache while
preserving peers and persistent fault evidence. Initial stop failure is retained
without a duplicate burst. Every other terminal retains actual all-address stop
and bounded reporting outcomes, including failures.

Berthier's busy tick discards retained motion intent and advances this owner;
new torque/gain/mode, typed model installation and official Pi overlay dispatch
refuse while reserved. Pi shutdown always cleans a live reference before storage,
including ordinary `disable_on_exit=false`, and reuses the receipt when true.
Direct legacy public fields remain CS15 work. Snapshot observes their mismatch
permanently; restoring fields cannot resume a live transaction or INITIAL grant.
Inspection retains the mismatch; mutable advance/cancel delivers cleanup on the
original installed routes. Hardware stop acknowledgement is not inferred.

## Actual regressions and test quality

Existing independent receive/reference/stop parity passed first: 161 Davout
tests, strict Clippy, 14.9683237 seconds, 1,044 unchanged bound inputs.
New APIs are candidate conformance, not old-main missing-method regressions.
Seven concrete defects in the candidate were reproduced before their repairs:

| Probe | Independent behavior | Executed result |
|---|---|---|
| cause | Keep the earlier Device hazard ahead of secondary WorkLimit. | 0/1 → 1/0; entire file unchanged |
| clock | Cap finite phase time before addition near Duration::MAX after actual target Enable. | 0/1 → 1/0; entire file unchanged |
| model | Reject typed private-model restore while reserved; preserve legal restore afterward. | 0/1 → 1/0; entire file unchanged |
| intent | Refuse new mode, torque and gain intent before mutation while reserved. | 0/1 → 1/0; entire file unchanged |
| overlay | Refuse actual overlay mutation/enqueue while busy; preserve real healthy disk/Durable neighbor. | 0/1 → 1/0; entire file unchanged |
| initial-mismatch | Snapshot/fresh Begin observation permanently revokes INITIAL permission after route reversion. | 0/1 → 1/0; entire file unchanged |
| busy-mismatch | Snapshot observation sticks in a live reservation; mutable advance cleans original routes after restoration. | 0/1 → 1/0; entire file unchanged |

The sticky busy probe reaches and cleans both Reserved and Armed fixtures before
classification. Reserved is its first executed red assertion; the unchanged
green executes both full contracts. Eleven core groups, two real controller-owner
groups, four shutdown cases in one group, and focused admission/continuity
probes give 21 new active groups. They use actual raw decoding, literal installed
wire/stop expectations, real configuration writer/publication and captured state
before fallback cleanup. INITIAL setup remains confined to explicit pre-existing
virtual-fixture controls. No synthetic grant is injected into acquisition.

The separately frozen positive protocol passes. Five isolated production mutants
are caught by actual single-test assertions: accepting untagged raw zero,
ignoring device epoch, losing equality expiry, skipping peer terminal stops and
skipping mandatory shutdown cleanup. Each binds and recompiles the actual source
tree; all test bytes remain identical and only one production file differs.
These are deliberate fault-injection checks, not original-source defects.

Retained excluded attempts include a nonexistent feature, nonexistent enum and
duplicate support-module lint. Two new tests had incorrect identity expectations:
a busy-issued unused stamp is stale after Cancel's stop, and an old sequence1
stamp expires before a privately injected MAX counter reaches arithmetic. The
corrected tests preserve production admission and pass; these failures are not
counted as defects. No new test is ignored to obtain a green result.

## Qualification and remaining work

Strict affected gate: **453 passed, zero failed or ignored**, 30.178839 seconds,
including the actual shutdown test; format and Clippy pass. Primary: **736 Rust tests passed, one existing ignored; 355 frontend and 72 Pi MCP passed**, with fatal ARM release, format/lint/proto/build/deny/audit checks.
Independent Standards and Spec find no scoped blockers, with authored files
excluded and reviewed by the other agent. All 1,055 compiled/package/gate/policy/
normative inputs match manifest `7d40dcaac8ee0d1508bab4582c44b56e46f1cf7af8a1912cf7541d9a96b05f4f`.
Minimal simulation smoke and all five sim-harness tests pass; this does not qualify the production plant. Mutable review records are excluded from that source binding. Raw logs, candidate
archives, hashes, exact timings and failure classifications live under
`J:/code/marengo-migration-backup-20260929/batch10`; the implementation ledger
records each receipt. Required exact-head GitHub delivery remains pending.

The 102 finding dispositions stay **14 verified, 10 partial, 78 open**. R2b must
retain accepted evidence, supply a bounded noncoalescing recoverable journal and
require a still-current matching commit before private permission. R3 installs
Pi/proto/gateway/MCP/CLI clients. Physical identity/reset/ack/readback, drive-local
limits/timeouts, E-stop/sensors/support, current model/plant and timing acceptance
remain external. No robot connection, movement, flash or deploy occurred. Limits
and Wave sign-off remain unchanged.
