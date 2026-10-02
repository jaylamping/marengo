# ADR 0037: qualify mechanical home without enabling the bench arm

Status: accepted for guarded qualification implementation, October 2, 2026.

## Context

ADR0036's actual five-drive inspection succeeded. The installed RS03 drives
report 0.3.1.42, RS02 0.2.3.34 and RS00 0.0.3.32. All are in MIT mode and
Disabled. All CAN timeouts read zero. Lower-arm yaw reports approximately
4.175 motor radians despite the operator's current mechanical-home placement.

The vendor protocol offers no boot counter. Its type-18 parameters are described
as volatile, so CAN timeout is a candidate continuity witness to measure, rather
than inventing a simulated epoch for physical drives.

## Decision

Add a separate explicitly confirmed standalone Davout qualification operation.
It requires supported mechanical home, unchanged/sign-attested mapping, Disabled,
clear fault authority, supported installed firmware/model pairs and no competing
CAN owner. It obtains its own actual protocol inspection; caller-supplied
receipts cannot satisfy its checks.

While the drives remain Disabled, set CAN timeout to 600 raw counts and read it
back with fresh query hosts. For each manually referenced joint, issue addressed
Set Zero using a distinct reply host, require the complete matching status reply
to remain Reset without faults, then read mechanical position and require it to
be within the configured zero tolerance. Repeat timeout readback after all zeros.
Every whole bounded report still reaches the shared hazard consumer first.
Canonical all-address stop precedes work and runs on every admitted outcome;
reporting stays Off. Never send Enable or nonneutral output.

The JSON qualification receipt is inspection/history only: actual MCU identity,
firmware, before/after raw replies and timeout readbacks. It cannot make any joint
Ready, enter Active, or create current-reference permission. Existing calibration
history is preserved. A later reference transaction must durably audit actual
acquisition and consume it in the owning process before admission.

After this operation, a supervised motor-power cycle followed by ADR0036
inspection can measure whether 600 returns to the startup value zero on each
actual drive. This is an explicit physical qualification step. Until measured,
600 is only a configured timeout, not a qualified reset witness. Its wall-clock
timeout and stopped-output behavior also require measurement before motion tests
rely on them; raw counts are not labeled milliseconds.

## Consequences

This isolates firmware Set Zero behavior from motor enable and gives the next
finite bench session actual protocol evidence. It adds no caller-specific safety
bypass, no imported grant and no automatic recovery/re-arm. Qualification can
fail safely and truthfully; physical current-reference acquisition remains the
next implementation dependency.

## Sources

Vendor manuals linked in [ADR0036](0036-disabled-drive-protocol-inspection.md):
Set Zero returns type-2 status, version-specific zero-calibration rules, type-17
readback and type-18 volatility. Exact installed evidence is recorded separately.
