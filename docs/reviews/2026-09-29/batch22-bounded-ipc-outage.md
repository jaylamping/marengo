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
