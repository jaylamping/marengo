# Batch23 — CPU counter columns and identity

G18 remains open pending Linux CI and delivery. Independent Standards and Spec
reviews at c349b3f report zero actionable findings. Baseline37a4dd1,
branch codex/cpu-counter-columns. No robot, deploy, limits, Wave or automation action.

The original collector was extracted into a fixture seam without changing parsing
or delta behavior. A HostMetrics wire-roundtrip fixture independently expects20%
busy and10%iowait and failed on original iowait0%. The complete SHA-bound frozen
probe replays unchanged green after repair; shipping formatting is restored.

The pure CPU module separates labels from counters, validates numeric fields and
checked totals, uses iowait's fifth counter, and avoids counting guest fields twice.
Prior samples are keyed by numeric CPU ID. Departed IDs lose their baseline;
returning/new IDs, per-field decreases, malformed input and zero deltas produce
explicit unknown samples. Aggregate hotplug intervals are unknown too. These are
reported with additive protobuf validity/CPU-ID fields; legacy scalar fields remain
compatible. Pi/Jetson cards show a dash and no CPU usage bar when validity is absent.

Six native tests pass, including five CPU wire tests for literal deltas, sparse IDs,
reordering, hotplug, reset with increasing total, truncation, duplicate rows, malformed
fields, overflow, IRQ distinction, guest exclusion and decreasing iowait. Two encoded
UI tests cover unknown→valid→unknown. Native strict clippy remains unavailable due to
existing non-Linux deadcode warnings; it is not treated as Linux parity. Full Consul build and357tests pass. Gateway demonstration samples explicitly
mark their supplied percentages valid; unknown real observations remain unknown.
Exact-head hosted primary/runtime gates are pending.

Counter order and decreasing-iowait treatment follow [kernel proc documentation](https://www.kernel.org/doc/html/latest/filesystems/proc.html)
and the [kernel CPU load guide](https://www.kernel.org/doc/html/latest/admin-guide/cpu-load.html).
These are kernel accounting observations, not an exact SD-stall measurement.

Integrated main0166970 preserves accepted IPC changes; the only merge conflict
was the generated checksum, resolved by regenerating the combined schema.
Both integration reviews at31298a2 are clear. Combined native76 tests and
Consul358 tests/build pass. PR237 exact-head Linux CI and delivery remain pending.
