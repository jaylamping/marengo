# Batch38: retained receive evidence in Pi safety faults

PR252 is merged. The existing persistent `SafetyState.Fault.message` now includes
the initiating receive envelope retained by Davout: interface, frame kind, full
CAN ID, extended flag, declared length, actual payload prefix and optional
malformed reason. Later errors, healthy feedback and Disable cannot replace the
first diagnostic. Remote requests have no invented payload; interface text is
bounded; no physical acquisition timestamp is fabricated. Fault code, severity,
joint, operational mode, latch, limits and motor admission are unchanged.

## Delivery and software evidence

- Baseline: `386651981664b6242b2beda4b333b85e617c1378`.
- Qualified source: `2d0fd4088b0ca0301b2e27b07560b54b40ec61ca`.
- [PR252](https://github.com/jaylamping/marengo/pull/252) merged at
  2026-10-02T17:48:22Z as `ba0fff7206877943fbca0f79b18d59089d5d4a24`.
  Source and main have identical Git trees (`875371bd2a1caf2be68128c0f6c1058d5dbfb877`).
- Six frozen actual producer tests decode published protobuf SafetyState.
  Original production, with test registration only, has five named assertion
  failures and one passing healthy control. The unchanged candidate module has
  six passes; five unchanged batch35 periodic-loop cases also pass.
- Primary and native Pi each pass **895 Rust tests, one existing ignored test,
  and 374 UI tests**. Strict auxiliary suites pass 72 Pi MCP, 27 writer, 83
  research, 15 audit and 10 installer cases; fmt/clippy/proto/deny/audit and fatal
  ARM release pass. All 2,099 tracked source inputs remain byte-identical before
  and after each qualification; 28 prior probes and the full batch35/batch37
  test modules are unchanged. There are 1,464 non-document source inputs.
- Independent source Standards and Spec reviews are CLEAR. Source
  [CI37039819887](https://github.com/jaylamping/marengo/actions/runs/37039819887)
  and main [CI37043206357](https://github.com/jaylamping/marengo/actions/runs/37043206357)
  each pass all five actual jobs, including the executed simulation step.

See [qualification](evidence/batch38/source-qualification.json), lossless
[red/green logs](evidence/batch38/red-green-logs.json), and the unchanged
[frozen test module](evidence/batch38/frozen-receive-diagnostics-tests.rs).
Complete primary/native logs and executable/source archives remain in the
external evidence root with [raw hash bindings](evidence/batch38/raw-log-bindings.json).

## Installed Pi and observations

The already authorized powered deployment succeeded once. Installed release is
`2d0fd4088b0ca0301b2e27b07560b54b40ec61ca 2026-10-02T17:30:02Z`.
All 214 actual installed payload hashes, every taught hard/soft bound, eight
configured motor identity fields, calibration/environment preservation, Store
integrity/schema3 and HTTPS200 with explicit local certificate trust verify.
This confirms configured identities were preserved; physical device/firmware
identity continuity remains unqualified. Gateway/log CLI/IMU binaries match the
prior release. Dirty primary edits were excluded from the immutable release.

| Observation | Seconds | Frames | Error frames | CAN0 RX errors | Motion scope |
|---|---:|---:|---:|---|---|
| Authorized installation capture | 59.997 | 30,579 | 0 | 502 to 502 | Five neutral MIT, five Disable and five ZeroSpeed writes; no Enable/SetZero |
| Existing owner passive capture | 299.998 | 152,889 | 0 | 502 to 502 | Feedback/reporting only; no restart, build, load injection or commanded motion |

The passive capture has 297 fresh Disabled samples, no fault message and no
software latch. Its only message-set change is the initial empty observation.
Pi PID708792 remains unchanged. Eight archive members and all 55 installation
observation members verify after download. The counter had already grown from
the prior checkpoint487 to494 at backup and502 before installation. Those
initiating frames were not captured. No cause is assigned to validation load.
Earlier captured overflows and mixed batch37 comparison outcomes remain valid.
Quiet windows do not establish a CAN reliability fix or hardware correlation
for the new owner diagnostic.

At the source-sync snapshot 2026-10-02T17:59:45Z: source is clean mainba0fff,
runtime remains2d0fd40, Disabled/five Unhomed/zero Faulted, no software latch,
CAN0 RX errors502. All three services are active with zero automatic restarts;
Pi/gateway PIDs708792/708603 are unchanged. Source-only sync verifies all1,464
non-document inputs, installed hashes and198 staged payload files. No installer
or restart is needed for the final documentation sync.

See [installation](evidence/batch38/post-install-file-verification.json),
[physical observation](evidence/batch38/physical-observation.json),
[passive observation](evidence/batch38/passive-owner-analysis.json), and
[source sync](evidence/batch38/pi-code-main-sync-receipt.json).

## Preservation and preparation outcomes

Primary mainba0fff retains the same five uncommitted URDF/config/kinematics
files, byte-identically and outside this release. Its verified all-refs bundle
and the previous owned-file backup remain retained. Pi pre-update backups cover
233 installed files, Store online backup/integrity, runtime environment,
calibration, units and Git refs. Private backup contents are not published.

The first bundle inventory caught four ignored Python cache files before any
archive or install. The original 218-entry attempt remains external; a fresh
immutable clone produces the verified 214-entry bundle with all2,099 source
hashes unchanged. A primary fast-forward preflight originally looked for the
prior receipt on the old checkout and failed before mutation; the corrected
preflight reads the continuation receipt, backs up refs, then preserves all five
files. These are retained preparation failures, not behavioral regression reds
or failed hardware captures.

## Owner-requested stopping point

No motor Enable, SetZero, target or physical movement test was commanded.
Software Disabled and accepted stop writes do not prove physical stop or fault
recovery. Gravity resting pose is not a current physical reference. The physical
reference adapter, owner priority stop/recovery and commissioning remain
unqualified; lower yaw remains outside its preserved taught envelope.

CS04/CS13 remain partial. All102 finding dispositions (26verified/13partial/
63open), eight maintenance tasks, historical batch16, Mac continuation, CAD,
previous branches/worktrees/archives and the PAUSED automation are preserved.
The owner asked to finish this pass, document it, pause and resume in a new
session. No batch39 or new experiment is started. No human reply is pending.
Start the next session with [RIGHT-ARM-VALIDATION-HANDOFF.md](RIGHT-ARM-VALIDATION-HANDOFF.md).
