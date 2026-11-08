# Review: `roark/TENETS.md`, `roark/PLAN.md` (+ `CLAUDE.md`, `README.md`)

**What I read:** all four `roark/` files in full; `marengo/hardware/STATUS.md`, `docs/kinematics.md`, `docs/safety.md`, `docs/sim-to-real-loop.md`, `docs/compute-plan.md`, `docs/research/can-stack-for-marengo.md`, `docs/decisions/0001`, `0002`, `0006`, `docs/commissioning/firmware/robstride-firmware-behavior.md`, `designs/left-arm/testing/CALIBRATION_LOOP.md`, `ELBOW_DUMMY_OUTPUT_TEST.md`, `electrical/pdb/REV_B_PARTS.md`, `electrical/wiring/left-shoulder-bench-harness.md`, `left-arm-trunk/README.md`, `review/sw-foundation-20261108/RECONCILIATION.md`, `config/motors_humanoid.yaml`, `config/control.yaml` (grep), and the relevant `crates/robstride`, `crates/davout`, `bins/motor-repl` sources.

**What I did not verify:** I did not open the four vendor PDFs (`marengo/hardware/docs/robstride/*.pdf`); every manual-derived number below is cited to the repo document that transcribes it. I did not read `reviews/` (other reviewers' output) and I ran nothing.

Format per item: **(a) where** · **(b) current text** · **(c) replacement** · **(d) why** · rank.

---

## Q1. Vagueness

**V1 · must-fix** · (a) `roark/PLAN.md` §6, table row "Drive | `canTimeout` 0x7028"
(b) "Write 1000 (= 50 ms; 20000 = 1 s) and read back before every enable; refuse on 0 or above."
(c)
> Write `0x7028 = 1000` (50 ms; 20000 = 1 s) **only if the readback is 0 or greater than 1000**, then read it back. Accept a readback of 1…1000 and continue; refuse to enable on 0 (timeout disabled) or on > 1000. Log the value written. A drive that already reads ≤ 1000 is not rewritten (the right arm's drives read 600 ≈ 30 ms today), so this rule never loosens a drive that is already tighter.
(d) "refuse on 0 or above" can be read as *refuse when the readback is at or above the limit*, i.e. refuse the correct value 1000; `STATUS.md:67` has the same wording ("0 or above the limit"). Worst case is either every drive refused or a 0-timeout drive accepted. (`STATUS.md:67`, `docs/decisions/0001-can-and-motors.md:67`, `docs/commissioning/firmware/robstride-firmware-behavior.md`.)

**V2 · must-fix** · (a) `PLAN.md` "Milestones", line under the table
(b) "Each milestone is 'these drives come online and pass their tests'. None of them adds a code path of its own."
(c)
> Each milestone passes only when all of the following hold, recorded in one `docs/milestones.md` entry per milestone: (1) every drive in the milestone's **enabled set** answers the preflight and reaches Run; (2) a 10-minute idle soak shows zero error frames, zero `rx_over_errors`, and TX/RX error counters at 0; (3) measured round-trip reply latency and per-tick jitter are inside the budget set in §5, printed from a logged run; (4) every host stop rule in §6 has a fault-injection test on `vcan` that fails on the unpatched code and passes after; (5) `git diff` contains no per-limb branch (`grep` for the limb name finds only skeleton and doc rows). Drives of limbs outside the enabled set stay Offline and never block (1).
(d) "these drives come online and pass their tests" names no test, so "done" is the builder's opinion; item (5) is the only checkable form of "no code path of its own".

**V3 · must-fix** · (a) `PLAN.md` §6, row "Host | Hang"
(b) "systemd `WatchdogSec` fed from the control thread; later the power board heartbeat (item 10)"
(c)
> The realtime thread increments an atomic tick counter at the end of each tick and does nothing else for the watchdog. A non-realtime thread writes `WATCHDOG=1` to systemd only when the counter advanced since its last write, so a blocked control thread starves the watchdog and a blocked socket write can never stall motion. `WatchdogSec` is set from the §5 latency measurement (2 × worst measured tick interval + 1 period) and the unit file ships in the repo. The power-board `WD_FEED` toggle (item 10) comes from the same non-realtime thread at 50–100 ms, and a falling edge at least every 270 ms.
(d) "fed from the control thread" makes the SCHED_FIFO, `mlockall` thread perform a Unix-socket write whose latency is unbounded, which contradicts tenet 5 ("everything else … can crash without affecting motion"); the 270 ms/50–100 ms numbers are in `REV_B_PARTS.md:113`.

**V4 · should-fix** · (a) `TENETS.md` §3
(b) "Every safety rule names the hardware it protects and the failure it covers. A rule that duplicates another layer, or protects nothing nameable, goes."
(c)
> Every safety rule states (i) the hardware it protects, (ii) the failure it covers, and (iii) the layer that already covers it. A rule that cannot name a failure in its own layer's failure list, or that repeats another layer's coverage, does not ship, and removing it is a reviewable change that cites this tenet.
(d) "nameable" is satisfied by any clever sentence; the test has to be "which listed failure", and the layer's failure list is already written above it.

**V5 · should-fix** · (a) `TENETS.md` §4
(b) "Code exists only if a test we're actually running needs it."
(c)
> Code exists only if a **milestone item** needs it, and the item names the acceptance check (§ Milestones) that fails without it. A library with no such consumer is deleted, not kept "ready".
(d) As written the rule is circular (a test needs the code, the code needs the test) and does not tell the builder what a "test we're running" is.

**V6 · should-fix** · (a) `TENETS.md` §7
(b) "An agent (or a person) new to the repo should know within minutes what every layer does…"
(c)
> `ARCHITECTURE.md` is at most one page: every crate and binary, what it owns, what it must never do, and who it talks to. Every crate's `//!` opens with those three lines, and `ARCHITECTURE.md` is updated in the same PR as any change that moves a seam. A PR that adds a crate or changes a dependency edge without touching it fails review.
(d) "within minutes" is not gradeable; a one-page file plus a crate-doc rule is.

**V7 · should-fix** · (a) `TENETS.md` §8
(b) "Detailed, concise, timely logs at every step…"
(c)
> Every record carries `t_mono_ns`, `t_wall_ns` and a session id. The runtime exports, at ≥ 1 Hz, its drop counters (frames not sent, replies missed, log records dropped, ticks over budget), and every motion command, drive reply, fault, limit hit, state change and operator action is a record.
(d) "timely" and "detailed" have no threshold; the drop counters are the only way a reviewer sees a silent loss, and they are what the sim-to-real loop consumes (`hardware/docs/sim-to-real-loop.md` §4).

**V8 · should-fix** · (a) `PLAN.md` §6, row "Host | Silence"
(b) "Stop the limb after 3 missed replies (well inside 50 ms); never hold drives energised after a drive goes silent"
(c)
> Count a reply missing when no reply for that address arrives by the end of its tick's receive window **and** that joint's last reply is older than 3 periods (12 ms at 250 Hz). Stop and latch at 12 ms, inside the drive's 50 ms timeout. A reply that arrives late but before the next period refreshes the pose and does not count as a miss.
(d) "3 missed replies" measured per tick conflicts with the §3 receive window (60 % of a 4 ms period = 2.4 ms) against a measured worst-case MIT reply of 4.96 ms and p95 of 0.25 ms (`robstride-firmware-behavior.md`, "Type-2 reply to every host frame"); as written a drive that consistently answers at ~3 ms is stopped every 3 ticks.

**V9 · nice-to-have** · (a) `PLAN.md` §6, row "Host | Sign"
(b) "…verified by a small torque-only pulse checked against the skeleton"
(c)
> …verified by a torque-only pulse whose magnitude and duration are per-joint skeleton fields (`sign_test_nm`, `sign_test_ms`), logged with the measured position change. The joint's `direction` flips only if the measured motion matches the sign of the applied torque; the result is written to the skeleton with `direction_verified_at`.
(d) "small" is the one number in the safety table with no source and no record; making it a logged per-joint value makes the test repeatable and leaves the value to the owner rather than the doc.

---

## Q2. Gaps

**G1 · must-fix** · (a) `PLAN.md` §1
(b) "One file, `config/skeleton.yaml`, describes the whole robot … every joint with its actuator type, bus, device ID, direction, zero, soft and hard limits, and torque, current and speed caps."
(c) Replace with a schema. Units in brackets are mandatory; the loader fails closed on unknown keys, non-finite numbers, `soft` outside `hard`, duplicate `(bus, id)`, or an id outside its limb's block.

```yaml
schema: 1
name: roark
control_period_us: 4000             # 250 Hz
buses:
  - limb: left_arm
    interface: can1                 # Linux name; sim maps it via `--interface-map`
    bitrate: 1000000
limbs:
  - limb: left_arm                  # left_arm | right_arm | left_leg | right_leg | waist
    bus: left_arm                   # inherited by every joint in the limb (tenet 5)
    joints:
      - joint: left_shoulder_pitch
        model: rs03                 # rs00 | rs02 | rs03 | rs04
        id: 11
        direction: unknown          # +1 | -1 | unknown
        zero_rad: 0.0               # q = direction * motor_pos + zero_rad
        limits: {hard: [-0.87, 3.14], soft_supervised: [-0.835, 3.105], soft_unattended: [-0.783, 3.053]}
        caps:   {torque_nm: 13.0, current_a: 6.0, speed_rad_s: 1.0}
        vbus:   {min_v: 22.0, max_v: 26.0}      # bench instance; robot instance 36–58 V
        gravity_ff: blocked         # allowed only after direction != unknown
```

  Follow it with: hard limits are joint-space radians and inclusive; soft limits are inside hard by construction; `caps.torque_nm` is never above the weakest printed part on that joint; every unit is rad, N·m, A, rad/s, V, µs. All 23 joints are present from day one: waist 1, each leg 6, each arm 5 (`hardware/docs/kinematics.md:28`).
(d) Every one of those fields is load-bearing (the codec needs `model`+`id`, the safety layer needs `caps`, the calibration loop writes `zero_rad`+`direction`), and an agent cannot invent units or a null encoding.

**G2 · must-fix** · (a) `PLAN.md` §1, paragraph under the item
(b) "Left arm: `can1`, IDs 11 to 15 … Right arm stays `can0`, IDs 1 to 5. Unbuilt limbs are listed and simply show offline."
(c)
> The whole bus/ID map is fixed in the skeleton on day one, before any unbuilt limb exists. Allocation follows `hardware/docs/decisions/0001-can-and-motors.md` §Device IDs: per-limb blocks, so a bus can be reassigned without touching an ID — left arm 11–20 (`can1`, ids 11–15 in use, 16 reserved for the hand), right arm 1–10 (`can0`, ids 1–5), right leg 21–30, left leg 31–40, waist 41+. Provisional bus map: right arm `can0`, left arm `can1`, waist `can2`, right leg `can3`, left leg `can4`. The software project owns this map only until the hardware side confirms it; the hardware project's outstanding bus-plan ADR is the confirmation. `config/motors_humanoid.yaml` (left arm `can3` 14–18, right arm `can4` 19–23) is a stale template and is **not** a source: it collides with the leg blocks and with the live arm.
(d) Tenet 2 requires one skeleton for the whole robot, but only 10 of 23 `(bus, id)` pairs are specified anywhere; leaving the rest to the builder guarantees a second, invented map, and the template in `config/motors_humanoid.yaml:192-340` actively collides with the real left arm (`can1` 11/12 vs right-leg template ids 11/12).

**G3 · must-fix** · (a) `PLAN.md` §1 ("zero") and §2 ("Set Zero timing")
(b) "direction, zero" listed as fields; "the TX allowlist, Set Zero timing" listed as knowledge to lift.
(c)
> **Zero convention.** Each drive's own zero is set by the firmware `SetZero` (type 6) with the joint physically at its reference pose; the drive then reports `0` there. The skeleton's `zero_rad` is the *residual* offset between the firmware zero and the model zero, added after `direction`: `q = direction * motor_pos + zero_rad`, and commands invert it. Only the zero procedure writes `zero_rad`, and it writes it with the timestamp and operator. The runtime never writes `ZeroSta` (0x7029), `AddOffset` (0x702B) or a type-22 save. After any `SetZero`, no Enable and no type-24 On/Off goes to that drive for **800 ms** (the drive is silent 44–66 ms starting 511–614 ms after the SetZero and ignores frames in that window); type-24 writes are held from 450 ms. SetZero is only ever sent with the drive Enabled and the joint at the reference.
(d) `zero` is listed as a skeleton field with no definition, and the three concepts are separate in the reference stack (home reference, semantic zero, home offset — `docs/decisions/0006-homing-zero-reference.md:12-24`); the 800/450 ms numbers are measured, not tuning (`robstride-firmware-behavior.md`, "Post-SetZero blackout"; `docs/safety.md:380-408`).

**G4 · must-fix** · (a) `PLAN.md` §3 and the crate table row `api`
(b) "`api` | The one interface to the realtime process" and "Only the realtime process transmits."
(c)
> `api` is the only ingress to the runtime and has exactly five message classes: **Observe** (state snapshot), **Stop** (always accepted from any client), **Set owner** (claims the motion lease), **Motion** (only from the current owner), **Config** (limits changes; refused while Active). Exactly one motion owner exists at a time and ownership ends when that client disconnects or the process exits; a second client can always stop and observe and can never move. A calibration stage plan is uploaded whole and executed **inside** the runtime — clients never stream per-tick commands — so a dropped network link cannot leave a limb mid-move.
(d) PLAN never says who may command motion, and the camera loop's own spec requires in-runtime execution ("Claude does not stream commands live; the plan runs on the host", `CALIBRATION_LOOP.md:180`); the legacy stack needed a whole lease mechanism for this and its absence is a live safety hole.

**G5 · must-fix** · (a) `PLAN.md` §6 (new subsection after the table)
(b) No state machine, no enable/stop sequence anywhere in either document.
(c)
> **Drive state:** `Offline` (no reply) → `Present` (any reply, mode Reset) → `Checked` (identity matches `model`; all five settings written and read back equal; damper 0; VBUS in window) → `Enabled` (mode Run) → `Fault`. Any non-zero fault bit, any mode other than Run while Enabled, any Reset report, or silence past V8 moves the drive to `Fault` and latches. `Fault` clears only by process restart or an explicit operator `reset`; nothing auto-clears.
> **Limb state:** `Offline` (every drive Offline) / `Partial` (some drives present) / `Ready` (every drive in the enabled set is Checked) / `Active` / `Stopped` (latched). Disabled limbs are normal: a limb whose drives are all Offline is not a fault and does not block enabling another limb (tenet 1).
> **Enable:** validate skeleton → preflight every drive in the enabled set (identity; write+read back `0x7028`, `0x700B`, `0x7018`, `0x7017`, damper; VBUS) → verify direction/zero for every joint that will carry feed-forward → type-24 Off to every drive and wait for its echo → Enable (type 3) + RunMode (0x7005 = 0), one drive per bus per tick, never within 800 ms of that drive's SetZero → wait for mode Run on every drive within the enable deadline, else stop all.
> **Controlled stop:** speed target 0 → neutral MIT → Disable (type 4) → type-24 Off. **Hard stop** (any fault, silence, bus-off, E-stop, shutdown): Disable + type-24 Off immediately, no neutral MIT, latch, no auto-clear.
(d) These sequences are measured behaviour, not preference: Enable must follow a reported type-24 Off (`docs/safety.md:323-345`), MIT returns one type-2 per frame so Enable is staggered one target per bus per tick (`robstride-firmware-behavior.md`, "Type-2 reply to every host frame"), Disable does not stop type-24 reporting so Off always follows (`bins/motor-repl/src/stop.rs:6-13`), and the stop sequence is named in `RECONCILIATION.md:83`.

**G6 · must-fix** · (a) `PLAN.md` §3, bullet "Codec"
(b) "test vectors from the manuals, cross-checked against the `motorcom` crate. Stay in the drive's default private protocol with active reporting off."
(c)
> Add: the codec is one `FrameSpec` row per manual table with its manual section cited; the cross-check against `motorcom` is a one-off reference, not a dependency. Command types the robot may send: 0 (identity), 1 (MIT), 3 (Enable), 4 (Disable), 6 (SetZero), 17 (read parameter), 18 (write parameter), 24 (active reporting). Never send: 7 (set CAN id) except from the set-ID tool with no runtime running, 22 (save), 23 (baud), 25 (protocol switch), 0x7029/0x702B writes. Fault bits: decode defensively — the vendor table lists bit 20 twice, so log unknown bits raw and do not act on the duplicated name.
(d) The allowlist exists in the reference stack (`hardware/docs/decisions/0002-robstride-protocol.md:55-60`) and the duplicated bit-20 row is a documented vendor error (`docs/research/can-stack-for-marengo.md:61`); an agent writing a fresh codec will otherwise pick up the Seeed SDK's base-ID scheme or a save command.

**G7 · must-fix** · (a) `PLAN.md` §6, "Drive" rows and the V7/`§5` "Re-check after any drive power cycle"
(b) "`limit_torque` 0x700B … Per joint from the skeleton" / "Re-check after any drive power cycle or reset report"
(c)
> Every drive setting (timeout, torque, current, speed, damper) is **written then read back** at every enable and after every reset-mode report — never read-only — because whether a write survives a power cycle is unverified and the save command (type 22) is on the never-send list. A drive that returns after a reset goes to `Present` and must pass the whole preflight again before Enable. Add the power-board's aggregate rule: the drive current and power limits are set so that everything that can move at once sums to ≤ 90 A, read back at startup like the timeout.
(d) The settings are volatile in the reference stack (which never wrote them at all, `RECONCILIATION.md:49`; type-22 save refused, `0002-robstride-protocol.md:58`), and the 90 A sum is a hard power-board requirement (`REV_B_PARTS.md:126`, `STATUS.md:69`) that PLAN omits — the packet explicitly asks about a drive power cycle, and this is the case that bites.

**G8 · must-fix** · (a) `PLAN.md` §3, bullet "**Linux:**"
(b) "bitrate and sample point set explicitly, error frames on, no automatic bus-off restart while moving, short TX queue."
(c)
> Add: the skeleton names a logical bus and the Linux interface separately (`buses[].interface`), because `gs_usb` adapters take the next free `canN` in enumeration order and today's `can1` is the MCP2515 HAT. A udev rule binds each adapter by USB port path to a fixed name (`can_arm_l` …) and the skeleton uses those names; a missing interface puts that limb Offline, not the process (tenet 1); a bus that was up and went down is a `Fault`. Five adapters need a powered hub on the Pi 5's two USB controllers (`hardware/docs/compute-plan.md:35`).
(d) "bus names come from the skeleton" (tenet 5) is not implementable while the skeleton has no interface field, and with five adapters the name/canN pairing is not stable across a re-plug; a lost bus at startup versus mid-run needs different answers.

**G9 · must-fix** · (a) `PLAN.md` §4
(b) "`vcan0` to `vcan4` plus one sim-drive process that speaks the RobStride protocol through the same codec, driven by MuJoCo, with fault injection built in (silent drive, fault bits, VBUS out of range, reboot, dropped and delayed frames)."
(c)
> One `sim-drive` process answers on `vcan0…vcan4` through the same codec, in two modes: **bench mode** (no physics — a per-joint 1-DoF integrator with the joint's skeleton inertia and friction) and **model mode** (driven by the MJCF from `dynamics`). It must reproduce, as the test oracle requires: one type-2 reply per host frame of types 1/3/4/6/18/24; own-frame echo on TX; the post-SetZero blackout (start 500–625 ms, length 40–67 ms, default 535/55); a 2-frame RX FIFO; mode and fault bits (including an unknown bit); and the injected faults listed above. Bus-off and error-frame paths cannot be faked on `vcan` — exercise those with `canerrsim` or on the real bench.
(d) PLAN names the fault list but not the behaviours that break the enable sequence in practice; the reference emulator's contract is the authority ("echoes, reply latency, post-SetZero blackout, a 2-buffer RX FIFO overrun", `AGENTS.md` Rust fixtures), and `vcan` has no bit timing (`docs/research/can-stack-for-marengo.md:71`).

**G10 · must-fix** · (a) `PLAN.md` §10 (and Milestone 4 "Power board and battery")
(b) "## 10. Power board interface (later) … heartbeat … port-fault reporting … INA237 … brake clamp caps … brown-out budget, E-stop input debounce."
(c) Add as item 11, **Battery and BMS**:
> BMS telemetry (JBD SP17S005, 13S, ~46.8 V nominal / 54.6 V full) arrives over an isolated USB-RS485/UART adapter and is part of the drive-enable gate: refuse Enable while the BMS reports charge current or telemetry is older than 5 s; shut compute down at min cell ≤ 3.0 V, filtered pack ≤ 38 V, or telemetry lost > 5 s; the total drive current budget is `(V_rest − 35 V) / R_pack_measured` and tapers as the pack falls. No drive load until bus voltage is above 45 V, and total drive bus capacitance is capped at 20 mF.
(d) Milestone 4 promises "power board and battery" but item 10 contains no battery row at all, and these are enable-gate rules, not telemetry (`STATUS.md:78-80`, `REV_B_PARTS.md:117,127`).

**G11 · must-fix** · (a) `PLAN.md` §6, row "Drive | VBUS 0x701C"
(b) "22 to 26 V on the bench supply; no faults present"
(c)
> VBUS window comes from the skeleton per joint (`vbus`), because there is no universal number: 22–26 V on the 24 V bench supply, 36–58 V on the pack, with the drive's own 60 V over-voltage and 12 V under-voltage faults as the backstop, and no drive load until bus voltage is above 45 V. Read `0x701C` at preflight and ≥ 1 Hz while Active.
(d) A 22–26 V constant in the runtime is exactly the "only for now" change tenet 0 forbids, and the upper bound is wrong on the robot by a factor of two.

**G12 · must-fix** · (a) `PLAN.md` §6, table (new rows)
(b) Missing: the E-stop and the lost-bus distinction.
(c)
> | Host | E-stop | Read `ESTOP_PI_N` (pressed = low, debounce, no pull, invalid until configured), log it, go to Stopped. E-stop release never re-enables: an explicit Enable is required. Drives lose power with the loop, so the limb goes limp — the arm is supported or over the pad whenever the E-stop can be pressed. |
> | Host | Lost bus | An interface that is down at startup puts its limb Offline (not a fault). An interface that goes down or bus-off while a limb is Active stops that limb and latches; on a shared bus, stop the whole robot. No automatic re-bring-up while Active; restarting the interface is an operator action. |
(d) Tenet 3's layer 1 is the E-stop, so the host rule that consumes it has to exist; and "stop the whole robot" for bus-off (already in the table) is ambiguous for the startup case, which would otherwise make the robot unbootable with three limbs unbuilt.

**G13 · should-fix** · (a) `PLAN.md` "Milestones", row 2
(b) "Right arm moved to the new stack, then the old code deleted | Two limbs on two buses from one skeleton with no per-limb code"
(c)
> Milestone 2 passes when the right arm runs the milestone-1 acceptance on `can0` through a **dedicated `gs_usb` adapter** (not the MCP2515 HAT — its 2-frame RX buffer overruns on send-all/collect-all traffic), with the taught right-arm signs and limits entered in the skeleton, and the `right_lower_arm_yaw` axis recorded as a yaw about the forearm (the URDF's Y axis is a known error). Only then is the legacy code deleted.
(d) The HAT cannot carry the new loop (2 RX frames, `docs/safety.md:309-322`; overruns measured), and the right arm has two known sign/axis traps that the migration will otherwise rediscover (`hardware/docs/kinematics.md:161`, `config/motors_humanoid.yaml:286-332`).

**G14 · should-fix** · (a) `PLAN.md` new §12 "Repo, toolchain, CI, deploy"
(b) Nothing in either document.
(c)
> Repo: `Cargo.toml` workspace; `crates/{skeleton,robstride,can,safety,control,dynamics,telemetry,api}`; `bins/{runtime,sim-drive,robot-cli}`; `ui/`, `mcp/`; `config/skeleton.yaml`; `systemd/runtime.service`; `docs/{ARCHITECTURE.md,milestones.md,adr/}`; `var/log/` (gitignored). Toolchain pinned in `rust-toolchain.toml` with `rustfmt` and `clippy` (the legacy pin is 1.88.0, `marengo/rust-toolchain.toml:2`). CI runs on Linux only and gates exactly four things: `fmt --check`, `clippy -D warnings`, `cargo test`, `vcan` integration tests (create `vcan0…vcan4` first) plus an `aarch64-unknown-linux-gnu` cross-build. Default `cargo test` needs no hardware; hardware tests are `#[ignore]` behind a feature. Deploy = `scp`/rsync the aarch64 binary + `config/` + unit file, `systemctl daemon-reload && restart`; the unit's `ExecStopPost` runs `robot-cli disable`.
(d) The layout table names crates but not the repo, the CI gate, the deploy path or the toolchain that the previous stack had (`AGENTS.md`, "Development Commands", "Pi deploy"); without it the first agent invents a build and a deploy that nobody can review.

**G15 · should-fix** · (a) `PLAN.md` "Observability (tenet 8), all items"
(b) "Every frame in and out goes to **MCAP** from the realtime process … `candump -L` runs per bus as an independent black box"
(c)
> The realtime thread copies each frame record into a bounded lock-free ring and never touches a file. One non-realtime thread writes MCAP and, on a full ring or a failed write, increments a drop counter and keeps going. Session layout: `var/log/<UTC ISO8601>/` containing `session.mcap`, `candump-<bus>.log` per bus, and `header.json` (git sha, skeleton sha256, host, kernel, rebuild count, control period). Retention is one line in the same section, not a policy document.
(d) "goes to MCAP from the realtime process" reads as a synchronous write in the loop, which is the failure the packet's "crashed process / stalled disk" case is about; and the sim-to-real loop needs a findable per-session directory, which neither document defines.

**G16 · nice-to-have** · (a) `PLAN.md` §1 last bullet
(b) "Directions for the left joints are unknown until the sign check; leave them as placeholders."
(c) "`direction: unknown` is a legal value. The loader accepts it; Enable, feed-forward and any signed command are refused for that joint until the sign check writes `+1`/`-1` and `direction_verified_at`."
(d) A placeholder with no encoding either fails the loader (blocking milestone 1) or silently becomes `+1`, which is the wrong-sign gravity failure tenet 3 exists to prevent.

---

## Q3. Contradictions

**C1 · must-fix** · (a) `PLAN.md` §1 vs `TENETS.md` §2/§6
(b) PLAN: "It replaces `motors.yaml`, `control.yaml`, `robot.yaml`, `homing.yaml` and the humanoid templates." Tenet 2: the skeleton holds "actuator type, bus, device ID, direction, zero, and soft and hard limits" — hardware facts only.
(c) PLAN: "It replaces `motors.yaml`, `robot.yaml` and `homing.yaml`. It does **not** absorb `control.yaml`: gains, slew rates and danger zones are software tuning that changes with logged tests (tenet 6), and putting them in the file the hardware side edits would put tuning in the hardware's hands. Tuning lives in `config/tuning.yaml`, keyed by joint, owned by the software project."
(d) `config/AGENTS.md` defines `control.yaml` as "kp/kd/slew/trim, bench settings, danger zones" and `motors.yaml` as "CAN ID, joint name, direction, gear_ratio, motor_type"; folding tuning into the skeleton breaks "one fact lives in one place" in the opposite direction — the hardware side would own gains.

**C2 · must-fix** · (a) `PLAN.md` §3 vs its own crate table and §5
(b) "**Only the realtime process transmits.** The one exception is the independent `disable` run by systemd after the process exits." vs `robot-cli` "Scan, set ID, independent disable" and §5 "Scan, set IDs one drive at a time".
(c) "Only one process owns a bus at a time. The set: `runtime`; `robot-cli` for scan/set-ID/disable **only while `runtime` is stopped**; and `robot-cli disable` from `ExecStopPost`, which never blocks on a running owner. Every other process, including `api` clients, the UI, the MCP tools and the camera service, is read-only and cannot open a CAN socket."
(d) As written the exception list contradicts the tool that milestone 1 depends on; the reference stack solved exactly this with an owner lock (`crates/robstride/src/bus.rs` open_owner, `bins/motor-repl/src/stop.rs:177-179`) and PLAN should keep the concept without the machinery.

**C3 · must-fix** · (a) `TENETS.md` §2 last bullet vs `PLAN.md` §1 and the `dynamics` crate row
(b) Tenet: "A URDF or MJCF for sim and visualisation may be generated … but the runtime doesn't depend on it." PLAN §1: "URDF and MJCF are for sim and dynamics only." PLAN crate table: "`dynamics` | Kinematics, gravity, calibration fits." PLAN §6 requires gravity feed-forward, gated on the sign check.
(c) Tenet 2: "…but no runtime behaviour depends on it **except gravity feed-forward**, which needs link masses and centres of mass the skeleton does not carry. The runtime reads exactly one generated inertial model (a reduced MJCF or a URDF) identified by path and sha256 in the skeleton; if it is missing or its hash mismatches, no joint may hold or carry feed-forward." Delete "for sim and dynamics only" from PLAN §1.
(d) The three sentences cannot all be true: gravity torque needs inertials, the skeleton has none, and `dynamics` is listed as runtime code. This is the one place where a builder must invent an answer with safety consequences.

**C4 · must-fix** · (a) `PLAN.md` §10 last sentence
(b) "Can be built against the sim now."
(c) Delete. Replace with: "Built at milestone 4, against the sim first and then the board."
(d) Directly contradicts tenet 4 ("Code exists only if a test we're actually running needs it") and PLAN's own milestone order, which puts the power board fourth.

**C5 · should-fix** · (a) `PLAN.md` §1 / `STATUS.md:46-50` vs `hardware/docs/kinematics.md:137-147`
(b) PLAN uses `left_elbow_pitch` / `left_lower_arm_yaw`; `kinematics.md` — which calls itself "Single source of truth for joint names" — still lists `left_elbow` / `left_wrist`, with different limits (−0.9…3.17 vs −0.87…3.14; elbow 0…2.5 vs 0…2.164; lower yaw ±1.6 vs ±1.05).
(c) In PLAN §1: "Joint names and limits for the left arm are those in `hardware/STATUS.md` (hardware names `left_elbow_pitch` / `left_lower_arm_yaw`). `hardware/docs/kinematics.md`'s left-arm table and `config/motors_humanoid.yaml` are stale for this limb and are not sources; the hardware side owns fixing them, and until it does, STATUS.md wins."
(d) Two documents claim to be the joint-name authority and disagree on every name and three limits; an agent will pick one at random.

**C6 · should-fix** · (a) `PLAN.md` §2 vs §3
(b) §2 lifts "the TX allowlist" from the legacy codec; §3 requires "one-drive-at-a-time set-ID".
(c) "Lift the allowlist **except** the set-CAN-id entry: the new `robot-cli` must send type 7, and must then power-cycle the drive (the ID is not saved by a type-22 write; save stays forbidden)."
(d) The reference allowlist refuses type 7 before the wire (`0002-robstride-protocol.md:56-58`); lifting it verbatim makes the set-ID tool PLAN requires impossible.

**C7 · should-fix** · (a) `PLAN.md` §3
(b) "**Rust:** `socketcan` 4.0 (blocking socket, netlink, candump-log)"
(c) "**Rust:** `socketcan` ≥ 3.6 for kernel RX timestamps (4.0 if it exists and builds; pin the exact version in `Cargo.lock`); features `netlink` + `dump`."
(d) The reference lockfile resolves `socketcan` **3.5.0** (`marengo/Cargo.lock:2795`, `crates/robstride/Cargo.toml:23`), and the research report says `read_frame_with_timestamp` arrived in 3.6 — so "4.0" is neither verifiable here nor the important constraint. "candump-log" is not a feature name in any source in this packet.

**C8 · should-fix** · (a) `PLAN.md` §2 vs `CLAUDE.md`
(b) PLAN: "the drive model in `davout/tests/physical_firmware/` for the simulator." CLAUDE.md: "Mine the legacy code for facts … never as patterns."
(c) "Lift the **behaviour** in `davout/tests/physical_firmware/` as a written spec (echo on TX, reply latency, post-SetZero blackout, 2-frame RX FIFO, reboot), with its measured ranges, and write new tests against it. Do not copy the code."
(d) The two instructions point opposite ways on the one artefact PLAN names by path.

**C9 · nice-to-have** · (a) `hardware/docs/research/can-stack-for-marengo.md:67` vs `PLAN.md` §6
(b) Research safety paragraph: "write `canTimeout`=1000, the torque, current and speed limits, and **the damper on**. Read all of them back". PLAN: "Damper 0x702A … Read only; must read 0. Never write 1."
(c) PLAN wins. Add to PLAN: "Do not write the damper at all — 0 is the on state and the default; writing risks a mis-typed 1."
(d) Writing 0 is harmless but pointless, and the research sentence invites a write to the one parameter that must never change (`left-arm-trunk/README.md:93`).

---

## Q4. Facts

Verified against the reference repo (nothing here needs changing):

| PLAN claim | Verdict | Path |
|---|---|---|
| Left arm `can1` 11–15, names, RS03/RS03/RS02/RS02/RS00; hand id 16 | correct (14/15 "proposed") | `STATUS.md:35-40`, `left-arm-trunk/README.md:60-66` |
| Right arm `can0` 1–5 | correct for the live arm | `docs/decisions/0001-can-and-motors.md:37-45` |
| Old left IDs "12 on can1, 11 to 14 on can0" | correct | `left-shoulder-bench-harness.md:44` |
| RS03 MIT velocity ±20 rad/s measured | correct | `crates/robstride/src/motor_type.rs:40-46` |
| RS00 code ±50/±17 vs manuals ±33/±14 | correct | `motor_type.rs:21-32`, `docs/research/...:54` |
| `0x7028` u32, 1000 = 50 ms, 20000 = 1 s | correct | `params.rs:34,78`, research:59 |
| `0x700B`, `0x7017` | correct | `params.rs:28-29` |
| `0x7018`, `0x701C`, `0x701A` | correct as vendor indices, **not present in legacy code** | `CALIBRATION_LOOP.md:79,142-144`; absent from `params.rs:23-43` |
| Damper `0x702A` (RS03 also `0x2026`), 0 = on | correct | `left-arm-trunk/README.md:93` |
| VBUS 22–26 V bench; 60/75 °C; ≥5° unattended; 2–3° supervised | correct | `CALIBRATION_LOOP.md:154-159` |
| RS03 13 N·m / RS02 6 N·m stalled | correct as *stall* vs rotating 20/7 | `docs/sim-to-real-loop.md:87` |
| Elbow cap ≤ 17 N·m design load, 0°/124° stops | correct | `ELBOW_DUMMY_OUTPUT_TEST.md:24,141` |
| ~40 N side push at the hand | correct | `ELBOW_DUMMY_OUTPUT_TEST.md:3` |
| Adapter: isolated STM32F0, mainline candleLight, ~$60, not G431 | correct | `docs/research/...:36-42`, `compute-plan.md:73` |
| 250 Hz default, 300 only if measured; 60 % receive window; PREEMPT_RT fallback | correct | `docs/research/...:34,65` |
| Heartbeat 50–100 ms; 30 W/60 s; 3 kJ; INA237; limbs 2 mΩ | correct | `REV_B_PARTS.md:113-125` |
| 3 A then 10 A bench fuse; no current limit | correct | `left-shoulder-bench-harness.md:57,111` |
| 120 Ω each end, ~60 Ω measured | correct | `left-shoulder-bench-harness.md:63-65` |
| T1–T6, ~1 kHz, order RS03→RS02→RS00→RS04, +10 % cap, 1 rad/s | correct | `sim-to-real-loop.md:76-89` |

Wrong, unsupported, or unverifiable:

**F1 · must-fix** · (a) `PLAN.md` "When the hardware is ready" row "Left shoulder and upper-arm drives (RS03 ×2, RS02)" with "Items 1 to 6"
(b) implied: three drives ready now.
(c) Split the row: "RS03 ×2 + RS02 loose (items 1–6)" and "elbow RS02 + RS00 (items 1–6 + item 8)". 
(d) The elbow is an RS02 with a 17 N·m printed-stop design load and the lower-arm yaw is an RS00 whose scale is unsettled (`STATUS.md:38-39`); lumping them into the "ready now" row hides that two of the five left drives cannot move under power until milestone 8's rig and §2's capture.

**F2 · must-fix** · (a) `PLAN.md` §6 and §3, `socketcan` version
(b) "`socketcan` 4.0"
(c) See C7.
(d) `Cargo.lock:2795` has 3.5.0; the version claim cannot be checked from this packet.

**F3 · should-fix** · (a) `PLAN.md` §7, "Limits" line
(b) "stalled holds stop at the **stall** rating (RS03 13 N·m, RS02 6 N·m; check RS00 and RS04)"
(c) Add the rotating values so the pair is unambiguous: "RS03 13 N·m stalled vs 20 rotating, RS02 6 vs 7; check RS00/RS04."
(d) `sim-to-real-loop.md:87` gives both numbers; the doc as written is one column of a two-column table, and `CALIBRATION_LOOP.md:250` uses a third number (RS02 "6 N·m rated", "allowable 4.0" after a 1.5 factor), so a reader can land on three different caps.

**F4 · should-fix** · (a) `PLAN.md` §8, "Elbow torque cap ≤ 17 N·m"
(b) "Elbow torque cap | ≤ 17 N·m (printed stop design load), lower if the rig fails low"
(c) "Elbow drive torque cap ≤ 17 N·m (the printed stop's design load, the RS02's own peak). This is a *structure* cap, not a working-load cap: `CALIBRATION_LOOP.md` §14 allows the elbow only 4.0 N·m as a steady holding load with the arm horizontal and a 1.5 dynamic factor, so no test may hold near 17 N·m."
(d) Two documents give 17 and 4.0 N·m for the same joint and only one of them is about a load you may apply (`ELBOW_DUMMY_OUTPUT_TEST.md:24`, `CALIBRATION_LOOP.md:246-251`).

**F5 · should-fix** · (a) `PLAN.md` §3 new dependencies
(b) "`rtrb` for realtime to non-realtime handoff, `thread-priority`", MCAP logging, `motorcom` cross-check.
(c) Add one line: "None of `rtrb`, `thread-priority`, `mcap` or `motorcom` is a dependency of the reference repo; the first three are new dependencies in the new repo and are chosen, not inherited."
(d) None of them appears in `marengo/Cargo.lock` (no match for `name = "rtrb"|"mcap"|"thread-priority"`), and the reference logging is SQLite (`crates/marengo-store`), so an agent told to "lift" logging facts will find nothing.

**F6 · nice-to-have** · (a) `PLAN.md` "Milestones" and `README.md`
(b) Neither document states 23 joints.
(c) Add "23 joints (waist 1, each leg 6, each arm 5)" to `README.md` and to the Milestones intro.
(d) `hardware/docs/kinematics.md:28` is the only place with the count and the limbs' split; the skeleton cannot be checked against "five limbs" alone.

**F7 · nice-to-have** · (a) `PLAN.md` §3, "pinned"
(b) "One blocking thread per bus, SCHED_FIFO, pinned, memory locked."
(c) "One thread per bus plus the control thread, all SCHED_FIFO on the isolated cores, with the map written in `ARCHITECTURE.md`."
(d) The research's RT plan isolates 2 cores (`docs/research/...:11`) and five buses plus a control thread is six threads; "pinned" with no map is unbuildable, and equal-priority FIFO threads can starve each other.

---

## Q5. Order

**O1 · must-fix** · (a) `PLAN.md` "Start here"
(b) Steps 1–6 as written (layout+skeleton → knowledge lift → CAN+sim → buy one adapter and bring drives up → test stand → camera).
(c) Reorder to:
> 1. Read the tenets. 2. Write `ARCHITECTURE.md` and the crate layout. 3. **Lift the hard-won knowledge into the new repo as notes and test vectors** (item 2) — before the skeleton, because it settles `zero_rad`, `direction`, the frame allowlist and the post-SetZero numbers. 4. Write the skeleton (item 1) and its loader. 5. **Buy the adapter and order the powered hub on day one** (shipping lead time; the checks are in §5). 6. Build the codec, `robot-cli` **scan / set-ID / disable** and the sim drive against `vcan` (items 3 and 4) — `robot-cli` first, because it is the first thing that touches a real drive and it must already be a proven, read-only-by-default tool. 7. Run the safety stop rules against the sim's fault matrix (item 6 gate). 8. Only then plug a drive in: `robot-cli scan` (type 0, read-only) before any Enable, then §5's soak and latency measurement. 9. Test stand (item 7). 10. Camera loop (item 9).
(d) The knowledge lift fixes fields the skeleton needs, so doing it after the skeleton guarantees a schema rewrite; the adapter has a shipping lead time and its checks are independent of the code; and `robot-cli` must exist and be boring before anything transmits to a real drive.

**O2 · should-fix** · (a) `PLAN.md` Milestones row 1
(b) "Left arm online: loose drives on the bench, then the actuator test stand, then the printed arm on the torso with the camera calibration loop | The whole stack: skeleton, CAN layer, safety layers, sim drive, logging"
(c) Split: **1a** drives online + safety proven (items 1–6); **1b** test stand (item 7) and the arm under power (item 8); **1c** arm on the torso + camera loop (item 9). Three acceptance entries, one milestone number.
(d) As one milestone, "milestone 1 passes" is unachievable for weeks after the stack works, so the old code cannot be deleted and the right arm cannot migrate (milestone 2 depends on milestone 1 "passing").

**O3 · should-fix** · (a) `PLAN.md` Milestones rows 3 and 4
(b) "3 | Waist | A third bus" / "4 | Power board and battery | The power board interface (item 10)"
(c) Swap: 3 = power board + battery (items 10/11), 4 = waist.
(d) The power board changes the enable path (heartbeat, E-stop read, VBUS window, 90 A budget, 45 V minimum) for every limb; doing it while two limbs exist is cheaper than after three, and `STATUS.md:23` has the waist **parked** while the power board is in routing.

**O4 · nice-to-have** · (a) `PLAN.md` §4 last sentence
(b) "There are no spare actuators, so every stop rule is proven here first."
(c) "…every stop rule is proven here first: the fault matrix in §4 is the gate for item 6, and no real Enable ships until each rule has a test that fails before the fix."
(d) Otherwise the sentence is a wish; attaching it to a gate is what makes the order safe.

---

## Q6. Over-specification

**X1 · must-fix** — see C4 (item 10 "Can be built against the sim now").

**X2 · should-fix** — see C8 (lifting the legacy emulator as code instead of as a behaviour spec).

**X3 · nice-to-have** · (a) `PLAN.md` §8 table
(b) Restates elbow cap, 2–3°, ≥5°, "never run into a hard stop" — values that §1 puts in the skeleton.
(c) Add one line above the table: "These are the sources for the skeleton rows; where the table and the skeleton disagree, the skeleton is the one the runtime reads and the table is wrong."
(d) Tenet 2's "If a joint name, limit or ID appears in two files, one of them is wrong" is violated by PLAN's own table the day it is written.

**X4 · nice-to-have** · (a) `PLAN.md` "Names and crate layout"
(b) "Proposed layout, about 10 crates in place of 15 crates and 6 bins."
(c) Delete the counts. Keep "names say what things do" and the mapping table.
(d) The count is the only part of the paragraph that can be wrong within a week, and it is not a requirement.

---

## Q7. Questions only the owner can answer

Each with a default so work is not blocked.

1. **Full-body bus/ID map for legs and waist.** Default: waist `can2` id 41; right leg `can3` 21–26; left leg `can4` 31–36; left arm `can1` 11–15 (+16 hand); right arm `can0` 1–5. Marked provisional in the skeleton until the hardware bus-plan ADR lands.
2. **Zero convention: is `zero_rad` a firmware-zero residual (C1 correction), or do you want the drive zeroed by the runtime at a fixed reference every session?** Default: residual offset, written only by the zero procedure, per G3.
3. **Gravity feed-forward in milestone 1a?** Default: yes, torque-only (`kp = kd = 0`) after the per-joint sign check, using a generated inertial model whose hash is in the skeleton; the arm is supported or over the pad.
4. **May an agent ever write to a drive's flash/ID?** Default: set-ID and SetZero only through `robot-cli`, one drive at a time, runtime stopped; no firmware flashing, no type-22 save. If that is wrong, say so before item 5.
5. **Who flashes the candleLight firmware onto the adapter, and on which host?** Default: buy pre-flashed; otherwise flash on the Windows PC per candleLight's DFU instructions, and the `gs_usb` check in §5 is the acceptance.
6. **Is the Pi 5 the bench host with the BRIO camera on it** (`CALIBRATION_LOOP.md:220` leaves this open)? Default: yes, Pi, so frames and CAN share one clock.
7. **Does the left arm branch share the right arm's E-stop** (open in `left-shoulder-bench-harness.md:153`)? Default: yes, one NC E-stop in the 24 V supply line for both.
8. **VBUS window on the finished robot.** Default: 36–58 V with no drive load below 45 V; the bench instance is 22–26 V, both in the skeleton.
9. **Is the waist RS03 in the test stand the uninstalled one, or does the waist come off the torso?** Default: the uninstalled unit — `sim-to-real-loop.md:85` assumes it.
10. **Do you want the runtime to be able to reject an Enable when any joint of an unbuilt limb is missing?** Default: no — unbuilt limbs are Offline and never block (tenet 1). If you want the opposite, tenet 1 needs amending.
11. **Repo host and visibility** (`jaylamping/roark`), and whether CI may run on GitHub's Linux runners with `vcan`. Default: private GitHub repo, `vcan` in CI.

---

### Smallest set that unblocks a first agent

If only five items can land first: **G1** (skeleton schema + units), **G2** (whole-robot bus/ID map), **G5** (state machines + enable/stop sequences), **C1/C3** (what the skeleton is and whether the runtime reads a model), **V1** (the `canTimeout` accept/refuse rule). Everything else in this list can be corrected while the codec and sim are being built, because none of it transmits to a motor.