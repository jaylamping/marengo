# Batch32 preparation: durable virtual reference history

Prepared October 2, 2026 against merged main
df468496eae0b3c30104874c3bc2b6ad218ec827. Worktree:
J:/code/marengo-worktrees/reference-history-journal, branch
codex/reference-history-journal. PR245 source qualification is complete; merged
main CI36977383698 is pending at this preparation. No R2b1 code, new test result,
finding closure, deployment or physical acceptance is claimed here.

## Actual remaining dependency

ADRs0023/0026/0027 and the current acquisition retain genuine closed virtual
raw-pop evidence, immutable cleanup outcomes, a private installed model and
live eligibility checks. There is no concrete durable writer or owner-consumed
completion. R2b1 must deliver that complete history path before R2b2 can install
a selected current virtual grant. History alone remains unusable for motion.
CS05/CS06/CS07 and other existing partial findings retain their current status.

Use the September30 batch11 next-r2b-implementation-plan.md as design context,
with the current source and a new ADR controlling implementation. The old
320KiB body proposal is superseded by a 1MiB cap: retained model and policy
already have independent 512KiB and 256KiB bounds. Preserve all qualified
acquisition, stop and initial-fixture tests unchanged.

## Concrete seams checked in current source

- reference_transaction.rs: Reservation stores a JSON Value policy; retain a
  private typed motors/control/homing snapshot at actual admission. RetainedStage
  holds AcceptedReferenceEvidence, original deadline, installed-model identity
  and reference continuity. The exact accepted position is f32, not f64. It does
  not retain a complete original CAN payload; do not claim reconstructed packets.
- reference_model.rs: InstalledModelStamp holds generation and an Arc containing
  the real RobotConfigFile and urdf_rs::Robot. Add only a private borrowed codec
  accessor. Its capped JSON/XML size check is not an exact numeric disk encoding.
- marengo-config types use Serialize/Deserialize and include flattened homing
  overrides and HashMap collections. Deterministic codec work must preserve
  scalar widths, optional values and collection keys; sort maps explicitly.
- Pinned urdf-rs0.8.0 uses YaSerialize, not serde. Full model coverage includes
  links, inertials, materials, visual/collision geometries and every joint field.
  Capsule geometry and spherical joints are supported pinned variants. Exhaustive
  field/variant coverage and independent bit checks precede journal qualification.
- reference_busy currently means acquisition reservation only. Berthier's actual
  busy tick advances that reservation. Extend both with the real commit lifecycle,
  fresh shared disabled receive drain and discarded old controller intent.
- Pi finish_owner_shutdown currently inhibits motion, cancels acquisition and
  attempts configured stop before draining the coalescing config writer. Journal
  shutdown must close both admissions and share one absolute wait budget while
  retaining separate actual stop and unfinished-worker evidence.

## Required implementation contract

Use private concrete journal/commit modules, one bounded noncoalescing worker,
eight credits including completed-unconsumed jobs, eight owner outcomes, at most
4096 rows and 64MiB of database pages. No public receipt importer, writer callback,
evidence constructor or grant setter. Enable the journal only through an explicit
Unreferenced SimulationBus factory and matching controller composition; ordinary
constructors and INITIAL fixtures retain no journal I/O.

Captured typed data is immutable. A versioned exact field codec preserves IEEE
f32/f64 bits, including signed zero and subnormals, with bounded strings and
collections and no silent truncation. Unsupported/nonfinite required values
refuse. Full decode and independent numeric coverage are prerequisites. The
codec's checksums detect mismatch and do not create private live causality.

SQLite is separate from telemetry Store. Verify exact schema/settings, preserve
valid hot journals and existing corrupt/incompatible bytes, take a real exclusive
lock, and use DELETE journal, synchronous EXTRA and zero busy timeout. Publish
durable history only after actual transaction commit and exact identity/body
readback. Failed, uncertain, cancelled and durable remain distinct. SQLite's
EXTRA adds directory synchronization for DELETE commits; actual media behavior
still requires external acceptance. Page limits cannot be lowered below existing
size, so reject oversized resources and verify the configured limit. See
[SQLite pragmas](https://www.sqlite.org/pragma.html#pragma_synchronous) and
[atomic commit assumptions](https://www.sqlite.org/atomiccommit.html).

Every completion advance first consumes the complete bounded ordered disabled
receive report, checks original deadline equality and owner/model/policy/device/
fault/stop/reference continuity, then accepts only its actual matching completion.
Cancellation immediately retires eligibility but cannot cancel OS writes. Startup,
recovery and copied databases leave targets Unhomed. R2b1 never grants motion.

## Qualification and preserved scope

Use real owned SQLite resources and closed virtual acquisitions to test actual
commit/readback, queue credits, lock/full/corrupt/schema/blocked-parent refusal,
cancel/late hazard/deadline/model mismatch and healthy neighboring work. Controlled
child crash tests distinguish uncommitted rollback from committed unpublished
history; they do not claim power-loss proof. Join/release owned resources before
decisive assertions. Actual controller busy ticks and shutdown must participate.
Freeze meaningful probes and use production mutants; missing APIs and setup
failures do not count as behavioral reds. Complete independent Standards/Spec,
primary gate, required CI and native Pi software qualification before delivery.

Work is confined to the owner's personal robot project and owned fixtures/Pi.
The installed robot remains unchanged. Gravity home is not current-reference,
support or E-stop acceptance. Before each physical movement, require a concrete
bounded proposal, explicit owner confirmation and commissioning checks. Ten
minutes without a reply leaves movement pending and permits independent work
only. Preserve CAD, branches, limits, Wave sign-off and paused automation.

## Interface points to settle before coding

Keep the existing ReferenceTerminal immutable: its ReferenceCommit::Unavailable
describes the acquisition, so adding a journal must not rewrite it retrospectively.
Give the new owner commit snapshot separate lifecycle, eligibility and actual
disk-result observations. A cancelled or expired lifecycle cannot become usable
because a worker later commits; the eventual durable row remains historical.
Cancellation/expiry and worker failure are distinct from a proven failed write.

Reserve bookkeeping and completion capacity for each accepted job until its
actual completion is consumed, even if the acquisition outcome cache rotates.
Never evict pending or completed-unconsumed correlation to admit another job.
An eligibility-cancelled job may release motion-busy state immediately while
retaining its worker credit and immutable input. A ninth accepted-write attempt
must refuse without replacing any of the eight retained jobs. Successful later
consumption returns a credit and allows genuine neighboring work.

An owner advance should consume no more than one real completion, after its
fresh ordered hazard drain. Retain the distinction between matching durable
history and current eligibility if a fault, model/policy change or original
deadline invalidates the stage while disk I/O is in progress. R2b1 still has no
permission consumer; R2b2 must explicitly apply its private current completion.
No disk parser or public snapshot can supply that private completion identity.

Exact event numbers need original typed capture, not conversion through today's
JSON Value stamp. HashMap iteration is nondeterministic and needs canonical key
order for idempotent bodies. Flattened configuration serialization needs explicit
coverage checks; the URDF projection should exhaustively name pinned fields and
variants so dependency changes cannot silently omit model data. Full real-plant
validation remains external to this storage codec.
