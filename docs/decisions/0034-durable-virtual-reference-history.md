# ADR 0034: durable virtual reference history without motion permission

Status: accepted, October 2, 2026. Implementation is under software qualification;
this decision creates no physical capability.

## Context

ADRs0023/0026/0027 leave genuine closed virtual reference evidence current only
after acquisition and all-address cleanup. They have no concrete journal or
owner-consumed durable completion. Acquisition terminals are immutable and
unusable. The coalescing configuration writer and telemetry Store cannot supply
the required reference event identity, retention and disk ordering.

## Decision

Implement R2b1 as a complete private producer, noncoalescing SQLite worker,
inspection/recovery reader and owner completion consumer. R2b2's explicit current
selected virtual grant follows separately. A durable row, copied database,
history object, public snapshot or startup never authorizes motion.

Only a specialized Unreferenced SimulationBus factory and matching ControlLoop
factory install a journal using explicit independent history/journal paths.
Resolve existing path ancestors before construction, refuse shared slots and
case-only aliases across the database and its `-journal`, `-wal` and `-shm`
namespace, and pin relative history paths to their original absolute
spelling. Compare existing files through the pinned portable file-identity
library too, so hard links cannot defeat independence. This read-only preflight
creates neither resource and opens no SQLite connection; the journal's existing
symlink/resource checks still run on its supplied spelling at lazy open.
Generic, physical and existing INITIAL constructors keep no journal I/O. The
worker opens lazily after accepted work; default owner shutdown remains a no-op
for absent journals. No public writer callback, evidence/completion constructor,
receipt importer, backend setter or grant method is introduced.

`begin_reference_commit` accepts an actual retained acquisition handle and
bounded operator/session audit text. It checks the latest still-current stage,
confirmation, complete successful cleanup and owner/realm/device/model/policy/
reference/stop continuity. It reserves one of eight credits before acceptance;
credits include queued, writing and completed-unconsumed work. QueueFull is
retryable before acceptance. Identical accepted retry returns its existing
handle; a conflicting audit body refuses. One accepted job per acquisition is
retained independently of acquisition outcome-cache rotation. Counter exhaustion
refuses before acceptance. Accepted failure or cancellation cannot renew the
stage or its original deadline.

Commit handles bind a private owner and checked sequence. A separate immutable
commit lifecycle, current eligibility and actual disk-result observation preserve
cancelled/expired/invalidated work even when its eventual disk write succeeds.
The existing acquisition terminal's `ReferenceCommit::Unavailable` is unchanged.
Eight completed owner outcomes are retained in addition to the eight reserved
completion slots; pending correlation is never evicted to admit other work.

Capture typed motors/control/homing at actual acquisition admission, beside the
existing bounded policy attestation. Retain the actual immutable installed
RobotConfigFile/URDF via a private borrowed descriptor accessor. Worker input
contains immutable data, not owner invalidation cells or current files. It
records virtual evidence class, acquisition/job/session identities, target and
address, original scalar widths and IEEE float bits, sign/confirmation/method/
offset policy, installed-model generation, the exact accepted f32 position,
actual raw-pop ordinal/CAN ID/device epoch, original deadline and stop/reference
continuity, complete cleanup/reporting attempts and receive summary. It does
not claim a complete original CAN packet or reconstruct private host causality.

Use an explicitly versioned binary field codec with primitive tags, bounded
strings/collections/depth and canonical ordered string-key maps. Serde directly
captures typed configuration; it never passes through JSON Value. Exhaustive
URDF field/variant projections preserve the pinned actual model. Decode and
canonical re-encode must match the original bytes, including numeric widths,
signed zero and subnormals. Nonfinite required values, unsupported tags,
oversize, duplicate/noncanonical fields and trailing data refuse. The encoded
body cap is 1MiB, not the obsolete 320KiB proposal; model/policy retention keeps
its existing 512KiB/256KiB bounds. Checksum is corruption evidence, not authority.
Complete independent numeric/field coverage precedes software qualification.

Use pinned rusqlite on a separate explicitly owned database. Refuse symlinked,
oversized, corrupt or incompatible resources without recreating them. Preserve
legitimate hot journals for SQLite recovery. Verify exact schema and pragmas;
reject incompatible persistent journal-format header versions through a bounded
read before SQLite can change a mode or create WAL sidecars. Supported rollback
files still let SQLite perform legitimate hot-journal recovery.
use DELETE journal, synchronous EXTRA, EXCLUSIVE locking after actual exclusive
access, zero busy timeout, 4096-byte pages and at most 16384 pages/4096 events.
Bound cache, SQL/blob lengths and rollback work separately. Disable cache spills
during each bounded transaction, so the pinned single-database pager needs one
sector header and at most 16384
old pages with eight bytes of record overhead each. Its 65536-byte sector cap
keeps this below the conservative 65MiB rollback limit; journal_size_limit is
only a retention setting. This is a pinned software resource bound, not media acceptance.
Allocate a checked persistent diagnostic session in a real transaction.
Event keys use session plus fixed-width unsigned job bytes. Equal key/body is
idempotent; conflicting bodies never replace history. Publish DurableHistory
only after successful transaction commit and exact identity/body/checksum
readback. Proven precommit failure and post-commit/commit-attempt uncertainty
remain distinct. Never purge, VACUUM or rotate history in the control owner.
SQLite's EXTRA adds DELETE-journal directory synchronization; its durability
still depends on VFS/filesystem/media behavior.
[SQLite pragmas](https://www.sqlite.org/pragma.html#pragma_synchronous),
[atomic commit assumptions](https://www.sqlite.org/atomiccommit.html).
The read/write format versions are defined by the
[SQLite file format](https://www.sqlite.org/fileformat.html#file_format_version_numbers).

Each explicit commit advance first performs the shared ordered disabled receive
drain, limited to 64 raw frames and 256 attempts. Whole-report hazards and the
original deadline at equality precede completion acceptance. Fresh checks use
the actual pinned stage even after cache rotation. Observed mismatch is sticky.
Pending eligible commit work inhibits ordinary intent and typed installation;
the real Berthier busy tick advances this owner and discards old intent without
a competing drain or MIT output. Cancellation retires eligibility immediately
and frees motion-busy state while accepted disk work keeps its credit. A later
matching completion may describe durable history but cannot restore eligibility.

Shutdown inhibits intent and cancels reference eligibility before mandatory live
acquisition cleanup or configured ordinary stop. Reuse actual acquisition
cleanup; unarmed journal failure invents no stop. Close both storage admissions
before either wait. One absolute budget covers both drains, with independent
stop, accepted/queued/in-flight/completed-unconsumed, failure/uncertainty and
observed thread-termination reports. Join only a finished worker. Timeout or
owner drop cannot cancel OS I/O; accepted work and truthful uncertainty survive
on the worker. Recovery remains history and every new owner stays Unhomed.

## Qualification

Keep all qualified acquisition, stamp/model, history-write, INITIAL and stop
probes unchanged. New APIs use genuine closed acquisitions and actual SQLite
transactions, independently decoded disk data and meaningful production mutants.
Exercise gates before actual open/commit and after commit/before publication,
eight accepted cancelled jobs with ninth refusal and credit return, real lock/
full/corrupt/schema/blocked-parent errors, idempotence/conflict, deadline equality,
peer hazards, model/policy change and late completion, worker unwind, bounded
shutdown and actual controller/Pi composition. Controlled child crashes distinguish
uncommitted rollback from committed unpublished history; they do not prove
power-loss acceptance. Release/join owned resources before decisive assertions.
Missing APIs, compilation and preparation errors are not behavioral reds.

Require independent Standards/Spec, primary gate, exact PR/main CI and native Pi
software qualification. CS05/CS06/CS07 remain partial until their full contracts
are proved. Physical firmware identity/reset/ack/readback, plant/timing/media,
support/sensors/E-stop, installed clients and current selected grant are separate.
No deployment, motor-limit increase, robot motion or Wave sign-off change is
included. Prompt before every physical movement; explicit owner confirmation
and commissioning are required. Ten-minute silence permits independent work only.
