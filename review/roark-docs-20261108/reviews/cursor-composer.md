Reading PACKET.md, then roark/ and marengo/, to answer the seven questions with grounded proposed edits.


## 1. Vagueness (testable “done” replacements)

### Must-fix

**(1.1)**  
(a) `roark/TENETS.md` §5, bullet “Everything else … talks to the realtime process through **one interface**”  
(b) Current: “through one interface”  
(c) Replace with: “through **one local IPC API** (Unix domain socket on the Pi, path from config). **Clients:** `ui/`, `mcp/`, `robot-cli` (non-realtime), and optional Jetson bridge later. **The realtime process alone** opens SocketCAN and transmits type-1/3/4/17/18 frames. **Done:** contract doc lists every request/response type; integration test proves a client crash does not stop the 250 Hz loop; `motor-repl`-class tools cannot open CAN while `runtime` is running (verified by test).”  
(d) Legacy `chappe` is Unix IPC only (`marengo/crates/chappe/README.md`); “one interface” is not implementable without transport and exclusivity.

**(1.2)**  
(a) `roark/TENETS.md` §3, drive layer “CAN host-loss timeout of **50 ms or less**”  
(b) Current sentence only  
(c) Replace with: “CAN host-loss timeout **`canTimeout` 0x7028 = 1000** (vendor: 20000 = 1 s; 1000 = 50 ms per `marengo/hardware/docs/research/can-stack-for-marengo.md` §RobStride). **Done:** read back matches before every enable; after any drive reset/power cycle; refusal if 0 (disabled) or &gt;1000.”  
(d) Without the parameter ID and numeric done-check, agents can set the wrong unit (CONFIRMED in can-stack + CALIBRATION_LOOP §9).

**(1.3)**  
(a) `roark/TENETS.md` §3, host layer “stop when a drive goes **silent**”  
(b) Current phrase  
(c) Replace with: “**Host silence rule (milestone 1+):** stop the affected limb after **3 consecutive missed type-2 replies** on a joint that was commanded this tick (≈12 ms at 250 Hz), never energise past **50 ms** without a valid reply, **no degraded hold** and no subtree shed (contrast `marengo/crates/davout/src/drive_loss.rs`). **Done:** vcan test injects silence; limb stops before sim drive’s `canTimeout` fires; no code path keeps MIT commands after miss threshold.”  
(d) RECONCILIATION names legacy degraded hold as tenet violation; PLAN item 6 states 3 misses but TENETS does not.

**(1.4)**  
(a) `roark/PLAN.md` §4, “driven by MuJoCo”  
(b) Current phrase  
(c) Replace with: “**Done (sim):** `sim-drive` reads joint targets from a **fixed shared-memory or socket feed** published at the control rate (document the feed in `sim-drive` README). MuJoCo (or a stub replaying logged torques) is **one producer** of that feed. **Acceptance:** with feed frozen, drives hold last state; with feed stopped, host silence rule trips; fault injection cases in PLAN §4 each have a scripted test that fails if the runtime does not stop.”  
(d) “MuJoCo” names a producer, not how the realtime binary is driven (`sim-to-real-loop.md` §5.5).

**(1.5)**  
(a) `roark/PLAN.md` each numbered item (1–10)  
(b) No per-item acceptance  
(c) Add after §“Rules that hold for every item” a subsection **“Acceptance per plan item”**, e.g. “**Item 3 done:** 250 Hz on `vcan1` with 5 emulated IDs 11–15, MCAP records every frame, measured p99 tick jitter &lt; 4 ms on Pi 5 OR documented fallback to PREEMPT_RT; ENOBUFS on TX does not panic; bus-off stops all limbs. **Item 5 done:** real `can1`, scan/set-ID, soak log with zero sustained bus-off during motion, latency/jitter log archived. **Item 6 done:** preflight binary refuses enable when any of 0x7028/0x700B/0x7018/0x7017/damper mismatch; vcan fault-bit test stops without auto-retry. **Item 9 done:** CALIBRATION_LOOP §8 gates pass on `vcan` then attended C1 on hardware.”  
(d) Milestones table says “pass their tests” without defining tests (`PACKET.md` Q2 examples).

### Should-fix

**(1.6)**  
(a) `roark/TENETS.md` §2, “direction, zero”  
(b) Listed without convention  
(c) Add bullet: “**`direction`:** ±1 multiplier from skeleton joint angle (rad, right-hand rule about joint axis) to drive MIT position. **`zero_offset_rad`:** added to commanded angle before encode; Set Zero on the drive defines mechanical reference per RECONCILIATION enable workflow. **Done:** sign-test script (small torque pulse) writes proposed `direction` diff for hardware commit; no motion enable until sign verified (PLAN item 6 Host Sign).”  
(d) PLAN item 1 says directions are placeholders; no single convention (`STATUS.md` joint table, `kinematics.md` sign notes).

**(1.7)**  
(a) `roark/TENETS.md` §42–43, damper wording  
(b) “anti-backdrive damper left on”  
(c) “anti-backdrive damper at vendor default **`0x702A = 0` (on)**; never write 1 (PLAN item 6; CALIBRATION_LOOP §9). **Done:** preflight read-only check returns 0.”  
(d) PLAN “must read 0” reads like “off” without vendor semantics (`can-stack-for-marengo.md`).

**(1.8)**  
(a) `roark/PLAN.md` §Observability, last sentence  
(b) “still to be researched”  
(c) “**Milestone 1 minimum:** MCAP from runtime + `candump -L` per bus + Prometheus scrape of miss/latency/bus-off counters (`can-stack-for-marengo.md`). Foxglove/Sentry/OpenTelemetry are optional later. **Done:** one recorded session opens in Foxglove or PlotJuggler and reproduces a injected vcan fault.”  
(d) Tenet 8 forbids blocking on an open-ended observability study.

**(1.9)**  
(a) `roark/TENETS.md` §7, “One short architecture map”  
(b) No path or update rule  
(c) “**`docs/architecture.md`** (≤1 page): process diagram (runtime, sim-drive, clients), CAN ownership, skeleton load path, log directories. **Done:** updated in the same PR as any seam move; CI fails if crates listed in map are missing from workspace.”  
(d) Agents cannot find the map (PACKET Q2).

### Nice-to-have

**(1.10)**  
(a) `roark/TENETS.md` §1, “shows as offline”  
(b) Vague  
(c) “**Offline:** no type-2 reply for that ID in the last **1 s** at rest scan, or N consecutive misses while enabled (same N as host silence rule). UI shows `offline`, runtime does not command that joint.”  
(d) Enables consistent UI/skeleton semantics.

---

## 2. Gaps (missing decisions → concrete text)

### Must-fix

**(2.1) Skeleton schema**  
(a) `roark/PLAN.md` §1  
(b) Field list without types/units/bus map  
(c) Add **“Skeleton schema (normative)”**:

```yaml
# config/skeleton.yaml — schema version 1
robot:
  name: string
  control_hz: 250  # u32; change only with measured bus load
buses:
  can0: { limb: right_arm, host_id: 0xFD }   # TBD can2–can4 until hardware ADR
  can1: { limb: left_arm, host_id: 0xFD }
  # can2: waist, can3: left_leg, can4: right_leg — OPEN; see owner Q1
limbs:
  left_arm:
    joints:
      - name: left_shoulder_pitch
        actuator: RS03
        bus: can1
        device_id: 11
        axis: [0, 1, 0]          # unit vector, parent frame
        direction: 1             # ±1, placeholder until sign test
        zero_offset_rad: 0.0     # host-side offset; drive Set Zero separate
        hard_limit_rad: [-0.87, 3.14]
        soft_limit_rad: null     # runtime derives: supervised 2–3° inside hard
        limit_torque_nm: 60
        limit_cur_a: …           # consistent with torque cap
        limit_spd_rad_s: …       # from manual / stage
joints inherit actuator MIT scales from actuator type table (RS00 ±14 N·m, ±33 rad/s per can-stack table until RS00 capture settles PLAN §2 open fact).
```

**Done:** loader rejects duplicate IDs, unknown buses, soft outside hard, and offline limbs (all joints on unbuilt limb still listed). Source: `marengo/hardware/STATUS.md` table, `can_topology.md` (full bus plan open).

**(2.2) Full-body CAN bus assignment**  
(a) `roark/PLAN.md` §1 and milestones  
(b) Only arms’ buses specified  
(c) Add: “**Bus assignment (2026-11-08 confirmed):** `can0` right arm IDs 1–5; `can1` left arm 11–15 (`STATUS.md`, `left-shoulder-bench-harness.md`). **Open before milestone 3:** map waist and legs to `can2`–`can4` and ID blocks; `config/motors_humanoid.yaml` and `hardware/docs/decisions/0001-can-and-motors.md` **do not** override live bench arms. New skeleton lists unbuilt limbs with `bus: TBD` until hardware commits.”  
(d) `can_topology.md` and ADR 0001 say full-body plan is unsettled; milestone 5 assumes five buses.

**(2.3) Drive enable/disable/stop + host crash**  
(a) `roark/PLAN.md` §3 or §6  
(b) Only in item 2 lift list  
(c) Add **“Drive session state machine (normative)”**:

- **Preflight (every enable):** active reporting off (type 24) and verify echo; write/read 0x7028, 0x700B, 0x7018, 0x7017, damper read-only; VBUS 22–26 V bench (`CALIBRATION_LOOP §9`, RECONCILIATION).
- **Enable:** type 3 Enable → wait for mode Run (type-2 bits 22–23 = 2); then MIT type-1 each tick.
- **Stop (normal):** neutral MIT → type 4 Disable (skip inert SpeedTarget in MIT per DeepSeek review in `sw-foundation` reviews).
- **Stop (fault/silence/bus-off):** Disable immediately on affected limb; whole robot on bus-off (`PLAN item 6`).
- **Process exit/crash:** `runtime` `ExecStopPost` sends type-4 Disable per address (`marengo/scripts/systemd/marengo-pi.service` pattern); **disable before any log flush** (RECONCILIATION blocker 5).
- **Post–Set Zero:** no Enable for **800 ms** per drive (`RECONCILIATION`, `robstride-firmware-behavior.md`).
- **Exit hook:** force active reporting off; it **persists across processes** (RECONCILIATION).

**Done:** vcan tests for each transition; power-cycle test re-runs preflight.

**(2.4) Realtime API message set (minimal milestone 1)**  
(a) New subsection under `roark/PLAN.md` crate table for `api`  
(b) “one interface” only  
(c) “**Milestone 1 API (Unix socket):** `GetSnapshot` (joint pos/vel/torque/temp, age, faults), `SubscribeSnapshot` stream, `StopAll`, `EnableLimb {limb, mode}`, `DisableLimb`, `RunPreflight {limb}`, `StartMcapSession`, `Health`. No client may send MIT frames. **Done:** MCP and `robot-cli` use only this socket; grep proves no `socketcan::CanSocket` outside `can` crate + `runtime` + `robot-cli` maintenance subcommands.”  
(d) Multi-writer CAN was a legacy blocker (RECONCILIATION major).

**(2.5) Repo layout, toolchain, CI, Pi deploy**  
(a) `roark/PLAN.md` after “Names and crate layout”  
(b) Missing  
(c) Add: “**Repo bootstrap:** Rust workspace (edition from new repo `rust-toolchain.toml` when created). **CI:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` (unit + vcan integration where no hardware). **Pi deploy:** cross-build `runtime` for `aarch64-unknown-linux-gnu`; install to `/opt/roark/bin/runtime`; systemd unit with `WatchdogSec=`, `ExecStopPost=/opt/roark/bin/robot-cli disable`; deploy script lives in repo (replaces deleted `marengo-deploy` per RECONCILIATION). **Done:** PR CI green; doc command flashes one Pi and runs vcan acceptance.”  
(d) PACKET Q2 explicitly lists this; no roark doc covers it.

**(2.6) Logging layout**  
(a) `roark/PLAN.md` Observability  
(b) MCAP mentioned, no paths  
(c) “**Log layout:** `/var/log/roark/sessions/<UTC-start>/session.mcap`; parallel `/var/log/roark/candump/<bus>.log` from systemd `candump -L`; metrics on `:9090/metrics`. Kernel timestamps on every MCAP record. **Done:** session from item 3 acceptance is replayable.”  
(d) Tenet 6/8 require session replay (`sim-to-real-loop.md` §4).

**(2.7) Right-arm migration**  
(a) `roark/PLAN.md` milestone 2 / “Before the old stack is deleted”  
(b) One line  
(c) Expand milestone 2: “(1) Skeleton lists right arm on `can0` IDs 1–5 with taught limits from legacy config lift. (2) Runtime commands both buses with **one** control thread. (3) Run right-arm smoke on new stack (no gravity until signs verified). (4) Switch systemd unit from `marengo-pi` to `runtime`. (5) Delete legacy crates per RECONCILIATION table. **Done:** right arm passes same preflight + 60 s soak as left; no `marengo-*` binaries on PATH.”  
(d) RECONCILIATION: deleting before skeleton strands right arm.

**(2.8) Item 8 before powered motion**  
(a) `roark/PLAN.md` §“Start here”  
(b) Steps 1–6 then 7, 9; skips 8  
(c) Insert step: “**5b.** Implement item **8** limits in skeleton + host/drive caps before **any** powered motion on printed upper arm (hardware table: item 8 before under power). Elbow dummy-output test (`ELBOW_DUMMY_OUTPUT_TEST.md`) gates forearm power.”  
(d) Hardware table vs Start here mismatch (`PLAN.md` lines 52–57 vs 38–43).

### Should-fix

**(2.9) Camera host**  
(a) `roark/PLAN.md` item 9  
(b) Silent on host  
(c) “Camera service runs on the **Pi 5 with BRIO** (`CALIBRATION_LOOP.md` §13 q2); timestamps aligned to CAN clock via chrony/PTP per `sim-to-real-loop.md` §4.”  
(d) CALIBRATION §13 assumes Pi+can1+BRIO.

**(2.10) Worst-case matrix**  
(a) `roark/PLAN.md` §6 or new §“Failure modes”  
(b) Scattered  
(c) Table: Pi SIGKILL → `canTimeout` then limp unless `ExecStopPost` disable; hung runtime → watchdog drop; USB detach → bus-off → stop all; drive power cycle → mode reset → preflight required; MCP client crash → no effect on CAN. **Done:** each row has vcan or bench test id.”  
(d) PACKET asks for fault/crash/bus-loss cases.

### Nice-to-have

**(2.11) Hand ID 16**  
(a) `roark/PLAN.md` §1  
(b) Not mentioned  
(c) “Reserve device_id **16** on `can1`; no joint until hand exists (`STATUS.md`, `left-arm-trunk/README.md`).”  
(d) Prevents ID collision when hand lands.

---

## 3. Contradictions (winner + fix)

### Must-fix

**(3.1) Control rate 200 vs 250 Hz**  
(a) `roark/PLAN.md` §3 vs `marengo/config/control.yaml` (`loop_hz: 200`) and `compute-plan.md` (“200 Hz across the whole body”)  
(b) 250 vs 200  
(c) PLAN: add note “**250 Hz is the new-stack default** (`can-stack-for-marengo.md`, owner PACKET). Legacy 200 Hz is reference only.” Optionally add `control_hz` in skeleton default 250.  
(d) **PLAN + can-stack win** over legacy config.

**(3.2) Left joint names**  
(a) `PLAN.md` §1 names vs `kinematics.md` `left_elbow` / `left_wrist`  
(b) Conflicting names  
(c) Skeleton and all roark docs use `left_elbow_pitch`, `left_lower_arm_yaw` only; add “Do not use `left_elbow`/`left_wrist` (`STATUS.md` mismatches).”  
(d) **STATUS.md / PLAN win** over stale kinematics rows.

**(3.3) Sim-to-real HIL still names Davout**  
(a) `marengo/hardware/docs/sim-to-real-loop.md` §5.5 (referenced by PLAN item 7)  
(b) “real Pi binary, Davout included”  
(c) In `roark/PLAN.md` item 7: “HIL = `runtime` + `safety` + `can`; legacy Davout is not a dependency (RECONCILIATION).”  
(d) New stack deletes Davout; hardware spec lags.

**(3.4) Elbow limit numbers**  
(a) `STATUS.md` elbow 0…2.164 rad vs `kinematics.md` humanoid table 0…2.5 for `left_elbow`  
(b) Different upper limits  
(c) Skeleton hard limit **0.0…2.164 rad (124°)** per left-arm URDF/STATUS; not 2.5.  
(d) **STATUS.md / left_arm.urdf win** for milestone 1.

**(3.5) `motors_humanoid.yaml` bus/id**  
(a) PLAN vs template  
(b) can3 left arm 14–18  
(c) Already partially stated; strengthen: “Skeleton must **not** copy `motors_humanoid.yaml` bus map (`STATUS.md`, ADR 0001).”  
(d) Verified conflict in STATUS.

### Should-fix

**(3.6) README joint count**  
(a) `roark/README.md`  
(b) “five CAN buses” only  
(c) Add “**23 actuated joints** (waist 1, legs 12, arms 10); see `marengo/hardware/docs/kinematics.md` humanoid table (left arm rows stale—use PLAN skeleton names).”  
(d) PACKET.md states 23 joints.

**(3.7) TENETS damper vs CALIBRATION “stays on”**  
(a) Cross-doc  
(b) Wording  
(c) Align per (1.7).  
(d) Same semantic, different phrasing.

### Nice-to-have

**(3.8) CLAUDE.md “Milestone 1 is the left arm”**  
(a) `roark/CLAUDE.md`  
(b) Could imply limb-scoped code  
(c) “Milestone 1 **tests** the left arm first; code scope is still whole-robot (tenet 0).”  
(d) Reinforces tenet 0.

---

## 4. Facts (PLAN vs reference; wrong or unverified)

| Item | PLAN claim | Reference | Verdict |
|------|------------|-----------|---------|
| Left arm bus/IDs | `can1`, 11–15 | `marengo/hardware/STATUS.md` | **Correct** |
| Right arm | `can0`, 1–5 | STATUS, `can_topology.md` | **Correct** |
| `canTimeout` 1000 = 50 ms | item 6 | `can-stack-for-marengo.md`, CALIBRATION §9 | **Correct** |
| RS03 MIT ±20 rad/s | item 2 | `motor_type.rs`, manuals cited therein | **Correct** |
| RS00 MIT scales | “manual ±33/±14” | `motor_type.rs` still uses ±50/±17; PLAN flags open | **Correctly open**; do not ship ±50/±17 in new codec until capture |
| Stall caps RS03 13, RS02 6 N·m | item 7 | `sim-to-real-loop.md` §2.3 | **Correct** |
| Elbow ≤17 N·m | item 8 | `ELBOW_DUMMY_OUTPUT_TEST.md` | **Correct** |
| Old left IDs | 12 on `can1`, 11–14 on `can0` | `left-shoulder-bench-harness.md` | **Correct** |
| Five limbs / five buses | throughout | compute-plan, STATUS | **Correct** for target; **leg/waist bus indices unassigned** (not wrong, incomplete) |
| Damper RS03 also **0x2026** | item 6 table | **Not verified** in files read (no robstride `.md` text in snapshot) | Mark **unverified** or cite manual section in PLAN |
| Host 3 missed replies | item 6 | Design choice, not vendor | **Supported as policy**, not vendor fact |
| 250 Hz default | item 3 | can-stack, BHL citation therein | **Correct** as target; must be measured per can-stack conclusion |

**Proposed edit (must-fix):** `PLAN.md` §2 RS00 bullet—add “**New codec defaults to vendor ±33 / ±14** until bench capture; refuse motion on RS00 if capture disagrees.” Ground: `motor_type.rs` comment + can-stack table.

**Proposed edit (should-fix):** `PLAN.md` §1 elbow ID 14/15—match STATUS “(proposal)” until harness committed.

---

## 5. Order (“Start here” and milestones)

### Must-fix

**(5.1)** Merge safety with CAN before bench power: reorder Start here to: tenets → skeleton schema → **item 2 knowledge extract in parallel with robstride codec tests** → **items 3+4+6 together on vcan1** (preflight is part of CAN bring-up per RECONCILIATION blocker 1) → item 5 adapter/bench → **item 8** → item 7 (stand can wait on prints) → item 9.  
**Why:** Drive safety layer missing was #1 blocker; `can-stack-for-marengo.md` puts codec/sim before real wire but PLAN already has vcan—adding 6 before 5 matches “prove stop rules on vcan first” (PLAN §4).

**(5.2)** Milestone order OK (left → right → waist → power → legs) but add note: **milestone 4 software (item 10) can be implemented on sim/mock I²C before fab** (PLAN already says; tie to milestone table “Proves”).

### Should-fix

**(5.3)** Item 2 before deleting anything should be **step 2**, not after skeleton only—RECONCILIATION “lift before delete” is parallel to codec, not after layout.

### Nice-to-have

**(5.4)** First agent reads `marengo/hardware/STATUS.md` + RECONCILIATION blockers in same sitting as tenets (add to `CLAUDE.md` step 2.5).

---

## 6. Over-specification (trim or defer)

### Should-fix

**(6.1)** `PLAN.md` crate name table (~10 crates)—add: “Table is **guidance**; split/merge allowed if names stay tenet-7 clear and acceptance tests pass (tenet 4).”  
(d) Avoids sacred cow crates.

**(6.2)** “Clean-up sweep” of all marengo `.cursor` skills—move to milestone 2 post-cutover only; not milestone 1 agent work (tenet 4).

### Nice-to-have

**(6.3)** Prescribing `socketcan` 4.0 + `rtrb` + exact thread model—keep (owner PACKET decided CAN stack); do not add more crates (e.g. OpenTelemetry) until milestone 1 MCAP path works.

---

## 7. Owner-only questions (recommended default)

1. **Waist + leg SocketCAN names and ID blocks (`can2`–`can4`)** — Default: keep arms as `can0`/`can1`; assign `can2=waist` (1×RS03), `can3=left_leg`, `can4=right_leg` with IDs per `kinematics.md` joint table once leg ADR lands; skeleton uses `TBD` until then (`can_topology.md`, ADR 0001).

2. **RS00 MIT velocity/torque scales** — Default: **vendor ±33 rad/s, ±14 N·m** in codec until one loose RS00 capture (`motor_type.rs`, PLAN §2).

3. **Control loop Hz** — Default: **250 Hz** skeleton field; fall back only with logged bus load/jitter (`can-stack-for-marengo.md`).

4. **API transport** — Default: **Unix domain socket on Pi**, same pattern as legacy chappe (`chappe/README.md`); no Jetson bridge in milestone 1.

5. **Observability stack beyond MCAP** — Default: **Prometheus + Grafana** for counters; defer Sentry/OTel (`PLAN` observability).

6. **Left joint `direction` placeholders** — Default: **+1** with mandatory sign test before enable (`PLAN item 6 Host Sign`).

7. **Provisional CAN IDs 14–15** — Default: commit **14=elbow, 15=forearm yaw** in skeleton; change only via hardware STATUS commit (`STATUS.md` table).

8. **Full-body `motors_humanoid.yaml` retirement** — Default: **never import**; skeleton only (RECONCILIATION).

9. **PREEMPT_RT on Pi 5** — Default: **stock kernel until** 250 Hz acceptance fails jitter budget (`can-stack-for-marengo.md`).

10. **Camera on Pi vs desktop** — Default: **Pi + BRIO** per CALIBRATION §13.

---

## Cross-cutting must-fix (crash / bus / power cycle)

**(W.1)** `PLAN.md` §3 — add bullet: “**Shutdown order:** Disable all drives **before** closing MCAP or flushing disks (RECONCILIATION blocker 5; legacy `marengo-pi` fixed stop-first—carry rule forward).”

**(W.2)** `PLAN.md` §6 Host Hang — specify: “`WatchdogSec` ≤ **2×** control period; missed feed → `SIGABRT` → `ExecStopPost` disable (RECONCILIATION blocker 2: no hung host with last MIT command).”

**(W.3)** `PLAN.md` §3 — “**Active reporting:** forced Off at runtime start and **again** on exit (persists on drives, RECONCILIATION).”

**(W.4)** Explicit **forbid** reintroducing `drive_loss` / degraded hold (RECONCILIATION major; `davout/src/drive_loss.rs`).

---

### Files read (grounding)

`PACKET.md`, `roark/TENETS.md`, `roark/PLAN.md`, `roark/CLAUDE.md`, `roark/README.md`, `marengo/hardware/STATUS.md`, `marengo/hardware/docs/research/can-stack-for-marengo.md`, `marengo/review/sw-foundation-20261108/RECONCILIATION.md`, `marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md`, `marengo/hardware/docs/sim-to-real-loop.md`, `marengo/hardware/docs/compute-plan.md`, `marengo/hardware/docs/kinematics.md`, `marengo/hardware/electrical/wiring/can_topology.md`, `marengo/hardware/electrical/wiring/left-shoulder-bench-harness.md`, `marengo/crates/robstride/src/motor_type.rs`, `marengo/crates/chappe/README.md`, `marengo/config/control.yaml`, `marengo/scripts/systemd/marengo-pi.service`, `marengo/hardware/designs/left-arm/testing/ELBOW_DUMMY_OUTPUT_TEST.md`, `marengo/hardware/docs/decisions/0001-can-and-motors.md`.

**Unverified in this pass:** RS03 damper register `0x2026`; exact RobStride stall N·m for RS00/RS04 (PLAN says “check”—sim-to-real only partially tables them).