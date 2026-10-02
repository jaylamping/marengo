# Batch35: retain faults in periodic Pi safety publication

The installed d1fad15 Pi reports all five joints Faulted while its periodic
SafetyState reports no faults and a clear software latch. Two read-only live
probes reproduce that contradiction after healthy disabled ticks. The periodic
publisher reads only the latest tick's transient error; the private Davout
authority already retains the initiating Transport fault.

The publisher now reads that authority on each publication. Every retained
fault preserves its stable ID, class, message and affected joint in the existing
protobuf Fault fields. HardwareEstop receives Estop severity; its observed input
comes from the same authority. Current-tick errors without retained records keep
their existing fallback. This change supplies no reset, motion grant, CAN send,
new wire schema or physical GPIO implementation.

Five regression cases run the actual installed control loop and Chappe envelope
publication using an isolated receive transport and exclusive copied master
config/model. Each actual-loop child binds config and absent calibration history
to its independently created fixture and clears ambient trace/subset overrides;
the shared parent process environment stays untouched. They cover one-shot
transport/device faults followed by healthy
data, two peer faults, an observed model E-stop input and a healthy control.
Three periodic publications straddle an ordinary Disable. Assertions inspect
wire fault identity/class/joint/severity/latch, retained first transport evidence,
all-address stop attempts, absence of Enable/SetZero and unchanged policy bytes.
The reference owner stays on its owning thread. The independent observer has a
bounded cleanup guard; the actual persistence worker drains and terminates.

## Qualification

The final complete five-case probe is frozen before baseline replay. Original
production d1fad15 with only cfg(test) registration fails four named assertions;
the healthy control passes. Restoring the candidate passes the byte-identical
five cases. Actual test executable hashes distinguish both runs. Strict targeted
clippy also passes. See the final resource-bound
[binding](evidence/batch35/resource-bound-final-probe-binding.json).
The earlier whole-module freeze is superseded because the independent Standards
review found ambient resource resolution in its fixture. The source repair is
unchanged; the complete corrected module was frozen and replayed against original
production again, with the same four behavioral failures and five restored passes.
Earlier fixture/API/thread/lint preparation attempts and earlier probe hashes
are retained externally and are not claimed as final frozen evidence.

Qualified source8e1871f64ebcb7445bd83a241e6b2d67705202f1 passes the full primary
and native Pi gates:886 Rust tests with one existing ignored full-humanoid model
test, and374 UI tests each. Primary also passes72 Pi MCP,27 local writer,
83 research,15 audit and10 installer checks, strict gates and fatal main-policy
ARM release. All1920 committed inputs verify unchanged, native before/after.
All28 prior qualified probe files are unchanged. Independent Standards and scoped
Spec are CLEAR on that source; the prior resource isolation finding is resolved.
PR249 source CI37012722613 passes all five jobs, including simulation/virtual CAN.
See [qualification](evidence/batch35/final-source-qualification.json).
Evidence-head/main CI and installed live replay remain pending. Native execution
uses isolated transport/resources and leaves installedd1fad15/services unchanged;
these are software results on the Pi, with physical acceptance separate.

## Remaining acceptance

CS13 remains partial: other runtime-owner error classes, durable boot/device
recovery, process-reconstruction bypass closure and queued command session/
generation admission remain open. The protobuf surface exposes the retained
fault's existing diagnostic message rather than private raw CAN envelopes.
Physical E-stop wiring, initiating raw installed error evidence and a qualified
physical recovery/reference procedure remain open. A healthy publication with
no observed GPIO input is not proof of physical E-stop readiness.

The actual Pi remains Disabled/Faulted after the authorized batch34 update.
CAN receive overflow counters and the upstream MCP251x error path are consistent
with RX overflow, but the exact initiating envelope and cause of servicing delay
are unqualified. No reset/restart is accepted as physical recovery. No Enable,
SetZero, target or movement test has been commanded. Taught limits, calibration,
runtime data and the owner's powered setup are preserved. Movement requires a
qualified reference and a concrete bounded test with an explicit user reply;
ten-minute silence leaves movement pending and allows independent work only.

Ledger counts remain102 findings/eight maintenance tasks:26 verified/13 partial/
63 open. The historical batch16 pause checkpoint and paused automation remain.
