# ADR 0036: inspect the physical drive protocol while Disabled

Status: accepted for diagnostic implementation, October 2, 2026.

## Context

The five-joint bench arm cannot acquire a physical current reference. The closed
virtual transaction proves software ordering, but its simulated device epoch
does not describe a Robstride drive. Before designing physical permission, we
need observations from the installed drives: identity, firmware, mode, position,
power-on wrapping and CAN timeout. The vendor manuals describe these queries,
but not a boot counter or a Set Zero transaction nonce.

## Decision

Add one synchronous Davout inspection operation for the standalone bench CLI.
It requires Disabled, no reference transaction and clear fault authority. It
attempts all-address stop before querying, consumes whole bounded receive reports
through the shared hazard consumer, and stops again on every result. It is not
called from the 200 Hz loop. The installed runtime must be stopped before the
standalone CLI takes CAN ownership.

Robstride owns typed query encoding and lossless reply decoding. A firmware
version response uses the status communication type with a special payload;
retain its status flags/mode but never project its bytes into pose freshness.
Inspection returns the actual request/reply bytes, address and reply host. The
CLI may serialize that diagnostic receipt as JSON; Chappe remains protobuf.

Inspection uses the canonical all-address stop (zero speed, neutral MIT and
Disable), explicit reporting Off and diagnostic read requests. A dedicated
constructor suppresses startup reporting On; cleanup never synchronizes it. It cannot send Enable, Set
Zero, nonneutral output or configuration writes, and it cannot create Ready or a
current-reference grant. MCU identity is not boot continuity; `zero_sta` is a
power-on wrapping setting, not a zero-valid flag. Missing, erroneous or
conflicting replies refuse the inspection.

## Consequences

This is a bounded prerequisite to supervised disabled Set Zero and a finite
commissioning run. Physical permission remains unsupported until actual
correlation, stationary Set Zero and reset behavior are qualified. Simulation
and history-only constructors retain their existing meaning. A later installed
owner interface must advance the same work incrementally, rather than calling
this blocking CLI operation from a control tick.

## Sources

Robstride's [RS00](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS00/RS00User%20Manual260713.pdf),
[RS02](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS02/RS02User%20Manual260713.pdf)
and [RS03](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS03/RS03User%20Manual260713.pdf)
manuals, version 260713: communication types 0, 4, 6, 17 and 18; parameter table;
zero-calibration rules. Installed firmware acceptance is still measured evidence.
