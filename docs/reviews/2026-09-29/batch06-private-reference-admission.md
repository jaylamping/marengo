# Sixth repair batch: private current-reference admission

Baseline: checked PR218 merge `c3068c09233aab8a142610fcdda0d8386057e70a`.
Decision: [ADR0023](../../decisions/0023-private-current-reference-authority.md).
Active checkout: `J:\code\marengo`. Status: software slice locally qualified and
implementation-head GitHub checks pass; final evidence-head and safe delivery
remain required. [PR219](https://github.com/jaylamping/marengo/pull/219).

## Problem and resulting boundary

Removing historical startup grants left naked state setters, bench grants,
unchecked Ready and direct scoped Enable able to authorize motors. Cached
precommand feedback could certify SetZero and persist success without any later
response. One-target calibration enabled all five drives, including on a missing
sign attestation. Scalar verification also accepted nonfinite inputs and recorded
unsupported Hall/None methods as successful manual reference.

Davout now owns private current-reference authority. Ready, normal/direct Enable,
Active shortcuts, receive conversion, facets and motion output share admission.
Historical rows and scalar results cannot mint that authority. Generic mutable
bus/registry, unchecked Ready and synthetic bench/cache grant paths are removed.
Ordinary constructors refuse reference acquisition, raw SetZero and cached
verification until the backend/transaction is qualified. CLI calibration delegates
to the same preflight and capability admission before any Enable; Pi's existing
central call inherits the truthful refusal. No physical success is invented.

The permission binds the owner/backend realm and relevant installed identity,
transform, homing and reference/limit policy. Observed changes permanently revoke
it, including changes observed during receive before public fields are restored.
Faults and uncertain stop revoke it. Successful ordinary Disable preserves intact
reference but cancels motion generations. Output-only policy changes still pass
shared safety validation and final caps. Original installed routes remain available
for stop and peer-fault evidence even when public motor configuration is corrupt.
Full immutable configuration/model generation installation remains CS15 work.

ControlLoop's existing `supervisor_mut` permits replacement of a whole owner
with another owner of the same concrete bus type. Its permission travels with
its own backend realm; it cannot be exported onto a physical bus. Coordinating
the controller dynamics and whole-owner model generations remains CS15/CS21
work. The factory/type proofs cover transport/private-authority isolation.

Positive tests use a concrete closed finite SimulationBus with private identity
and specialized Supervisor/ControlLoop factories sharing production initialization
and execution. Scripts cannot wrap arbitrary transports, sockets or callbacks.
A restricted mutable facade exposes finite observations/rules/trace and cannot
extract or replace the backend. InitialVirtualReference declares initial virtual
conditions only; it does not qualify acquisition, persistence or firmware.

The Homing scalar validator rejects nonfinite position/bounds/tolerance/offset,
reversed/equal bounds, negative tolerance, joint mismatch and unsupported methods
before changing state or history bytes. Valid manual-history checks retain their
existing behavior while providing no Davout permission.

## Actual baseline proof and unchanged replay

All evidence stays under
`J:/code/marengo-migration-backup-20260929/batch06`.

| Executed baseline group | Exact pre-repair observation | Candidate proof |
| --- | --- | --- |
| Existing-public numeric matrix |22 actual assertion failures, one finite-history control; all 1,414 original Git blobs unchanged | Identical23 cases plus four prior scalar probes pass in a separate bound snapshot;54 complete Homing cases and strict lint pass |
| Old-public grant/verification diagnostics |8 actual reds plus 3 controls: setters/bench/unchecked Ready, healthy authoritative facets, cached SetZero success, arming before sign refusal and peer arming | Removed-API isolation and new authority/output conformance; missing-API compiler failure is not behavioral replay |
| Arc recording witness using unchanged public APIs |3 actual reds plus 2 controls: Unhomed scoped Enable, sign preflight arming and unqualified arbitrary-bus calibration success | Identical5 pass in the final separate bound snapshot; all 1,425 original candidate files and 39 frozen active overlays remain unchanged |
| Rejected restore atomicity |2 old-public exact-c306 assertion reds;2 private-authority candidate assertion reds and 1 successful replacement control | Identical5 pass in two independent final snapshots; proof kinds and original hashes retained separately |

The prior checked-c306 direct Unhomed/NaN/Hall/None reds remain in batch05's
next-authority preparation receipt. Overlapping groups above do not count as
additional independent defects. Initial fixture compile/config-path failures and
the missing-PROTOC launcher failure are preserved separately and excluded from
behavioral red claims. Source/compile binding and before/after hashes identify the
actual archived production files; candidate-only interfaces are conformance.

## Test quality and retained finding

Old cap, wire transform, freshness, watchdog, ordered fault, partial/failed stop
and controller cases migrate setup into the closed transport. Finite post-send
rules must actually trigger, and asserted failure cases reach actual MIT output
first. Raw frames exercise the real decoder; bounded typed reports cover
impossible-on-wire consumer cases only. Factory conformance separately proves
ordinary SimulationBus construction still grants no permission.

Berthier replaces one arithmetic identity property with actual friction wire
behavior across Impedance and GravityComp. Small-move replays supply independent
stationary raw observations and check actual rate/lead/output reachability. The
progress-reset case calls the production law with coherent measured q/dq and fixed
dt, removing2.5s of wall pacing. Actual controller stationary stall, persistent
fault/stop and bootstrap/expiry coverage remain. The first intermediate controller
run exposed three setup failures; the next focused run passes 175 cases, zero
ignored, about 0.68s assertion runtime. Final integrated Linux and primary reruns
also pass with the last reachability assertions and Davout changes.

Those failures prompted an independent probe of real motion followed by encoder
stoppage. On unchanged c306, a positive EMA residue prevents the ascent fuse from
tripping after 500 stationary samples. One actual assertion fails, zero ignored,
51 unrelated included-module cases filtered; all 1,414 original files are unchanged.
This is new [CS24](control.md#cs24--a-positive-velocity-filter-tail-prevents-stall-detection-after-motion-stops),
retained open with measured-progress/noise/actual-controller acceptance work.
Correcting a fabricated initial-pose jump in stationary fixtures does not fix it.
The control algorithm, fuse bound, limits and Wave sign-off are unchanged here.

## Review-driven restore correction

Independent review found `restore_limit_snapshot` assigned private model and
public policy before its Active/error guards. Actual probes showed an Active
rejection changing a hard bound while reference stayed Verified, and a Disabled
validation failure partially installing policy/model. The replacement now refuses
Active before mutation, validates and builds staged candidates, then revokes and
installs only on success. Rejection preserves complete model/policy/limits,
reference, stop generation and TX. Five in-repo cases strengthen these contracts.
Two targeted receive mutants also fail their actual regression assertion when
binding observation or installed-address lookup is removed. These are candidate
invariant checks, not additional c306 defect IDs or physical evidence.

## Standards

Independent review passes with zero documented violations or actionable smell
findings, including the bounded restore followup. External report:
`batch06/standards-review.md` under the evidence root above.

## Spec

One detected model-restore issue is resolved with actual red/green and integrated
evidence; zero remaining blockers for ADR0023's scoped admission slice. Qualified
transactions, whole-controller model installation and physical acceptance remain
explicit follow-ups. External report: `batch06/independent-spec-review.md`.

## Qualification and delivery

Final primary main-branch check passes **689 Rust tests, one existing ignored**,
**355 frontend** and **72 Pi-tool tests**, including fatal aarch64 release smoke.
Affected Linux strict lint/socketcan passes **388**, zero ignored; MuJoCo minimal
smoke and 5 harness tests pass. Native isolation executes 2 compiled controls and 14
meaningful expected denials; Linux executes 2 controls and 4 concrete transport or
private-factory denials, all on Rust 1.88. A guessed never-existing pairing name is
excluded from required counts. No socket is opened by compile checks. Test
assertions run separately from source review and from physical acceptance.

Final primary 100.59s, affected gate18.03s, minimal simulation 2.45s; command timing
includes startup/compilation and does not infer a cross-host speedup. The earlier
684-test primary is preserved as pre-restore evidence and superseded. Davout's
final 159 cases retain133 after 3 obsolete APIs are retired and add 26; Berthier's
173→175 change replaces one identity oracle and adds two factory contracts.
No newly ignored tests. The sole lock change adds already-pinned serde_yaml as a
Davout dev dependency; package identities/checksums/versions are unchanged.

All five implementation-head GitHub jobs pass at
`7951f10f874c3258602e8774b19882daf8f6b735` in
[run36706934339](https://github.com/jaylamping/marengo/actions/runs/36706934339),
including **73 actual virtual-CAN driver tests, zero ignored**. The check job
repeats the 689 Rust/one ignored, 355 frontend and 72 Pi-tool results; simulation
passes five harness tests. `github-implementation-verified.json` preserves counts
and exact head binding under the evidence root above.

The final evidence-head must pass all five jobs before safe merge, and the
postmerge main run must pass with the identical checked tree. The ledger records
delivery and review receipts; external `batch06/merge-receipt.json` will preserve
exact final/main hashes, tree equality, checked jobs and the recoverable branch
bundle for reconciliation in the next iteration. This avoids a self-referential
commit for its own merge receipt.

CS05, CS06 and CS07 stay partial. Bounded target-only acquisition, stop before storage, durable
outcomes, correlated installed-owner clients and reference-independent priority
stop remain required. A fresh CLI whose configuration/history startup fails is
still not an emergency stop. Physical device identity/reset continuity, drive
limits/timeouts, E-stop/Hall, plant/CAD parity and commissioning remain explicit
acceptance gates. This loop performs no robot operation, flash or deployment.
