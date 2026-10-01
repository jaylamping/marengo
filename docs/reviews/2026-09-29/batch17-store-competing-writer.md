# Batch17: migration marker reread after a competing writer

Status: locally qualified on Mac; required Linux primary gate and delivery pending.
Baseline: `b23c82ba73b6372889f950379db97273053fe147` (fetched main, clean checkout).
Branch: `codex/store-competing-writer`. G15 remains partial.

The delivered production migration owner already passes. This batch adds one
public `Store::migrate` behavior test, with no production change or claimed
original regression. The test seeds a literal schema/history/FTS fixture,
opens an idle Store in WAL mode, then independently publishes coherent marker2
and an obsolete cache. A separate Immediate writer removes the cache and updates
marker3 with timestamp777 without committing. The worker's public caller-installed
busy handler signals actual reservation contention over flushed owned pipes.
The controller commits and independently observes marker3/777, cache absence and
unchanged ordinary history before acknowledging the retry.

The child counts callbacks across its entire lifetime, performs no SQLite work
inside the callback, and refuses additional contention. The controller bounds
protocol and exit waits, kills/reaps its owned worker on failure, and joins its
reader. A close/reopen verifies marker3/777, settings/timestamps, full event/session/
configuration rows, message and fields FTS, external-content FTS integrity and
SQLite integrity. All handles and the child close before actual directory removal
and the sole collected controller assertion.

## Executed evidence

[evidence/batch17/qualification.json](evidence/batch17/qualification.json) binds
whole probe and production hashes to actual captured output:

- Delivered baseline: one passed, zero failed/ignored. Independent competing
  commit observed, exactly one callback, marker3/777, preserved history, cleanup.
- Candidate stale-read mutant: move marker read before Immediate reservation;
  the read helper releases its statement/autocommit read transaction. One actual
  assertion failure after successful migration, competing-commit/history controls
  and cleanup. The stored timestamp becomes1790866826713 instead of777. No
  compiler, timeout or BUSY_SNAPSHOT failure is claimed as sensitivity evidence.
- Byte-identical test replay after restoring byte-identical production: one
  passed, zero failed/ignored; marker3/777 preserved.

Native `cargo test --locked -p marengo-store` passes all30 tests, no failures or
ignored tests. Restored-source strict affected clippy and workspace formatting
pass. Rust1.88.0 plus rustfmt/clippy was installed in the user's Rust home with
no shell-profile modification. The host is Apple Silicon macOS; these results
are separate from Linux qualification.

`docker compose run --rm check` could not start: `docker` is absent. No primary
gate, CI, PR, merge or independent-review acceptance is claimed. Docker setup
preference was requested while independent work continued.

## Excluded preparation and remaining scope

Initial compilation failed on a Result comparison and mapped-row lifetime; the
next preparation attempt refused a controller connection still using its cached
non-WAL journal mode. These are preparation failures, not production regression
reds. WAL is now established before seeding. Preliminary logs/receipt in the
evidence directory belong to the earlier probe without detailed observations;
the final qualification receipt supersedes them. The first clippy invocation
was not used as final evidence; strict clippy passed again after restoration.

This test overrides the caller connection's busy policy. It does not qualify the
default timeout, busy failure/retry, first-open races, parallel openers, recovery
concurrency, statement/process interruption, other historic prefixes or physical
power-loss/media durability. The existing102 finding dispositions and all eight
maintenance tasks remain unchanged. No robot, deploy, limits, Wave sign-off or
paused-automation action occurred. Windows-local evidence/CAD remain untouched.
