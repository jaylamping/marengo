# Marengo failure signatures

Grep this file for the exact text you saw. Entries are grouped by the phase in which the text appears. Each entry gives:

- **Means**: what the line says has happened.
- **Causes**: known causes, with dates and fixing commits where there are any.
- **Confirm**: the check that turns a match into a diagnosis.
- **Action**: what to do once confirmed.

Sources: `docs/safety.md`, `docs/commissioning/handoff-*.md` and `docs/commissioning/firmware/robstride-firmware-behavior.md`.

Add an entry whenever a cause is confirmed (see `SKILL.md` step 5).

## Startup and Transport

### `receive ended without observed quiescence: WorkLimit; raw frames=64`
- **Means:** Transport latched on the first control tick. The first bounded drain (64 frames / 256 reads) found more traffic queued than it may consume.
- **Causes:** drives still streaming type-24 reports with no owner, left On by a previous process. 2026-10-04: an exit path left all five streaming, about 500 frames/s, and 5 of 5 soak starts latched. Exits now send Off, but Pi power loss, a kernel hang or an Off dropped inside a post-SetZero blackout can still leave streams running.
- **Confirm:** `pi_candump_once` shows periodic drive reports (`18ss0NFD`, e.g. `180001FD` from drive 1, about every 10 ms) while nothing owns CAN.
- **Action:** `pi_motor_disable` (Disable + type-24 Off per drive) or `pi_protocol_inspect`, then retry. Do not relax the latch: that changes ADR 0021 and needs an ADR.

### `rx_over_errors` increases, or a kernel error frame (`2000xxxx#`, `CAN_ERR_CRTL_RX_OVERFLOW`) appears
- **Means:** the mcp251x controller (2 RX buffers) dropped a frame. Davout latches Transport persistently.
- **Causes:**
  - **A** (fixed `f49fd9be`): a host write's replies coincided with type-24 reports, so more than two frames arrived inside the driver's service time. Residual: any single paced write can still meet two coincident reports.
  - **B** (open): a 3.8–5 ms receive blackout 0.2–6.4 ms after the host Disabled a drive in Run (end of a reference), after which three frames arrived in a window that holds two. Seen in about 3 of 40 soak cycles in early October.
- **Confirm:** the soak report's `kernel error frames:` section, or the error frames in the candump. Correlate each with the host write just before it (`can-timeline`). A at a burst or solicit = A; at the Disable that ends a reference = B.
- **Action:** A recurring = regression (`bench-to-test` with `RxFifo`). B = run the physical diagnostics in `handoff-2026-10-04-liveness-hardening.md`: CANH–CANL resistance (about 60 Ω, power off), can0 ground/shield tie, scope INT across a stop, IRQ tracing. Never relax the latch.

### `SocketCAN kernel receive timestamp unusable` (WARN at power-of-two counts)
- **Means:** frames fell back to host read time (missing stamp, realtime clock stepped backward, or age over 1 s). Liveness then credits queued frames late.
- **Causes:** kernel or driver change, clock steps.
- **Confirm:** the count grows during a session; check `uname -r` against the last good run.
- **Action:** expect 0 per session. Investigate any appearance after a kernel or driver update before trusting grant-liveness results.

### Pre-session `disable` did not reach every drive (MCP aborts the session)
- **Means:** `motor-repl disable` could not send to some address before the run.
- **Causes:** CAN interface down, wiring.
- **Confirm:** `pi_can_status`; the per-drive `disable can0:N sent` lines.
- **Action:** `pi_can_up` (it refuses while something owns CAN), check power and harness.

## Reference (`home … sign-tested`)

### `physical reference grant revoked` (with joint, cause, counted silence)
- **Means:** one joint's process-local grant lapsed (ADR 0036). The revocation latches for the rest of the process.
- **Causes** (the line names one):
  - UID change, or a missing or mismatched UID in the type-0 check at Enable;
  - a coordinate discontinuity;
  - Calibration drive mode;
  - silence beyond `comm_watchdog_ms` outside reference work;
  - `owed type-24 On not written within OWED_ON_WRITE_BOUND`.

  Host-caused silence (stream restarted into a blackout, Enable echo before Run, host read gap, owed On) was fixed on 2026-10-03; a recurrence of one of those is a regression.
- **Confirm:** the counted silence against the candump for that drive: did it really stop transmitting, or was the host not writing or reading? (`can-timeline`)
- **Action:** a real drive silence points to harness, power or a drive reboot (see *Drive loss*). Host-caused silence = regression: `bench-to-test` with FirmwareBus.

### `home failed: … no private current-reference permission` / `enable failed: joint <j>: no private current-reference permission`
- **Means:** the joint has no live grant when home or enable needs one.
- **Causes:** an earlier revocation (look for the line above), or a reference never acquired in this process. Grants do not cross processes: a `motor-repl set-zero` grant dies with that process.
- **Confirm:** an earlier `physical reference grant revoked` for the joint, or no `reference <j> current pos=…` line in this process.
- **Action:** fix the revocation cause. Acquire and enable in the same `marengo-pi` process.

### `Enable held until the post-SetZero quiet elapses`
- **Means:** informational. An Enable to an address is held for 800 ms (`POST_SET_ZERO_QUIET`) after its SetZero echo, because the drive goes silent for 45–61 ms starting 511–614 ms after SetZero and ignores frames in that window.
- **Action:** none. Only a Reset report after the held Enable's echo is a fault (see `unexpected drive mode`).

### `reference <j> failed: …` / `reference skipped after earlier failure`
- **Means:** acquisition for one joint failed: no type-2 ack after SetZero, a readback outside tolerance, or a journal commit failure. Later joints are skipped.
- **Confirm:** the failure reason in the line; candump around that joint's SetZero (`0600FD0N`).
- **Action:** depends on the reason. A readback outside tolerance means the arm moved or was not at the reference.

## Enable

### `enable refused: …` then `discarded N deferred command(s)`
- **Means:** the gravity preflight refused. The coupled τ_g sweep over the live command envelope saturates `tau_ff_max`, the model could not be evaluated, or the sweep was voided by a stop, a new fault, reference work or a limit change. `force` does not bypass it.
- **Confirm:** the reason text. `pi_gravity_preview` at the envelope extremes shows which joint saturates.
- **Action:** a model or limit problem; fix the URDF or the limits. Raising caps needs bench evidence.

### `Enable requires full-master Robot Ready`
- **Means:** some master joint lacks a live grant (Robot Ready = every master joint Joint Ready).
- **Causes:** a revoked or missing grant on any joint. Historically, a host read gap during the preflight (fixed 2026-10-03).
- **Confirm:** a `physical reference grant revoked` line, or a joint missing from the `home` list.
- **Action:** as for the revocation.

### `enable failed: …` (after `waiting for enable to complete`)
- **Means:** the completion gate did not see every target Active, with no pending Enable writes and fresh feedback, within 2 s (`ENABLE_COMPLETION_TIMEOUT`). Every drive was stopped.
- **Confirm:** candump: the Enable echo, then the drive's first Run-mode status (`(id>>22)&3 == 2`)? Enable→Run normally takes 1.4–5.2 ms.
- **Action:** a missing Run reply = drive or wire problem (`can-timeline`). A held Enable that never got written = regression.

### `unexpected drive mode Reset for Disabled` / DriveState latch / `type-24 Off not observed`
- **Means:** a Reset-mode report was read after the Enable echo, or the type-24 Off that must precede the Enable was never read back.
- **Causes:** a stream left On through the Enable (2026-10-03, fixed by Off-before-Enable); an Enable written inside a post-SetZero blackout; a missing Off echo.
- **Confirm:** candump order for that drive: Off echo → Enable echo → status mode, and the SetZero time.
- **Action:** a recurrence is a sequencing regression: `bench-to-test` with FirmwareBus.

### `drives were disabled by an operator`
- **Means:** a motion command was refused after an operator disable (`forbid_implicit_enable`). This is expected.
- **Action:** send an explicit `enable`.

### `motion command refused` / `motion_refused` audit
- **Means:** the command came from the non-owner source (stdin vs Chappe), or arrived during a degraded hold.
- **Confirm:** the process's `--motion-owner` / `MARENGO_MOTION_OWNER`. An ad-hoc ssh session needs `MARENGO_MOTION_OWNER=stdin`.
- **Action:** send motion from the owner. In degraded hold only stop commands and `lower` are accepted.

### `waiting for enable to complete` (refusing `hold-on`/`hold-at`/`wave`)
- **Means:** normal. Position arms wait until enable completes and are deferred.

## Active motion

### `AscentStall` / `HoldTracking` / `WaveStall` (controller fault; reports `q`, `target`, `tau_p`, `tau_ff`, `tau_g`)
- **Means:** a Berthier fuse saw no measured progress for 2000 ms. AscentStall: an outbound target stays ahead. HoldTracking: off target by more than 0.03 rad with the net commanded torque pointing away. WaveStall: a wave not moving.
- **Causes:** HoldTracking at home = a gravity model fault (the 2026-10-03 pitch trip). Also friction/stiction exceeding what the law can push, a payload mismatch, or a clamp limiting torque.
- **Confirm:** the trip numbers. If `tau_g` sign or magnitude is wrong for the pose, it is the model. `trace-forensics` up/down bins at that q.
- **Action:** fix the model (gravity calibration, URDF). Never raise kp or ki to get past a model fault.

### `MIT total torque clamped` (at most once per second per joint)
- **Means:** Davout predicted that kp·e + kd·ė + τ_ff would exceed the joint's cap, and pulled `q_des`/`dq_des` toward `q`/`dq`. It also fails the bench score (`total-torque clamps 0`).
- **Causes:** τ_g near the cap at the pose, a large position error with high kp, a payload heavier than the model.
- **Confirm:** trace rows around the line. `trace-forensics` shaping counts, and τ_g against the cap.
- **Action:** a model or trajectory issue. Raising the cap needs bench evidence.

### `Feedback` fault, e.g. `right_elbow_pitch velocity 2.43 > 2.0 rad/s`
- **Means:** measured velocity exceeded the joint's limit, so the tick errored and drives stopped (2026-10-04).
- **Causes:** gravity-driven fall after a torque loss or stop, a fast commanded move plus gravity assist, a feedback glitch.
- **Confirm:** trace `dq` before the line, and whether a stop or CanTimeout preceded it.
- **Action:** find what removed the holding torque. Check that `docs/safety.md` *Reporting Off at exit* still holds after the stop.

### `DangerZone` / danger-zone clamp
- **Means:** a `control.yaml` danger-zone rule (evaluated on measured q/dq) clamped or faulted, e.g. `elevated_shoulder_pitch_fall`.
- **Action:** expected protection. If it fires in normal moves, check the rule thresholds against the RS03 ±20 rad/s velocity-scale fix (values tuned before `1ceeeb5` need a bench re-check).

### `WrongSign`
- **Means:** the wrong-sign watchdog (ADR 0015) saw motion opposite to the command.
- **Confirm:** `direction` in `motors.yaml` against the live sign check; the URDF axis.
- **Action:** a sign or config error. Disable and fix the config; never tune around it.

### Drive goes limp mid-motion with no host fault line
- **Means:** probably the drive-side CanTimeout. The right arm reads 600 (about 30 ms), so a host stall over about 30 ms stops the drives.
- **Confirm:** a gap of 30 ms or more in host MIT writes in the candump; a trace `t_ms` gap; `pi_protocol_inspect` CanTimeout readback.
- **Action:** find the stall: synchronous work in the tick, I/O, CPU. Changing CanTimeout is undecided (WP-I).

## Degraded hold and drive loss (ADR 0038)

### `drive lost <joint>: shed <a,b>; holding <c,d>; auto-lower in <s> s`
- **Means:** one Active joint with `on_drive_loss: shed_subtree` went silent. Its subtree is disabled, proximal joints hold and then lower, a Communication fault latches, and a restart is required.
- **Causes:** 2026-10-04, drive id 5 (`right_lower_arm_yaw`): a loose harness across the elbow, then a drive reboot.
- **Confirm:** candump: last frame from that device, then whether it came back with reset traffic (reboot). `pi_protocol_inspect` afterwards.
- **Action:** physical check of the harness and power to that drive (ask the operator). Restart `marengo-pi` to enable again.

### `stop during degraded hold not delivered to every drive`
- **Means:** a stop write failed for some address during an episode.
- **Action:** physical E-stop first, then CAN health.

## Stop and exit

### `shutdown stop delivery evidence; physical stop unconfirmed`
- **Means:** informational. Stop writes were accepted by the bus, which does not prove the drives stopped.
- **Action:** none, unless the next start shows streams left On (see the WorkLimit entry).

### stderr `error: capture date is unknown or invalid for new session smoke-hardware-sot-20260809T204048Z; register authoritative metadata explicitly before retrying`
- **Means:** log-store registration complaining about one legacy session. It appears at the end of every 2026-10-04 motion-suite session, after `finalized session`.
- **Action:** not a motor-path fault [unverified beyond that observation]. Note it and move on during bench diagnosis.

## Tool gates

### `FAIL gravity_model_mismatch`
- **Means:** the MCP gravity gate refused before enable. |τ_meas − τ_g| ≥ 0.20 Nm on some joint (live basis), or |τ_g| ≥ 0.20 Nm at the profile's hanging rest pose (fallback basis).
- **Causes:** URDF mass/COM error, a payload different from the profile, the arm not hanging at the rest pose.
- **Confirm:** the per-joint residual lines above it; `pi_gravity_preview`.
- **Action:** correct the profile or payload, or run a gravity calibration (the calibration tools skip only the hanging-rest refusal, by design).

### `gravity gate (basis=hanging_rest): gateway RobotState snapshot unavailable …`
- **Means:** informational. No live torque was available, so the gate checked model τ_g at the hanging rest pose.

## Physical and electrical

### Intermittent silence from one drive, especially across a joint that moves
- **Causes:** a harness strained by motion (2026-10-04: id 5 across the elbow).
- **Confirm:** silence correlated with the pose of the joint the cable crosses.
- **Action:** ask the operator to inspect and strain-relieve the harness.

### Unexplained receive blackouts or overruns with a clean host
- **Causes (unverified):** CAN termination (the checklist box is unticked) or ground/shield.
- **Action:** CANH–CANL resistance with power off (expect about 60 Ω); report the can0 GND/shield tie.
