# Proposed batch17 G15 slice — unexecuted

This design reads the current Store owner, ADR0029/0030, G15 acceptance and the normal/recovery tests. Start only after batch16 is delivered and its exact main checks pass; the next baseline must be recorded then. No new execution or demonstrated production defect is claimed here.

## Remaining software acceptance

Qualified behavior includes per-step DDL/data/FTS rollback on marker failure, retained prior commits, marker readback, supported upgrades, current timestamps, future/nonempty/OFF refusal, and one deliberately recovered complete v2/marker1 profile.

Still separate: actual process interruption at v2/v3 statement/commit boundaries; first-open journal-mode races and parallel openers; busy failure/retry; malformed normal markers; recovery against competing writers; backup/publication/cleanup failure races; and recognition/disposition of other historical prefixes. The existing output-stage failure and corrupt-FTS refusal do not establish every backup or publication failure. Physical power-loss/media durability and hardware acceptance cannot be inferred.

## Smallest next contract: reread after a competing writer

Qualify existing public Store::migrate against a separate SQLite writer. This is migration concurrency coverage, not first-open or recovery concurrency.

1. Independently seed a literal current schema with supplied settings/timestamps, log/session/config rows and working message/fields FTS. The worker opens this valid store, installs the existing public Connection::busy_handler through Store::connection, and announces readiness.
2. While that handle is idle, the controller independently creates the known obsolete cache and publishes marker2, preserving the complete literal v2 data. Observe that coherent state.
3. A separate controller connection starts an Immediate transaction, drops only that cache and writes marker3 with literal timestamp777, but leaves the transaction uncommitted.
4. Command the worker to call public migrate. Its busy callback sends one pipe message and waits for the controller's acknowledgement. This is actual SQLite reservation contention, not a sleep or an assumed scheduling overlap.
5. Only after receiving that callback message, commit the competing writer. Independently observe marker3/777, then acknowledge the worker. The callback returns true for that retry and false on unexpected additional contention.
6. Collect migrate success and final marker3/777, unchanged supplied metadata/history/sessions/config, working FTS, absent obsolete cache and integrity. Reopen through Store, close all handles, reap the worker and prove fixture cleanup before one collected oracle.

The noncapturing callback is supported by installed rusqlite0.32.1 without a new feature. It uses owned child stdin/stdout; no production hook, field, clock or injection interface is added. It deliberately replaces that caller connection's default busy policy, so this test does not qualify the default five-second timeout.

## Proof and failure containment

First run the delivered baseline positive. If it passes, add only the behavior test/documented qualification unless a real defect appears. A separate candidate conformance mutant moving schema_version read before Immediate reservation should cache2, then overwrite the competing marker timestamp after waiting. Require the sole final assertion red and unchanged positive replay; never call it an original regression.

Use bounded owned processes/pipe waits, kill/reap only owned children on protocol failure, and preserve unsuccessful evidence. Timeouts are containment failures, never successful concurrency observations. The executor must explicitly support the owned process tree before execution. Actual contention, competing commit and cleanup controls must precede the final oracle. No robot, deployment, limits or Wave changes follow.
