# ADR 0035: consume a current durable virtual reference for one joint

Status: accepted for software implementation, October 2, 2026. Superseded in
part by [ADR0036](0036-physical-robstride-reference.md): physical owners built
with the explicit physical constructors now acquire and select per-joint grants.

## Context

ADR0034 provides actual correlated virtual acquisition, mandatory cleanup and a
durable journal completion. Its explicit history factories always remain
unreferenced. The next R2b2 dependency must qualify the conversion from that real
owner workflow into private current permission before physical acquisition and
installed clients can use an equivalent lifecycle.

## Decision

Keep every existing history-only and INITIAL constructor and probe contract.
Add explicitly named `from_simulation_with_current_reference_journal` factories
for the closed Supervisor and ControlLoop. They start Unreferenced and install
one private selection policy in the existing commit owner. Only these concrete
factories select current virtual permission. No generic capability, public grant
method, supplied completion/writer callback or persisted permission is added.

The real completion consumer first consumes one fresh ordered bounded disabled
report. Complete receive, peer hazard/fault/E-stop handling, exact owner/realm/
target/device epoch, installed model, full captured policy, current reference and
post-cleanup stop continuity, latest actual acquisition, original unexpired
deadline, confirmation/sign and successful complete cleanup all precede
selection. It must take its actual matching queued job after COMMIT and exact
readback. Failed/uncertain writes, cancellation, shutdown and invalidation can
remain truthful durable history but never grant output. Reference counter
exhaustion refuses selection before changing permission. No acquisition terminal
or history object changes its immutable unusable meaning.

Select only the acquired joint. The private permission retains its own exact
job identity, backend realm, installed model identity, address and actual device
epoch beside the existing motor/homing/reference-policy binding. It survives
diagnostic cache eviction independently of the retained acquisition/commit
entries. A commit snapshot's `usable_reference` only observes whether that exact
job still owns current permission; copying it creates no authority. Consuming
the stage retires its transaction eligibility explicitly.

Selection starts a reference lifetime separate from the old transaction deadline
and stop generation. Passing the old deadline does not revoke an intact selected
grant. Ordinary successful Disable cancels motion and pending work while
preserving the current reference, as ADR0023 requires. A new acquisition, actual
device reset, installed model replacement, observed relevant public policy
change, owner/backend replacement, fault/E-stop, uncertain stop or shutdown
revokes it. Observed revocation stays permanent when old public values return.
Output-only policy changes retain reference only after the shared validator
accepts them. Explicit operator cancellation of the owning commit revokes its
current permission; cancellation of unrelated historical work cannot revoke a
different selected job.

Ready, ordinary/scoped Enable, Active shortcuts, commissioning facets and every
output continue using the existing common private authority. A selected grant
cannot make unreferenced peers or all-joint Ready eligible. The normal controller
busy tick remains the only receive acquisition during reference work, discards
old intent and produces no ordinary motion output in the consuming tick.

## Qualification and remaining work

Exercise actual Unreferenced acquisition with literal raw replies and proof pops,
all-address cleanup, real SQLite COMMIT/readback, fresh matching completion
consumption, selected Enable and nonzero output. Cover completed-unconsumed
hazards/work bounds/deadline equality, cancel/shutdown/late durability, foreign
identities, copied/inspected history and fresh startup, counter exhaustion,
post-selection deadline/Disable lifetime, device/model/policy revocation and
restoration, independent cache retention and healthy neighboring work. Use
meaningful production mutants after freezing probes; missing APIs and preparation
failures are conformance/setup results, not original-release behavioral reds.

Require independent Standards/Spec, primary and fatal ARM checks, exact PR/main
CI and native Pi software qualification. CS05/CS06/CS07 stay partial. Generic
physical owners still refuse acquisition and output. Physical firmware identity,
reset/ack/readback, reference-independent installed-owner stop, client migration,
plant/media/timing and commissioning remain separate dependencies. No physical
movement, motor-limit change, Wave sign-off or automation change is included.
Before each eventual movement, propose concrete bounds/duration/caps/stop and
require explicit owner confirmation. Ten-minute silence permits other work only.
