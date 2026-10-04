# ADR 0037: read-only protocol inspection of Disabled drives

Status: accepted for software implementation, October 3, 2026. Ported from the
disabled-only inspection of the closed PR #254 (`codex/right-arm-commissioning`),
reduced to reads and rebased on [ADR 0036](0036-physical-robstride-reference.md).

## Context

ADR 0036 qualifies physical reference with type-0 identity, type-6 SetZero and a
type-17 `mechPos` readback, but nothing on main reads what the installed drives
actually run. Two facts depend on it:

- **Firmware.** The bench right arm is mixed: drives 1-2 (RS03) report
  0.3.1.42, drives 3-4 (RS02) 0.2.3.34 and drive 5 (RS00) 0.0.3.32 (PR #254
  type-4/`C4` replies, 2026-10-02). Only the drive can confirm it.
- **Drive CAN timeout.** `docs/safety.md` and `AGENTS.md` say there is no
  drive-side CAN timeout and that `CanTimeout` (0x7028) is never written or
  read back. WP-I D2 (`docs/reviews/2026-10-03-crate-audit/phase-b/WP-I.md`)
  asks for a read-only 0x7028 probe before deciding anything. PR #254 wrote 600
  counts (30 ms) to all five drives and the value is RAM-persistent, so the bench
  state is unknown until read.

`motor-repl status` only opens SocketCAN, and every ordinary `Supervisor::from_repo`
writes type-24 On at startup when `active_reporting_diagnostics` is set.

## Decision

Add one blocking Davout operation for the standalone CLI,
`Supervisor::inspect_drive_protocol(joints)`, on an owner built by
`Supervisor::from_repo_for_protocol_inspection(repo_root, bus)`. That constructor
loads and validates like `from_repo` but transmits nothing. The operation is
exposed as `motor-repl protocol-inspect [joint...]` and the MCP tool
`pi_protocol_inspect`. It is never called from the control tick.

**Admission.** Disabled, no reference transaction or pending commit, clear fault
authority and no E-stop. Unknown or repeated joints refuse before any write.

**Wire.** Only these frames are written, all built by `robstride::encode_*` on
host 0xFD:

```text
stop       type-4 Disable (Byte[0] = 0) to every installed address
quiet      type-24 Off to every installed address; 50 ms settle drain
per drive  type-4 C4 firmware version, type-0 identity, type-17 reads of
           RunMode, MechPos, MechVel, CanTimeout (0x7028), ZeroSta, AddOffset
finish     50 ms settle drain, then the Disable-only stop again (every result)
```

The stop is Disable-only: the canonical stop's zero-speed write is a parameter
write and its neutral MIT is not a read frame, and a drive answering a Disable is
in Reset. The firmware query is itself a Disable. Nothing here can send Enable,
SetZero, MIT, a parameter write or a type-22 save, and nothing creates Ready or
a current-reference grant.

**Pacing.** Every Disable, Off and query starts its own `BURST_GROUP_SPACING`
(2 ms) group on its interface, and queries go one at a time: the next is written
only after the previous reply was accepted. At most one solicited reply is
outstanding, inside the mcp251x's two-frame receive buffer.

**Replies.** Robstride decodes a type-2 payload starting `00 C4 56` as
`FeedbackEvent::FirmwareVersion` (version, header flags, mode and host), never as
an MIT pose. Every drain is consumed whole by the shared hazard consumer with a
new `ReceiveContext::DisabledInspection`: status flags latch Device, and any
header that is not Reset latches DriveState. A version reply in any other
context latches Feedback, because only this inspection asks for one. Each reply
is then correlated by address and query kind:

- a reply popped before its request, for a query not pending, for an unknown
  register, or a version reply to another host: refused and latched (Feedback).
  It means another CAN owner is querying;
- two different replies to one query, in the same drain or after acceptance:
  refused and latched (Feedback). An identical duplicate is ignored;
- a nonzero type-17 status or no reply within 300 ms: refused, not latched.

The MCP tool is read-only and needs no confirmation. Like `pi_motor_repl_status`
it is skipped while `marengo-pi` or another `motor-repl` holds CAN. It never
stops `marengo-pi`; the operator does that with `pi_restart_marengo_pi`.

## Consequences

WP-I D2's read-only 0x7028 probe and firmware confirmation are one command. The
CAN-timeout decision itself stays open: this ADR adds no write path, and
`docs/safety.md` keeps "never written" while gaining "read back by
`protocol-inspect`". Results are diagnostics, not history; nothing persists them.

The inspection relies on the MCP guard, not Davout, to keep a second local owner
off the bus. Wire-level detection (unsolicited replies, Run headers) refuses but
cannot see another owner that stays silent.

PR #254's other parts are not ported: the parallel physical bench owner, the
disabled bench-home qualification (Disabled SetZero and a 30 ms CAN-timeout
write), the Berthier bench runners, motor-repl `bench-*` commands and the
session-authorization policy. ADR 0036 supersedes them.

## Sources

Robstride [RS00](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS00/RS00User%20Manual260713.pdf),
[RS02](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS02/RS02User%20Manual260713.pdf)
and [RS03](https://github.com/RobStride/Product_Information/blob/main/Product%20Literature/RS03/RS03User%20Manual260713.pdf)
manuals, version 260713: communication types 0, 2, 4, 17 and 24; parameter table
(0x7028 `canTimeout`, 20000 counts = 1 s). The `C4` selector and the `00 C4 56`
reply layout follow PR #254's bench captures of all five right-arm drives.
