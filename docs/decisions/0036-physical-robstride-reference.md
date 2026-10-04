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
  (see *Host-caused silence* and *Solicited silence*, below).

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
  held counts from the On's actual write, within a bound (see *Owed On*,
  below). A stream applied On is not excused.
- An Active session discards a target's traffic as pose until its Enable echo,
  so its last pose is as old as the session, and the first Run reply (1.4-5.2
  ms) can be read a tick after the echo. Silence counts from the echo.
- Receive times are host read times. Enable runs the gravity preflight
  (64-68 ms on the Pi) without reading CAN, so a drive whose post-SetZero
  blackout covered the last read before it lapsed with its reports still
  queued (`pi_enable_soak` at 84e80653, 6 of 20 cycles). Enable resolution
  drains before it builds the facets, and a non-Active drain judges liveness
  after it reads the queue. A drive that stops during a host stall loses its
  grant up to one stall later, and Enable still needs a fresh type-0 reply.
  Active drains now judge after reading too (see *Solicited silence*, below).

Not adopted: raising `comm_watchdog_ms` or shortening the stale retry (a
stream restarted at 200 ms would still miss a 100 ms bound), and counting
silence only from a confirmed first report (a dropped On would still lapse).

Bursts that overran the mcp251x during reference work are spaced:
BaselineStop's and finish's all-address stop start one address group per
`BURST_GROUP_SPACING` (2 ms) per interface, as do the baseline's type-24 Offs,
the type-0 admission requests (`IDENTITY_ADMISSION_SPACING`, with the admission
deadline growing by two spacings per further target) and the status solicit.
A fault, E-stop, cancellation or shutdown stop is never paced.

### Solicited silence and owed Ons (amendment, 2026-10-03)

Two gaps remained after the soak fixes above (handoff
`docs/commissioning/handoff-2026-10-03-audit-soak.md`): a host stall of about
95 ms or more while Active revoked every grant and stopped every drive at the
next drain, which can drop an elevated arm held in GravityComp; and
synchronous work of `comm_watchdog_ms` or more spanning a held On's quiet end
revoked that joint before any sync could write the On. Both are host-caused
silence. The rule is now stated once: **a drive's silence counts from the
host frame that asked it to speak.**

*Receive-time contract.* `received_at` stays a `std::time::Instant`. On
SocketCAN it becomes the kernel RX time mapped onto the monotonic clock and
clamped between the previous frame's stamp (or the last empty read) and the
read instant; that change lands from the RxTimestamps branch. Own-TX echoes
carry the same stamp (TX completion on the mcp251x). A missing or future
stamp falls back to the read instant. Other buses (MemoryBus, SimulationBus,
FirmwareBus) supply their own instants, and Davout does not depend on which
bus produced one.

*Drains judge after reading, in every mode.* A drain checks policy, realm,
model, identity and coordinate epoch before it reads the queue, and judges
grant liveness once `consume_feedback_report` has run. While Active a failure
of either check disables every drive and returns `Homing`. The one exception
is an Active target whose Enable echo is pending: it is judged before the read,
because its traffic is not pose (reading cannot renew it) and a staggered
Enable may be written to it before the read. Post-read judgment is safe with
kernel timestamps: each frame carries its true wire time, so a queued report
proves only what the wire carried, and a drive that stopped during a stall is
judged on its true last frame at the first read after the stall. With the
read-time fallback, queued frames are stamped at the read, so a drive that
stops during a stall is credited with its queued frames and loses its grant up
to one stall later (one stall of delayed detection).

*Solicited silence while Active.* Active streams are Off: a drive speaks only
when the host writes to it. An Active target's silence counts from the
**earliest host frame it answers that no admitted pose has followed**: its
Enable, and every MIT batch, whether or not the batch commands that target (the
controller commands every Active joint each tick, so an omission fails closed
as before). The instant is the host's write instant, sampled before the write,
never later than the frame's wire or echo time, so it fails closed and needs no
echo; robstride does not classify MIT echoes, and a lost echo cannot open an
excuse. With nothing outstanding no silence counts: the host is the silent
party. Once the host writes, an unanswered solicit lapses after
`comm_watchdog_ms` exactly as before; at 200 Hz every tick solicits, so normal
operation is unchanged and **the rule never excuses a drive while the host is
soliciting it**. A drive that stops answering while the host ticks is revoked
`comm_watchdog_ms` after the first write it leaves unanswered, at most one
control period later than when silence counted from its last frame. A pose
admitted in a drain answers every write before that drain, so a late reply to
the previous batch can be credited to the next one (about one more period).
The same rule decides when Active session pose is current for the MIT pose
watchdog (`CommWatchdog`) and `joint_feedback`: a pose is stale once a write
it answers has gone unanswered for `comm_watchdog_ms`. An Active session that
ends carries this count into the Disabled rule: each target counts from its
outstanding solicit, or from the stop (which every drive answers) when none
is outstanding, so a stall right before a Disable is still the host's.

Physical behaviour during a host stall while Active: there is no drive-side
CAN timeout, so the drives keep applying the last command they received
(GravityComp keeps holding). The first batch after the stall is computed from
a pose as old as the stall, through every Davout filter unchanged (envelope
clamp, `tau_ff` rate limit, danger-zone caps); the next reply refreshes it.
A drive that died during the stall is revoked `comm_watchdog_ms` after the
first post-stall solicit, so detection takes the stall plus
`comm_watchdog_ms`. That latency is unavoidable: the host was not watching.
The excuse ends at the host's next frame to the drive, and every action that
relies on the grant (MIT output, Enable, Disable) sends one. Judged checks
that run before the first drain after a stall (a status query) still count
the queued, unread replies as silence; the control loop drains first.

*Owed On.* While the reporting sync holds a due type-24 On (the
`POST_SET_ZERO_BLACKOUT_FROM`..`POST_SET_ZERO_QUIET` window), the drive's
silence is excused until the On is actually written; from the write the
ordinary `comm_watchdog_ms` rule applies. The excuse is bounded: if the On is
still unwritten `OWED_ON_WRITE_BOUND` (200 ms) after the later of the quiet's
end and the end of owner reference work (owner work suspends the sync), the
joint is revoked with the distinct cause `owed type-24 On not written within
OWED_ON_WRITE_BOUND`. 200 ms is twice the slowest synchronous host work
measured on the Pi (the gravity preflight, 96 ms), rounded up to 50 ms, so one
preflight starting just before a quiet ends never revokes. It stays well under
anything that matters physically: an On is owed only outside an Active
session, so the drive is disabled and no command depends on it; the whole
excused window after a SetZero is at most 1.0 s plus `comm_watchdog_ms`; and
Enable still needs a fresh type-0 reply and, once Active, an answer to its
Enable within `comm_watchdog_ms`. An owed On that stops being wanted (an
Active session, an expired lease) is released and counts from the quiet's
end, as before. A stall longer than the bound with four streams queues more
than the 64 frames one drain reads, so the incomplete drain latches Transport
(ADR0021) as well.

Residual risk: a reboot that both starts and ends inside one excused window
(a host stall while Active, or an owed On's quiet plus bound) and keeps the
coordinate within the continuity bound is not detected by liveness. While
Active a rebooted drive answers in Reset, which latches DriveState; outside
Active the next Enable needs a fresh type-0 reply, which does not detect a
reboot that kept the UID. Keep the arm supported during commissioning.

Not adopted: counting Active silence from the last frame with kernel
timestamps alone (a stall of 95 ms or more would still revoke every grant,
since the drives were not asked), and counting from MIT echoes (robstride
drops them as host commands; the write instant is earlier, so stricter).

Tests (`crates/davout/tests/physical_reference.rs`, FirmwareBus with
wire-time stamps where named): `an_active_session_keeps_every_grant_through_a_host_stall`,
`a_drive_dead_through_an_active_host_stall_is_revoked_after_the_first_solicit`,
`a_drive_that_stops_answering_while_the_host_ticks_is_revoked_as_before`,
`leaving_active_after_a_host_stall_counts_silence_from_the_stop`,
`a_host_stall_across_a_held_on_keeps_the_grant_until_the_on_is_written`,
`a_drive_silent_after_its_owed_on_is_written_loses_its_grant`,
`an_owed_on_never_written_loses_the_grant_at_its_bound`; unit tests
`comm_watchdog_fires_on_silence` and
`comm_watchdog_does_not_count_the_hosts_own_silence`.

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

### Journal history is schema-tolerant for integrity checks (amendment, 2026-10-03)

Opening the journal no longer decodes stored rows with the current typed
config schema. History integrity (SHA-256 checksum of the body, session/job
identity against the row key, session bounds, body length and capacity, exact
schema/meta checks) runs on a version-tolerant identity view
(`diagnostic_session`, `job_sequence`) that skips the policy, robot and URDF
payloads. A row written by an older binary — e.g. one carrying a retired
control key like `allow_firmware_speed_mode` — still opens, and new commits
append normally. Checksum or identity mismatches still fail closed and
preserve the stored bytes. History never grants, so payload semantics are not
rechecked on open. Inspection reports such rows as legacy records (session,
job and checksum only; typed accessors return `None`) instead of failing the
whole listing.

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
