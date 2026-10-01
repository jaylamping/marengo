Read-only critique by /root/batch13_spec; proposal remains unexecuted and has no implementation acceptance.

The barrier is sound if controls establish the held WAL writer and isolate the worker's migrate call. Observed callback proves actual SQLite contention; absent callbacks/timeouts prove nothing.

- Verify WAL mode, coherent marker2/cache state and idle worker without a retained read transaction. Independently observe committed marker3/timestamp777 and cache absence before acknowledging.
- Count callback invocations across the entire owned child. SQLite's callback argument resets for each locking event, so count == 0 cannot detect unexpected additional contention.
- Flush protocol messages, bound controller waits, prohibit SQLite access from the callback, and reap every owned process before cleanup and the sole oracle.
- Freeze a mutant whose pre-reservation version read releases its statement/autocommit read transaction. A retained snapshot may produce BUSY_SNAPSHOT and test a different failure.
- Keep commit, fixture preservation, callback and cleanup controls independent of migrate's result.

Run the future exact checked merged baseline as positive qualification. Stale-read mutant evidence is candidate sensitivity followed by unchanged replay, never an original regression. Default timeout, first-open races, recovery concurrency and process interruption remain separate. No hardware or production execution occurred.
