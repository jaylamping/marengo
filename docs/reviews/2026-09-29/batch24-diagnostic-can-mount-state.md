# Batch24 — truthful CAN and mount diagnostics

G19 remains open. Baseline0166970, branch codex/diagnostic-can-mount-state.
The existing successful-command CAN text parser is extracted unchanged into a
pure seam used by the collector. Two executed HostMetrics wire assertions fail:
normal ERROR-ACTIVE becomes restart delay100; flagged ERROR-ACTIVE becomes empty.
The latter fixture follows the [kernel SocketCAN example](https://docs.kernel.org/networking/can.html).
The original probe bytes and baseline are frozen under evidence/batch24.

Production repair remains pending. Full scope includes normal/flagged CAN states,
command failures, real mount flags with escaped paths, filesystem versus device
identity, explicit unknown collection, and an injected command/file adapter through
the collector. Independent reviews, exact-head Linux gates and delivery are pending.
No physical CAN, root remount, robot, deploy, limits, Wave or automation action.
