# Batch24 — truthful CAN and mount diagnostics

G19 remains open. Baseline0166970, branch codex/diagnostic-can-mount-state.
The existing successful-command CAN text parser is extracted unchanged into a
pure seam used by the collector. Two executed HostMetrics wire assertions fail:
normal ERROR-ACTIVE becomes restart delay100; flagged ERROR-ACTIVE becomes empty.
The latter fixture follows the [kernel SocketCAN example](https://docs.kernel.org/networking/can.html).
The original probe bytes and baseline are frozen under evidence/batch24.

The initial frozen checkpoint planned the full scope: normal/flagged CAN states,
command failures, real mount flags with escaped paths, filesystem versus device
identity, explicit unknown collection, and an injected command/file adapter through
the collector. Independent reviews, exact-head Linux gates and delivery are pending.
No physical CAN, root remount, robot, deploy, limits, Wave or automation action.

The CAN correction now reads the state token after flags, validates known states,
and publishes UNKNOWN on command failure, invalid UTF-8 or missing/malformed state.
The production reader and injected command fixture use the same collector seam.
Five native tests pass; both complete frozen original probes remain byte-identical
in shipping source and now pass. At that CAN-only checkpoint, mount repair was pending and G19 remained open.

Mount repair reads process-local mountinfo, separates filesystem type/source,
decodes kernel path escapes, uses the most specific unambiguous mount, and combines
per-mount/superblock ro flags. Invalid/ambiguous/failed mount observations remain
explicitly unknown. Capacity is independently validated from df; failed output,
zero totals and inconsistent sizes remain unknown. Additive wire validity fields
are consumed by the Pi card and diagnostic warning logic. An injected file/command
adapter drives the same collector as the real Linux backend.

Integrated CPU mainad09ee4 is preserved. Native84 tests and Consul361 tests/build
pass; fmt, proto lint/checksum and source whitespace checks pass. Both frozen CAN
and disk probe byte hashes are preserved and all original assertions pass unchanged.
The first full UI run exposed a pre-existing delayed Radix unmount event after jsdom
teardown; its error log is retained. The inventory fixture now awaits that queued
callback before returning from cleanup (and uses repository LF line endings).
A full suite rerun is clean. This is fixture lifetime qualification, not a production
UI behavior change. Independent reviews and exact-head Linux CI/delivery remain pending.

Mount field definitions follow [kernel proc documentation](https://www.kernel.org/doc/html/latest/filesystems/proc.html).
Unknown observations establish no hardware or filesystem health guarantee.

Spec review at34059b3 is clear. Standards reported no hard breaches and one minor
command/UTF-8 duplication; CAN now reuses SystemSources::command, and15host-metrics
tests pass after correction. Final review recheck and Linux CI/delivery remain pending.
CPU batch23 merged-main validation is complete; G18 is verified in the ledger.
