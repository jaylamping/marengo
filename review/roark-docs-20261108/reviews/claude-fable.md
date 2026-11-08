Verification done. Below is the full set of proposed edits. Facts cited as `path:line` are files I read in the packet. The four RobStride PDFs could not be rendered in this session, so every "manual" number is checked only against repo documents that quote the manuals (the CAN research report, `pdb/PLAN.md`, `CALIBRATION_LOOP.md`, legacy code comments). Those are marked "manual-derived, not read directly".

## Must-fix

**M1. Damper rule contradicts the tenet unless the value is explained.**
Where: `roark/PLAN.md` §6, row "Damper 0x702A (RS03 also 0x2026)".
Current: "Read only; must read 0. Never write 1".
Replace: "`damper` 0x702A. Read only; never write it. The vendor default 0 means the post-power-off anti-backdrive protection is **on**; 1 disables it (RS00/RS03/RS04 manuals §4.2.5; the RS02 manual lists the parameter without the section, so confirm by hand-spinning an unpowered RS02). Preflight refuses to enable if it reads anything but 0. The research report's 'write the damper on' is superseded: never write 0x702A."
Why: TENETS §3 says "the anti-backdrive damper left on"; an agent reading "must read 0" alone will conclude "damper off". Source: `marengo/hardware/electrical/pdb/PLAN.md:245-250`, `marengo/hardware/electrical/wiring/left-arm-trunk/README.md:93`; conflicting instruction at `marengo/hardware/docs/research/can-stack-for-marengo.md:67`. The "0x2026 on RS03" alias appears only in the trunk README line 93 and is not verified against the manual; mark it "(unverified, trunk README)" or drop it.

**M2. CanTimeout preflight condition is ambiguous and omits the known bench state.**
Where: `roark/PLAN.md` §6, row "`canTimeout` 0x7028".
Current: "Write 1000 (= 50 ms; 20000 = 1 s) and read back before every enable; refuse on 0 or above."
Replace: "Write 1000 (50 ms; the unit is 50 µs, 20000 = 1 s, 0 = disabled) and read it back before every enable; refuse unless the read-back is exactly 1000. Treat the value as volatile: write it at every runtime start and after any drive power cycle or reset report, and never persist it with type 22 (parameter save), which the TX allowlist forbids. The right-arm drives currently read 600 (30 ms) left by PR #254; whether any value survives a power cycle is unmeasured, so the first bench session logs a power cycle followed by a read-back and records the answer here."
Why: "0 or above" has no upper bound stated. Sources: `can-stack-for-marengo.md:59` (manual-derived), `marengo/hardware/docs/decisions/0001-can-and-motors.md:67`, `marengo/docs/safety.md:596-599`, `marengo/docs/commissioning/2026-10-02-right-arm-pr254-findings.md:21-26`, type-22 refusal at `marengo/crates/robstride/src/bus.rs:40-45`.

**M3. "Re-check after any drive power cycle" has no detection rule.**
Where: `roark/PLAN.md` §6, same row, and new text under §6 table.
Current: "Re-check after any drive power cycle or reset report".
Replace (add after the table): "**Preflight invalidation.** A drive's preflight result is dropped, and it must pass preflight again before any enable, when: it misses 3 consecutive replies; any reply shows mode ≠ run (bits 23-22) while it is enabled; any fault bit is set; the bus goes bus-off, down or disappears; or the runtime restarts. A drive that reappears after silence is in reset with unknown parameters until read back."
Why: a power cycle, an E-stop press, or a cable pull all look the same on the wire (silence then a reset-mode reply); without a stated trigger the agent must guess. Mode bits per `can-stack-for-marengo.md:50`.

**M4. The runtime must never enable on its own after a restart.**
Where: `roark/PLAN.md` §6, row "Hang".
Current: "systemd `WatchdogSec` fed from the control thread; later the power board heartbeat (item 10)".
Replace: "systemd `WatchdogSec` fed from the control thread (a hung loop is killed); `ExecStopPost` runs the independent disable on every exit (normal, crash, SIGKILL, watchdog); `Restart=always` is allowed only because the runtime starts with every drive disabled and enables nothing without an operator command. Later the power board heartbeat (item 10)."
Why: the legacy unit already uses `Restart=always` plus `ExecStopPost=motor-repl disable` (`marengo/scripts/systemd/marengo-pi.service:16-25`); without the no-auto-enable rule a crash loop could re-energise the arm. Legacy `main.rs` has no watchdog feeding at all (grep for `WatchdogSec|sd_notify|watchdog` returns 0 matches).

**M5. Lost bus and adapter naming are not covered.**
Where: `roark/PLAN.md` §6, row "Bus-off", and §5.
Current: "Stop the whole robot; no automatic restart while moving".
Replace: "Bus-off, interface down, or interface gone (USB adapter unplugged, `ENODEV`/`ENETDOWN`): stop the whole robot; the drive timeout covers the drives on the lost bus. No `restart-ms` while any drive is enabled."
Add to §5 after "confirm `dmesg` shows `gs_usb`": "Bind each adapter to its skeleton bus name with a udev rule keyed on the adapter's USB serial or port path, because gs_usb adapters otherwise enumerate as `canN` in plug-in order and can swap names across reboots. While the MCP2515 HAT is still fitted it owns `can0` and `can1`, so the USB adapter will appear as `can2` unless renamed; do the udev rule before the first scan. At every runtime start, scan each bus and refuse to enable a limb unless every skeleton ID on that bus answers and no unlisted ID answers."
Why: worst case is the left-arm command stream landing on the right-arm bus after a reboot. HAT ownership of `can0`/`can1`: `marengo/scripts/systemd/marengo-can.service:11`, `0001-can-and-motors.md:17`. Startup-scan rule from `can-stack-for-marengo.md:32`, missing from PLAN.

**M6. "Only the realtime process transmits" contradicts `robot-cli` scan and set-ID.**
Where: `roark/PLAN.md` §3 bullet "Only the realtime process transmits" and the crate table row `robot-cli`.
Current: "Only the realtime process transmits. The one exception is the independent `disable` run by systemd after the process exits." and "`robot-cli` (bin) | Scan, set ID, independent disable, one-off checks".
Replace §3 bullet: "Only the realtime process transmits. Scan, set-ID and parameter reads are runtime commands issued through `api`; `robot-cli` is a client. The one exception is `robot-cli disable`: it opens the sockets itself, reads only (bus, id) pairs from the skeleton, sends one Disable (type 4) and one active-reporting Off (type 24) per drive, needs nothing else to work, and is what systemd runs after the runtime exits."
Why: as written an agent will build a second CAN owner, which RECONCILIATION calls out as a fault of the old stack (`RECONCILIATION.md:37,40,68`). Independent-disable shape from `marengo/bins/motor-repl/src/stop.rs:3-13`.

**M7. Enable, stop and SetZero sequences are referenced but never stated.**
Where: `roark/PLAN.md` §2 (new subsection "Sequences the codec must honour").
Current: "…Set Zero timing, enable and stop order, active reporting persisting across processes…" (pointer only).
Replace (add): "**Enable one drive:** preflight read-backs pass → type-24 Off → wait for its echo at least one tick → Enable (type 3) → write RunMode 0x7005 → expect mode = run in the first reply after the Enable's echo; a reset-mode reply after that is a fault. **Stop:** SpeedTarget 0x700A = 0, then a neutral MIT frame (kp = kd = 0, τ = 0), then Disable (type 4); at process exit also type-24 Off to every drive, because reporting persists across host processes. **SetZero (type 6):** only with the limb stopped. After a SetZero to an enabled drive the drive goes silent 511-614 ms later for 45-66 ms and drops every frame received in that window; send no Enable or type-24 write to that drive for 800 ms after the SetZero. Whether a disabled drive shows the blackout is unmeasured. The silence stop rule applies only while a limb is enabled for motion, so a referencing SetZero does not trip it."
Why: these are the bench-paid lessons; the agent has only the pointer. Sources: `marengo/review/sw-foundation-20261108/reviews/claude-opus.md:291-298`, `marengo/crates/davout/src/lib.rs:177-203`, `marengo/docs/commissioning/firmware/robstride-firmware-behavior.md:126-134,216-225,329-337`, `pr254-findings.md:27-32`.

**M8. Skeleton schema, units and the zero convention are undefined.**
Where: `roark/PLAN.md` §1 (replace the prose list with a schema block).
Current: "…every joint with its actuator type, bus, device ID, direction, zero, soft and hard limits, and torque, current and speed caps… Directions for the left joints are unknown until the sign check; leave them as placeholders."
Replace:
```yaml
name: roark
buses:
  can1: { adapter: "<USB serial or port path, udev>", bitrate: 1000000, sample_point: 0.75 }
actuator_types:            # manual values; the codec's only source of scales
  rs03: { position_rad: 12.566, velocity_rad_s: 20.0, torque_nm: 60.0, kp_max: 5000, kd_max: 100 }
limbs:
  left_arm:
    joints:
      left_shoulder_pitch:
        actuator: rs03
        bus: can1
        id: 11
        direction: null      # +1 or -1 after the sign check; null means enable is refused
        zero_rad: 0.0        # joint angle at the drive's firmware zero, in the joint frame below
        reference_pose: "<how the link is placed when SetZero is issued>"
        hard_min_rad: -0.87
        hard_max_rad: 3.14
        soft_min_rad: null   # taught; null means enable is refused
        soft_max_rad: null
        limit_torque_nm: null
        limit_cur_a: null
        limit_spd_rad_s: null
```
Followed by: "Units: rad, rad/s, N·m, A. Joint frame and positive direction follow STATUS.md: pitch raises forward, roll abducts, yaw rotates outward, elbow flexes; `direction` maps motor +q to joint +q. **Zero convention:** the drive's firmware zero (SetZero, type 6) is set once per session with the link at `reference_pose`; the host never writes 0x7029 or 0x702B and never saves parameters (type 22), so the drive's zero is treated as volatile and re-referenced after any drive power cycle until CALIBRATION_LOOP stage C5 proves otherwise. Unbuilt limbs are listed with `bus: null`, `id` from the ADR 0001 blocks, and all limits null."
Why: PACKET names this gap; TX allowlist forbids the host-offset alternative (`bus.rs:40-45`, `params.rs:39-42`); the legacy treated every startup as unreferenced (`marengo/docs/decisions/0036-physical-robstride-reference.md:28-29`). Frame convention from `marengo/hardware/STATUS.md:42`. ID blocks from `0001-can-and-motors.md:29-35`.

**M9. Gravity model source contradicts tenet 2.**
Where: `roark/TENETS.md` §2 and `roark/PLAN.md` §1 bullet "URDF and MJCF are for sim and dynamics only".
Current (TENETS): "The runtime reads that file and nothing else about the hardware."
Replace: "The runtime reads that file and nothing else about the hardware's identity, wiring and limits. Measured dynamics (link mass and centre of mass, friction, fitted by the calibration loop) live in one second committed file that tools write and the runtime reads; the URDF seeds it once and is never read at runtime."
Why: gravity feed-forward (`dynamics` crate, PLAN §6 "Sign" row) needs masses; as written the agent either reads the URDF at runtime (forbidden) or duplicates limits into a dynamics file (tenet 2 "one fact in one place"). The right-arm fits in `marengo/config/control.yaml:150-156` are the kind of data that must move.

**M10. Enable granularity: a half-built limb must still enable.**
Where: `roark/TENETS.md` §1 and `roark/PLAN.md` §6 (new host row).
Current: "A limb that isn't built yet is just a limb whose drives are all offline. It needs no separate code path."
Replace (TENETS): "A limb that isn't built yet is just a limb whose drives are all offline. Enable is per joint set: every joint named in the command must answer and pass preflight; joints not named stay offline and block nothing. There is no separate code path and no 'all joints must be online' gate."
Add to PLAN §6: "Gravity feed-forward is active only for a chain whose every joint answers; otherwise the joint set runs in torque-only or position modes without feed-forward."
Why: milestone 1 starts with three shoulder drives loose on the bench; the old enable gate refused any offline joint, which RECONCILIATION lists as blocker 3 (`RECONCILIATION.md:54-56`).

**M11. Drive and limb state machines are missing.**
Where: `roark/PLAN.md` new §6a "States".
Replace (add): "Drive: `offline` (no reply) → `reset` (replies, mode 0) → `preflighted` (all §6 drive rows read back) → `enabled` (mode run, in the tick) → `faulted` (latched: any fault bit, 3 missed replies while enabled, mode left run, over-temperature). `faulted` leaves only by an operator `clear` through `api`, which succeeds only if the drive currently reports no fault bits and passes preflight again. Limb: `idle` (all drives ≤ preflighted) → `enabled` → `stopping` (stop sequence sent) → `faulted` (any drive faulted). Robot: bus-off or lost bus faults every limb. Every transition is logged with cause."
Why: PACKET lists it; "stop on any fault and never auto-clear it" has no clearing path otherwise.

**M12. "Start here" skips the safety layer and logging before the first real drive.**
Where: `roark/PLAN.md` "Start here", steps 4-6.
Current: "4. Build the new CAN layer and its simulated drive together (items 3 and 4), on `vcan` first. 5. Buy one USB-CAN adapter, confirm it, and bring the left drives up on it loose on the bench (item 5). 6. Then the actuator test stand tooling (item 7), and the camera loop (item 9)."
Replace: "4. Codec with manual test vectors, then the sim drive and the runtime loop on `vcan1`, with MCAP and `candump -L` recording from the first run (items 3, 4, Observability). 5. The drive preflight, the host stop rules and the independent disable (item 6), each proven by a `vcan` fault-injection test before any real drive is powered. 6. Buy one USB-CAN adapter, udev-name it, scan, set IDs, soak, measure latency and jitter (item 5). No enable yet. 7. First enable on the bench: preflight, sign check by torque-only pulse with the link supported by hand, unloaded jog (bench-harness §6). 8. Then the test stand tooling (item 7) and the camera loop (item 9)."
Why: the current list never names item 6, and tenet 6 says every tuned number comes from a logged test, so logging must precede the first measurement. Bench-harness steps at `marengo/hardware/electrical/wiring/left-shoulder-bench-harness.md:106-116`.

**M13. "Done" is undefined for milestones and items.**
Where: `roark/PLAN.md` Milestones table and items 1-10 (add one "Done when" line each).
Replace (add):
- Item 1: loader rejects a skeleton with a duplicate (bus, id), an overlapping soft/hard pair, or a joint with `direction: null` that is asked to enable; the right arm's five joints and the left arm's five load from one file.
- Item 2: every fact in §2 and M7 is a codec test vector or a comment citing its manual section or bench capture.
- Item 3: 10 minutes on `vcan1` with five sim drives at 250 Hz, zero missed ticks, every frame in MCAP with a kernel timestamp.
- Item 4: one injected fault per §6 row, and for each the stop frames are on the bus within one tick of the trigger.
- Item 5: bench-harness §6 step 5 passes (10-minute idle soak, no error frames, no RX overflow, error counters 0), plus the 250 Hz latency and jitter numbers recorded in the repo.
- Item 6: preflight refuses an enable for each of: 0x7028 ≠ 1000, a cap mismatch, damper ≠ 0, VBUS outside the window, any fault bit; verified on `vcan` and once on a real drive.
- Item 7: one fitted-parameter file for RS03 from T1-T6 with its raw MCAP logs.
- Item 8: the five left joints' caps and soft limits are in the skeleton with the source of each number.
- Item 9: C0-C2 attended pass, then one C3 + C4 unattended run with section 9 stop rules.
- Item 10: not before milestone 4.
- Milestone 1: items 1-6, 8, 9 done; one unattended C8 soak of at least the duty cycle in CALIBRATION_LOOP §9 ends with no fault.
- Milestone 2: the right arm enables from the same skeleton on its own bus, its taught limits and friction fits moved, and the legacy tree deleted.
Why: PACKET Q2; without these an agent cannot know when to stop.

**M14. Deletion trigger disagrees between TENETS and PLAN.**
Where: `roark/TENETS.md` intro, sentence 3.
Current: "…once the left arm passes its tests, the right-arm-era code that wasn't carried over is deleted."
Replace: "…once milestone 2 is done (left arm passing, right arm moved onto the new stack) the legacy code in the marengo repo is deleted."
Why: deleting after milestone 1 strands the right arm (`RECONCILIATION.md:93`); PLAN milestone 2 and PACKET intent say after milestone 2. PLAN wins.

**M15. "Built beside the existing code" contradicts "new repo".**
Where: `roark/PLAN.md` "The approach" paragraph.
Current: "a fresh, lean stack that follows the software tenets, built beside the existing code."
Replace: "a fresh, lean stack that follows the software tenets, in this repo; the legacy code stays in the marengo repo, read-only, until milestone 2."
Why: `roark/CLAUDE.md:8-12` and PACKET say new repo; "beside" reads as same workspace.

## Should-fix

**S1. Tenet 3 "holds even if the Pi crashes" is misread as "holds position".**
Where: `roark/TENETS.md` §3 item 2. Current: "**Drives** (holds even if the Pi crashes)". Replace: "**Drives** (still acts when the Pi is dead: the drive drops to reset and goes limp, it does not hold position, so the arm falls onto the pad)". Why: manual-derived behaviour at `can-stack-for-marengo.md:59`; CALIBRATION_LOOP §5 "Catch pad" assumes the fall.

**S2. Legacy CAN link settings must not be copied.**
Where: `roark/PLAN.md` §2, add a bullet. Replace (add): "Do not carry over `scripts/can-up.sh` values: `restart-ms 100` and `txqueuelen 1000` were MCP2515 workarounds and contradict §3 (no automatic bus-off restart, short TX queue)." Why: `marengo/scripts/can-up.sh:10-12,25-26` would be "mined" as fact.

**S3. Silence rule needs its scope.**
Where: `roark/PLAN.md` §6 row "Silence". Current: "Stop the limb after 3 missed replies (well inside 50 ms); never hold drives energised after a drive goes silent". Replace: "While a limb is enabled: stop it after 3 consecutive missed replies from any of its drives (12 ms at 250 Hz, inside the 50 ms drive timeout). Never keep other drives energised after one goes silent. A drive silent while disabled is `offline`, not a fault." Why: post-Disable silences of up to 48.7 ms exist (`robstride-firmware-behavior.md:395-406`).

**S4. Temperature row has no resume threshold.**
Where: `roark/PLAN.md` §6 row "Temperature". Current: "Pause at 60 °C, stop at 75 °C". Replace: "Pause at 60 °C (hold at rest pose, torque off if the pose allows) until 45 °C; stop at 75 °C. Drive protection is 135-145 °C, far above." Why: `CALIBRATION_LOOP.md:154-155`.

**S5. Tick rate is per bus, because the test stand needs 1 kHz.**
Where: `roark/PLAN.md` §3 first bullet. Current: "**250 Hz default**; 300 Hz only if measured." Replace: "**250 Hz default** per bus, a runtime argument not a skeleton fact; 300 Hz only if measured. The test stand (item 7) runs one drive on one bus at 1 kHz with the same binary; whether a 1 ms period fits the collect window is measured first." Why: item 7 says "about 1 kHz" (`sim-to-real-loop.md:76`), §3 says 250 Hz; nothing says both are the same binary.

**S6. Sim drive: MuJoCo is over-specified for milestone 1.**
Where: `roark/PLAN.md` §4. Current: "…speaks the RobStride protocol through the same codec, driven by MuJoCo, with fault injection built in…". Replace: "…speaks the RobStride protocol through the same codec, with a built-in first-order joint model (inertia, friction, gravity load from the dynamics file) and fault injection (silent drive, late replies, fault bits, wrong mode, VBUS out of range, reboot, dropped and delayed frames, `canTimeout` behaviour). It runs with no physics engine so CI can run it. A MuJoCo-coupled mode is added when item 7's fitted models exist and a test needs it." Why: the research report's sim section names no engine (`can-stack-for-marengo.md:71`); the legacy MuJoCo path is Python in Docker (`marengo/sim/README.md:21`), and tenet 4 defers it.

**S7. Observability section decides and defers the same thing.**
Where: `roark/PLAN.md` "Observability". Current: "…health metrics … go to Prometheus/Grafana. … A wider observability stack choice (Grafana, Sentry, OpenTelemetry, Foxglove, Rerun) is still to be researched." Replace: "Decided now, needed by item 5: MCAP from the runtime (raw per-bus channel `{bus, id, dlc, data, t_kernel, t_mono, dir}`, decoded joint channel, injected-fault channel) and `candump -L` per bus as a systemd service. Sessions live in one directory per run, `sessions/<UTC start>/`, holding `bus-<name>.mcap`, `candump-<name>.log`, and `session.json` (skeleton hash, binary version, operator, stage). Deferred until item 7's soak needs dashboards: the metrics exporter and viewer; default Prometheus + Grafana with node_exporter for host metrics. Sentry, OpenTelemetry, Foxglove and Rerun are evaluated only against a need." Why: tenet 4; channel schema from `can-stack-for-marengo.md:69`; node_exporter from `RECONCILIATION.md:32`.

**S8. Item 10 contradicts tenet 4.**
Where: `roark/PLAN.md` §10 last sentence. Current: "Can be built against the sim now." Replace: "Not built before milestone 4. When it is, the sim drive gains a power-board model first." Why: tenet 4.

**S9. Who edits the skeleton.**
Where: `roark/TENETS.md` §2 first bullet. Current: "When the hardware changes, the hardware side edits the skeleton, in one commit, and says so in STATUS.md." Replace: "When the hardware changes, the hardware side opens one PR in this repo that touches only `config/skeleton.yaml`, and updates STATUS.md the same day. The software side edits the skeleton only when STATUS.md asks for it." Why: `marengo/CLAUDE.md` says software edits "skeleton values the hardware side asks for"; the three documents disagree on the actor.

**S10. Repo layout, toolchain, CI and deploy are unstated.**
Where: `roark/PLAN.md` new section "Repo, toolchain, CI, deploy". Replace (add): "Cargo workspace, `rust-toolchain.toml` pinned (legacy pins 1.88.0; pin the current stable at repo creation). Target: Pi 5, 64-bit Pi OS, kernel 6.1 or newer for gs_usb hardware timestamps. CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` on a host with `vcan0`-`vcan4` (CAP_NET_ADMIN), running the item 4 fault tests; no hardware in CI. Deploy: cross-build for aarch64, rsync the binaries and the systemd units (`runtime`, `can@<bus>`, `candump@<bus>`, disable in `ExecStopPost`) to `/opt/robot`; no self-update machinery (RECONCILIATION deletes `marengo-deploy`). Branches and PRs for every change." Why: PACKET Q2; legacy pin at `marengo/rust-toolchain.toml:2`; CI vcan setup exists as `marengo/scripts/ci-vcan-host-setup.sh`; kernel version from `can-stack-for-marengo.md:38`.

**S11. `api` has no interface definition.**
Where: `roark/PLAN.md` crate table row `api`, plus new §3a. Replace (add): "`api`: one Unix-domain socket on the Pi, one schema file in the crate. Messages: `snapshot` broadcast every tick (per joint: q, dq, τ, temperature, mode, fault bits, age; per bus: state, error counters; robot state); requests `enable(joints)`, `disable(all)`, `stop`, `clear(drive)`, `run_stage(plan file)`, `abort_stage`, `read_param(bus, id, index)`, `scan(bus)`, `set_id(bus, old, new)`, `read_session`. Who commands motion: only a stage plan executed inside the runtime (CALIBRATION_LOOP §10.3) or an operator jog through `api`; agent tools start and stop stages and read, they never stream targets. One client may hold the motion lease at a time. Encoding is the builder's call (default: length-prefixed JSON)." Why: PACKET Q2; "Claude does not stream commands live" at `CALIBRATION_LOOP.md:180`.

**S12. Right-arm migration steps.**
Where: `roark/PLAN.md` "Before the old stack is deleted". Replace (add): "Migration: add `right_arm` on `can0`, IDs 1-5, directions from `config/motors.yaml` (pitch −1, roll +1, yaw −1, elbow −1, lower-arm yaw −1), hard limits from its `bench` ranges, friction fits from `control.yaml`; buy the second adapter and move `can0` off the HAT with a udev rule; rewrite 0x7028 from 600 to 1000; run the sign check and C1 again before trusting the moved directions." Why: `marengo/config/motors.yaml:16,30,44,60,75,19-22`; `0001-can-and-motors.md:67`.

**S13. "Every stop rule is proven here first" needs a pass condition.**
Where: `roark/PLAN.md` §4 last sentence. Replace: "Every §6 row has one `vcan` test that injects its trigger and asserts the stop sequence is on the bus within one tick and the drive state is `faulted`." Why: testable "done".

**S14. Vague tenet phrases.**
- `TENETS.md` §4: "Code exists only if a test we're actually running needs it" → "Code exists only if a bench or sim test on the current milestone's checklist needs it."
- `TENETS.md` §5: "every frame in and out can be logged" → "every frame in and out is logged to MCAP by default; logging cannot be turned off while any drive is enabled."
- `TENETS.md` §6: "every session is logged" → "a session is one runtime start to stop, written as one directory (PLAN Observability)."
- `TENETS.md` §7: "One short architecture map" → "`ARCHITECTURE.md` at the repo root: one table of crate, purpose, must-never, talks-to; changed in the same PR as any seam move."
- `TENETS.md` §8: "on one shared clock (Pi and Jetson synced)" → "on the Pi's CLOCK_REALTIME with CLOCK_MONOTONIC alongside; the Jetson joins via chrony when it exists."
Why: each names a goal without a check.

**S15. Milestones sentence is false for milestone 4.**
Where: `roark/PLAN.md` Milestones intro. Current: "Each milestone is 'these drives come online and pass their tests'. None of them adds a code path of its own." Replace: "Milestones 1, 2, 3 and 5 are 'these drives come online and pass their tests' and add no code path. Milestone 4 adds the power-board interface (item 10)." Why: item 10 is I²C, GPIO and RS485 code.

## Nice-to-have, facts checked, owner questions

**N1.** `PLAN.md` §3: "`socketcan` 4.0 (blocking socket, netlink, candump-log)" → "(blocking socket, `netlink` and `dump` features)". Research names the features (`can-stack-for-marengo.md:12`); crate feature names not verified here.

**N2.** `PLAN.md` §3: drop `rtrb`, `thread-priority`, "pinned", "60 %" to a "suggested, measure first" sentence; keep "no async runtime in the loop" and "one blocking thread per bus". Tenet 4 and the report's own "measure first" (`:11`).

**N3.** `PLAN.md` crate table: move `ui/` to "when a test needs an operator screen"; keep `mcp/` for item 9 only. Tenet 4.

**N4.** `PLAN.md` §2: note that `RECV_OWN_MSGS` in the legacy socket options existed for the echo ledger across several local sockets (`bus.rs:78-83`); with one writer per bus keep it only for TX timestamps in the log.

**N5.** `TENETS.md` §1 says every actuator is RobStride; the hardware plan has STS3215 bus servos for the neck, parked (`STATUS.md:23`, `REV_B_PARTS.md:38`). Add "the parked head and neck are outside the skeleton until they exist."

**N6.** `PLAN.md` §1: add "STATUS.md wins over `hardware/docs/kinematics.md` for the left arm (its rows still say `left_elbow`/`left_wrist`, lines 144-145)."

**N7.** `TENETS.md` intro paragraph is plan history, not a rule; move it to PLAN.

**N8. Facts checked against the reference repo.** Verified: left IDs 11-15 on `can1` and names (`STATUS.md:33-40`); right arm `can0` 1-5 (`motors.yaml`); old IDs 12/`can1` and 11-14/`can0` (`0001-can-and-motors.md:47`); RS00 ±50/±17 vs manual ±33/±14 and RS03 ±20 (`motor_type.rs:21-53`, `robstride-mit-ranges.md:13-15`); parameter indices 0x700B/0x7017/0x7018/0x7028/0x702A/0x701C/0x701A (`CALIBRATION_LOOP.md:142-145`, `sim-to-real-loop.md:50`, `params.rs:28-34`); 60/75 °C and 22-26 V (`CALIBRATION_LOOP.md:154-156`); 3 missed replies (`can-stack-for-marengo.md:67`); stall caps RS03 13, RS02 6 (`sim-to-real-loop.md:87`, `actuator-test-stand/README.md:14,109`); elbow 17 N·m, 0-124°, 40 N (`ELBOW_DUMMY_OUTPUT_TEST.md:3,24,141`); 3 A then 10 A, 60 Ω, 120 Ω each end (`left-shoulder-bench-harness.md:57,63-65,111`); adapter class and price (`can-stack-for-marengo.md:3,38-40`); heartbeat 50-100 ms, INA237 addresses, 2 mΩ, 30 W/60 s/3 kJ (`REV_B_PARTS.md:113-125`); 6808 "arriving Monday" (`BUY_ORDER.md:16`, no date given). Manual-derived only, not read from the PDFs: 0x7028 units and reset behaviour, damper semantics, fault-bit layout, MIT scale tables. Unsupported: "0x2026 on RS03" (see M1).

**N9. Questions only the owner can answer, with defaults.** Proposed as a new PLAN section "Open with Joey (defaults apply until answered)":
1. Second committed file for fitted dynamics (M9)? Default: yes, `config/dynamics.yaml`, tools write it.
2. Does the firmware zero survive a drive power cycle, and what is each left joint's reference pose? Default: treat as volatile; reference poses documented in the skeleton; C5 decides.
3. Does the HAT stay fitted during milestone 1? Default: yes until milestone 2, with the USB adapter udev-named `can1` and the HAT's `can1` disabled.
4. Bus names and ID blocks for waist and legs (hardware ADR 0001 says the full-body plan is open)? Default: ADR 0001 ID blocks, `bus: null` until the hardware ADR lands.
5. Camera service host? Default: the bench Pi 5, per `CALIBRATION_LOOP.md:68,220`.
6. Operator UI at milestone 1? Default: none; CLI and MCP only.
7. Install root and service names, given no robot name in them? Default: `/opt/robot`, units `runtime`, `can@`, `candump@`.
8. VBUS window on the 48 V pack later? Default: `power: {vbus_min_v, vbus_max_v}` in the skeleton, bench values 22-26.
9. Kernel: stock 64-bit Pi OS first, PREEMPT_RT only on a failed measurement (already in PLAN; confirm the Pi is on kernel ≥ 6.1).
10. Metrics stack timing (S7)? Default: defer to item 7.