# ADR 0020: Lossless feedback and persistent fault authority

Status: Accepted for the bounded software slice below. September 30, 2026 UTC.

## Problem

Latest-state maps erase a device fault when healthy status follows it in the
same receive drain. Detailed reports lose upper bits and warnings. A receive
failure can discard earlier observations, and callers can ignore an error and
restore motion permission with healthy feedback. Berthier also discards its
post-send and mode-entry receive errors. Stop currently reports success even
when CAN delivery fails.

## Decision

Robstride supplies ordered, addressed feedback observations and a receive
report retaining observations alongside a terminal transport error. Status
flags, drive mode, detailed fault bytes and warnings are separate evidence
domains. Pose time is never supplied by a fault-only report. Latest-state
methods remain compatibility projections, not the safety authority.

Every observation is inspected for device, drive-mode and raw hard-position
hazards before choosing a pose for cache admission. Velocity inference runs
once per address per drain using its newest admissible pose. Socket read time
does not identify when a queued frame was physically acquired: inferring speed
between consecutive dequeues creates false trips under valid traffic. Old or
equal timestamps never renew freshness or derivative history; their hazard
evidence is still inspected. Reserved drive mode never authorizes a pose, and
current-session Reset/Calibration is a fault only for an enabled address.

Davout consumes the lossless report, retains faults outside its replaceable
pose cache, and gates enable, calibration, SetZero and all motion routes with a
private latch. A read-only snapshot records the initiating fault, evidence and
monotonic stop generation. Healthy feedback, Disable and cache operations
cannot clear this authority. Invalid operator input remains a rejected request,
distinct from an observed runtime hazard.

Stop attempts every configured address and retains delivery failures without
replacing the initiating cause. A Disabled software mode is not confirmation
that physical motion stopped. Ordinary stop payloads do not clear firmware
faults. Berthier propagates both post-send drains and makes planner initialization
refresh fallible; failed refresh cannot install new motion intent.

## Qualification and remaining migration

Status bit meanings are taken from the official RS02/RS03 manuals. Preserve all
raw detailed bytes and unknown bits; an unqualified byte order or installed
firmware must not become an invented decoded identity or recovery proof.
Nonzero fault evidence is conservatively blocking. Warning policy is explicit
and separate from fault masks.

This slice provides no fault-reset capability. Qualified explicit recovery,
Pi/protobuf persistent publication, command session/generation admission and
reference transactions remain under ADR 0019 and the finding ledger. It does
not raise caps, change Wave sign-off, or qualify hardware. The physical robot
is not part of the software test environment.

The existing SocketCAN adapter still pads short payloads and has no inner bound
on a continuously busy receive drain. These are unqualified protocol/runtime
contracts under CS04 and M06. No test here proves firmware byte order, physical
acquisition time, stop acknowledgement or delivery to a motor.

## Verification

Use public Supervisor and ControlLoop interfaces with recording/scripted CAN
adapters and independent literal frame oracles. Baseline regressions must fail
for observable unsafe behavior before repair; a new interface failing to compile
on the baseline is not red evidence. Exercise fault/healthy ordering, peer and
interface isolation, terminal receive failure, persistent admission refusal and
all-address stop attempts. Required primary and applicable simulation/vCAN
checks run before merge.
