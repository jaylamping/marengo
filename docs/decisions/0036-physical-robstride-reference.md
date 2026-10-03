# ADR 0036: qualified physical Robstride reference

Status: accepted for software implementation, October 2, 2026. Supersedes in part
[ADR0035](0035-consume-current-virtual-reference.md): physical owners no longer
refuse acquisition.

## Context

ADR0034/0035 qualify correlated acquisition, mandatory cleanup, durable journal
completion and selection of current permission for the closed virtual backend.
Ordinary physical owners still refused every reference, so the bench arm could
not be homed and then enabled. The Robstride wire protocol supplies a type-0 MCU
identifier, a type-6 SetZero answered by a type-2 status frame and a type-17
parameter read of the read-only load-side `mechPos` (0x7019). Those are enough
for a target-only physical transaction with post-command evidence.

## Decision

Add explicitly named physical owners:
`Supervisor::from_repo_with_physical_reference(repo_root, bus, journal_path)`,
`from_repo_with_physical_reference_and_record_path(..)` and
`berthier::ControlLoop::from_repo_with_physical_reference(.., loop_hz, chappe_hz)`.
They install the physical backend, the durable journal and `CurrentPhysical`
commit selection. The journal path must be absolute and distinct from the
calibration history. `marengo_config::resolve_reference_journal_path` uses
`MARENGO_REFERENCE_JOURNAL`, or `reference-journal.sqlite3` next to the
calibration record. Plain `from_repo` installs no backend and still returns
`DavoutError::ReferenceUnsupported`. Every startup is Unreferenced. Neither
history nor the journal grants anything.

The workflow API is `request_reference(joint, sign_verified, ReferenceAudit)` →
`ReferenceHandle`. After that, `advance_reference_work()` is called on each tick
(Berthier calls it while `reference_work_pending()`), and
`reference_outcome(&handle)` returns `InProgress | Current { position_rad } |
Failed { message }`. `calibrate_joint_zero` is the blocking form. It polls every
2 ms until `search_timeout_s` plus 10 s have passed. If that deadline expires, it
calls `disable_all` and returns `HomingVerify`.

The phases are:

```text
BaselineStop   all-address stop, one address group per 2 ms on a real bus; applied
               active reporting off, except a peer in its possible blackout
               (450-800 ms after its SetZero), and the target's reporting off
               whatever this process applied
DrainOld       complete bounded drain of queued feedback
RequestIdentity type-0 to the target (host 0xFD)
AwaitIdentity  type-0 reply popped after the request; UID not claimed by another address
AwaitReportingOff echoing bus: the target's type-24 Off echo read at least one control period ago, and POST_SET_ZERO_QUIET (800 ms; measured blackout end <= 667 ms) since the target's last SetZero
ArmTarget      Enable to the target only
DrainPostArm   complete drain
SetZero        type-6 to the target; any attempt starts a new device coordinate epoch
AwaitAck       type-2 from the target popped after SetZero, |q_joint| <= tolerance
RequestReadback type-17 read of 0x7019
AwaitReadback  reply popped after the request, status 0, |mechPos/scale| <= tolerance
finish         all-address stop → durable journal row `physical_robstride` → grant
```

Each new phase gets a 2 s deadline, capped by the overall `search_timeout_s`.
Repeated await phases do not renew it. A failed or uncertain write is a
`Delivery` failure, and an invalid internal state is a `Backend` failure. Both
latch a transport fault. `Identity` and `Readback` failures record no fault.

Drives keep type-24 reporting across host processes, and a report a drive
built before acting on the Enable can be read after the Enable's echo, still in
Reset. The strict post-echo Run check would then fail the reference
(bench 2026-10-03 14:55, right_lower_arm_yaw). The target's Off is therefore
written regardless of what this process applied. On SocketCAN, the target is
armed only after that Off's echo has been read and one control period has
passed. A missing Off echo times out before any Enable. Ordinary Active
enables follow the same rule (see [safety.md](../safety.md)).

### Evidence

Two pieces of evidence are required, and neither replaces the other:

1. a type-2 (`OperationStatus`) frame from the target, popped after the
   addressed SetZero, with its joint-space position within
   `homing.zero_verify_tolerance_rad`. Type-24 active reports do not count;
2. a type-17 0x7019 reply popped after its own request, which is issued only
   after the ack. It must have status 0 and a joint-space value
   (`mechPos / position scale`) within the same tolerance.

Readback also requires the UID and coordinate epoch to be unchanged since
identity, and the readback must pass the continuity check. The UID, ack order,
ack CAN id and ack position are retained with the journal row.

### Stop, storage, grant

`finish` performs the all-address stop before storage. The row is committed as
`physical_robstride`. Only the real completion consumer selects a grant, after
the commit is durable and current continuity checks pass. Failed, uncertain,
cancelled or shut-down transactions remain truthful history and never grant.

### UID binding and revocation

Each grant binds the joint's address, the MCU UID from acquisition and the
device coordinate epoch. Revocation is per joint for:

- a UID change in any type-0 reply;
- Enable admission. Before any Enable frame, the owner sends type-0 to every
  target and waits up to 50 ms. A mismatched or missing UID revokes that joint
  and returns `HomingVerify`;
- coordinate discontinuity. A jump larger than the vendor MIT velocity range ×
  dt plus two quantization steps (joint space) breaks continuity. Across an
  owner reference-work gap the bound is the at-rest
  `zero_verify_tolerance_rad`;
- a drive reporting Calibration mode;
- no feedback for longer than `control.comm_watchdog_ms` outside owner
  reference work. Liveness restarts at the end of owner work, and periodic
  reporting resumes after selection. Silence the host caused is not counted
  (see *Host-caused silence* below).

Fault, E-stop, uncertain stop, shutdown, a model change and an observed relevant
policy change revoke all grants. A revoked grant stays revoked. A successful
ordinary Disable preserves intact grants (ADR0023), subject to the liveness rule
above.

### Host-caused silence (amendment, 2026-10-03)

Revocation is latched but derived on demand: each check recomputes a joint's
liveness from its last frame, the end of owner work and the instants below, and
the check that finds a lapse revokes the joint for the rest of the process.
`pi_enable_soak` at e6add09 failed 14 of 20 cycles because two silences the
host itself caused lapsed the 100 ms bound (see
[safety.md](../safety.md#known-software-gaps-see-also-position-hold-control-reviewmd)
and the [behaviour doc](../commissioning/firmware/robstride-firmware-behavior.md)):

- Each reference's baseline turns every streaming peer's type-24 Off and the
  commit's sync turns it On about 100 ms later. The first joint's Off and On
  straddle its post-SetZero blackout (start 511-614 ms after SetZero), the drive
  drops the On, and the 200 ms stale retry is later than the liveness bound.
  Decision: from `POST_SET_ZERO_BLACKOUT_FROM` (450 ms) to `POST_SET_ZERO_QUIET`
  (800 ms) after a SetZero's echo no type-24 On or Off is written to that drive.
  The reporting sync holds them, the baseline leaves a peer's stream On
  (the target's own Off is unchanged: it precedes its SetZero), and an On that
  was due goes out when the quiet ends. The drive's silence while a due On is
  held counts from the quiet's end. A stream applied On is not excused.
- An Active session discards a target's traffic as pose until its Enable echo,
  so its last pose is as old as the session, and the first Run reply (1.4-5.2
  ms) can be read a tick after the echo. Silence counts from the echo.

Not adopted: raising `comm_watchdog_ms` or shortening the stale retry (a
stream restarted at 200 ms would still miss a 100 ms bound), and counting
silence only from a confirmed first report (a dropped On would still lapse).

Bursts that overran the mcp251x during reference work are spaced:
BaselineStop's and finish's all-address stop start one address group per
`BURST_GROUP_SPACING` (2 ms) per interface, as do the baseline's type-24 Offs,
the type-0 admission requests (`IDENTITY_ADMISSION_SPACING`, with the admission
deadline growing by two spacings per further target) and the status solicit.
A fault, E-stop, cancellation or shutdown stop is never paced.

### Per-joint accumulation

Physical selection uses `SelectionScope::Accumulate`. Grants for different
joints accumulate under one realm and captured policy. Acquiring joint X
replaces only X's previous grant. A different policy starts a fresh set. This is
a deliberate change from the virtual rule, which still uses
`SelectionScope::Replace` (single joint). Ready still requires every configured
joint to be granted.

### Trusted coordinates

During reference work, measured-limit checks apply only to trusted
coordinates. The target is trusted only after its SetZero attempt
(`ReferenceReceivePhase::Enabled`), and peers only while they hold current
permission. An old target coordinate is not limit-checked against a reference
it is about to replace.

### Process lifetime

Grants are private to the owning process and are never persisted. Homing and
Enable must therefore happen in one `marengo-pi` process: stdin
`home <joints...> sign-tested`, then `enable <operator>`. `motor-repl set-zero`
runs the same workflow, but its grant ends when the process exits.

### Renames and non-writes

`ReferenceStageStatus::CurrentVirtualEvidence` is renamed `CurrentEvidence`,
because both backends produce it. The owner never writes `zero_sta` (0x7029) or
`add_offset` (0x702B), and never sends a type-22 parameter save. Robstride only
encodes and decodes those registers.

## Firmware assumptions [INFERENCE]

These come from the vendor manual and from probe frames
(`0000FD01` → `000001FE`, `1100FD01#1970…` → `110001FD#1970…`). They are not yet
qualified on the bench:

- the drive accepts SetZero while enabled and answers with a type-2 frame;
- the drive processes frames in order, so the 0x7019 readback reflects the
  post-SetZero zero;
- `mechPos` uses the same zero and frame as MIT position feedback;
- it is unknown whether SetZero persists across power cycles without a type-22
  save. The design does not rely on persistence, because every process starts
  Unreferenced;
- a drive reboot is silent for longer than `comm_watchdog_ms`, so liveness
  revokes the grant.

## Residual risk

A reboot faster than `comm_watchdog_ms`, combined with a zero shift smaller than
the at-rest tolerance, is not detected. The UID is unchanged, and the
discontinuity check only catches larger jumps. Keep the arm supported at
mechanical home during commissioning.

## Qualification and remaining work

Software is implemented in 88f2b9e (robstride) and e96798d (davout), plus the
marengo-config, Berthier, motor-repl and marengo-pi wiring. Tests use scripted
buses with literal wire frames and no hardware. Bench qualification is pending:
on the Pi, with the arm supported at mechanical home and hands off, confirm the
type-0, type-6→type-2 and type-17 0x7019 exchanges in candump. Then confirm
`reference <joint> current`, `home` → Ready, and a supported `enable bench` in
the same process (see [homing.md](../homing.md)). Before any movement, propose
concrete bounds, duration, caps and a stop plan, and require explicit owner
confirmation.
