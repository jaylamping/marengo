# Batch37: healthy reporting refresh pacing and physical comparison

Healthy reporting streams previously received five refresh requests together once
the one-second threshold elapsed. Davout now attempts one healthy refresh per
shared5ms gate and rotates across joints, consuming the gate even on a failed
write. Initial enables, stale-feedback retries and Off/Active remain immediate.
The control fault latch, current-reference admission and limits are unchanged.
This is a verified pacing change; it is not a qualified physical overflow fix.

## Software qualification

Source0a3fc6b has three frozen tests that fail against original8a5eb85 production
with only test registration added, then pass byte-identically after the change.
They inspect encoded type24 writes, repeated-call spacing, fairness after a
persistent failure, and immediate stale/off behavior. Fourteen reporting cases
pass. Primary and native Pi each pass889 Rust/1existingignored and374 UI cases;
strict primary auxiliary gates and fatal ARM release pass. All2037 frozen source
inputs are unchanged before/after. The28 prior qualified probes and batch35's
complete cfb fault-publication module remain unchanged. Both source reviews are
CLEAR; sourceCI37026990187 passes all five actual jobs. See the
[source qualification](evidence/batch37/source-qualification.json).

Compile/import, container mode/npm-volume and local evidence-preparation failures
remain external as preparation failures. The first A2 oracle call ran before its
download completed and failed to read the locked file; after actual transfer
completion the unchanged oracle passes. The first archive replay compares LF
and CRLF exit markers literally; only marker comparison is corrected to integers.
These are not behavioral reds or failed physical captures. No successful installer
or source test suite is repeated for these preparation failures.

## Physical comparison

The powered/stable/supported/clear/reachable-E-stop setup is already authorized.
All214 installed hashes, taught limits/actual motor identities, calibration/env,
Store integrity/schema3 and trusted HTTPS UI verify. Both qualified packages have
identical policy/model/scripts/UI/gateway; only Pi/motor-repl binaries and release
marker differ. B1/A2/B2 preserve the same four policy/calibration hashes and all
sampled software states remain Disabled. No build or native test runs during
these controlled windows. See the [comparison](evidence/batch37/physical-comparison.json).

| Window | Seconds | Frames | Error frames | CAN0 overflow counter | Median request gap ms |
|---|---:|---:|---:|---|---:|
| A1 earlier baseline | 59.994 | 30709 | 2 | 399 → 401 | 0.306 |
| B1 | 299.999 | 152938 | 0 | 487 → 487 | 5.058 |
| A2 | 299.999 | 152939 | 0 | 487 → 487 | 0.306 |
| B2 | 299.998 | 152940 | 0 | 487 → 487 | 5.058 |

The earlier A1 baseline captures two controller receive-overflow frames. Its
first occurs about38.53ms after a refresh group starts, outside the immediate
request burst; the second is inside a group. It does not establish burst causality.
The candidate initial60s installation capture is clean, but its later runtime
fault at16:02:18.618858Z is256.070s after startup; counter486→487. That initiating
frame is outside the capture, so its exact bytes are unqualified. Counter401→486
already occurred before candidate installation, without a bound passive trace.

Three matched quiet300s windows then all pass the unchanged bounded oracle:
candidateB1, restored priorA2, restored candidateB2. Thus both versions can pass
this window and no hardware reliability fix is accepted. The observed request
spacing changes as intended. Host delivery timestamps do not establish physical
acquisition timing, and workload/IRQ/SPI servicing remain competing explanations.

Every file in the three1191-file observation archives and36-file selected
activation archive verifies after download; the251-file earlier baseline and
233-file installed release backup also verify. Full captures and source/test
executables remain external with SHA256 bindings; selected receipts, snapshots,
logs and actual programs are committed. No secret environment or key contents
are copied into review evidence.

## Current checkpoint and next dependency

Latest installed/source0a3fc6b is clean, services active with zero automatic
restarts, CAN0 overflow counter487, Disabled/all five Unhomed. Lower yaw remains
outside its unchanged taught envelope; no current physical reference is granted.
Restarts deliver existing stop writes with physical stop unconfirmed. No motor
Enable, SetZero, target or movement test is commanded. A clean observation or
restart does not qualify physical recovery, reference, stop or motion.

Next retain/expose exact owner transport evidence and isolate servicing/workload,
then qualify device identity/reset/ack/readback, installed-owner reference/priority
stop and commissioning. Ask before each concrete bounded right-arm movement and
wait for an explicit reply; ten-minute silence allows independent work only.
There is no human setup/deployment reply pending. CS04/CS13 remain partial;
counts102/eight maintenance/26verified/13partial/63open, historical batch16,
all prior work/CAD, limits, history and paused automation are preserved.

## Delivery checkpoint

PR251 merges independently reviewed5db4394 as mainbab38c6 with the same tree.
Exact finalCI37034964098 and mainCI37036180858 both pass all five actual jobs.
All1463 non-document source inputs remain equal to qualified0a3fc6b; all48
committed raw/normalized artifact bindings, including protobuf bytes, verify.
See [delivery](evidence/batch37/delivery-receipt.json).

Pi source fast-forwards cleanly tobab38c6 after a verified all-refs backup.
Its installed release remains0a3fc6b with identical production code: the53
source changes are review records. Installed binaries, taught policy/model,
calibration/environment and both service PIDs remain unchanged; no installer,
service restart or CAN write is issued for this sync. Store integrity/schema3
pass. The qualified candidate's214-file and trusted HTTPS receipts remain bound.

Primary main fast-forwards after preserving five existing local edits byte for
byte in a verified separate archive and patch. Those URDF/config/kinematics edits
are outside the qualified release and remain uncommitted. Prior branches,
worktrees, CAD, history and runtime backups remain intact. Continuation branch
codex/pi-fault-receive-evidence will expose retained owner transport diagnostics;
CAN cause/reliability, reference/priority-stop and motion acceptance remain open.
No setup or movement reply is pending; per-test explicit consent is still required.
