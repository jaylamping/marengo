# Phase B, WP-I: stop-path independence

Branch `audit/wp-i`, verified against `cc47280f` (main after the phase A merge, with c879b6a, the RS03 ±20 fix and POST_SET_ZERO_QUIET=800 ms). No hardware was touched. The Pi was not contacted.

## Verdicts

| Lead | Verdict | Evidence | Fix commit |
|---|---|---|---|
| L-motor-repl-05 `disable` depends on the full Supervisor | **CONFIRMED, fixed** | Old path: `control.yaml`, `motors.yaml` validation, router open on every interface, `ControlLoop::from_repo` (URDF, calibration history) all preceded the stop (old `main.rs:144-220, 300-307`). Now `run_disable` (`bins/motor-repl/src/main.rs:138`) reads only `can_interface` + `device_id` per row (`marengo_config::load_motor_stop_targets_from`, `crates/marengo-config/src/lib.rs:344`) and sends one type-4 Disable per drive (`stop::disable_drives`, `bins/motor-repl/src/stop.rs:83`). Tests: `bins/motor-repl/tests/stop_independence.rs:23` `disable_needs_only_motors_yaml_and_reports_every_drive` (no control/robot/homing YAML, no URDF, a directory at `zero_registry.yaml`, no CAN: one `FAILED` line per drive, no config error); `:57` missing `motors.yaml` says "NO stop frame was sent"; `stop.rs:244` `dead_interface_fails_only_its_own_drives`; `stop.rs:222` exactly one type-4 frame per address; `stop.rs:277` a refused write is per drive; `marengo-config` `stop_targets_survive_config_that_fails_full_validation` (`lib.rs:1372`). | d805d62d |
| L-motor-repl-10 no SIGTERM/exit disable; MCP swallows stop failures | **CONFIRMED, fixed** | `enable`, `jog`, `speed`, `speed-stop`, `set-zero` arm an exit stop (`ExitStop::arm`, `main.rs:178`): a SIGTERM/SIGINT/SIGHUP thread with its own sockets (`stop::install_signal_stop`, `stop.rs:139`) and a cleanup stop on every error exit (`stop::exit_stop_required`, `stop.rs:126`). They refuse to start if the stop cannot be armed. Tests: `stop_independence.rs:116` `sigterm_disables_every_drive_then_exits_143` (real child process blocked on a FIFO, real SIGTERM), `:80` error exit of `set-zero` runs the stop, `:66` refusal without an armable stop, `:104` read-only commands do not arm, `stop.rs` `exit_stop_*`. MCP: `pi_motor_disable` and `pi_hold_off` audited exit 0 unconditionally; now `exitCodeOfRemoteOutput` (`tools/marengo-pi-mcp/src/ssh.ts`). The pre-session disable (`motion.ts` `benchLogWrapper`) no longer ends in `2>/dev/null \|\| true`; a disable that did not reach every drive aborts the session before `marengo-pi` launches. `pi_motor_recover` reports `RECOVER_FAIL`. `pi_set_zero` keeps both the set-zero and disable outcomes. Tests: `tools/marengo-pi-mcp/test/stop-wiring.test.ts` (8 cases, real bash for the set-zero body). | d805d62d, ddfbac66 |
| L-motor-repl-09 0 % coverage on `main.rs` | **CONFIRMED, fixed** for `disable`, the exit stop and `set-zero` error exit | 9 unit tests + 6 process-level tests above (was 0 tests). `set-zero` success path needs the physical reference owner and stays covered by Davout (`tests/physical_reference.rs`). Remaining 0 % commands (`status`, `gravity-preview`, `torque-cmd`) are WP-O/WP-G. | d805d62d |
| L-davout-27 (matrix gap #6) reference error exits leave the target enabled | **CONFIRMED structurally; unreachable through the public API today** | White-box `crates/davout/src/reference_transaction.rs:1847` `error_exit_after_arming_runs_the_all_address_stop` failed before the fix: an `Err` leaving `advance_reference` after `ArmTarget` left `reservation.armed` live and sent no stop. No input reaches the listed `?` exits today: `AdvanceReceiveBudget` is fresh on every call so `acquire` cannot return `Err` (`feedback_consumer.rs:116`); the target is resolved from the immutable `stop_motors`, and the policy check (`reference_transaction.rs` binding check, `reference_policy`) turns any change to it into `BindingChanged` with cleanup; `phase_deadline` overflow loses to the overall deadline; `motor_position_scale` is validated at load. Integration regression `tests/physical_reference.rs:532` (bus read error after arming) already passed: that path goes through `fail_reference`. `calibrate_joint_zero` (`lib.rs:896`) needs no `disable_all` once `advance_reference` cleans up: it can only see an `Err` from before a matching reservation exists, or after the terminal stop. Fix: `advance_reference` wraps `advance_reference_phase`; an `Err` with the reservation still live runs `finish_reference` (all-address stop) and still returns the original error. ADR 0026 updated. | bbb183e1 |
| L-davout-29 `stop_speed_command` refuses during a reference | **CONFIRMED, fixed** | `reference_transaction.rs:1913` `speed_stop_during_a_reference_stops_instead_of_refusing` failed with `ReferenceBusy { operation: "individual speed stop" }` and no frames. ADR 0023 says stop is unconditional with respect to reference. Now `stop_speed_command` (`lib.rs:2299`) escalates to `disable_all` while a reference is busy (cancels the reference, stops every drive, reports `StopDelivery`). Only caller is `motor-repl speed-stop` (prune candidate P-motor-repl-02). | bbb183e1 |
| L-marengo-pi-16 failed Chappe disable keeps intent | **CONFIRMED, fixed** | `bins/marengo-pi/src/stop_path_tests.rs:85` `chappe_disable_sets_mode_disabled_even_when_the_stop_was_not_delivered` fails with the old order (verified by reversing the two statements). `handle_chappe_enable` (`main.rs:364`) now sets `ControlMode::Disabled` before propagating the `disable_all` error. Impact was small: Berthier's `synchronize_stop_generation` discards the intent on the next tick (≤ 5 ms), and Davout is Disabled, but the published control mode was stale. | 2d97d1b0 |
| L-berthier-25 contradictory CommWatchdog policy | **CONFIRMED (berthier card right), fixed** | The preserve-hold branch is dead: `ControlLoop::tick` calls `discard_motion_intent` for every `LoopError::Safety`, `MissingFeedback`, `AscentStall` and `HoldTracking` before returning (`crates/berthier/src/loop.rs:1096-1105`). Pinned by new `crates/berthier/tests/tick_error_intent.rs:49,69` (a receive error and feedback silence in GravityComp both leave `control_mode()==Disabled`). The live defect was `let _ = disable_all()` swallowing a failed stop. `stop_after_tick_error` (`main.rs:348`) removes the dead branch, always sets Disabled, and publishes "stop after tick failure not delivered" in the `SafetyState` fault text (Davout also latches `StopDelivery`). Tests `stop_path_tests.rs:124,143`. Cards `intent/marengo-pi.md` §10 L5 and App D L-04 are wrong. | 2d97d1b0 |
| L-marengo-pi-14 untested owner safety surface | **CONFIRMED, partly fixed** | Added 7 tests (`stop_path_tests.rs`): Chappe set-zero confirm / sign-test / unknown joint / name trim / busy queue (`:171,189`), Chappe enable refused under a queued reference (`:207`), Chappe disable and tick-error stops. **Not done:** gravity-saturation preflight tests. Its semantics (fail-open `unwrap_or(0.0)`, single-joint sweep) belong to WP-G (L-marengo-pi-03/04) and any test written now would pin behaviour WP-G must change. | 2d97d1b0 |
| L-marengo-pi-21 queue E-stop cancel does not stop Davout | **REFUTED (moot)** | The queue reads `safety_snapshot().hardware_estop_asserted`, which is set only by `Supervisor::set_hardware_estop`; that call aborts the in-flight transaction with the all-address stop in the same call (`lib.rs` `set_hardware_estop` → `abort_reference_for_hazard`). `tests/physical_reference.rs:730` `estop_during_an_armed_reference_stops_the_transaction_itself` (armed target, E-stop, no reservation, every drive disabled, outcome `Failed`) passes without a fix. | none needed |
| L-davout-38 `DavoutError::Estop` unreachable | **CONFIRMED (dead code); NEEDS-DECISION, not changed** | Every `if self.hardware_estop { return Err(Estop) }` (`lib.rs:866,1095,1329,1350,1370,2122,2253`) sits after `require_fault_clear()`. `set_hardware_estop(true)` records a `HardwareEstop` fault in the same call and no API clears a fault in-process, so the flag is never true while the fault is clear. Operators see `FaultLatched{HardwareEstop}`. No caller matches `DavoutError::Estop`. See decision D4. | none |
| L-davout-20 hardware E-stop never wired | **CONFIRMED; NEEDS-DECISION** | Only definition and tests call `set_hardware_estop` (`lib.rs:1298`; callers: `tests/*.rs`, `reference_grant_tests.rs:793`). See D1. | none |
| L-robstride-04 drive CAN timeout never written; no independent watchdog | **CONFIRMED (gap); NEEDS-DECISION** | Zero writers/readers of `ParameterId::CanTimeout` outside `params.rs` (`params.rs:34`, encode/read test `:362`). See D2. | none |

Matrix gaps: **#3** (a) `disable` dependency fixed, (b) motor-repl exit stop fixed, (c) Chappe disable error path fixed, (d) Lagged Chappe channel is L-marengo-pi-06, WP-F, untouched, (e) E-stop wiring, D1. **#4** drive timeout and independent watchdog, D2. **#6** fixed, see L-davout-27.

Counts for the 11 assigned leads: CONFIRMED and fixed 6 (L-motor-repl-05, -09, -10, L-davout-29, L-marengo-pi-16, L-berthier-25), CONFIRMED and partly fixed 1 (L-marengo-pi-14), REFUTED 1 (L-marengo-pi-21), CONFIRMED but NEEDS-DECISION 3 (L-davout-20, L-robstride-04, L-davout-38). Also handled: L-davout-27 (matrix gap #6, WP-B lead) CONFIRMED structurally and fixed; L-robstride-02 wording fixed in the MCP, fault-clear frame NEEDS-DECISION (D3).

## Decisions needed

### D1. Hardware E-stop input (L-davout-20, gap 3e, L-marengo-pi-21)

What exists:

- `hardware/electrical/wiring/connectors.md:5-17`: normally-closed chain in series with the motor enable relay; 2-pin JST-XH to Pi `ESTOP_SENSE`, **BCM 17, input, pull-up, 0 = asserted**. Header says "Prototype bench values".
- `hardware/docs/decisions/0001-can-and-motors.md:64-70`: asserted ⇒ no torque, software must read the input and stay Disabled, reset needs E-stop released and a homing sequence.
- `docs/pi-commissioning.md:14`: `[ ] E-stop ordered: NC mushroom in motor power path … GPIO 17 sense optional initially`. Nothing in the repo says the button or the sense wire is installed.
- No code reads a GPIO: no gpio crate in any `Cargo.toml`, no sysfs/gpiod access in the workspace. `Supervisor::set_hardware_estop` has no production caller. `docs/safety.md:152` and `docs/position-hold-control-review.md:75-82` already say so.
- What still works without wiring: if the chain cuts motor power, the drives go silent and Davout's comm watchdog (100 ms, `control.yaml`) disables and latches. What is lost is the explicit `hardware_estop_asserted` telemetry and the reference-queue cancel (moot, L-marengo-pi-21).

Options:

1. **Do nothing until the E-stop and sense wire are physically installed**, then implement: a `GpioEstop` reader in marengo-pi behind a trait, polled each tick, calling `set_hardware_estop`. Configured-but-unreadable input counts as asserted (fail closed). Line number and polarity come from config (BCM 17, active-low). Needs a GPIO library (gpiod char device, no `unsafe`) and a bench check of polarity with the button pressed.
2. Declare the sense line permanently unsupported, delete `set_hardware_estop`, the `Estop` variant (D4) and the `SafetyState` field, and rely on power cut + comm watchdog. Smallest code, loses the telemetry.
3. Wire it through a Pi-independent path (a CAN-side or relay-side signal), out of scope here.

Recommendation: option 1, gated on the hardware. I did not implement it because nothing confirms the wiring, and an unverified polarity or a floating pull-up on the safety input would either latch every session or read "clear" with the button pressed.

### D2. Drive-side CAN timeout and an independent watchdog (L-robstride-04, gap 4)

Manual evidence (`/tmp/rs03.pdf.txt:1095-1100`, `rs02.pdf.txt:1381-1387`, `rs00m.txt:958-964`, `rs04m.txt:1273-1279`; same text in all four, RS03 manual 260713 §3.3.6 "can communication failure protection"):

> When the value of CAN_TIMEOUT is 0, this function is disabled. When the CAN_TIMEOUT value is non-0, when the motor does not receive the can command within a certain period of time, the motor enters the reset mode, and 20000 is 1s.

The parameter table lists `0x7028 canTimeout`, "can timeout threshold, 20000 is 1 s", `uint32`, 4 bytes, default 0, read/write (`rs03.pdf.txt:1746`, `rs02.pdf.txt:2035`, `rs04m.txt:1950`; the RS00 table is garbled, `rs00m.txt:1688`). RS03/RS02 also list `0x200c CAN_TIMEOUT` ("status2", uint32, default 0) in the persistent table (`rs03.pdf.txt:607-615`). `crates/robstride/src/params.rs:34,75` already encodes `CanTimeout = 0x7028` as `U32`, and `hardware/docs/decisions/0002-robstride-protocol.md:57` records the same kind.

So the unit is **50 µs per count** (20000 = 1 s; 100 ms = 2000 counts) and "reset mode" is a torque-off Disable. I did not implement the write, because the semantics are not certain enough to ship:

1. **Installed firmware not verified.** The manuals are 260713; the bench drives report firmware 0.3.1.42. `hardware/docs/decisions/0002-robstride-protocol.md:62-67` says the model/firmware timeout readback and physical acceptance are still open (M02), and old vendor documents conflict on register IDs.
2. **"Can command" is undefined.** The manual does not say whether an MIT type-1 frame, a type-17 read, a type-24 reporting command or a type-3 Enable resets the timer. If only some count, the write defeats the watchdog or fires during a healthy hold.
3. **Davout leaves an enabled drive without MIT traffic.** A reference transaction arms the target with Enable only (`reference_transaction.rs` `ArmTarget`), then waits for ack and readback with no MIT stream, up to `PHASE_TIMEOUT` 2 s per phase and ~0.8 s of post-SetZero blackout. Any timeout below ~3 s would reset the target mid-reference. The value persists after Marengo's session (type-18 write is RAM; Marengo never sends type-22, `safety-invariants.md` S-19), so it also outlives the Active session that wrote it.
4. **Policy trade-off.** A host that dies mid-hold leaves the arm either on its last MIT frame (today: τ_g feed-forward keeps holding, but a kp·e term can also drive into a stop) or limp after the timeout (an elevated arm drops, the upright-pose incident class in `docs/safety.md:59-61`). That is a physical-safety choice, not a software one.
5. **Writability in Reset mode unknown** (some Robstride parameters reject writes unless the drive is in a particular mode).

Read-only bench probe (no motion; owner stopped; arm supported anyway):

1. `pi_restart_marengo_pi` `mode=stop` (confirm), `pi_can_status`, check that no `marengo-pi`/`motor-repl` runs (`pi_health`).
2. On the Pi: `candump -L can0,110000FD:1F0000FF > /tmp/ct.log &` (type-17 replies to host id 0xFD; status in id bits 23..16, device id in bits 15..8).
3. For each configured device id N (hex, 1..5): `cansend can0 1100FD0N#2870000000000000` (type 17 read of index 0x7028, little-endian). Wait ≥ 50 ms per request (reply latency is in `docs/commissioning/firmware/robstride-firmware-behavior.md`, "Type-17 parameter read reply").
4. Decode each reply line `11SSNNFD#2870 0000 V0V1V2V3` (`SS` = status, 00 = ok; `NN` = device id; data bytes 4..7 are the u32, little-endian), as `decode_read_parameter_reply` does (`params.rs:283`). Record per drive: value, and divide by 20000 for seconds. Also read `0x200c` the same way (index bytes `0C 20`).
5. Expected from the manual: 0 (disabled). A non-zero value means a previous tool (Motor Studio) set it. Record firmware (`pi_health` / type-0 identity).
6. Do **not** write 0x7028 in this probe. A later, separate write test (value 2000 or larger, drive Disabled, read back, then run a no-torque enable and kill the host to see whether the drive leaves Run mode within the timeout) needs an explicit go-ahead because it changes drive state.

Options once the probe is in:

- **A. Active-session timeout.** Davout writes and reads back `CanTimeout` for each target in `enable_targets` (after reference, before Run), with a value ≥ 2× the longest legitimate silence on an Active drive (suggest 250 ms = 5000 counts), and writes 0 in the reference `BaselineStop` so a stale value cannot reset a target mid-reference. Needs a test pinning the `0x7028` U32 write encoding and the readback.
- **B. Long timeout set in every stop/enable** (≥ 3 s, above every reference phase). Simple, but the arm keeps driving for 3 s after the host dies.
- **C. Process-level independent stop, no protocol change.** Add `ExecStopPost=/opt/marengo/bin/motor-repl disable` (and optionally `WatchdogSec=` with `sd_notify` from the control loop) to `scripts/systemd/marengo-pi.service`. The now-independent `motor-repl disable` then runs after SIGKILL, panic, OOM and a hung loop that systemd kills. It does not cover a kernel hang or power loss of the Pi.
- **D. Both A and C.**

Recommendation: C now (cheap, no firmware semantics, needs only the service-file change and an install-pi.sh daemon-reload), A after the probe confirms semantics and the user decides the "limp vs hold" trade-off in item 4.

### D3. Fault clear (L-robstride-02)

Manual evidence: communication type 4 data field, "When the motor is running normally, 0 must be cleared in the data field. **Byte[0]=1: The fault is cleared.**" (`rs03.pdf.txt:1337-1341`, identical in RS02, RS00, RS04). `encode_disable` (`crates/robstride/src/lifecycle.rs:19`) sends all-zero data, so the existing Disable is not a fault clear, as ADR 0020 already says ("Ordinary stop payloads do not clear firmware faults … This slice provides no fault-reset capability").

Done: the MCP descriptions and comments no longer call Disable a "primary fault clear" (`motion.ts`, `pi_motor_disable`, recover body), `docs/safety.md` states it, and `pi_motor_recover` still reports `RECOVER_FAIL` for a non-zero fault and says to power-cycle.

Not done: a Byte[0]=1 frame. The wire semantics are written in all four manuals, but (1) the manual says nothing about which faults clear or whether the drive re-enters Reset, (2) it is unverified on firmware 0.3.1.42, (3) clearing a drive fault is a recovery authority that ADR 0020/0019 reserve for a qualified, explicit recovery path (Davout's fault latches do not clear in-process; an operator-triggered drive clear must not look like safety recovery), and (4) there is no safe way to produce a fault on the bench to verify it.

Options: (a) leave it: power-cycle remains the recovery (current behavior); (b) add `encode_clear_fault` plus `motor-repl clear-fault <joint>` that sends the frame and then reads type-21/status to report the result, never touching Davout's latches, after a bench check with a naturally occurring fault; (c) same as (b) but exposed through `pi_motor_recover`. Recommendation: (a) until a fault actually needs clearing on the bench, then (b) with the probe above.

### D4. `DavoutError::Estop` dead branches (L-davout-38)

Tied to D1. If D1 option 1: reorder so the `hardware_estop` check precedes `require_fault_clear()` (operators see `Estop`, keeps the defence in depth). If D1 option 2: delete the variant and the seven guards. Either is a small change; neither alters safety behaviour today.

## Bench behaviour changes

- `motor-repl disable` now sends only type-4 Disable, one per drive: 5 frames instead of 15 stop frames plus a type-24 reporting sync. It prints one line per drive, exits 1 if any drive's interface could not be opened or written, and no longer needs `control.yaml`, URDF or homing history.
- Every MCP motion session (`pi_hold_on`, harness, gravity calibrate, `pi_enable_soak`) now refuses to launch `marengo-pi` if the pre-session `motor-repl disable` did not reach every drive. Before, it continued.
- `pi_set_zero` exit code is now the set-zero exit code when set-zero fails (before: always the disable's).
- `motor-repl set-zero/enable/jog/speed/speed-stop` run a Disable on every drive at an error exit and on SIGTERM/SIGINT/SIGHUP, and refuse to start if `motors.yaml` cannot be read. `enable`, `jog`, `speed` and `speed-stop` also stop at a *successful* exit (one-shot processes with no host behind the drive), but those success paths are unreachable since ADR 0036 (L-motor-repl-01, WP-O).
- marengo-pi: a failed Chappe disable and a failed tick-error stop are now reported; no behaviour change when the stop succeeds.

## Verification run

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings`, `cargo test --workspace` and `npm ci && npm run build && npm test` in `tools/marengo-pi-mcp`, run in the worktree after the last commit. See the final report.
