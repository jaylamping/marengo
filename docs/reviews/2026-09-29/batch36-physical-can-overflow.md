# Batch36: captured physical CAN receive overflow

The live Pi repeatedly starts Disabled/Unhomed and later latches a Transport
fault across all five joints. Batch35 correctly keeps that fault visible; the
physical cause still prevents current reference and motion commissioning.
This checkpoint collects evidence without changing production source or policy.

## Observed input and installed response

A passive CAN observer successfully receives frames before one authorized runtime
restart. The installed runtime starts at14:26:27.810904Z. At14:26:49.838195Z,
22.027291 seconds later, it observes the sole error in its60-second window:

```text
(1790951209.838195) can0 20000004#0001000000000000
```

The class mask is4 and payload byte1 is1: controller receive-buffer overflow,
under the [Linux CAN error ABI](https://raw.githubusercontent.com/raspberrypi/linux/rpi-6.18.y/include/uapi/linux/can/error.h).
The actual installed header has matching constants. CAN0 receive and overflow
error counters both increase396->397. The bound source observation contains
30682 total frames,29981 periodic feedback frames, and one error. A fresh
snapshot just before the event has no faults; the next has all five Faulted and
retained Transport1/software latch. Later snapshots retain the latch. Journal
fault timestamp14:26:49.841067Z follows the passive frame by2.872ms.

The observed module is mcp251x and both configured SPI maximums are10MHz.
The [upstream driver](https://raw.githubusercontent.com/raspberrypi/linux/rpi-6.18.y/drivers/net/can/spi/mcp251x.c)
provides context for controller-overflow reporting; an exact installed-module
source match is not qualified. Passive input and installed response correlate;
the installed API does not expose the private owner's raw first-envelope bytes.

## Competing causes and next comparison

1. A short IRQ/SPI servicing delay could fill the controller's receive buffers.
   Hard interrupt counters concentrate onCPU0, while requested/effective
   affinities permit cores0-3. One-second CPU0 samples range0-6.06% busy and
   cannot resolve a brief scheduling delay or establish its cause.
2. Periodic100Hz feedback overlaps additional reporting requests. The error
   occurs within a burst of type24 reporting reassertions and type2 responses.
   The5ms host delivery slice is preserved; dequeue timestamps do not establish
   physical acquisition time or prove that this burst caused the overflow.
3. Runtime startup/reporting state or a motor-side burst could change the short
   traffic profile. Startup recurrence varies15-22 seconds; one controlled
   variable comparison is needed before changing reporting or kernel settings.

Use the captured controller-overflow frame and the actual counter/latch transition
as the oracle. Compare one servicing or reporting variable at a time. Preserve
the conservative latch and effective limits. This checkpoint supplies no causal
fix, firmware decoding/recovery, physical reference or movement acceptance.

## Preservation and qualification

The owned252-file observation archive is downloaded and every file hash verifies.
The full CAN log and installed header remain external with SHA256 bindings;
selected protobuf snapshots, error/sample lines, runtime journal, actual collector,
analysis and raw manifest are committed. See
[diagnostic receipt](evidence/batch36/diagnostic-summary.json).
Production source, tests and prior qualification artifacts are unchanged; batch35
primary/native886 Rust/1existingignored/374UI and all-five source/final/main CI
remain their exact qualified results. B35 delivery metadata reviews are CLEAR
at0b6b915, after correcting activation-start versus installer-result wording.

Runtime/source remain01a5c40. Pi PID568641/gateway560138 are active with zero
automatic restarts at the captured sample. The restart preserves taught policy,
model, calibration and runtime environment hashes. Its shutdown issues existing
stop writes with physical stop unconfirmed; no motor Enable, SetZero, target or
movement test is commanded. Restart is not accepted as physical recovery.

All five current references remain Faulted. Lower yaw remains outside preserved
taught limits. Device identity/reset/ack/readback, installed-owner reference/
priority stop and commissioning must qualify before a concretely bounded,
explicitly confirmed right-arm movement. Powered supported setup is already
authorized, no user reply is pending, and ten-minute silence only allows other
work. CS04/CS13 stay partial; counts102 findings/eight maintenance tasks,
26verified/13partial/63open, historical batch16, CAD and paused automation remain.
