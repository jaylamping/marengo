# WP-B design — explicit enable/reference sequencer

## Scope and constraints

This is a design only. No sequencer rewrite is proposed in this package. The immediate implementation remains the proven Davout logic at `lib.rs`, `reference_transaction.rs`, `feedback_consumer.rs`, and `active_reporting.rs`; any later migration must preserve wire behavior and the 20/20 `pi_enable_soak` result. It must not pace or otherwise delay fault/E-stop stops. Hardware timing observations are profile bounds, not a claim that the firmware contract is fully qualified.

## Current timing rules and anchors

The current implementation has three clocks and four relevant anchors, spread across methods/modules. `Instant` below means monotonic host time; only an echoed host write or a received drive reply establishes the stated wire-order boundary.

| Rule | Current behavior | Anchor / clock | Code anchor |
|---|---|---|---|
| R0 | Complete bounded RX drain before gate/Enable setup and again before the Active session marker; incomplete drain refuses | flush call / shared receive allowance | `lib.rs::enable_targets_inner`, `poll_feedback` |
| R1 | A target's post-enable traffic becomes strict Run evidence only after its own Enable echo; earlier frames are inspected for hazards, not admitted as pose | per-address Enable echo pop time | `feedback_consumer.rs` echo handling and strict Run predicate |
| R2 | Missing Enable echo faults at `comm_watchdog_ms` from that address's enable-bounds start | per-address `enable_bounds_start` | `lib.rs::enable_echo_overdue` |
| R3 | Reference DrainPostArm waits for the target Enable echo under the unrenewed phase deadline | reference phase-entry monotonic time; 2 s capped by search timeout | `reference_transaction.rs::DrainPostArm` |
| R4 | At most one Enable+RunMode target per interface per loop period; remaining ready targets catch up at half watchdog | `enable_write_due`, plus each address's bounds start | `lib.rs::take_enable_wave`, `issue_due_enable_writes` |
| R5 | All session targets are echo-pending from activation; old echoes cannot release unwritten addresses | activation / current session membership | `lib.rs::enable_targets_inner`, `feedback_consumer.rs` |
| R6 | Type-24 writes (sync and gate Off) use one per-interface 5 ms slot | last write per interface | `active_reporting.rs::slot_free`, `sync_holding`, `write_off` |
| R7 | Berthier first-feedback grace remains open while Enable writes are pending | live Davout pending-work query | `berthier/src/loop.rs` and `lib.rs::enable_writes_pending` |
| R8 | On echoing buses, every target's type-24 Off is issued in the current gate, echoed, then allowed one loop period to settle before Enable | gate-open time; Off write and echo read times | `lib.rs::begin_reporting_off_gate`, `reporting_off_settled` |
| R9 | A missing gate Off echo faults at the R2 bound | address bounds start | `lib.rs::enable_echo_overdue` |
| R10 | Grant liveness for echo-pending targets counts from the bounds start, not stale prior pose | address bounds start / echo time | `reference_physical.rs::withheld_since`, `lib.rs::enable_bounds_start` |
| R11 | Before Active Enable, retry type-0 identity requests every 10 ms, accept only post-watermark reply, bounded by 100 ms plus two spacings per additional target | first request + per-target request watermark; type-0 retry clock | `lib.rs::verify_physical_identities`, `reference_physical.rs` constants |
| R12 | No target Off/Enable in the 800 ms post-SetZero quiet; quiet starts at host write and is superseded by host echo read | per-address latest SetZero write/echo | `lib.rs::set_zero_on_wire`, quiet helpers; `feedback_consumer.rs` |
| R13 | Enable bounds start at max(activation, quiet end), re-anchoring catch-up, echo watchdog, bootstrap grace, and liveness | per-address later of activation and quiet end | `lib.rs::enable_bounds_start` |
| R14 | Physical SetZero proof requires target type-2 after the SetZero write; readback follows its request and ack | request watermark / ordered receive index | `reference_transaction.rs::AwaitAck`, `AwaitReadback` |
| R15 | One bounded phase per advance; new phase receives at most 2 s capped by overall search timeout; repeated await does not renew | phase entry / overall deadline | `reference_transaction.rs::phase_deadline` |
| P1 | Reference baseline/finish stop address groups, baseline Offs, Active identity requests, and status solicit are spaced 2 ms per interface; fault/E-stop/cancel/shutdown stop remains immediate | last burst-group start per interface | `burst.rs::BurstPacer`, its four production callers |

Firmware timing model: type-24 reports are nominally 100 Hz (7–13 ms measured), and each host frame solicits a reply. The mcp251x has two RX buffers; modeled service is 350 μs/frame. A frame burst can overflow before the 200 Hz host drain. After SetZero the drive ignores frames and emits no traffic in a blackout beginning 500–625 ms after the command and lasting 40–65 ms in the committed emulator model; the measured worst end is 667 ms after SetZero. Davout's 800 ms quiet margin intentionally exceeds that model. These assumptions are in `tests/physical_firmware/mod.rs`, `tests/firmware_profile.rs`, and `docs/commissioning/firmware/robstride-timing-profile.json`; they are not a universal vendor guarantee.

The current timing state is distributed: enable fields in `Supervisor`, echo ownership in `feedback_consumer`, SetZero/quiet gates in reference and enable paths, Off-slot state in active reporting, and grant liveness in `reference_physical`. This duplication is the architectural issue (L-davout-25), not evidence that the existing gates may be relaxed.

## Proposed sequencer model

One private `Sequencer` owns admission and per-address phase state. It uses typed inputs/events and returns a bounded set of TX intents; the existing Supervisor remains the sole executor of bus I/O and the authority for faults/stops. Keep the state machine deterministic and independently testable with a supplied monotonic `now`; never sleep inside a state transition. The physical model is an explicit immutable policy containing loop period, watchdog, type-24 spacing, identity retry/deadline, post-SetZero blackout/quiet, receive allowance, and reference phase limits. The constructor must validate impossible/zero timing relationships rather than silently substitute values. Constants may retain current deployed values until bench-qualified alternatives are approved.

### States

- `Idle`: no sequencing transaction owns the gates.
- `AdmissionFlush`: collect all target bindings and prove current reference; bounded pre-enable drain.
- `Identity { targets, pending, request_watermarks, deadline }`: issue spaced type-0 requests and admit only matching post-watermark UIDs. No Enable intent exists before all identities pass.
- `PrepareOff { gate_opened_at, per_target }`: for each address, wait for its quiet end, then issue Off through the type-24 scheduler.
- `AwaitOffEcho { written_at, deadline }`: accept only Off echo associated with the active gate and not earlier than its write; require one full loop period after echo. Missing echo refuses/faults under existing bound.
- `EnablePending { per_target, activation_anchor, next_wave_due }`: eligible targets require quiet elapsed and Off settled. Emit at most one address/interface/period except current catch-up policy. Target remains pending for strict Run and liveness until its own Enable echo.
- `AwaitEnableEcho { written_at, deadline }`: inspect all traffic; only own echo advances target to `AwaitRun`; timeout keeps existing DriveState fault semantics.
- `AwaitRun`: strict post-echo feedback validates Run and provides first pose; preserve first-feedback grace policy and per-joint grant liveness anchor.
- `Active`: all requested addresses passed their admission sequence; normal command path remains independently bounded by reference and watchdog checks.
- `ReferenceTransaction`: use the same timing policy and echo/quiet/Off primitives for target-only reference arm, while retaining its existing acquisition phases and evidence rules. Reference failure/cancel always funnels through same-call all-address cleanup.
- `Failed/Stopped`: terminal diagnostic state only; stop delivery remains handled by existing fault authority and is not paced for fault, E-stop, cancellation, or shutdown.

The sequencer must maintain per-address timestamps/identities, not global booleans: activation time, quiet end, Off write/echo, Enable write/echo, request watermark, and next due time. Distinguish `host_withheld_since` from real drive silence so R10 and post-SetZero reporting silence remain truthful. A successful TX is not a wire acknowledgement. If the transport cannot provide the echo contract required by a physical owner, refuse physical sequencing rather than silently skipping echo gates.

### Transitions and effects

1. `Idle → AdmissionFlush` only after current grants, policy/model binding, fault/E-stop and requested target set pass checks.
2. `AdmissionFlush → Identity` only after complete bounded preflush. Identity requests are serialized per interface; any TX error uses the established Transport latch/stop path.
3. `Identity → PrepareOff` only after every target's UID matches its grant. Missing/changed identity fails before all Enable writes.
4. `PrepareOff → AwaitOffEcho → EnablePending` per target only after quiet end, gate-window Off, echoed Off, and one loop period. Never let an unrelated/earlier Off echo satisfy the active gate.
5. `EnablePending → AwaitEnableEcho` emits Enable+MIT RunMode under current wave budget; maintain echo-pending membership from admission, not merely after write.
6. `AwaitEnableEcho → AwaitRun` only on that target's own host echo. Strict Run validates drive status; a missing echo faults at unchanged deadline.
7. `AwaitRun → Active` is per target for admission/liveness accounting. Public/session `Active` reporting semantics must be reviewed as a separate compatibility decision; do not claim a drive is energized before TX merely to start its watchdog clock.
8. Reference uses these shared gates but retains R14/R15 evidence/deadlines and target-only arming. Its SetZero write immediately updates the per-target blackout model, and the echo advances the quiet anchor. Cleanup stays all-address and same-call.
9. Any abort goes to `Failed/Stopped`; fault/E-stop/cancel/shutdown TX is immediate and must not be delayed by the normal TX scheduler.

## Migration plan

1. Add the pure state/data model and unit tests without changing production routing or timing.
2. Adapt only one gate at a time behind existing entry points: first Active Off/Enable wave, then echo/liveness timestamps, then reference Off/Arm phases, then identity admission and reporting holds. Keep public APIs and phase diagnostics compatible through the cutover; remove old duplicated fields only after all callers move.
3. Make the emulator's firmware profile and RX FIFO the conformance oracle for exact write/echo order, dropped frames, deadline equality, and per-interface pacing. Keep simulator/memory-bus behavior separately explicit; do not use non-echo behavior to claim physical qualification.
4. Run Davout physical-firmware/reference and pacing suites, workspace tests, then deploy only through the normal integration path. On Pi, run `pi_enable_soak` (20 cycles) and inspect `pi_candump_summary` plus position trace. A timing-policy change that modifies operator-visible delay requires recorded bench comparison; any slower fault/E-stop stop is excluded and needs an explicit safety decision, never an implementation side effect.

## Test strategy

Emulator tests should assert wire traces, not state-field wiring: each Off precedes the target Enable by a full period; stale/early echoes do not advance; target Enable follows echo before strict Run; requests and writes meet per-interface spacing; no Off/Enable/reporting On/Off enters modeled blackout; quiet and missing-echo deadlines hold at equality and recover only at specified anchors; one target's quiet does not delay another interface/address; drop/corrupt identity and bus failures result in no unsafe Enable and retained stop evidence; all reference `Err` exits after possible arming stop every installed address; disable/fault/E-stop bypass normal pacing. RX FIFO enforcement must turn unpaced ordinary test traffic into overflow evidence while all accepted sequenced cases stay below capacity. Keep specific tests for late Run reply/liveness and for repeated dropped type-24 On/Off.

Bench validation complements, never replaces, deterministic tests: `pi_enable_soak` 20/20 no-motion cycles after deploy; candump confirms type-0, Off echo, Enable/RunMode echoes and no overflow; compare actual time offsets to the committed timing model and record Pi position trace. No motion test is implied by a no-motion soak.
