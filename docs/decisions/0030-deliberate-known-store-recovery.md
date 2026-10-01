# ADR 0030: deliberate recovery of a recognized historical store

**Status:** Accepted  
**Date:** 2026-10-01

## Context

ADR0029 refuses marker1 when `log_events.fields_json` already exists. That
column alone cannot distinguish a completed v2 transition from an interrupted
column/FTS prefix or arbitrary damaged data. Normal Store open must retain this
refusal. A separate explicit operation can recover one independently recognized
historical state while preserving evidence for retry and further diagnosis.

## Decision

`marengo-store` owns one public synchronous operation:
`recover_known_v2(source, backup, output) -> Result<RecoveryReceipt>`.
The CLI's explicit `recover-known-v2 --source ... --backup ... --output ...`
dispatches before normal Store open. Gateway startup never opts into recovery.
This operation reads one existing source snapshot, completes a verified backup,
and publishes a separate recovered database. It does not replace the source.

The first supported profile is the complete known v2 schema with marker1:
settings, log events, their indexes, sessions, configuration overrides, the
three-column external-content FTS index and its three expected triggers. Compare
the complete schema with a trusted private v2 reference. SQL recognition keeps
token boundaries and quoted bytes/escapes; comments, unknown syntax, additional
objects and ambiguous prefixes refuse. The optional obsolete candump cache is
outside this profile until its exact historical schema is qualified separately.
Invalid/missing/future markers and disabled rollback journals also refuse.

Open the source read-only, without CREATE, URI tricks, migration or journal-mode
changes. Pin a Deferred read transaction across classification and SQLite online
backup. It includes committed WAL content in that snapshot, not later commits.
Do not acquire a source write transaction or copy only the main database file.
Use finite `Backup::step` work, require observed Done, propagate Busy/Locked,
and bound source size, SQL verification and total ordinary work. The initial
local ceilings are 256 MiB for the complete source filename namespace and each
image, and a 30-second cooperative work budget. Individual SQLite busy waits
retain the existing five-second ceiling and may finish after that work budget;
operating-system I/O is not forcibly cancellable. This is not a strict wall-clock
deadline.

Protect the full source/backup/output SQLite filename namespaces, including
`-wal`, `-shm` and `-journal`. Resolve existing parent identities, refuse aliases,
symlinks, ambiguous filenames and existing destinations of every kind. Use
exclusively owned staging paths and no-overwrite publication. A preflight
existence check or path-string comparison alone cannot authorize overwriting a
path or deleting an artifact placed by someone else.

Backup completion requires close/reopen, SQLite integrity, exact ordinary-row
preservation against the pinned source and FTS5 external-content consistency.
Run FTS5's `integrity-check` with rank1 on an owned verification copy/transaction;
never write to the source. Publish/hash a closed standalone rollback-journal
image with no dependency on unpublished WAL/SHM/journal files. Store destruction
alone is not proof of a standalone image. Preserve the completed backup before
any marker repair and reopen the published backup to verify its identity/state.

Clone that completed backup into an exclusively owned output staging database.
Inside one rollback-capable transaction, promote only the recognized stale
marker to2 and read back its stored value before commit. Then invoke the
unchanged private normal migration owner to reach3. Verify preserved history,
supplied settings/timestamps, sessions, configuration, FTS and integrity; close
and verify a standalone image before no-overwrite publication. A typed receipt
exists only after both artifacts complete. It identifies source/backup/output
paths, SHA256 digests, byte lengths and source/output versions1/3.

Backup failure publishes no successful recovery output. Later failure retains
the completed backup and reports its path. Explicitly remove only owned
incomplete staging artifacts; retain and report artifacts when cleanup fails.
Never suppress cleanup errors or remove the source, an existing destination,
or a completed backup. Logical source preservation does not promise identical
SQLite sidecar bytes.

## Acceptance and scope

Use literal independent schema/data fixtures, prove a committed WAL-only row is
absent from a main-file-only copy, and preserve the existing public historical
refusal as a positive control. New-interface behavior is candidate conformance;
a missing method/command or compiler failure is never an original regression
red. A real completed-backup/WAL-preservation mutation must fail at a collected
behavioral oracle after controls and cleanup, followed by unchanged replay.
Qualify actual backup failure, fresh-path/alias refusal, unsupported schema/FTS,
retained completed backup on later failure, retry, and the real CLI.

This is one historical profile, not arbitrary corruption repair. Other prefixes,
full statement interruption, competing openers/writers and publication/crash
recovery remain separate qualification. G15 stays partial until its remaining
software acceptance is complete. File sync and software checks do not establish
power-loss or physical media durability. No robot operation, deployment,
torque/velocity increase or Wave sign-off follows.

SQLite's [online backup contract](https://sqlite.org/c3ref/backup_finish.html)
and [external-content FTS integrity contract](https://sqlite.org/fts5.html#the_integrity_check_command)
define the corresponding library checks; the pinned rusqlite implementation is
also inspected before choosing its bounded stepping interface.
