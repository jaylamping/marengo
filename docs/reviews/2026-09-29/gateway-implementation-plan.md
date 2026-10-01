# Gateway, messaging and storage implementation plan

This plan covers every **G01–G21** finding in the [gateway review](gateway.md). It was
revalidated against `52f12678277a2cae786d8df648f035895789f026` and the working remediation
branch on September 29, 2026. Source and tests use the Windows checkout at
`J:/code/marengo`; Linux runtime checks use Docker from that checkout. No robot,
deployment, production database or physical CAN interface was used.

G11 duplicates CS22. G10 overlaps F05. Those IDs should share implementation and
acceptance evidence rather than become separate refactors. **G02 and G03 have been
repaired and tested in this branch.** The remaining nineteen G findings are open.
The original appendix is a dated review baseline; this document records subsequent
implementation status without claiming that planned work has already passed.

## Boundaries to preserve and deepen

1. **Chappe owns transport, queue policy, framing and peer lifecycle.** Its consumer
   should receive a current peer/session identity, bounded publication outcomes and
   disconnect notifications without inspecting a socket or guessing from configuration.
   It must never block the control producer or decide motor policy. Keep the portable
   bus and frame codec usable on Windows/Mac; isolate Unix socket I/O behind a backend.
2. **The Pi owns effective configuration and permission to manage the runtime.**
   Introduce a config/management authority with a small interface: validated mutation,
   effective snapshot, durable receipt and a quiescence grant. Existing pure parsing,
   validation and URDF merge functions in `marengo-config` remain reusable. Gateway
   routes translate requests and responses; they stop independently mutating master
   files, reconstructing live state from disk or authorizing management from a cached
   heartbeat. An ADR must settle the authority's library location before extraction.
3. **Store owns archive identity, database migrations and retention semantics.**
   Keep SQLite in `marengo-store`; deepen its public operations instead of distributing
   SQL/file logic into HTTP routes. Gateway schedules these operations on a bounded
   blocking executor and streams downloads. A read-only config snapshot should remain
   available when the observation/audit store is unavailable.
4. **Gateway startup owns listener readiness, authentication and TLS lifecycle.**
   Required listeners are acquired before startup succeeds and are supervised together.
   A single mutation/stream authorization policy applies to both typed and legacy APIs.
   Process liveness, robot readiness, sample freshness and persistence health are
   distinct states.

The authority changes are substantial enough to justify extraction or a new crate;
the HTTP framework, protobuf, SQLite and existing bus do not need blanket replacement.
First extract the contracts and their behavior tests, then migrate callers in small
reviewable slices. No hardware policy or wire protocol should change silently.

## Dependency order and release slices

| Slice | Findings | Dependency and completion boundary |
|---|---|---|
| GW-0, completed here | G02, G03 | Restore observability and retention immediately; no protocol changes. |
| GW-1, containment | G01, G07, initial G06/G10 guards | Bound transport queues; reject unknown/stale management state; authenticate every mutation and sensitive stream. These guards can ship before the larger authority extraction. |
| GW-2, platform and identity | G11, complete G01/G10 | Portable framing/bus plus backend lifecycle, peer boot/session identity, actual disconnect/age and bounded writes. Coordinate with F05 and CS22. |
| GW-3, effective config authority | G04, G08 | Full-generation write-behind, all request waiters, runtime CAS/no-op and replayable effective/durable snapshots. Requires protocol identity from GW-2. |
| GW-4, safe management and activation | G05, G06, G09, G21 | Pi quiescence grant, one YAML/URDF writer, prospective validation, transaction recovery and immutable commit receipts. Depends on GW-3 and Davout disable/fault outcome fixes (CS04/CS08/CS09/CS13). |
| GW-5, historical store correctness | G13, G14, G15 | Merge sparse imports, preserve chronology and atomically migrate schemas. Can proceed independently of GW-2–4; protect existing DBs before schema/data repair. |
| GW-6, bounded archive I/O | G16, G17 | Checked untrusted timestamp conversion, bounded decompression/page/request work, streaming downloads and executor admission. Builds on reliable Store operations in GW-5. |
| GW-7, service readiness and diagnostics | G12, G18, G19, G20 | Listener supervision first, then coherent TLS rotation and truthful metrics. Can proceed alongside GW-3 after identity/freshness contracts are agreed. |

GW-1 is containment, not permission to commission the arm. A fresh gateway heartbeat
cannot certify that an unacknowledged disable physically stopped a drive. Hardware
acceptance stays in the commissioning playbook after software failure-path gates pass.

## Finding-by-finding work and acceptance contracts

### G01 — bound disconnected and stalled IPC

**Status:** open; `IpcFanout` still creates an unbounded `mpsc::channel`, connects before
draining it and uses unbounded-duration synchronous socket writes.

**Scope:** `crates/chappe/src/{ipc,transport,lib}.rs`, Pi connection metrics and gateway
peer ingestion. Replace the raw FIFO with explicitly bounded classes: latest-value
telemetry coalesced per allowed topic, bounded logs/audit traffic and reserved current
safety/heartbeat capacity. Enqueue remains nonblocking and returns/records coalesced,
dropped, disconnected or accepted outcomes. Set write deadlines and invalidate stale
state on reconnect. Commands need independent admission/expiry rules; never replay
old enable/motion commands as part of an outage backlog. Expose queue age, capacity,
drop totals and actual connection state. WARN/ERROR bypassing the log quota must still
be constrained by transport memory limits.

**Acceptance:** exercise the actual transport with an absent listener and a peer that
accepts but never reads. Publish a deterministic large sequence of states and logs;
assert a declared queue/byte ceiling, nonblocking admission and accurate counters.
Release/reconnect the peer and inspect decoded frames: the state must be the latest
eligible sample, stale commands must be absent, and preserved audit ordering must match
the stated bounded policy. Coordinate blocked writers with channels/barriers; use a
generous process deadline only to detect a hang, not wall-clock sleeps or RSS luck.

### G02 — refill the structured log quota

**Status:** repaired in this branch. A captured monotonic origin replaces the
per-event `Instant::now().elapsed()` clock. Bucket and count share one atomic value,
so concurrent resets cannot separately race the count. An event sampled before a
newer bucket cannot roll the bucket back. WARN/ERROR bypass the normal-event quota.

**Scope:** only `crates/chappe/src/tracing_layer.rs`; no new dependency or global tracing
subscriber. An internal injected elapsed clock provides deterministic testing.

**Acceptance evidence:** the regression sends actual tracing events through the layer
and decodes `Envelope`/`LogEvent` from `Bus`. Before the repair, the second simulated
one-second window contained **0**, rather than **40**, normal events. The repaired test
checks the initial 40-event limit, suppression at 999 ms, refill at 1000 ms, warning/error
delivery and typed structured fields. Four simultaneous producers verify one shared
40-event quota with no duplicate publication. The tests contain no clock sleeps.

Two private helper assertions (literal level mapping and JSON substring lookup) were
replaced by those observable pipeline tests. JSON truncation coverage remains because
it protects a distinct payload-size contract. Suppression metrics should be exposed
with the GW-1/GW-2 transport health interface; this slice does not invent a separate
metrics wire contract or claim that overloaded IPC is fixed.

### G03 — remove recursive retention locking

**Status:** repaired in this branch. Retention selects session rows and artifact paths
using the connection guard it already holds, then deletes by the selected ID. It no
longer calls lock-acquiring `get_session` under that guard.

**Scope:** `crates/marengo-store/src/store.rs` and a public integration test in
`crates/marengo-store/tests/retention.rs`.

**Acceptance evidence:** before the repair, a child test process purging an expired
session exceeded its **five-second** deadline and was killed. After the repair, the
same test finishes in about **30 ms** locally. It verifies all three expired artifacts
and the old SQL session disappear, a future session and unregistered file remain,
only expired structured logs are deleted, the FTS query remains correct after reopen,
and a repeated purge is idempotent. A pipe/channel completion signal bounds the test
without polling sleeps; every fixture is isolated under a temporary directory.

Existing blob-removal failures are still ignored and SQLite/file deletion is not an
atomic cross-resource transaction. GW-5/GW-6 must define retry/tombstone and accounting
semantics for missing/unremovable files; that residual policy work is not the repaired
recursive-mutex defect.

### G04 — preserve configuration artifacts and every coalesced requester

**Status:** open; `ConfigPersistQueue::enqueue` still replaces the entire pending
`PersistRequest`, including `motors` and its one completion identity.

**Scope:** `bins/marengo-pi/src/{limit_persist,overlay}.rs`, authority library, protobuf
receipts and gateway ACK matching. Queue a complete effective snapshot/generation,
an artifact dirty set and all included request IDs. Coalescing may supersede older
snapshots; it cannot remove still-dirty artifacts or silently drop accepted waiters.
Each waiter gets one terminal outcome identifying the durable generation containing
its mutation. Persistence work remains off the 200 Hz thread.

**Acceptance:** block the real persistence worker at a deterministic seam, enqueue
limit→gain, gain→limit and different-joint limit updates, then release it. Inspect
resulting YAML/URDF using independent loaders and assert every accepted ID receives
exactly one correct receipt. Repeat with an injected write failure and worker shutdown.
A queue length assertion alone is insufficient.

### G05 — replace global persistence flags with authoritative generations

**Status:** open; any Durable event clears global pending/degraded flags, and normal
restart/update checks pending only. Gateway restart also forgets prior failure state.

**Scope:** authority snapshot/receipts, `state.rs`, `restart.rs`, `deploy.rs`. Publish
boot ID, effective generation, durable generation, outstanding mutations and explicit
failed artifacts. Replay this snapshot on reconnect. A request completion changes only
the generations/requests it actually includes. Normal management requires a Pi-issued
quiescence grant with a durable, nonfailed effective generation; an unrelated receipt
or gateway process restart cannot grant permission.

**Acceptance:** Pending A/Pending B/Durable A remains pending; Failed motors A/Durable
control B retains failure; reconnect/restart gateway during paused or failed persistence
retains refusal. Only a successful retry of the required generation unlocks normal
management. Drive the actual route and assert that its helper is never invoked when
permission is absent, rather than testing a Boolean predicate in isolation.

### G06 — require disabled, quiescent management authorization

**Status:** open; the current refusal predicate allows missing or old Active heartbeat
and even has a test asserting stale-Active restart succeeds.

**Scope:** all restart/update/activation routes, runtime authority, restart helper.
First reject unknown/stale state. Then replace gateway inference with a current,
boot-bound Pi grant obtained after it fences new motion/config commands, establishes
the required disable outcome and drains durable writes. Grants have identity, expiry
and single-operation ownership. Define a separately explicit recovery-stop operation
for disconnected failure cases; do not relabel recovery as normal safe restart.

**Acceptance:** an Active runtime with lost telemetry refuses all three normal routes;
missing state, future wall timestamps and mismatched boot IDs also refuse. A successful
disabled handshake followed by concurrent Enable must leave Enable rejected while
management owns the grant. Lost/expired grants cannot launch helpers. Replace the
existing stale-Active success test with this intended contract and retain the helper
stub solely to observe that no process launch occurred.

### G07 — one authorization policy for mutations and sensitive streams

**Status:** open; legacy Enable/Testing MIT/SetZero lack credential checks and CORS
allows every origin. Existing log-token checks do not cover these paths.

**Scope:** gateway router/middleware, legacy adapters, fallback stream, HTTPS/QUIC
admission and Consul runtime credentials. Apply fail-closed credentials/capabilities
to the entire mutation surface and sensitive subscriptions. Attestation, joint
allowlisting, rate admission and Davout safety remain independent gates. Restrict
origins and body/frame sizes. Replace static `VITE_*_TOKEN` secrets with a usable
runtime credential flow; a served JavaScript bundle cannot protect a reusable secret.

**Acceptance:** table-test every actual route with absent, incorrect and valid
credentials; decode the bus to prove unauthorized commands never publish. Include
legacy protobuf commands, JSON mutations, fallback streams and real QUIC admission.
Valid authentication must not bypass attestation, scope, rate or management checks.
Build a fixture with runtime credential entry and verify its static assets contain
no supplied secret. Browser preflight success alone is not authorization coverage.

### G08 — evaluate no-op against the effective runtime

**Status:** open; `limit_patch.rs` reports Durable without contacting the Pi when
the requested patch matches disk.

**Scope:** gateway limit patch adapter and authority mutation API. Forward patches
with the client's expected effective generation; the Pi alone decides no-op. A live
equal result is Durable only when the corresponding durable generation actually
contains that value. Expose the resulting effective/durable snapshot and exact request
receipt. Disk snapshots are boot/archive evidence, not runtime acknowledgements.

**Acceptance:** live limit changes from 1 to 2, persistence fails and disk stays 1;
requesting 1 must change the live aggregate back. Inspect live Supervisor policy and
the resulting receipt. Also test effective-equal-but-dirty and a stale client CAS token.
Handwritten response JSON cannot establish this contract.

### G09 — give all YAML/URDF writes one transaction owner

**Status:** open; gateway activation and Pi limit persistence share the same temporary
URDF name, with uncoordinated read/merge/promotion/rollback.

**Scope:** authority, gateway hardware adapter, `marengo-config` transaction helpers,
local sync/deploy/MCP bypasses (T02/T03). Require an expected master/effective generation
and send activation to the owner. Validate and stage a complete candidate with unique
temporary identity, then commit under one serialized transaction. A rollback may only
restore the generation it owns. Document crash recovery, journal/manifest commit point
and migration of current root files; cross-resource durability is not obtained merely
by adding unique `.tmp` names.

**Acceptance:** pause limit persistence before promotion, request activation, inject
YAML failure and reorder completion. Read resulting model and limits independently:
either one valid generation commits or a conflict is returned; later geometry cannot
be overwritten by an older rollback. Kill/reopen at transaction boundaries and recover
exactly one committed generation and its receipt. Keep stage/archive identities unique
and immutable for repeated imports or restores.

### G10 — make cached telemetry explicitly fresh, stale or disconnected

**Status:** open; snapshots retain payloads indefinitely, `/health` says `ok=true`
and Pi `ipc_connected` currently means merely that fanout was configured.

**Scope:** Chappe connection lifecycle, gateway snapshot cache/HTTP responses and
Consul F05. Record monotonic receive age and peer boot/session for each topic. Invalidate
or clearly label caches on disconnect/new boot. Split process liveness from robot
readiness; stale samples remain historical observations with visible age. Publish real
connection state rather than a configuration flag.

**Acceptance:** send valid telemetry through a real backend, sever the peer and advance
an injected clock. Health/readiness and snapshot responses must become stale/disconnected
even if the gateway and browser stream remain open. Reconnect with another boot and
ensure old safety/config samples cannot authorize operations. Browser fake-timer tests
must independently age its samples while no frames arrive.

### G11 — portable bus/framing, explicit robot platform backend

**Status:** open; unconditional `ipc` references Unix types and `Transport` embeds them.

**Scope:** Chappe module boundary, platform runtime entry points and CI. Keep bus,
codec and contracts portable; isolate Unix transport behind `cfg(unix)`. Choose an
explicit local Windows backend (loopback TCP or named pipe) if desktop runtime
integration is supported. Linux robot I/O stays a target/backend and does not require
an Ubuntu source checkout. Unsupported platform binaries must fail clearly or be
excluded deliberately, not force every library test through Unix imports.

**Acceptance:** native Windows and macOS compile/test portable libraries, plus a
desktop backend round-trip if provided; Linux runs IPC and virtual-CAN integrations.
Use the same wire fixtures and malformed-frame corpus on every backend. A native
Chappe build and in-process publication test are required evidence for CS22/G11;
documentation alone does not close them.

### G12 — rotate owned TLS material and supervise reload

**Status:** open; automatic cert validity is 13 days and material is loaded at startup
only. The validator also does not enforce its documented key algorithm.

**Scope:** TLS identity/lifecycle module and supervised HTTPS/QUIC listeners. Renew
automatically owned material before expiry and coherently update both listeners,
published fingerprint and reconnect behavior. Validate actual ECDSA P-256, lifetime,
usage and key pairing. Separately provisioned credentials need explicit ownership and
expiry handling; they should not be overwritten as if they were generated defaults.

**Acceptance:** injected wall/monotonic clocks cross renewal and expiry without a
multi-day wait. Real client connections after rotation validate the currently
published fingerprint for the actual listener cert. Failed renewal preserves the
last usable identity and reports degradation; malformed/wrong-key/custom-expired
material follows the documented policy. Couple reload failures to G20 readiness.

### G13 — merge omitted imported artifacts without erasing siblings

**Status:** open; sparse `register_session` upserts assign omitted artifact columns
to NULL, so a bench/candump/trace import overwrites prior references.

**Scope:** `Store` registration/import API and CLI import reporting. Group files by
session or add explicit preserve/set/clear artifact semantics. Preserve existing
label, capture start/end and siblings unless explicitly changed. Count unique sessions
and artifacts separately. Explicit removal should have its own operation.

**Acceptance:** import all three files for one session in each order, import again,
archive and reopen. Public session/download lookups must return all three correct
contents and preserve metadata; unique session count is one. Include missing sibling,
duplicate candidate and an explicit clear operation. Inspect outcomes rather than SQL
statement strings.

### G14 — preserve capture chronology

**Status:** software verified in completed batch13/PR227: implementation cabfe945,
final1d3d85b and equal-tree main4c1800d pass all five jobs in runs36818692724,
36820454955 and36821026729. Verified backup and branch cleanup complete. ADR0028 defines UTC,
invalid-date refusal, authoritative metadata and unknown-end policies. Four actual
original-public groups, candidate cutoff conformance/mutation, timezone and required
local gates qualify the repair. Automatic legacy metadata rewriting remains deferred.

**Scope:** Store timestamp parser, import/archive metadata and an optional backed-up
repair command. Parse the capture ID as UTC explicitly. Record unknown/invalid dates
as unknown with a stated policy; do not silently fabricate capture chronology. Repairs
must preview proposed changes and preserve manually supplied authoritative metadata.

**Acceptance:** actual import/archive round-trip of `20200101T000000Z` yields the
independent expected UTC epoch **1577836800000 ms** after reopen. Test leap dates,
invalid IDs and preservation of explicit metadata. Verify date/time filters include
the historic session at its capture time, regardless of host timezone/import date.

### G15 — atomic, serialized schema migrations

**Status:** partial in [batch14](batch14-store-migration-atomicity.md), ADR0029.
Normal upgrades use one guarded owner and an Immediate transaction per schema
step plus its marker. Six original-public assertion reds replay unchanged green;
eight positive behavior cases and a production marker mutant/replay qualify
rollback, per-step commits, future/nonempty/OFF refusal and metadata preservation.
Required local gates and PR228 implementation5b3f56f/run36831085548 all five
CI jobs pass. Finala7fb73d/run36832672947 and equal-tree main3345f129/run36834154060
pass all five jobs, with fatal main ARM release and verified backup/branch cleanup.
Known historic
fields_json-present/version1 schemas receive an actionable refusal, with no
automatic recovery. Full historic backup/recovery and remaining interruption,
first-open/parallel-opener, busy and malformed-marker acceptance remain open.

[Batch15](batch15-store-marker-progress.md) adds stored-marker readback before
commit. The exact original public failure leaves cache removal committed under
marker2/counter1; unchanged repaired proof preserves the entire prior version
and permits real retry after trigger removal. Affected30 and primary765Rust
(1existingignored),355frontend/72PiMCP/fatalARM pass. PR229 bb22fc7/main d660112
equal-tree all-five-job delivery, independent reviews and scoped backup/cleanup
complete. This bounded trigger proof does not qualify competing writers or
first-open concurrency.

[Batch16](batch16-store-historic-recovery.md), ADR0030, adds explicit recovery of
one complete known v2 schema with stale marker1: read-only pinned WAL snapshot,
verified standalone backup before separate output repair, FTS consistency and
source/history/settings preservation. Actual refusal/namespace/failure/retry/CLI/
default tests and WAL/FTS production mutants with unchanged replay qualify this
profile. Native34/affected36/primary771Rust/1existingignored and fatalARM pass.
Final review/exact-head GitHub delivery remain required later receipts at this
checkpoint. Other historic prefixes and interruption/concurrency remain open;
G15 stays partial.

**Scope:** Store open/migrate and migration recovery. Acquire a SQLite write transaction
for each migration plus its version marker and required data/index transformations.
Serialize parallel openers and reject unsupported future versions. Detect and recover
known partially migrated historic schemas deliberately; blindly retrying ALTER TABLE
does not repair an interrupted old release. Back up before any nontrivial recovery.

**Acceptance:** seed an independently constructed v1 DB with logs, interrupt at each
v2/v3 boundary, reopen and verify preserved logs/FTS plus `PRAGMA integrity_check`.
Rollback after injected failure must keep a fully usable prior version; successful
reopen must have one valid final version. Cover parallel openers, schema-ahead-of-binary
refusal and the known fields_json-present/version-1 partial schema.

### G16 — bound blocking archive work and stream downloads

**Status:** open; page reads accumulate the whole decompressed file, HTTP handlers
run synchronous Store/file work on Tokio workers, and downloads read the full artifact.

**Scope:** Store paged reader/metadata, gateway archive handlers and bounded executor.
Retain only requested page rows, cap line/decompressed/body/page sizes and define total
count/index caching. Execute SQLite and file work on a bounded blocking worker pool
with bounded admission; stream download bytes with cancellation. Define archive/read
snapshot semantics and count writer failures/last success separately from queue drops.

**Acceptance:** read real plain/gzip fixtures through public routes and verify exact
pages, truncation/error limits and streamed bytes. A counting reader/allocation seam
checks retained memory rather than asserting elapsed milliseconds. Hold all archive
workers at a barrier: additional admitted work is bounded/rejected while cheap health
and telemetry requests complete before the barrier releases. Aborted downloads release
their resources; corrupt/oversized gzip cannot monopolize unbounded work.

### G17 — reject out-of-range candump durations without panic

**Status:** open; scanner and JSON offset deserializer still use `from_secs_f64`
after checking finite/nonnegative only.

**Scope:** `marengo-candump` scan/serde error contract and CLI. Use checked conversions,
declare accepted timestamp/delta domains and classify malformed frames consistently.
Validate ASCII DLC versus payload count. Do not turn an untrusted capture into an
unwind or unlimited line/decompression allocation.

**Acceptance:** public scan and JSON decode handle `1e30`, NaN/infinity, negative and
regressing timestamps, rounding edges, truncated gzip and inconsistent DLC. The actual
CLI returns a documented input error instead of exit 101. A small deterministic corpus
and checked-conversion boundaries are sufficient for the default gate; extended fuzzing
can run separately.

### G18 — correct CPU counter indexing and identity

**Status:** open; cpuN labels leave N in parsed counters, and iowait reads IRQ's index.

**Scope:** host metrics pure parser/sample-delta module. Parse the CPU label separately,
validate fields, use the Linux documented counter positions and key prior samples by
CPU ID. Regression/hotplug produces an explicit unknown/new baseline rather than a
misleading percentage.

**Acceptance:** fixtures from `/proc/stat` with known independent deltas produce
20% busy from 10 user + 10 system + 80 idle; iowait and IRQ are distinguished.
Include cpu0/cpu1, sparse IDs, hotplug, truncated/malformed fields and counter reset.
Observe published HostMetrics, not merely a hostname or parser-shaped arithmetic test.

### G19 — report real CAN and mount states

**Status:** open; the last token on a CAN state line is returned and read-only is
searched in `df` columns that contain no mount flags.

**Scope:** host metrics collectors/parsers. Read structured `ip -json -details` or
the token immediately following `state`; determine read-only from mountinfo/statvfs
or another real mount flag source. Keep filesystem type distinct from device source.
Represent failed/unsupported collection as unknown rather than healthy writable.

**Acceptance:** captured normal/flagged/BUS-OFF/ERROR-PASSIVE CAN fixtures preserve
the true state despite restart-ms values. Mount fixtures distinguish ro/rw, escaped
paths, device and filesystem. A fake command/file adapter drives the actual metrics
collector with failed outputs and verifies published unknown states. No physical CAN
or root remount is necessary.

### G20 — fail or degrade coherently when a required listener fails

**Status:** open; HTTP/HTTPS listener tasks are detached and only log failures while
the main QUIC server continues.

**Scope:** gateway runtime supervisor/startup. Bind required endpoints before readiness,
declare optional listeners explicitly and supervise their lifetime together. A required
listener failure exits for service-manager recovery or enters a specified degraded
retry state; neither may remain nominally ready. Ensure cancellation stops sibling
tasks and flushes bounded observation work without delaying motor safety cleanup.

**Acceptance:** occupy each required port and start the real gateway process with
isolated paths: startup fails without a success-ready state. Inject one listener's
post-start termination and observe process exit or explicit degraded retry. Optional
listeners follow their declared contract. Assert on real process/readiness behavior,
not a `tokio::spawn` source substring.

### G21 — report committed URDF activation truthfully

**Status:** open; promotion succeeds, staging is removed and a fallible completeness
read can still turn the response into HTTP 500.

**Scope:** prospective validation and commit receipts in the G09 owner. Compute candidate
completeness before committing. Store request identity, generation, checksum, archive
identity and `restart_required` in an immutable outcome that survives a lost response.
After the commit point, advisory refresh/cleanup errors are secondary diagnostics,
not proof that the mutation failed. Retry retrieves the original receipt.

**Acceptance:** inject a candidate-config read failure before commit and verify unchanged
master bytes. Inject reporting/cleanup failure after commit and verify the applied
receipt is returned or recoverable. Retry after a lost response yields the same
generation/checksum/archive with no second mutation. Restart/crash recovery must agree
with master contents, and the currently loaded Pi model remains explicitly older until
the coordinated restart completes.

## Test selection and validation of this slice

The tests added here assert external effects: decoded tracing messages, real artifact
contents/existence, reopened SQLite rows and FTS results. The rate clock seam avoids a
one-second sleep; the retention subprocess prevents the original deadlock from hanging
the suite. Its five-second deadline is failure containment, not the expected test cost.

On this branch, the focused Docker command
`cargo test -p chappe -p marengo-store` passes **13 tests** (7 Chappe, 5 Store unit,
1 Store integration). Test execution reported about **60 ms**, **40 ms** and **30 ms**
for the three binaries, respectively; those are observations, not brittle timing gates.
`cargo clippy -p chappe -p marengo-store --all-targets -- -D warnings` also passes.
The parent remediation change still requires its complete repository gate before merge.

The future default gate should run one small deterministic acceptance scenario per
contract, with fixture matrices inside it where cases share setup. Keep separate tests
when they protect distinct public behavior or isolate failures. Retire tautological
private helper/default/string-shape checks only once a stronger behavioral test replaces
their intent. Build dependency cost, fixture I/O and simulation cost need separate
measurement; deleting instantaneous assertions cannot substantially speed compilation.
Scheduled stress/fuzz/platform/hardware suites should state their own budgets and
must not masquerade as a passing default safety acceptance gate.

Batch16 final local correction: independent Spec found stdout failure after
completed recovery omitted artifact paths. Actual line415 CLI regression
fails on implemented2b0 and passes byte-identically after serializer/write/
newline/flush error reporting. Phase05 library proof scope remains unchanged;
only two CLI core files differ. Final phase06 native34/affected37/primary772
Rust/1existingignored,355frontend/72PiMCP/fatalARM qualify1464 unchanged inputs.
V3 startup failure is preserved/excluded; final-local-qualification.json binds
actual red/green/gates and exact delta. Final independent review and GitHub
delivery remain pending; G15 partial and102ID/eight-task dispositions unchanged.

## Completed delivery and owner-requested stopping point (October 1)

PR230 finalf7769e0/run36860217322 and equal-tree main63cbe7e/run36861055844
pass all five jobs:772Rust/1existingignored,355frontend/72PiMCP,5sim/73vcan
(zero ignored in sim/vcan), including fatal main ARM release. Independent
Standards/Spec and separate library/CLI execution audits accepted. Verified150-ref
Git backup and exact repair-branch cleanup preserve145 unrelated refs and both
historical worktrees. Earlier pending checkpoint text is superseded by these
completed receipts; it remains historical evidence. Portable exact receipts and
reviews are in [evidence/batch16](evidence/batch16/MANIFEST.json).

The owner requested a pause and will resume on Mac. The Windows loop is PAUSED;
no batch17 code or proof has started. [HANDOFF.md](HANDOFF.md) is the restart entry
point and distinguishes Git-tracked records from Windows-local CAD/raw archives.
G15 remains partial; the same102IDs/statuses and eight maintenance tasks remain.
No robot operation, physical acceptance, limits or Wave sign-off change occurred.
