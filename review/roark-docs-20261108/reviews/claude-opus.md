# Proposed edits to Roark TENETS.md, PLAN.md, CLAUDE.md and README.md

I read every file in `roark/`. In `marengo/` I read the key files PACKET names, plus `docs/commissioning/firmware/*`, `docs/decisions/0036-physical-robstride-reference.md` (from line 140 on), the `docs/safety.md` excerpt, `config/motors.yaml`, `config/homing.yaml`, `crates/robstride/src/{params.rs,bus.rs}`, and the bench-harness, arm-trunk and elbow-test docs.

**What I could not verify:**
- **The RobStride PDFs.** The PDF reader failed (`pdftoppm is not installed`). Every "manual" fact below is checked only against repo documents that quote the manuals. Those are the CAN research report, CALIBRATION_LOOP §9, `pdb/PLAN.md` §3.4, the arm-trunk README, `robstride-mit-ranges.md` and `params.rs`.
- **The six review files.** I only searched them; I did not read them in full.

Items are tagged **[MUST]**, **[SHOULD]** or **[NICE]**. Each gives where, current text, new text and why.

---

## Q1. Vague sentences

**[MUST] V1. PLAN §6, Drive `canTimeout` row**
- **Current:** "Write 1000 (= 50 ms; 20000 = 1 s) and read back before every enable; refuse on 0 or above. Re-check after any drive power cycle or reset report"
- **New:** "Write 1000 (= 50 ms; 20000 = 1 s) and read it back before every enable. Refuse if the value read back is 0 or greater than 1000. A drive counts as power-cycled when it misses ≥ 3 replies and then answers again, or reports mode 0 (reset) while the host has it enabled. Its limb stays stopped until the full preflight (timeout, caps, damper, VBUS, faults) has run again, and until it has been re-referenced if zero persistence isn't proven (see G3). Never assume a parameter survives a power cycle: we never send type 22 (save), and whether 0x7028 persists is unverified."
- **Why:** "or above" has no number. The plan also never says how a power cycle is detected. `docs/safety.md:599` records that persistence is unverified.

**[MUST] V2. PLAN §6: "stop the limb" / "stop the whole robot" are used but never defined**
- **New paragraph at the top of §6:** "**Stop** means: send SpeedTarget 0, neutral MIT, then Disable to every drive in scope. Send at once, unpaced, transmit-only, retrying on ENOBUFS. The stop latches until an operator clears it through `api`. If Disable can't be delivered (bus lost, process dead), the drive's 50 ms `canTimeout` is the backstop and the drive goes limp. Arms fall onto their support; that is accepted (CALIBRATION_LOOP §13). A stage runner may first make a **controlled stop**: a slow move to its rest pose, then Stop. It may only do this when every drive in scope is healthy (CALIBRATION_LOOP §9, 'How a stop happens')."
- **Why:** These are the two most-used words in the safety table. The sequence is the lifted stop order in RECONCILIATION.

**[MUST] V3. PLAN §3**
- **Current:** "PREEMPT_RT only if the Pi 5 measurement misses its budget."
- **New:** "Budget at 250 Hz (4 ms period): each bus thread starts its command burst within 0.5 ms of the tick, and ≥ 99.9 % of replies arrive before the 2.4 ms (60 %) deadline. Measure over a 1-hour soak with every connected bus running. Miss either → try a 6.12+ PREEMPT_RT kernel, then PEAK adapters."
- **Why:** There is no number to test against. 4 ms, 60 % and the fallback order come from the research report. **0.5 ms, 99.9 % and 1 h are my proposals** (see Q7).

**[SHOULD] V4. PLAN §3**
- **Current:** "bitrate and sample point set explicitly, error frames on, no automatic bus-off restart while moving, short TX queue."
- **New:** "`bitrate 1000000 sample-point 0.75`, `berr-reporting on`, no `restart-ms` ever, `txqueuelen` left at the default 10. Count ENOBUFS and drop the frame."
- **Why:** The research report gives exact values. "Short" and "while moving" leave room for guessing.

**[MUST] V5. PLAN §6, Limits row**
- **Current:** "total commanded torque bounded, backed by the drive's own limit"
- **New:** "Every tick, the host computes each joint's MIT torque from the latest measured state: |Kp·(p_des−p) + Kd·(v_des−v) + τ_ff|. It refuses to send any frame whose result exceeds the joint's cap, and stops the limb instead. The drive's `limit_torque`, set to the same cap, is the backstop."
- **Why:** RECONCILIATION blocker 4 found that the old "total bound" wasn't a total.

**[MUST] V6. PLAN §8, elbow torque cap**
- **Current:** "≤ 17 N·m (printed stop design load), lower if the rig fails low"
- **New:** "≤ 17 N·m, valid only if the rig's printed stop fails at ≥ 25.5 N·m (its pass line). If it fails lower, the part is redesigned and retested, and the forearm is not powered until it passes."
- **Why:** "Fails low" has no threshold. The pass line and the redesign action are in ELBOW_DUMMY_OUTPUT_TEST.md (lines 149 and 201).

**[MUST] V7. PLAN §8, "Never" row**
- **Current:** "run into a hard stop, hold a stall, exceed rated torque"
- **New:** "Never command more than the joint's skeleton torque cap. Never hold a stalled output above the actuator's stall rating: RS03 13 N·m, RS02 6 N·m; RS00 and RS04 come from the manuals before their tests. Never drive into a hard stop."
- **Why:** The elbow cap of 17 N·m is the RS02's *peak*. sim-to-real-loop.md lists RS02 *rated* torque as 6 N·m. Read literally, "rated" contradicts the cap.

**[SHOULD] V8. PLAN §6, Temperature row**
- **Current:** "Pause at 60 °C, stop at 75 °C"
- **New:** "At ≥ 60 °C on any joint, start no new move. Go to rest (torque off if the pose allows) and resume at ≤ 45 °C. At ≥ 75 °C, Stop."
- **Why:** The resume threshold and the pause behaviour are in CALIBRATION_LOOP §9 but missing here.

**[SHOULD] V9. PLAN §5**
- **Current:** "then a CAN soak (error frames, overruns, bus load)"
- **New:** "Soak: 1 hour at 250 Hz with every connected drive enabled at neutral MIT (kp = kd = τ = 0). Pass: zero error frames, zero RX overruns, zero missed replies. Record bus load."
- **Why:** As written there is no duration and no pass criterion. Neutral-MIT soaks are the legacy pattern (`firmware-behavior.md`, "Wire-level MIT neutrality"). **The 1 h is a proposal.**

**[SHOULD] V10. PLAN, Observability section**
- **Current:** "A wider observability stack choice (Grafana, Sentry, OpenTelemetry, Foxglove, Rerun) is still to be researched."
- **New:** "Decided for milestones 1–2: MCAP from the runtime, `candump -L` per bus, Prometheus + Grafana for metrics, `node_exporter` for the host. Nothing else is added until a named need appears, and that choice is Joey's."
- **Why:** As written, an agent can't tell whether research has to come before building.

**[SHOULD] V11. TENETS §7**
- **Current:** "One short architecture map, kept current with every change that moves a seam."
- **New:** "`ARCHITECTURE.md` at the repo root, one page: one line per crate/bin (owns / must never / talks to) and a diagram of the seams. Any PR that adds, removes or re-routes a crate edits it in the same PR."
- **Why:** Gives the map a filename and a testable trigger.

**[SHOULD] V12. TENETS §4: add a closing sentence**
- **New:** "Tenet 0 sets the *shape* (names, data model, no limb-specific paths). Tenet 4 sets the *amount* (features). Example: the bus loop handles every bus the skeleton lists, but no leg control mode is written until legs are on the bench."
- **Why:** As written, tenets 0 and 4 pull against each other and an agent has to pick one.

**[SHOULD] V13. TENETS §5**
- **Current:** "every frame in and out can be logged, with timestamps"
- **New:** "every frame in and out **is** logged, with kernel timestamps, by default"
- **Why:** RECONCILIATION (Major) found "no per-frame log by default". "Can be" leaves that open again.

**[SHOULD] V14. PLAN §1**
- **Current:** "Directions for the left joints are unknown until the sign check; leave them as placeholders."
- **New:** "Write `direction: unverified` and `zero_rad: unverified`. The loader accepts them. For such a joint the runtime refuses gravity feed-forward and refuses any target more than 5° from the joint's position at enable. It allows the sign-check pulse and the C1 ±5° move."
- **Why:** A wrong sign mirrors the soft limits, so the joint could be driven into a hard stop. "Placeholder" doesn't say what the runtime should do with one.

**[SHOULD] V15. PLAN, "Before the old stack is deleted"**
- **Current:** "keep the `gravity-fit` and `firmware-timing` tools and the commissioning docs"
- **New:** "Port `gravity-fit` (into `dynamics`) and `firmware-timing` (into `robot-cli` or a script) to this repo. Copy `docs/commissioning/{limb-playbook.md, bench-ledger.md, firmware/}` into this repo's `docs/`."
- **Why:** "Keep" is ambiguous. Both tools live in `marengo-log-cli`, which RECONCILIATION deletes.

**[SHOULD] V16. PLAN §2**
- **Current:** "Copy them into the new codec's docs and test vectors"
- **New:** "Put them in `crates/robstride/` docs, one section per fact citing the legacy `file:line` and the manual section, with test vectors in its tests. Put firmware timing facts into the sim drive's model."
- **Why:** Says where the lifted knowledge goes.

**[NICE] V17. PLAN status line**
- **Current:** "Written by the hardware project. Joey pastes this page's link into the software project as its starting brief."
- **New:** "Owner: Joey. Hardware facts: STATUS.md. Precedence: see 'Which file wins'."
- **Why:** The current line is stale now that the plan lives in the Roark repo.

---

## Q2. Gaps (new text)

**[MUST] G1. CLAUDE.md: add a rule for real hardware**
- **New:** "No agent sends Enable, motion, Set Zero, set-ID or a parameter write to a real drive unless Joey is present and has said go for that session. `vcan` is free. Every joint's first motion is attended. The legacy `marengo-pi-mcp` tools stop the service and start their own CAN owner (RECONCILIATION table). Never use them on a bus this runtime owns."
- **Why:** Nothing currently stops an agent from moving a real arm. There are no spare actuators.

**[MUST] G2. PLAN §1: skeleton schema**
- **New:**
  ```yaml
  schema: 1
  name: roark                  # the only place the robot's name appears
  host_id: 0xFD
  power: { source: bench_24v, vbus_window_v: [22, 26] }
  limbs:
    left_arm:
      bus: can1
      joints:
        - name: left_shoulder_pitch
          actuator: rs03         # model only; MIT scales live in the codec
          id: 11
          uid: null              # MCU UID from type-0, filled at commissioning
          direction: unverified  # +1 | -1 | unverified
          zero_rad: unverified
          hard_rad: [-0.87, 3.14]   # STATUS.md, provisional
          soft_rad: null         # optional, tighter than the derived margin
          torque_cap_nm: TBD
          current_cap_a: TBD
          speed_cap_rad_s: TBD
  ```
  - **Units:** SI and radians, joint space.
  - **Conversion:** `q_joint = direction·(q_motor − zero_rad)` and `τ_joint = direction·τ_motor`.
  - **Loader refuses:**
    - duplicate names;
    - duplicate IDs anywhere in the robot;
    - a cap above the model's MIT range;
    - a hard window wider than 2π (see G3);
    - a sum of `current_cap_a` over all joints above 90 A (STATUS.md).
  - **Soft limits:** derived as hard − 3° (supervised) or hard − 5° (unattended), unless `soft_rad` is tighter.
- **Why:** Tenet 2 lists the fields but gives no units, sign convention or validation rules.

**[MUST] G3. New PLAN section: the zero convention**
- **New:** "The drive reports output position within a single-turn window after power-on. The 0x7029 flag sets it to 0..2π or −π..π, and it is volatile unless saved (`params.rs:39-42`). We never write 0x7029 or 0x702B. Whether Set Zero survives a power cycle is unknown (ADR 0036:347).
  - **Persistence test.** Before milestone-1 motion, take one loose drive of each model. Mark a pose, read 0x7019, power-cycle, read again; repeat after one Set Zero.
  - **If the reading repeats within 0.5°** (the C1 gate): `zero_rad` is a skeleton fact and the runtime never sends Set Zero.
  - **If it doesn't:** every power-up needs an attended reference at a marked pose. Until that reference is done, the joint is treated as `zero_rad: unverified`.
  - **Wrap rule.** No joint's hard window may contain the drive's wrap point. No Enable within 800 ms of a Set Zero (blackout data in `firmware-behavior.md`)."
- **Why:** Without this the skeleton's "zero" field means nothing, and C5 (power-cycle re-reference) has no defined workflow.

**[MUST] G4. New PLAN section: drive and limb states, and the exact sequences**
- **New:**
  - **Drive states:** `offline → seen` (answers type 0; UID matches) `→ checked` (preflight passed since this power-up) `→ enabled` (Run mode seen) `→ stopped` (latched). Any silence or reset report drops it back to `seen`.
  - **Enable, one drive per bus per tick:**
    1. Active reporting off; wait for its echo plus one period.
    2. Preflight writes and read-backs.
    3. Check it has been ≥ 800 ms since any Set Zero.
    4. Enable (type 3).
    5. Wait for a type-2 reply in mode 2 (Run). Measured 1.36–10.3 ms; refuse after 50 ms (proposal).
    6. Write RunMode = 0 (MIT).
    7. No non-neutral MIT frame until Run is seen.
  - **Stop:** as in V2.
  - **Exit:** Stop, then active reporting off.
  - **Silence counting:** don't count silence during the enable→Run window or for 100 ms after a Disable. Post-Disable silences of up to 48.7 ms have been measured.
  - **Limb enable:** an operator enables a named set of joints. Each must be `checked`. Offline joints elsewhere never block it (tenet 1). Gravity feed-forward needs every joint of that limb online with a verified direction.
- **Why:**
  - The order and timings are lifted from RECONCILIATION and `firmware-behavior.md`.
  - The research report says the scan should "refuse unless every skeleton ID answers". That inverts tenet 1, and RECONCILIATION blocker 3 hit the same problem.
  - Milestone 1 starts with only 3 of the 5 left drives connected.

**[MUST] G5. PLAN, `api` row: the interface contract**
- **New:** "The transport is the builder's choice, but it must be local to the Pi, and a slow or dead client must never block the loop.
  - **Reads:** decimated snapshot (joint state, age, misses, faults, temperatures, limb state); the node table; events.
  - **Commands:**
    - `stop(scope)`: always accepted, from any client.
    - `enable(joints)`: runs the preflight first.
    - `run_plan(file)`: one at a time. Executed inside the runtime with the stop rules; no live streaming (CALIBRATION_LOOP §10.3).
    - `scan(bus)` and `set_id(bus, old, new)`: only while every limb on that bus is disabled.
    - `clear_stop(scope)`.
  - **Disconnects:** a client disconnecting doesn't change motion (tenet 5). The physical E-stop is the operator's backstop."
- **Why:** The plan names one interface but says nothing about what it carries or who may command motion.

**[MUST] G6. PLAN §3: a hung control thread must make drives time out**
- **New:** "A bus thread never re-sends a stale command. If the control thread hasn't published for 2 ticks, the bus thread runs Stop for its limbs. systemd `WatchdogSec` is fed only from the control thread."
- **Why:** With separate bus threads, a hung control thread whose bus threads keep re-sending the last command array keeps feeding the drives. `canTimeout` then never fires, and gravity feed-forward stays on. That is RECONCILIATION blocker 2 reintroduced.

**[MUST] G7. PLAN §3: crash and restart behaviour**
- **New:** "The runtime starts with every drive disabled and never sends Enable on its own: not at start, not after a restart, not when a drive comes back. A panic in any thread ends the process. `ExecStopPost` runs on every exit (clean, panic or watchdog kill) and sends Disable plus active-reporting-off to every skeleton ID on every bus. The stop path disables drives before it flushes any log (RECONCILIATION blocker 5)."
- **Why:**
  - If systemd restarts the runtime and it auto-enables, a crash loop moves the arm.
  - Blocker 5 is missing from the plan.

**[MUST] G8. PLAN §3/§5: stable bus names**
- **New:** "gs_usb adapters get `canN` names in probe order. The Pi's MCP2515 HAT already provides `can0` and `can1`; `can1` is wired to the HAT today (left-shoulder-bench-harness.md:31,43).
  - Pin each adapter to its skeleton name with a udev rule on its USB serial.
  - Disable the HAT's `can1` overlay before the adapter takes that name.
  - At start, check each interface's driver against the skeleton.
  - If a scan finds an ID the skeleton puts on another bus, refuse enable on both buses."
- **Why:** The plan's "`can1` = USB adapter" collides with the HAT's existing `can1`. Enumeration can swap buses after a reboot.

**[MUST] G9. PLAN §7 and a new codec note: run modes and whether caps hold in them**
- **New:** "Joints use only MIT mode (0x7005 = 0). Before T1 uses speed mode (2) or current mode (3), check on a loose drive that `limit_torque`, `limit_cur` and `limit_spd` are actually enforced in that mode. Until a cap has been shown to hold in a mode, treat it as host-only in that mode."
- **Why:** The codex-astra review: "Register readback alone does not establish how every cap behaves in MIT mode."

**[MUST] G10. Observability section: log layout and disk failures**
- **New:**
  - **Session file:** one MCAP per runtime session in `<data>/sessions/<UTC>/`. Contents:
    - raw frames per bus `{bus,id,dlc,data,t_kernel,t_mono,dir}`;
    - decoded joints, commands, injected faults;
    - events: state changes, stops, limit hits, operator actions with the client's identity;
    - the skeleton file, its hash and the git revision at start.
  - **Black box:** `candump -L` to `<data>/candump/<bus>-<UTC>.log`.
  - **Logger feed:** fed through `rtrb`. When the ring is full, drop and count.
  - **Failures:** a logging error never touches the loop. If the session log can't be opened or the disk is full, refuse the *next* enable; never stop a running motion for it.
- **Why:**
  - Tenet 6 ("every session is logged") and tenet 5 ("logging can crash without affecting motion") conflict unless this is decided.
  - Raw frame fields come from the research report.

**[MUST] G11. PLAN §4: sim-drive behaviour and "done"**
- **New:**
  - **Interfaces:** create the vcan interfaces with the skeleton's bus names (`ip link add dev can1 type vcan`), so the runtime runs literally unchanged.
  - **Frames answered:** types 0/1/3/4/6/17/18/24, through the same codec.
  - **Behaviour simulated:**
    - `canTimeout` → reset mode;
    - parameters revert to defaults on a simulated reboot (`canTimeout` = 0);
    - enable→Run delay 0–11 ms;
    - Set-Zero blackout starting at 500–625 ms, lasting 40–67 ms;
    - post-Disable silence ≤ 50 ms;
    - duplicate IDs;
    - parameter read-back mismatch.
  - **Plant:** a per-joint first-order plant until a test needs coupled gravity.
  - **Done:** every rule in §6 has a seeded scenario that injects its fault and asserts the Stop within its bound.
- **Why:**
  - `vcan0..4` names would force a second skeleton or a name-mapping flag.
  - The behaviours come from `firmware-behavior.md` and its emulator ranges.
  - Without a "done", item 4 can't be finished.

**[MUST] G12. PLAN, Milestones: acceptance criteria**
- **New:**
  - **M1:**
    - (a) all §6 sim scenarios pass;
    - (b) 3 loose drives scanned, IDs 11–13 set, preflight passes, and a deliberately wrong `canTimeout` is refused;
    - (c) the soak (V9) passes, and latency/jitter are logged against V3;
    - (d) `kill -9` while enabled leaves drives in reset within 50 ms, and the `ExecStopPost` Disable shows in candump;
    - (e) a drive power-cycled while enabled (pull its drop fuse) can't re-enable until the preflight runs again;
    - (f) all 5 joints sign-checked and zeroed (C1);
    - (g) the elbow rig has passed before the forearm is powered;
    - (h) T1–T6 output files exist for RS03 and RS02;
    - (i) C0–C2 attended passes plus one clean unattended C3.
  - **M2:** both arms enable from one runtime and one skeleton. Silence on one drive stops only its limb. Legacy code deleted.
  - **M3:** waist on its own bus.
  - **M4:**
    - stopping the heartbeat drops the motor latch within 270–355 ms;
    - port-fault decode names the port;
    - the self-test runs under its conditions.
  - **M5:** legs on the gantry; measured bus load < 70 %.
- **Why:** The "Proves" column can't be tested. Thresholds come from REV_B_PARTS.md, the research report and CALIBRATION_LOOP. (i) is a proposal (see Q7).

**[SHOULD] G13. PLAN, new section: repo, toolchain, CI and deploy**
- **New:**
  - **Layout:** Cargo workspace with `crates/`, `bins/`, `config/skeleton.yaml`, `deploy/` (systemd units, udev rules, CAN link config), `docs/`.
  - **Toolchain:** `rust-toolchain.toml` pinned to stable; target `aarch64-unknown-linux-gnu`, 64-bit Pi OS.
  - **CI on every PR:** fmt, clippy `-D warnings`, tests including the vcan scenarios. I haven't verified that hosted runners can load `vcan`; if they can't, run them on the Pi.
  - **Units:**
    - `runtime.service`: `Type=notify`, `WatchdogSec`, `LimitRTPRIO`, `LimitMEMLOCK=infinity`, `ExecStopPost` disable;
    - `candump@<bus>.service`.
  - **Naming:** no paths or unit names carry the robot's name.
- **Why:** None of this is written down. Legacy `marengo-deploy` is deleted and must not be rebuilt.

**[SHOULD] G14. PLAN §2/§3: the transmit allowlist**
- **New:** "Transmit only types 0, 1, 3, 4, 6, 17, 18 and 24.
  - Type 7 only from set-ID, with exactly one drive answering on that bus.
  - Never types 22, 23 or 25, and never write 0x7029, 0x702B or 0x702A.
  - Never type 4 with byte0 = 1 (fault clear) until a recovery policy is approved."
- **Why:**
  - `bus.rs:40-45` refuses type 7, but the plan needs set-ID.
  - Fault-clear is not bench-qualified (`WP-AC.md` L-robstride-02).

**[SHOULD] G15. PLAN §6: recovering from a fault**
- **New:** "A latched stop clears only by an operator action through `api`. A drive fault bit clears only by power-cycling that drive (on the bench, an E-stop cycle), followed by the full preflight."
- **Why:** "Never auto-clear" is stated, but no recovery path is (legacy decision, `WP-I.md` option (a)).

**[SHOULD] G16. PLAN §6: stage caps versus skeleton caps**
- **New:** "The cap written to a drive is min(skeleton cap, stage or test cap). A plan may lower a cap, never raise it. The preflight compares the read-back with the value written for this run."
- **Why:** CALIBRATION_LOOP §9 sets per-stage caps (1.5 × gravity); sim-to-real sets test max + 10 %; PLAN only says "from the skeleton".

**[SHOULD] G17. PLAN §3, scan: drive identity**
- **New:** "The scan reads each drive's MCU UID (type 0). Once commissioned, the skeleton stores it per joint. On a mismatch, refuse enable on that limb."
- **Why:** Joints 13 and 14 are both RS02 but have different caps. Swapped IDs after rework would otherwise go unseen. The UID check is one of the ideas RECONCILIATION keeps.

**[SHOULD] G18. PLAN §6: monitor VBUS while running**
- **New:** "The type-2 reply has no VBUS. Read 0x701C in the slack at least once per second per drive (proposal) and Stop outside the window."
- **Why:** CALIBRATION_LOOP §9 makes VBUS a run-time stop rule; PLAN only checks it in preflight.

**[SHOULD] G19. PLAN §10: requirements the plan leaves out**
- **New, add to the list:**
  - refuse enable while the BMS shows charge current or its telemetry is stale;
  - shut compute down at min cell ≤ 3.0 V, filtered pack ≤ 38 V, or BMS telemetry lost > 5 s;
  - no drive load until the bus is > 45 V;
  - a falling edge on `WD_FEED` at least every 270 ms;
  - self-test only with VBUS < 3 V, ≥ 5 s after the last stop, pack ≥ 40 V;
  - name the faulted port from the meters;
  - tell the operator to wait for VBUS < 3 V before pressing reset.
- **Why:** All of these are in REV_B_PARTS.md and STATUS.md but absent from §10.

**[SHOULD] G20. PLAN §1/§2: right-arm migration specifics**
- **New:** "From `marengo/config/motors.yaml`, take IDs 1–5 on `can0`, the directions (sign-checked 2026-08-11) and the bench envelopes (Set Limits, 2026-07-22). The envelopes are relative to the legacy manual reference, so re-reference before trusting them. Don't carry the `control.yaml` gains: the RS03 gains were tuned while the drive saw 0.4 × v_des (`robstride-mit-ranges.md`). `right_lower_arm_yaw`'s URDF axis is wrong (kinematics.md:161)."
- **Why:** "Move the right arm into the skeleton" hides these traps.

**[SHOULD] G21. Observability: one clock**
- **New:** "The Pi is disciplined by chrony. MCAP carries the kernel CAN timestamp and monotonic time. Camera timestamps use the same clock (CALIBRATION_LOOP §11)."
- **Why:** Tenet 8 asks for one shared clock but doesn't say how.

---

## Q3. Contradictions (which wins, and the fix)

**[MUST] C1. When the legacy code is deleted**
- TENETS L4–5: "once the left arm passes its tests".
- PLAN L13: after M1 plus the right arm moved.
- Milestone table: M2.
- Owner's intent: after M2.
- **Owner wins.** TENETS L4–5 → "…once milestone 2 passes (the right arm runs on the new stack), the legacy code is deleted from the reference repo." PLAN L13 → "When milestone 2 passes…".

**[MUST] C2. Same repo or new repo**
- PLAN L13 "built beside the existing code" and §Names "the old crates are not renamed, they're deleted when their replacement passes" contradict CLAUDE.md (new repo; reference is read-only). `marengo/CLAUDE.md` also says "build new crates beside it".
- **CLAUDE.md wins.** PLAN L13 → "built in this new repo; the legacy code stays read-only in jaylamping/marengo until deleted after milestone 2." Flag `marengo/CLAUDE.md` to Joey as stale.

**[MUST] C3. TENETS §2: a cross-repo edit can't be one commit**
- **Current:** "the hardware side edits the skeleton, in one commit, and says so in STATUS.md"
- **New:** "the hardware side edits `config/skeleton.yaml` in this repo in one PR, and adds a change-log line to STATUS.md in the hardware repo."

**[MUST] C4. Where logging runs**
- Tenet 5 puts logging outside the process, behind `api`. PLAN Observability says MCAP is written "from the realtime process".
- **PLAN wins on location.** Add to tenet 5: "Logging runs on a non-realtime thread inside the runtime, fed by a lock-free ring; it can fail without affecting motion." Plus G10.

**[MUST] C5. Who transmits**
- PLAN §3 says "Only the realtime process transmits", but the `robot-cli` row says "Scan, set ID".
- **Fix the `robot-cli` row:** "Asks the runtime (via `api`) to scan or set an ID; runs the independent `disable` after the runtime exits; never opens a bus while the runtime runs."

**[MUST] C6. Scan rule**
- The research report: the scan refuses enable "unless every skeleton ID answers". Tenet 1: an unbuilt limb is just offline.
- **Tenet wins.** Use G4's per-joint-set rule.

**[SHOULD] C7. Damper wording**
- The research report says write "the damper on". PLAN says "Read only; must read 0. Never write 1". TENETS says "left on".
- **PLAN wins** (arm-trunk README: "never set damper = 1").
- TENETS §3 layer 2 → "the anti-backdrive damper left on (0x702A reads 0 = on; never written)".

**[SHOULD] C8. Where MIT scales live**
- PLAN §3 "per-model scales from the skeleton" vs tenet 2 "one fact in one place" and "transcribed from the manuals". The skeleton is hand-edited by the hardware side; the RS03 ±50 bug was exactly a wrong scale.
- **The codec wins.** → "per-model scales as constants in `robstride`, each citing its manual section; the skeleton names the model only."

**[SHOULD] C9. Loop rate**
- PLAN §7 "about 1 kHz" vs PLAN §3 "250 Hz default; 300 Hz only if measured".
- **§3 wins** (see O3).

**[SHOULD] C10. What `can1` is**
- `can1` is a HAT channel today (bench-harness doc). PLAN says `can1` is the new USB adapter. Fix with G8.

**[SHOULD] C11. Temperature and VBUS rules**
- PLAN §6 is weaker than CALIBRATION_LOOP §9 on both. **CALIBRATION_LOOP wins** (V8, G18).

**[SHOULD] C12. 90 A current sum**
- STATUS.md: drive current limits must sum to ≤ 90 A. PLAN omits this. **STATUS wins.** Add to the §6 Drive caps row and to the loader checks (G2).

**[SHOULD] C13. TENETS §2 field list**
- TENETS §2 omits torque/current/speed caps, which PLAN §1 includes. **PLAN wins.** TENETS §2 → "…direction, zero, soft and hard limits, and torque, current and speed caps."

**[SHOULD] C14. Is the left arm finished hardware?**
- STATUS marks left IDs 14 and 15 as "(proposal)"; PLAN states them as settled. Owner question (Q7-3).

**[SHOULD] C15. The fixed robot**
- TENETS §1: "two arms, two legs and the waist… pre-assigned RobStride actuators". STATUS reserves hand ID 16 on `can1` and plans a neck of 3 × STS3215 servos, which are not RobStride and not on CAN. Owner question (Q7-13).
- Add to tenet 1: "Adding a limb (hand, neck) is a skeleton change agreed by Joey, not a plugin."

**[NICE] C16. Calibration record location**
- CALIBRATION_LOOP §10 puts records in `marengo/docs/commissioning/calibrations/`. CLAUDE.md says the reference repo is read-only. Owner question (Q7-11).

**[NICE] C17. Precedence between files**
- STATUS says linked docs win over STATUS. PLAN says "check STATUS before acting on a number here". Nothing says how the skeleton ranks.
- **Add to PLAN, "Which file wins":** "Joint facts: `skeleton.yaml`. Other hardware facts: STATUS.md and the docs it links. What to build: PLAN. How: TENETS. Legacy docs never win."

---

## Q4. Facts

**Checked and matching the reference:**
- 15 crates / 6 bins.
- Left IDs 11–15 and joint names; right arm `can0` IDs 1–5; old IDs (12 on `can1`, 11–14 on `can0`).
- Adapter class, ~$60, G431 warning.
- `canTimeout` 1000 = 50 ms and 20000 = 1 s.
- Parameters 0x700B, 0x7018, 0x7017, 0x702A.
- VBUS window 22–26 V; 3 missed replies.
- Soft limits 2–3° supervised, ≥ 5° unattended; 60/75 °C.
- Heartbeat 50–100 ms; INA237 2 mΩ shunts; clamp 30 W over 60 s and 3 kJ per burst.
- Stall ratings RS03 13 / RS02 6; test cap = max + 10 %; speed cap 1 rad/s; test order RS03 → RS02 → RS00 → RS04.
- Elbow 17 N·m and 0–124°; 40 N side load; 3 A then 10 A fuse.
- RS00 dispute (±50/±17 vs ±33/±14); RS03 measured ±20 rad/s; Set Zero 800 ms / 450 ms.
- 6808 bearings "Monday" (BUY_ORDER.md:16; 2026-11-09 is a Monday).

**Wrong, unsupported or incomplete:**
1. **[MUST] PLAN §2, "Capture a loose RS00 before any forearm motion."** A built RS00 is already on the bench: `right_lower_arm_yaw`, ID 5 on `can0`, firmware 0.0.3.32 (`motors.yaml`). It has simply never moved in a capture (`robstride-mit-ranges.md`).
   - **New:** "Settle it with a motion capture on the right arm's RS00 (ID 5) or a loose RS00, with Joey present, before any RS00 motion on the new stack."
2. **[SHOULD] PLAN §6, "RS03 also 0x2026".** The only source is `left-arm-trunk/README.md:93`; I couldn't check it against the PDF. Add "(verify in the RS03 manual's parameter table)".
3. **[SHOULD] 0x701C (VBUS) and 0x701A (`iqf`).** Cited only in reviews (`claude-opus.md:13`, `opencode-deepseek.md:55`). They're not in legacy `params.rs`, and I couldn't check them against the PDFs. Add "verify against the manual before coding".
4. **[SHOULD] RS02 torque numbers disagree in the source PLAN inherits.** `sim-to-real-loop.md:70` gives RS02 rated = 6 N·m. Line 87 says "RS02 6 [stall] vs 7 [rotating]". Also, T2 holds "to 100 % of rated" while stalled, which for RS03 (20 N·m) is above its 13 N·m stall rating.
   - **New in PLAN §7:** "T2 and T5 stop at min(test step, stall rating)".
   - Flag the RS02 numbers for a check against the manual.
5. **[SHOULD] PLAN §5 omits two adapter facts.**
   - The adapter ships with slcan and must be flashed with candleLight or bought pre-flashed (research report).
   - The "HAT end" terminator becomes the adapter's termination jumper.
6. **[SHOULD] PLAN §10 omits "falling edge at least every 270 ms"** and that the latch then needs a manual reset (REV_B_PARTS.md:113, STATUS.md).
7. **[NICE] PLAN §1 states IDs 14 and 15 as fact;** STATUS marks them as proposals.
8. **[NICE] Milestone 3 "Waist":** STATUS lists waist yaw as **parked**. The order isn't wrong, but say that M3 waits on that hardware.
9. **[SHOULD] PLAN §6, "3 missed replies":** correct, but the measured enable→Run delay (≤ 10.3 ms) and post-Disable silence (≤ 48.7 ms) would trip it unless those windows are excluded (G4).

---

## Q5. Order

**[MUST] O-Start. Replace "Start here" steps 1–6 with:**
1. **Joey, now, in parallel:** order one isolated STM32F0 candleLight adapter (pre-flashed, or flash it) and the luggage scale.
2. Read TENETS and this plan.
3. Write `skeleton` and `config/skeleton.yaml` with all five limbs, including the right arm's existing values (item 1).
4. Lift the legacy knowledge into notes and test vectors (item 2).
5. Build the `robstride` codec against the manual vectors.
6. Build the bus loop and `sim-drive` together, on vcan interfaces named after the skeleton buses (items 3 and 4).
7. Build safety (item 6) on vcan until every stop-rule scenario passes. Add the systemd units and `ExecStopPost` disable.
8. Build minimal telemetry: MCAP + candump.
9. On the real adapter, with no Enable yet: scan, set IDs, soak, latency (item 5); the zero-persistence test (G3); the caps-in-mode test (G9).
10. Attended Enable of the loose drives; sign check.
11. Test stand (item 7). Elbow rig result → item 8 limits. Camera loop (item 9).

**Why:**
- The current list never schedules item 6, yet step 5 brings real drives up. The "When the hardware is ready" table says items 1–6 are needed for the loose drives.
- The adapter has a lead time and is the first step in the research report.
- Tenet 6 needs logging before any real session.
- Putting the right arm in the skeleton on day 1 exercises "two limbs, no per-limb code" in sim long before M2.

**[SHOULD] Milestone table, M1 row: add**
- **New:** "The test stand's first unit is the waist RS03 (M3 hardware). It runs on its own skeleton bus and ID, and the runner refuses if any other drive answers on that bus."
- **Why:** As written it looks as if M1 depends on M3 hardware.

---

## Q6. Over-specification

**[SHOULD] O1. Start here, step 2: "Set up the new crate layout"**
- **New:** "Create each crate when an item first needs it. The table is the naming target, not a to-do list."
- **Why:** Creating ~10 empty crates up front is the scaffolding tenet 4 bans.

**[SHOULD] O2. PLAN §4: "driven by MuJoCo"**
- **New:** "driven by a simple per-joint plant. Add MuJoCo when a test needs coupled dynamics (calibration C6, replay)."
- **Why:** None of M1's stop-rule tests needs it, and it adds a C dependency.

**[SHOULD] O3. PLAN §7: "command and reply at about 1 kHz"**
- **New:** "at the runtime's rate (250 Hz)."
- **Why:**
  - T1, T2, T3 and T5 are quasi-static.
  - T4 uses kernel timestamps.
  - T6 should match the rate the actuator model will run at.
  - A second loop rate adds code no other test needs.

**[SHOULD] O4. TENETS §8: the candidate list ("OpenTelemetry with Grafana (Loki, Prometheus or Tempo), Sentry…")**
- **New:** "Use off-the-shelf formats and viewers (MCAP, candump, Prometheus/Grafana, Foxglove). Add a tool only for a named need."
- **Why:** The list invites an evaluation project.

**[SHOULD] O5. PLAN, "Clean-up sweep" paragraph**
- Move it out of the build plan into "After milestone 2 (Joey, in the reference repo)".
- **Why:** It is work in a repo CLAUDE.md declares read-only.

**[NICE] O6. Crate table, "Replaces" column header**
- **New:** "Legacy code to mine for facts (never copy)".
- **Why:** Mapping new crates onto old ones (`api` ← Chappe/armee-proto) invites copying patterns.

**[NICE] O7. PLAN §6 table: tenet 3 compliance**
- Replace it with a table that has **Protects** and **Failure covered** columns, one row per rule in G4/G6/G8/V5. Example row: "Drive `canTimeout` | every drive and its limb | Pi crash, hang, killed process, lost bus → drive goes limp 50 ms after the last command".
- Move the VBUS and fault-bit rows from "Drive" to "Host preflight".
- **Why:** The current table doesn't meet tenet 3's own "names the hardware and the failure" rule.

---

## Q7. Questions only the owner can answer (each with a recommended default)

1. **CAN buses and IDs for the waist and legs.** *Default:* IDs unique across the robot, in blocks of ten. Right arm 1–10 on `can0`, left arm 11–20 on `can1` (existing ADR 0001 blocks), waist 21–30 on `can2`, left leg 31–40 on `can3`, right leg 41–50 on `can4`. Everything except the two arm blocks is my proposal.
2. **Right arm at M2: keep it on the MCP2515 HAT, or give it its own gs_usb adapter?** *Default:* its own adapter, bought after item 5 passes. Retire the HAT.
3. **Confirm left IDs 14 and 15.** *Default:* yes.
4. **Zero:** approve the test-based rule in G3. Is type 22 (save) ever allowed? *Default:* never.
5. **Does a client disconnect stop a running plan?** *Default:* no (tenet 5); the E-stop and the runtime's stop rules cover it.
6. **Who may `enable` and `clear_stop`?** *Default:* a human at the UI or CLI. Agents may plan, run on vcan, start approved plans, and stop.
7. **Real-time budget and soak duration.** *Default:* 0.5 ms start jitter, 99.9 % of replies before the deadline, 1 h soak (V3, V9).
8. **Camera-loop stages that close M1.** *Default:* C0–C2 attended plus one clean unattended C3.
9. **Test-stand rate.** *Default:* 250 Hz.
10. **Bus and ID for test-stand runs.** *Default:* the drive keeps its skeleton bus and ID, and the runner refuses if any other drive answers on that bus.
11. **Where calibration results and session logs live.** *Default:* summaries committed to this repo's `docs/calibrations/`; raw MCAP files stay on the Pi or storage, referenced by hash.
12. **Who deletes the legacy code and runs the clean-up sweep in the reference repo, and when.** *Default:* Joey, or an agent he assigns there, after M2 acceptance.
13. **Hand and neck in the skeleton?** *Default:* not until built; keep ID 16 reserved.
14. **Store each drive's UID in the skeleton?** *Default:* yes, after commissioning.
15. **Fault recovery.** *Default:* power-cycle only; no type-4 fault-clear.
16. **Battery VBUS window.** *Default:* 38–54.6 V, from STATUS (shutdown ≤ 38 V, full 54.6 V).
17. **Merging.** *Default:* Joey merges, CI must be green, and his review is required for `robstride`, `can`, `safety` and skeleton values.
18. **Run the legacy stack once for the RS00 capture on the right arm?** *Default:* yes, one candump capture with Joey present.
19. **Real fault tests at M1** (pull a drop fuse, unplug the adapter while enabled). *Default:* yes, attended, neutral MIT, loose drives only.