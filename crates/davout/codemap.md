# crates/davout/

## Responsibility
Safety gateway and operational state machine — the **only** crate permitted to send motion commands to `robstride`. Every MIT or legacy command from Berthier, Talleyrand, or REPL tools must pass through Davout's filter pipeline before reaching CAN hardware.

Enforces: joint position envelope (URDF hard/soft limits + velocity-scaled kinetic margin, ADR 0009), kp/kd caps per motor type, tau_ff rate limiting, tau_ff max clamp, wrong-sign watchdog, communication watchdog, feedback velocity limit tripping, danger zone rules from config, E-stop assertion.

## Design

### Operational state machine (`OperationalMode`)
```
Disabled ──[set_homing_complete]──► Ready ──[request_enable(true)]──► Active
   ▲                                                                  │
   └────────────────────[disable_all / E-stop]────────────────────────┘
```
- `Disabled`: no motion possible, firmware may be idle.
- `Ready`: every loaded joint has private current-reference authority, motors not yet enabled.
- `Active`: motors enabled; servo/FF motion requires current-session pose from every active motor address.

### Core types
- `Supervisor<B: MotorBus>` — owns installed state/motor/model policy, inspection-only homing history, pose cache, persistent `FaultAuthority`, private reference permission and the `MotorBus`. Ordinary repo constructors have no qualified acquisition capability.
- `ReferenceAuthority` (`reference.rs`) — private owner-local, nonserializable, noncloneable reference permission. Relevant installed motor/frame/envelope/effective homing policy and the closed backend realm are bound independently of ordinary motion-stop generation. A consumed virtual grant also retains exact job, immutable installed model, address and device epoch independently of diagnostic caches and the old transaction deadline.
- `ReferenceOwner` (`reference_transaction.rs`) — one opaque reservation plus eight retained outcomes; one phase/one actual bounded report per advance, finite owner deadlines and immutable terminal cleanup. Closed virtual acquisition ends EvidenceStaged/CommitUnavailable without permission or history writes. Generic/physical owners remain Unsupported. The private consumer (`feedback_consumer.rs`) shares ordered hazard policy while each lifecycle owns its stop.
- `CommitOwner` (`reference_commit.rs`) — explicit unreferenced virtual commit lifecycle with owner-bound handles, eight accepted credits and eight completed outcomes. A fresh bounded disabled report and sticky continuity checks precede consuming a private matching actual completion. Existing factories remain history-only; the explicit current-consuming factory may select only the acquired joint after actual durability. Lifecycle, eligibility and disk result remain separate (ADR0035).
- `Journal` (`reference_journal.rs`) — lazy dedicated noncoalescing SQLite worker with exact schema/pragma verification, bounded real transactions and commit/readback before publication. `reference_journal_event.rs`, `reference_codec.rs` and `reference_urdf_codec.rs` preserve immutable typed policy and all pinned URDF fields/float bits. Reopened history is inspection only (ADR0034).
- `InstalledReferenceModel` (`reference_model.rs`) — bounded immutable private robot/URDF values and checked installation identity. Each successful typed rebuild/patch/restore replaces that identity, including equal values. Acquisition stamps and retained matched evidence bind it. `ReferenceStageStatus` is a live inspection-only continuity projection after actual cleanup, distinct from the immutable unusable terminal (ADR0027).
- `SimulationBus` (`simulation.rs`, `simulation_reference.rs`) — closed concrete finite raw/enveloped/timed/error scripts, source-indexed queues, typed impossible-wire consumer fixtures and declarative TX effects. Its specialized constructor installs a private virtual acquisition backend independently of optional INITIAL coverage; actual addressed SetZero and exact raw pops carry private epoch/correlation metadata. No wrapped bus, socket, callback, import or physical-state conversion exists.
- `SafetySnapshot` — owned read-only persistent records, complete and partial vendor domains, bounded first/latest receive-envelope and incomplete-work evidence, hardware input, stop generation, latest stop and first failed stop. Qualified recovery is unavailable in this slice.
- `StopReport` — every address's zero-speed, neutral-MIT and ordinary-disable attempt, including bounded errors. Accepted writes do not prove physical acknowledgement.
- `ControlMode` — re-exported to `berthier`: `Disabled`, `GravityComp`, `TorqueOnly`, `Impedance`, `Position`.
- `JointCommand` — legacy single-joint command (position + velocity + torque).
- `MitJointCommand` — filtered MIT command for one joint (kp, kd, position, velocity, tau_ff).
- `SpeedCommand` — firmware speed-mode command (bench diagnostics only).
- `DavoutError` — typed errors including command/feedback numeric invalidity, `NotActive`, `Estop`, `Limit`, addressed `CommWatchdog`, `MotorFault`, and `Homing`.

### Command admission (`admit_and_send_mit`)
Single-joint, legacy, and batch MIT sends share one admission path. Validate the whole batch before emitting its first frame:

1. Check E-stop, private current-reference authority, Active membership, configured motor mapping, and repeated joint names.
2. Reject nonfinite position, velocity, gains, and FF; gains must be nonnegative.
3. Filter each joint: kp/kd ceilings, velocity-scaled position envelope, hard bounds, danger-zone clamps, velocity ceiling, and wrong-sign policy.
4. Apply the hard FF ceiling before and after slew. First enable starts from zero, and a delayed tick earns at most 10 ms of slew credit.
5. Transform joint→motor values and reject nonfinite/overflowing `f32` wire fields before sending.
6. Require a valid pose newer than enable and within `comm_watchdog_ms` for every active address. Until the enable deadline, only zero-gain, zero-velocity, zero-FF MIT frames may solicit initial status.
7. Send the prepared addressed batch through Robstride's checked API. Admission failure restores output-history scratch state. Rejected numeric/gain/neutral-position requests remain nonlatching; runtime watchdog/sign/danger faults and transport uncertainty latch and automatically stop after any valid prefix. Motion owners separately discard planner/torque intent.

Startup validates the combined robot/motor/control/homing policy. `validate_control_candidate` checks proposed control overlays against the installed companion configuration before installation or persistence; it does not install policy or rebuild limits.

Calibration history is inspection data: every ordinary new Supervisor starts Unhomed, even with matching persisted rows. Ordinary and closed simulation construction share validation/initialization; simulation chooses the supplied root's `config/` independently of installed/ambient configuration (ADR0031). `from_repo` selects the legacy OS-path environment override or configured root-relative path; `from_repo_with_calibration_record_path` takes its path as supplied and ignores that override. Corrupt/unreadable history returns before startup reporting TX. Public mutable history, unchecked Ready, synthetic pose insertion and generic mutable transport access are removed (ADRs 0022/0023).

Ready, normal/scoped Enable, Active shortcuts, commissioning facets and output use the private permission. Legacy cached verification, raw SetZero and calibration arming refuse before TX/persistence; target, method and sign refusals remain specific. Unknown/unqualified physical protocols cannot produce a successful reference. Only `Supervisor<SimulationBus>::from_simulation` and its explicit-history counterpart can declare virtual INITIAL coverage. Ordinary `from_repo` remains unreferenced even for SimulationBus. Read-only `bus()` is generic; specialized `bus_mut()` returns a restricted script/trace facade without transport extraction/replacement.

Bounded virtual acquisition reserves without TX, stops every installed address,
suspends actually applied reporting, completes old and post-arm flushes, Enables
only its target and attempts addressed SetZero once. Wrong/untagged correlation
is diagnostic until the finite deadline; whole-report hazards win over matching
proof. Every terminal retains the actual all-address stop and reporting attempts;
neither a terminal nor a request stamp is a permit. Busy normal receive, reporting
sync/status solicitation and typed model/limit installs cannot interfere. Direct
legacy field mutation is observed before forward actions and cancels against the
original stop routes; a fresh stamp cannot reinstall a different drive/frame.
Snapshot inspection permanently revokes mismatched INITIAL coverage and retains
a live policy mismatch until mutable advance/cancel delivers cleanup. Restoring
public fields cannot resume that transaction. Phase time is capped by finite
overall remaining time before arithmetic; expiry wins a tied raw reply.

Retained matched evidence is bounded by the existing outcome cache. One private
validator observes post-cleanup owner/realm/address epoch/model/policy/reference/
stop continuity and the original finite deadline. A stage is current only for
the latest acquisition and successful cleanup. Observed mismatch is sticky;
restoring public policy or the old model cannot revive it. Old-handle snapshots
project that handle's own stage, and shutdown invalidates even a stopped stage.
Inspection emits no frames and never supplies reference permission. Durable
journal/commit and installed-owner clients remain separate work.

Relevant policy mismatch is permanently observed through facets, admission and receive; restoring public fields does not revive reference. Receive uses installed address/type/transform lookup, preserving original peer fault evidence despite corrupted public routing. Active mismatch stops the original installed addresses after consuming every ordered receive event. Rebuild/limit patches, new faults and uncertain stop revoke reference. Successful ordinary Disable preserves intact reference while advancing motion-stop generation; it still requires new post-enable pose for later motion. Output-only gain/friction/torque-cap/watchdog changes may preserve reference after complete shared validation; envelope/trim/resolved velocity and homing/frame changes cannot. The current motor and type torque caps still bound output after slew. Fully immutable coordinated policy/model installation remains CS15 work.

### Joint↔motor transform
- `direction` and `gear_ratio` from `motors.yaml`: position_rad *= scale, kp /= scale^2, kd /= scale^2, tau_ff /= scale where scale = direction * gear_ratio.
- inverse transform applied on feedback: motor→joint state. Direction must be ±1 and gear ratio finite and positive.

### Feedback processing
- `drain_feedback` — non-blocking poll (control loop path, budget = 0), with Robstride's total 64-raw-frame/256-read-attempt limits. Unknown and unsupported traffic consumes raw work before decoding.
- `refresh_feedback` — blocking poll up to `feedback_poll_budget_us` (REPL / set-zero).
- Merge motor observations, CAN Error envelopes and the first backend failure by their raw per-poll delivery ordinals. A backend failure precedes later same-ordinal peer frames; host timestamp ties cannot reorder the initiating cause. Every configured status is checked for finite raw/transformed fields, original receive time and measured hard-position evidence before chronology can skip it. All peer faults are retained even if an earlier pose or transport event is invalid.
- Status types 2/24 and detailed type 21 require exact eight-byte extended Data envelopes. Recognized malformed shapes latch without installing pose or renewing freshness. Data-only status headers and partial detailed/warning bytes remain separate evidence with available-byte masks; Remote identifiers never supply vendor status/fault proof. Error envelopes remain global transport evidence without vendor device-ID decoding.
- Idle/Quiet report completion is an observed host quiescence fact. WorkLimit/Deadline/Failed completion latches Transport independently of the terminal error, attempts all-address stop, and blocks later admission even when callers ignore the receive Result. Stops/re-enable do not synchronously empty an unread saturated suffix. Default empty blocking refresh and ordinary RecvTimeout remain benign when completion is complete.
- Vendor status flags, drive mode, four detailed-fault bytes and four warning bytes are separate domains. Full detailed word byte order and physical recovery remain unqualified; `JointFeedback.fault` is a compatibility nonzero indication, not a union of vendor bit identities.
- Reserved mode cannot publish admissible pose. A post-enable status from an enabled address must be Run; Reset/Calibration latches and stops. Reset from an unenabled scoped peer remains diagnostic. Mechanical SetZero enable is not a qualified factory-calibration context.
- "Post-enable" follows the wire, not the write. On a bus that `echoes_transmissions` (SocketCAN), a successful Enable write only queues the frame, so each enabled address stays pending until its own Enable echo is popped. Frames popped before that echo may predate the Enable on the wire: their hazards are inspected, but they are not held to Run and cannot become session pose. Frames popped after it keep the strict Run check. An Active address whose echo is still missing `comm_watchdog_ms` after activation latches DriveState and stops; the reference transaction does not leave `DrainPostArm` for SetZero before the target's echo, and its unrenewed phase deadline refuses a missing one. Buses without echo keep the pop-time rule (`received_at > active_since`). Disable has no strict post-disable mode check, so there is nothing to order there. Echoes are never pose, liveness, refresh counts or identity/readback/ack replies.
- Enable is staggered on echoing buses. Every host frame solicits a drive reply and the bench mcp251x holds only two received frames, so `poll_feedback` writes Enable + RunMode for at most one target per interface per control period (`loop_hz`); half of `comm_watchdog_ms` after activation any remainder is written at once. Every target is in `enable_echo_pending` from activation; an Enable echo for a target whose staggered write is still queued (`enable_writes_pending`) is ignored, so only its own echo arms the strict Run check. Berthier holds its first-feedback grace while `enable_writes_pending()`. Non-echoing buses write every target before activation. Type-24 writes (`sync` and the gate's Offs) take one slot per interface per control period (`active_reporting.rs`).
- Reporting Off precedes every Enable on echoing buses. Drives keep type-24 reporting across host processes, and a report built before a drive acts on Enable can be read after the Enable's echo, still in Reset. `enable_targets` and the physical reference baseline write each target's Off whatever this process applied. A target's Enable is written only once an Off written since the session start (`reporting_off_since`, any writer: `ActiveReportingState::off_written_at`) has its echo (`EchoedCommand::ReportingOff`) read at least one control period earlier (`reporting_off_settled`). Reference: `AwaitIdentity → AwaitReportingOff → ArmTarget`; a missing Off echo times out before any Enable. Active: an unwritten target whose Off echo is missing at the Enable-echo bound latches DriveState ("type-24 Off not observed"). Grant liveness for a target with a pending Enable echo counts from activation (`live_epoch` `withheld_since`), so the echo bound, not liveness, fails it closed.
- Empty drains, unknown addresses, fresh peers, and fault-only reports cannot refresh another motor's pose. Fault-only reports retain fault evidence without creating a zero pose. Older/replayed samples cannot replace a newer pose or mutate derivative policy state.
- Invalid feedback latches independent fault authority despite an older valid cache. A newer valid pose may restore diagnostic visibility, but it cannot restore motion permission. Active getters omit absent, invalid, stale and prior-enable pose.
- Inspect every raw frame's fault/mode/position evidence, including timestamp ties, then select the latest admissible pose per address for derivative/cache admission. Position-derived velocity/trips update once per address per drain: host dequeue spacing cannot recover physical acquisition spacing in a queued burst.
- Require complete bounded drains before enable writes and again after enable/run-mode writes, before creating the session marker. An incomplete preflush refuses activation; an incomplete/malformed final flush stops and rolls back. Status queued during those writes cannot authorize the new session. CAN status has no command-generation identifier; traffic arriving after the final drain remains uncorrelated, and session freshness cannot identify every delayed physical packet.

### Fault and stop lifecycle
- The first runtime/device/feedback/transport/controller hazard retains its stable ID/cause, attempts all stops, and increments a Supervisor-lifetime stop generation. Later healthy/empty diagnostics do not clear authority or repeat the stop burst. Additional hazard evidence and secondary delivery failures are retained.
- All motion, enable, calibration and SetZero routes consult the latch. `check_fault_authority` provides the same read-only gate to controller mode entry. `latch_control_fault` is a trusted owner hook for actual controller failures, not an operator reset.
- Explicit Disable always attempts all configured addresses and advances stop generation; it never clears faults. Every newly asserted hardware-input edge attempts a stop, even after an existing fault; releasing the boolean does not reset authority. GPIO integration is still absent.
- Checked Ready transition refuses Active; unchecked Ready is removed. Legacy calibration enable refuses before arming, including existing Active motion.
- The latest stop report and first failed report distinguish transport acceptance from unconfirmed physical stop. No automatic recovery or firmware fault-clear transaction is implemented. See ADRs 0020/0021 and the remediation ledger for remaining Pi/protobuf generation/publication, reference and drive-local qualification. Receive bounds do not qualify TX latency, command-dispatch priority, kernel queue loss, physical acquisition time or Pi loop jitter.

## Flow
```
Berthier MitJointCommand batch
        │
        ▼
  Supervisor::send_mit_batch
        │
        ├─ whole batch: numeric/identity/estop/mode admission
        ├─ per joint: filter_mit_core() + wrong_sign_step() (pure; staged)
        │   ├─ kp/kd cap
        │   ├─ position envelope clamp (armee-kinematics)
        │   ├─ danger zones (marengo-config)
        │   ├─ velocity cap
        │   ├─ tau_ff clip + rate limit
        │   └─ wrong-sign watchdog
        ├─ checked joint→motor transform (direction × gear_ratio)
        ├─ every active address: current-session pose watchdog
        ├─ commit staged tau_ff/wrong-sign state (rejected batch commits nothing)
        ▼
  robstride::mit_control_all_at
```

## Integration
- **Depends on**: `robstride` (MotorBus + CAN frames), `armee-kinematics` (limit envelope, URDF parsing), `marengo-config` (YAML configs), `marengo-homing` (homing registry), `chappe` (telemetry), `armee-proto` (wire types).
- **Called by**: `berthier` (ControlLoop::tick → send_mit_batch), REPL binaries (motor-repl, homing tool).
- **Does not**: compute tau_g, plan trajectories, encode CAN bytes, open SocketCAN.
- **Intended safety contract**: application motion enters through Supervisor. Mutable configuration remains compatibility surface: observed reference-changing edits permanently revoke permission, while installed cleanup routes cannot be redirected. Generic raw mutable transport and synthetic grants are closed. Persistent Pi publication/command generation, fully immutable coordinated policy/model installation, qualified reference transactions/recovery, physical stop confirmation and drive-local readback remain separate work. Virtual INITIAL fixture tests prove admission/output behavior, never physical acquisition or persist ordering.
