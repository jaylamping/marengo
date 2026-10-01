# ADR 0029: one owner for each store schema transition

**Status:** Accepted  
**Date:** 2026-10-01

## Context

Store migrations currently execute DDL and update the schema version in separate
autocommit operations. Refusing the version write after the v2 column/FTS change
or v3 cache removal leaves those changes committed behind an old version marker.
Reopening then repeats a non-idempotent ALTER. The Store mutex alone cannot
serialize independent gateway and CLI connections.

## Decision

Keep the public Store open/migrate interfaces. Store holds one mutable connection
guard through a private migration owner in `migrations.rs`. That owner starts a
SQLite Immediate transaction before reading the version. Each transaction applies
one next schema transition, its data/FTS transformation and its matching version
marker, then commits. The next iteration reads the version again under a new write
reservation. A later failure leaves the last completed version available for retry.
Propagate SQLite failures; rollback belongs to the transaction, with no suppression
of duplicate-column errors or unbounded busy retries. Retain the existing bounded
five-second busy policy.

Public connection access can disable rollback journaling after open. Refuse a
connection in journal_mode OFF before any migration/default write; a transaction
alone cannot provide rollback in that mode. Empty-database detection excludes
only SQLite's literal reserved `sqlite_` prefix, preserving valid user names
such as `sqliteXneighbor` as evidence that an unversioned database is nonempty.

Private marker/default helpers use the owner's connection or transaction. They
never call Store methods that acquire the same mutex. Bootstrap an empty database
as version 1; preserve the marker timestamp on an already-current database. Insert
missing archive/budget defaults only if absent, preserving supplied values/times.

Refuse future, invalid or missing-on-nonempty version markers before logical schema
changes. Recognize version 1 with an existing fields_json column as requiring
backed-up historic recovery, and return an actionable refusal. This normal upgrade
slice does not automatically repair historic prefixes or classify every corrupt
schema. Existing open-time WAL/NORMAL setup stays in place; logical refusal does
not promise an unchanged journal mode or byte-identical SQLite file.

## Consequences

Normal upgrades have a transaction and marker per step; a marker3 refusal after a
successful version2 transition leaves coherent version2 rather than rolling back
the whole chain. A current version3 open performs no legacy bootstrap DDL or marker
rewrite. SQLite's write reservation serializes migration work across connections;
deterministic opener races and complete first-open journal-mode races still require
separate qualification.

G15 remains partial until known historic prefixes have independently classified,
completed backups and verified recovery, and the remaining interruption/refusal/
concurrency acceptance is complete. These SQL contracts do not prove power-loss or
physical media durability. No robot operations, limits or Wave sign-off changes
follow from this decision.
