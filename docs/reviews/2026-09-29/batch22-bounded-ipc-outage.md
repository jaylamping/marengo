# Batch22: G01 bounded IPC outage transport — in progress

Fixed original baseline a65ae3f08e2601575b8038116bac03c998776bf8.
Branch `codex/bounded-ipc-outage`, isolated managed worktree. G01 stays open.

The frozen public regression sends1000 sequential robot/state samples before a
listener exists. The actual reconnect wire delivers sample0 instead of999,
reproducing stale FIFO backlog. An owned child confines the existing immortal
transport threads; a15second channel deadline kills/reaps a hang, reader threads
join, and the parent removes its temporary socket before the final assertion.
The initial missing-protoc build failure is excluded; protoc28.3 enables the real
assertion red. See original-binding.json for complete probe hash.

## Full remaining acceptance scope

Replace unbounded FIFO with finite latest-value slots for explicit telemetry
allowlist, independently reserved safety/heartbeat slots and bounded ordered
logs/audit traffic. Enqueue must not wait for a writer: declare item/byte ceilings,
record accepted/coalesced/dropped/disconnected outcomes and queue age/counters.
One in-flight write also belongs in the memory ceiling. Bound socket writes and
close/join old reader lifetimes on disconnect. Reconnect must not replay expired
state or any old enable/motion command. Gateway ingestion must invalidate prior
producer state on disconnect/reconnect. Expose actual connected status and queue
metrics through proto-first host metrics, replacing configured-as-connected.

Add actual absent-listener and accepting/non-reading peer tests with explicit
barriers and decoded frames. Assert queue/byte ceiling, latest eligible state,
bounded ordered audit policy, no stale command replay, deadline teardown and
truthful counters. No sleeps/RSS sampling as behavioral synchronization; generous
process deadlines only detect hangs. Native tests, strict clippy/fmt, independent
Standards/Spec review and exact-head Linux primary/runtime checks precede delivery.
No control logic, hardware motion, deployment, limits or Wave signoff changes.

## Current implementation receipt

The frozen complete probe replays green. Preserve its original bytes under
`evidence/batch22/frozen-outage-probe.rs`; shipping test formatting occurred only
following that exact replay. Seven explicit latest slots reserve safety/heartbeat
independently from128events/512KiB FIFO. Each payload is at most64KiB, maximum
queued payload983040bytes/135items. Overflow drops new ordered events, unknown or
command topics and contention attempts; telemetry replaces the prior same-topic
sample. Dequeue refuses telemetry older than1second. Round-robin classes avoid
an always-busy topic starving the event FIFO. Admission exposes typed outcome and
counters, including disconnected admission. No event-delivery guarantee is claimed.

One in-flight publication and its encoded frame are additional bounded64KiB copies;
queue payload counters intentionally exclude those and object/buffer overhead.
Receive headers refuse >128byte topics or >64KiB payloads before unbounded growth.
Each complete frame has an overall1second write deadline. Failure closes the
connection, shuts down its cloned reader and joins that reader before reconnect.
An explicit shutdown stops new admission/reconnection. Actual bounded connection
notifications support supervision; configured-as-connected wire metrics remain to fix.

Native Chappe11tests pass (one is the child entrypoint’s inert parent invocation),
strict all-target clippy passes. Actual absent-peer and accepting/nonreading-peer
checks assert queued item/byte bounds, overflow counters, reserved safety/heartbeat
admission and deadline disconnect. G01 remains OPEN: all remaining scope above is
still required. No PR yet; no independent/full Linux acceptance claimed.

## Qualification update

Current source includes owned listener/fanout shutdown, serialized peer generations,
gateway snapshot retirement and a proto RuntimeConnectionState transition for all
allowed subscriptions. Consul decodes that transition, clears previous producer
facts and cancels queued telemetry callbacks. A new IPC socket alone does not
repopulate facts. This is gateway-local connection identity, not Pi boot identity;
F05/G10 producer-age/boot/run-admission requirements remain separate and open.

Known commands must carry an Envelope timestamp within the preceding1second;
unknown, stale and future-dated commands never reach the runtime bus. Commands are
never admitted into the outbound outage queue. Failed in-flight writes have uncertain
delivery, increment a separate counter and are never replayed. Accepted queued audit
records retain their drop-new FIFO suffix on reconnect; this is bounded transport,
not durable audit delivery (G11 remains open).

Independent WIP review identified shutdown vs peer-install and shutdown vs queue
admission races; both cutovers now share their admission mutex. Controlled barriers
qualify both races. Injected monotonic time qualifies actual reconnect frames at
1000ms eligible and1001ms expired, without sleeps. A stalled actual peer is retired,
its socket name temporarily unavailable, then its replacement decodes exactly the
pending accepted audit suffix and latest reserved telemetry. Queue age/counters,
nonblocking contended admission, protobuf health conversion, actual gateway stream
transition and Consul late-throttle cancellation are qualified.

The complete original SHA-bound probe replays green on final source; the formatted
shipping test is restored after that replay. Native affected tests,356Consul tests,
build, strict Chappe/gateway clippy, workspace fmt and proto lint pass. Existing
non-Linux host-metrics deadcode warnings prevent treating native Mac as Linux parity.
Final Standards and Spec reviews report zero actionable findings. Broadcast-ring loss now emits a typed observation gap for both transports, discards retained backlog, and retires UI facts before fresh telemetry. A deterministic overflow test and encoded UI gap regression pass. The affected native suite passes71 tests (including the inert subprocess entry); exact-head Linux primary/runtime CI and delivery remain pending.
G01 is still open and no hardware or performance acceptance is claimed.
