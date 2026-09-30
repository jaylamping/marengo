# ADR 0019: Runtime authority and independent verification

Status: **Proposed implementation design**. September 29, 2026. The repair roadmap
authorizes implementation in reviewed slices; the interfaces below are targets,
not capabilities already supplied by the current runtime.

## Problem

The repository review found safety and lifecycle rules spread across Davout,
Berthier, Pi/gateway binaries, browser timers and SSH tools. A successful HTTP
request can mean queued, live-applied, durable, or verified, depending on the path.
Cached state can authorize actions after its producer disappears. Several tests
check formatting or the same formula as the implementation while important
failure paths remain uncovered.

## Design

Retain the existing crate responsibilities. Replace distributed ownership with
three deep modules rather than add another facade around the same behavior:

1. **Davout safety supervisor** owns admission to physical output, validated
   configuration, per-drive freshness, normalized latched faults, legal output
   and explicit stop outcomes. Robstride owns checked vendor protocol encoding
   and transport; it does not own joint-space policy.
2. **Pi MotionSession** owns a run's admission, boot/run identity, command
   generation, lease, phase, completion and cancellation. Berthier remains the
   joint-space control executor. Browser and CLI clients request intent and
   observe receipts; their timers cannot own physical playback or re-enable.
3. **Pi ConfigAuthority** owns the live and durable config generations,
   serialized complete snapshots, correlated receipts, reference invalidation,
   and the quiescent management handshake. General model promotion remains
   restart-required. Gateway and deploy tooling use that authority rather than
   writing competing master files or inferring permission from cached telemetry.

Every mutation carries a request ID, expected boot/config/run generation where
applicable, and a typed outcome. Queue acceptance, applied state, durable state,
reference verification, and stop uncertainty remain distinct. Unknown or stale
state cannot authorize a mutation. Authentication protects these operations but
does not replace runtime admission.

Stop invalidates earlier command generations before downstream cleanup or disk
work. A software stop has a defined effect for each mode and supported-arm
condition. An unsuccessful CAN stop remains visible; Disabled does not prove a
physical drive has stopped. Resetting a fault and enabling are separate explicit
operations. Drive-local timeout/torque readback and physical E-stop behavior need
hardware acceptance before the software can claim those guarantees.

Telemetry carries producer identity and original acquisition time. Consumers
derive age and distinguish current, stale and unknown. Transport queues are
bounded and cancelable without blocking the 200 Hz owner. State may coalesce;
correlated outcomes and safety-critical events cannot silently disappear.

## Verification and migration

Public interfaces are the main test surfaces: Supervisor with a fake CAN
adapter, actual protocol frames, MotionSession request/receipt flows,
ConfigAuthority transactions with injected I/O failures, public Store operations,
transport lifetime and operator interactions. Expected behavior comes from a
written invariant, protocol capture, analytic rigid-body example, or independent
plant, rather than rerunning implementation formulas in the assertion.

Introduce the owner and its tests, migrate all callers, then remove the old
owner, flags and overlapping tests. Do not keep both command paths live during
migration. Preserve the five-joint master, joint-space transforms, explicit
commissioning scope and existing torque/velocity ceilings. Proto changes precede
generated code and coordinate the Pi, gateway and browser cutover. Old clients
must fail closed when required identities or receipts are unavailable.

The first repair batch changes the torque limiter, log quota, Store retention
locking and gravity COM transformation. Those bounded correctness repairs do not
implement MotionSession or ConfigAuthority. The
[implementation roadmap](../reviews/2026-09-29/implementation-roadmap.md) tracks
the remaining migration and the test budget.

## Alternatives and consequences

A whole-repository rewrite would discard useful driver, model and commissioning
knowledge without proving better behavior. Rewrite an owning module when its
current interface cannot express the invariant, and retire its superseded
implementation in the same cutover. Extraction from the large Pi/gateway bins is
appropriate for motion/config ownership; planning/perception scaffolds need not
be expanded to repair current bench workflows.

This design adds explicit identities and recovery states to wire contracts, so
compatibility and interrupted-transition tests are necessary. It concentrates
reasoning and test coverage in fewer interfaces and removes caller ordering
requirements. A passing software suite still cannot certify physical calibration,
arm support, drive firmware settings, CAD geometry or commissioning completion.
