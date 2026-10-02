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
Final PR heade0bec83 and merged main01a5c40 pass all five CI jobs in runs
37015241566 and37016360032. Their Git trees are identical; runtime inputs match
qualified source8e1871f. The earlier native gate used isolated transport/resources
and left installedd1fad15 unchanged during qualification. Actual delivery is now
verified below; physical acceptance remains separate.

## Remaining acceptance

CS13 remains partial: other runtime-owner error classes, durable boot/device
recovery, process-reconstruction bypass closure and queued command session/
generation admission remain open. The protobuf surface exposes the retained
fault's existing diagnostic message rather than private raw CAN envelopes.
Physical E-stop wiring, initiating raw installed error evidence and a qualified
physical recovery/reference procedure remain open. A healthy publication with
no observed GPIO input is not proof of physical E-stop readiness.

The actual Pi remains Disabled/Faulted after the authorized batch35 update.
CAN receive overflow counters and the upstream MCP251x error path are consistent
with RX overflow, but the exact initiating envelope and cause of servicing delay
are unqualified. No reset/restart is accepted as physical recovery. No Enable,
SetZero, target or movement test has been commanded. Taught limits, calibration,
runtime data and the owner's powered setup are preserved. Movement requires a
qualified reference and a concrete bounded test with an explicit user reply;
ten-minute silence leaves movement pending and allows independent work only.

Ledger counts remain102 findings/eight maintenance tasks:26 verified/13 partial/
63 open. The historical batch16 pause checkpoint and paused automation remain.

## Delivered and replayed on the actual Pi

PR249 merged at13:56:09 UTC. Pi source and installed release are exact main01a5c40.
The authorized direct installer exits0. All214 installed payload hashes verify,
using independently previewed taught merges for the three preserved policy/model
files. All five taught envelopes and eight motor identity fields, calibration,
runtime environment, Store integrity/schema3 and trusted HTTPS index pass. Pi and
gateway services are active with zero automatic restarts at the captured sample.
The owned233-file pre-update release verifies natively and after download;
separate native Git/Store/config/calibration/env/unit backups are retained.

Actual startup14:13:22.553717Z naturally faults15.026017s later. Unlike the preserved
pre-update snapshots, SafetyState now reports retained Transport fault1 and a
true software latch while all five references remain Faulted. Three later live
snapshots retain that state during healthy feedback. A compatible30s passive
observer records15290 frames, including about2997-2999 feedback frames per motor,
with zero error frames and unchanged receive/overflow counters396. This is
positive installed publication evidence; the post-fault window cannot reveal the
initiating envelope or qualify fault recovery or motion.

The first activation wrapper used sudo bash and was rejected before installation.
The direct authorized executable succeeds. Its helper then failed on the gateway's
first startup503; the activation observer also rejected incompatible-L/-e flags
and captured zero bytes. Both failures are preserved, not claimed as successful
observation. Fresh read-only verification and the corrected post-fault observer
pass without another restart. See [delivery](evidence/batch35/delivery-receipt.json)
for raw/normalized artifact hashes and exact timestamps.

No Enable, SetZero, target or movement test was commanded. Deployment shutdown
issued zero-speed/neutral-MIT/Disable writes with physical stop unconfirmed.
The owner's powered setup authorization is established. All five remain Faulted,
physical reference is unqualified, and lower yaw remains outside its preserved
taught hard envelope. Next work qualifies CAN evidence/recovery, installed-owner
reference/priority stop and commissioning before an explicitly confirmed bounded
right-arm test. CS13 and the unchanged finding counts remain partial/open as above.
