# ADR 0027: retain virtual reference evidence with checked model continuity

Status: accepted for software implementation, September 30, 2026.

## Context

[ADR0026](0026-bounded-virtual-reference-acquisition.md) delivers bounded closed
virtual acquisition and mandatory cleanup. Its immutable terminal is unusable
and has no durable commit. The actual matched pose/private raw-pop correlation
and original reservation continuity are currently discarded. The reference
authority is already revoked, so its generation alone cannot identify a later
successful private URDF replacement. A future journal must receive actual
retained evidence and reject a stale installed model.

## Decision

Deliver the small R2b0 continuity contract before adding storage or permission.
Keep existing acquisition phases, immutable terminals, cleanup and
`CommitUnavailable`/`usable_reference=false`. Add inspection-only
`ReferenceStageStatus` to `ReferenceSnapshot`: `NoEvidence`,
`CurrentVirtualEvidence`, or `Invalidated(ReferenceStageInvalidation)`. This is
an owner observation, never a durable receipt, readiness flag or permit.
Snapshot for an old retained handle projects that exact outcome. A newer
current stage cannot supply an old retry's status.

Only the real AwaitEvidence match can create private accepted evidence. Retain
the exact owner/realm/transaction, installed address, actual device epoch,
raw-pop ordinal/CAN ID/time and finite decoded joint position. The retained
request already captures complete bounded motors/control/homing policy and
operator confirmation/sign attestation. Also retain its original overall
deadline, captured model identity, actual post-cleanup stop/reference continuity
and cleanup result. Retention uses the existing eight-outcome cache; there is no
separate unbounded history. No public evidence constructor, importer, epoch
setter, writer callback or grant method is added.

Davout owns a private immutable model snapshot containing the actual privately
installed robot and parsed URDF values. Check its serialized size with a capped
writer (512 KiB) before cloning it at startup or installation; retain the typed
values behind an Arc, including their original numeric values. That size check
is a memory bound, not a persistence encoding or physics-validation oracle.
Snapshots and acquisition share the immutable object and do not read files or
clone the model in a tick. A separate checked installation generation advances
on every successful `rebuild_limits`, `apply_limit_patch` and
`restore_limit_snapshot`, even for byte-equivalent replacement. Stage all
validation/model allocation before assignment. Exhaustion refuses before
installation or reference mutation; invalid candidate restores preserve the
old installation. Opaque acquisition stamps also bind this installed model, so
a pre-install stamp cannot attest a later installation.

One private current-stage validator is used by actual finalization after stop
and backend end, snapshot observation and the existing reference-binding
observation paths. Eligibility requires the latest actual acquisition, complete
matching evidence, successful all-address cleanup, unchanged owner/realm/device
epoch/reference/model/policy/stop continuity, no fault/E-stop/Active intent and
owner time strictly before the original finite deadline. Query the closed
device epoch privately by captured installed address after backend end.
Stop-generation saturation cannot prove continuity: MAX is ineligible. The
virtual clock retains ADR0026's declared deterministic clock contract.

Observed mismatch invalidates retained stage eligibility permanently, even if
legacy public fields are restored. Use direct private generation/policy checks
to avoid recursion through reference-binding observation. A new acquisition,
explicit Disable, shutdown or deadline expiry makes the old stage ineligible.
Shutdown records this invalidation even with no live reservation, retaining
existing optional exit-stop/Option behavior. Read-only observation adds no TX;
the acquisition's immutable terminal and original cleanup report never change.
This stage rule does not change ordinary Disable's treatment of an already
installed future grant under ADR0023.

## Verification and following work

Keep every previously qualified R2a test. New interfaces use candidate
conformance and selected production mutants, not missing-method baseline reds.
Use a real Unreferenced closed owner, actual target Enable/SetZero, literal raw
reply and the exact proof pop, followed by literal all-installed-address cleanup.
Observe CurrentVirtualEvidence only after that real cleanup. Then exercise
successful URDF-only replacement and restoration, invalid-restore preservation,
equal installs/rebuild/patch, sticky public-policy edits/restoration, old-handle
retry, new acquisition, Disable, shutdown and exact deadline expiry. Capture
state/trace before fallback cleanup and prove healthy neighboring acquisition.
Private counter-boundary injection tests exhaustion only; it supplies no pose,
proof or permission. Tests must discriminate actual behavioral mistakes rather
than repeat the generation formula.

R2b still requires a bounded noncoalescing recoverable journal and a matching
still-current durable completion before a consuming private grant. R3 remains
the installed Pi/proto/gateway/MCP/CLI migration. CS05/CS06/CS07 stay partial;
immutable configuration authority, current model/plant validation and physical
identity/reset/ack/readback, drive limits/timeouts, E-stop/sensors/support and
timing acceptance remain separate. No robot operation, increased motor limit
or Wave sign-off change is included.
