# ADR 0023: private current-reference admission

Status: accepted for software implementation, September 30, 2026.

## Context

ADR0022 separates persisted history from startup readiness. On its checked merge
`c3068c0`, direct scoped Enable still arms an Unhomed target, and the public
scalar verifier accepts NaN and unsupported Hall/None methods. Existing mutable
registry, unchecked Ready and synthetic feedback/grant APIs also let callers
fabricate current reference. Those states cannot constitute physical evidence.
The current Robstride adapter has no qualified device identity, reset continuity
or correlated zero readback contract. A newer host timestamp is insufficient.

## Decision

Davout owns one private current-reference authority at its output boundary.
History and scalar policy checks in marengo-homing do not confer permission.
Checked Ready, every Enable entry point, Active shortcuts and motion output
consult the same authority. Public facets project that authority. Arbitrary
registry mutation, unchecked Ready, cache insertion and bench grants cease to
be production permission paths. Stop remains unconditional with respect to
reference, and preserves fault and per-address delivery evidence.

Ordinary `Supervisor<B>::from_repo*` constructors have no qualified reference
capability, including arbitrary recording buses. They refuse successful legacy
calibration, raw SetZero and cached verification before arming or persistence.
Useful input/target/sign/method validation remains explicit. No physical success
path, protocol acknowledgement or recovery evidence is invented by this slice.
Legacy clients must handle refusal truthfully and must not energize first.

A private permit is tied to its owner/backend realm, installed relevant
motor/homing/model/policy binding and reference/fault continuity. It cannot be
serialized, imported, cloned into another owner or transplanted onto a new bus.
Mutable installed policy cannot retain a stale grant: close the seam or compare
its complete bound policy at admission. Fault, uncertain stop, reset, replacement,
reference-changing configuration or recovery revoke reference. An ordinary
successful Disable cancels motion while preserving intact reference continuity;
the old acquisition stop counter is not an ongoing validity predicate.
Output-only gains, torque caps and watchdog timing may preserve reference if the
shared safety validator accepts them before admission/output. Identity, joint
membership, model, homing, transform and hard-reference/limit changes revoke it;
restoring old public fields after observed invalidation cannot revive a permit.

Positive safety/controller tests use a closed concrete in-memory
`davout::simulation::SimulationBus` and specialized Supervisor/ControlLoop
constructors sharing the production admission, receive, stop and output logic.
The bus stores only finite data/scripts and records writes; it cannot wrap an
arbitrary bus, descriptor, socket, callback or send function. Ordinary constructors
do not opt into virtual permission even when supplied that concrete type.
No bus/state conversion exports the private permit to physical owners.

This admission slice permits a clearly named initial virtual reference fixture
at specialized construction. It covers admission/output/fault/stop from declared
initial conditions only. It does not prove reference acquisition, SetZero
causality, persistence ordering or physical qualification. The later bounded
owner transaction must replace that fixture with explicitly virtual correlated
evidence through the shared transaction engine.

## Verification and remaining work

Keep the existing independent torque, freshness, fault, stop and controller
oracles while migrating their setup to the closed virtual type. Exercise real
raw receive logic; typed impossible-on-wire reports cover only consumer behavior.
Require actual scripted triggers and no-TX/no-write refusal assertions. Preserve
the exact unchanged-source behavioral baseline and distinguish API-removal
conformance from unchanged-test red-to-green proof.

CS05/CS06/CS15 remain partial until their complete authority/capability contracts
are qualified. Target-only begin/advance/cancel, stop before storage, durable
outcome worker, owner/client receipts and reference-independent installed-owner
stop are separate CS07/CS09/F14/F23/T05/T12 work. A fresh CLI that fails startup
is not an emergency stop. Hardware acceptance, CAD/model parity, firmware
limits/timeouts, E-stop and Wave sign-off remain separate. This loop performs
no robot operation or deployment and raises no limits.
