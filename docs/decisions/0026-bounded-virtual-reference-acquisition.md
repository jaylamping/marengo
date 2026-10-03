# ADR 0026: bounded virtual reference acquisition without a grant

Status: accepted for software implementation, September 30, 2026.

## Context

Private current-reference admission now refuses cached calibration and ordinary
unqualified SetZero. Acquisition, recoverable durable commit and installed-owner
clients remain incomplete CS05/CS06/CS07 work. A new transaction must not regain
permission through a public bus callback, a recent pose or a historical row.
Shared feedback processing currently combines acquisition, hazard consumption
and automatic stop, which would create competing drains or recursive cleanup.

## Decision

Deliver the complete R2a virtual acquisition/cleanup sub-contract in Davout.
Begin, advance, cancel and snapshot own one reservation and a bounded terminal
cache. Owner-issued opaque stamps and handles bind a unique owner and monotonic
request identity; they are not permits or serializable receipts. An identical
request retries its retained outcome before stale-stamp rejection. Conflicting
reuse, foreign handles, expired outcomes and counter exhaustion refuse explicitly.

Generic/ordinary constructors have no acquisition capability, even when passed
SimulationBus. Only its concrete specialized constructor installs a sealed
private virtual backend, independently of INITIAL reference coverage. Starting
Unreferenced does not acquire permission. Physical/custom backends remain
Unsupported before any transaction arming or reference mutation.

Validate target, complete scalar policy, operator confirmation/sign attestation,
fault/E-stop/mode and the supported zero-offset ManualReference profile before
reservation. Selected sensor/offset workflows remain unsupported. Capture the
installed routes, policy and continuity; revoke old permission before mutation.
R2a may conservatively revoke all INITIAL coverage and must document that scope.
It never writes history or installs a new permit. Whole immutable config/model
ownership remains CS15/CS21. Typed installation APIs refuse while busy; direct
legacy public-field assignments cannot be refused before assignment. Observe
their mismatch before every forward action, cancel permanently, and always stop
the original installed routes. Snapshot inspection also retains an observed live
policy mismatch; restoring public fields cannot resume the reservation. The next
mutable advance or cancellation delivers its cleanup.

One advance performs one phase and at most one actual bounded FeedbackReport
acquisition: 64 raw frames/256 read attempts total. Old/post-arm flush must be
complete; incomplete work terminates instead of obtaining another fresh budget.
Move the existing ordered report consumer privately with its existing hazard,
chronology and error-precedence behavior. Ordinary callers retain their current
automatic stop. Reference callers use an explicit private mode context and own
same-call cleanup after the whole report is consumed. No receive result or
private context supplies output authority.

The closed virtual protocol is explicit: initial all-address stop, applied
reporting Off/quiescence, complete old drain, target Enable only, complete
post-arm drain, addressed SetZero once, then correlated evidence. There is no
run-mode write or ordinary MIT motion. Already-applied type24 streams require
actual bounded Off attempts; leases/sync/status solicitation cannot interfere.
All installed stop actions and finite reporting maintenance are declared
exceptions to the single forward target mutation. An uncertain initial stop
cannot arm; retain that completed report without a duplicate retry burst.

At attempted SetZero, invalidate only the selected old coordinate/derivative
cache, including uncertain writes. Retain peer caches and persistent hazards.
The sealed virtual rule mints correlation only from an actual addressed SetZero
occurrence, with owner/realm/transaction/device-reference epoch. A private
sidecar binds its exact raw-frame pop and per-report delivery ordinal. The frame
passes the real decoder/ordered consumer; only afterward can matching evidence
stage. Cached/raw zero, host timestamps, trigger counts, a typed queued report,
wrong realm/transaction/epoch or an earlier report cannot impersonate it.

Owner time and finite overall/phase deadlines govern progress; expiry wins at
equality before accepting a tied reply. The closed facade may advance its checked
monotonic virtual clock as data, independently of host receive time. Callers
cannot supply an evidence timestamp or an advance-now parameter.

Success ends EvidenceStaged, the actual immutable stop outcome and
CommitUnavailable, with usable_reference=false. Cancel, timeout, delivery
uncertainty and ordered hazards end with the initiating cause and same-call
all-address cleanup. Later healthy input, Disable or late proof cannot rewrite
the terminal result or restart it. Normal Enable/Ready/output remain denied.
Unconditional Disable cancels the live reservation through this lifecycle.
An unexpected error leaving `advance_reference` with its reservation still live
(2026-10-03 audit, WP-I) ends the transaction through the same lifecycle: the
all-address stop runs before the original error is returned.

Berthier's actual busy tick inhibits motion intent and advances this same owner
without a competing drain or MIT keepalive. Pi Quit/observed shutdown cancels
before storage drain. Armed transaction cleanup is mandatory even when ordinary
disable_on_exit is false; report it separately and do not duplicate the stop.
Normal skipped-exit behavior stays explicit when no reservation exists.

## Verification and following work

Qualify parity first with existing independent receiver/stop/reference tests.
New transaction APIs have candidate-only conformance, not missing-API baseline
reds. Literal wire controls must reach actual target Enable/SetZero and exact
proof pop, all installed cleanup actions, truthful failed writes, cancellation,
tied deadlines, peer hazards, reporting suspension and incomplete flush.
Use selected production mutants to prove sensitivity without multiplying old
receiver permutations. Retain useful INITIAL admission/output fixtures.

R2b supplies a bounded noncoalescing recoverable journal and a still-current
matching commit before any private grant. R3 migrates installed Pi/proto/gateway/
MCP/CLI request and terminal receipts. No synchronous historical writer in a
tick substitutes for that work. Physical identity/reset/ack/readback, drive
limits/timeouts, E-stop/sensors/support, model/plant and timing acceptance remain
external. No robot operation, raised limits or Wave sign-off change is included.
