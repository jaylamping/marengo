Starting with PACKET.md, then sampling the repo against its fourteen questions.
## Reading scope (what this review did and did not do)

**Read in full or substantial parts:** `PACKET.md`, `repo/hardware/SOFTWARE_TENETS.md`, `repo/README.md`, `repo/docs/architecture.md`, `repo/docs/safety.md` (first ~200 lines), `repo/docs/status.md`, `repo/AGENTS.md` (architecture/contracts), `repo/config/codemap.md`, `repo/config/robot.yaml`, `repo/hardware/designs/left-arm/testing/SOFTWARE_PREP.md`, `repo/hardware/docs/sim-to-real-loop.md`, `repo/hardware/electrical/pdb/REV_B_PARTS.md` (software requirements), `repo/crates/{davout,berthier,robstride,chappe,marengo-store}/src/lib.rs`, `repo/crates/davout/src/total_torque.rs`, `repo/crates/davout/src/lib.rs` (`enable_targets_inner`, timing constants), `repo/crates/davout/codemap.md`, `repo/bins/marengo-pi/src/main.rs` (header + bus open ~1746), tool READMEs.

**Sampled via search/grep:** safety mechanisms across `davout`, enable/preflight paths, fault policy, config/URDF loading, speculative proto/ADR references, test layout counts.

**Not read:** Most of `davout` (~36k LOC), `berthier` (~22k LOC), `marengo-pi` body beyond `main.rs`, `consul/` (~35k LOC TS), `marengo-gateway` implementation, `tools/marengo-pi-mcp` bodies, full `proto/marengo/v1/marengo.proto`, RobStride vendor PDFs under `hardware/docs/robstride/`, full CALIBRATION_LOOP.md, CI workflow files, `.agents/skills/`, historical review folders under `docs/reviews/`.

---

## 1. Per-component verdict (left-arm lean stack)

| Component | Verdict | Primary tenet(s) | Evidence (verified in tree unless noted) |
|-----------|---------|------------------|------------------------------------------|
| `crates/davout` | **Rebuild** (lift safety + RobStride sequencing into smaller supervisor; delete virtual/journal paths not needed for left-arm bench) | 4, 5, 7 | ~36k LOC; `lib.rs` ADR 0036 journal/SQLite, `BurstPacer` MCP2515 pacing (`burst.rs`, `codemap.md` L91); enable path at `lib.rs:1612ΓÇô1680` has no param readback |
| `crates/berthier` | **Keep but slim** | 4, 7 | `lib.rs`: GravityComp, TorqueOnly, Position, impedance, wave, dual position laws (`scaled_pd`); ~22k LOC |
| `crates/robstride` | **Rebuild** (owner decision) | 5 | `lib.rs` boundaries clear; vcan tests exist; still owned inside `marengo-pi` tick |
| `crates/chappe` | **Keep but slim** | 5, 8 | `lib.rs`: pub/sub only; separate gateway process (`AGENTS.md` L27ΓÇô31) |
| `crates/marengo-config` | **Rebuild** ΓåÆ one skeleton file | 2 | `config/codemap.md`: four YAMLs + URDF; `motors_humanoid.yaml` test-only per `config/AGENTS.md` L13 |
| `crates/armee-kinematics` | **Keep** (merge limits into skeleton later) | 2 | Used from `marengo-config` `completeness.rs` `load_urdf` |
| `crates/armee-dynamics` | **Keep** | 6 | README/architecture: `gravity_torques(q)` |
| `crates/armee-proto` | **Keep but slim** | 4 | Deprecated Jetson fields `marengo.proto` ~235, 267, 441 (inferred: still on wire) |
| `crates/marengo-candump` | **Keep** | 5, 6, 8 | README role; left-arm item 2 soak |
| `crates/marengo-store` | **Delete** (after log export path) or **keep but slim** until MCAP/Foxglove | 8 | `lib.rs`: home-grown SQLite store |
| `crates/marengo-deploy` | **Keep** | 6 | Pi self-update |
| `crates/marengo-host-metrics` | **Keep** | 8 | Chappe metrics publisher in `marengo-pi` |
| `crates/marengo-imu` | **Delete** (left-arm phase) | 4 | `docs/status.md` IMU issues; no arm need |
| `crates/marengo-support` | **Keep** | 8 | tracing init |
| `crates/sim-harness` | **Keep** | 6 | URDF/MJCF parity |
| `bins/marengo-pi` | **Keep but slim** | 5, 7 | `main.rs:1746` `RuntimeBus::socketcan_from_motors`; 200 Hz loop + Chappe + CAN same process (`AGENTS.md` L45ΓÇô53) |
| `bins/motor-repl` | **Keep as-is** | 6, 7 | `AGENTS.md` L74: `protocol-inspect` has `INSPECTED_PARAMETERS` incl. CanTimeout (`davout/protocol_inspection.rs:29ΓÇô33`) |
| `bins/marengo-gateway` | **Keep** if Consul; else **defer/delete** for MCP-only bench | 4, 5 | Second process; UI dependency |
| `bins/marengo-log-cli` | **Keep but slim** | 6, 8 | gravity-fit, candump tooling |
| `bins/marengo-limit-sync` | **Keep** | 2 | Set Limits git sync |
| `bins/imu-probe` | **Delete** (left-arm phase) | 4 | README table |
| `consul/` | **Keep but slim** (commissioning) or **delete** until left arm needs Set Limits UI | 4, 7 | 35k LOC UI; SOFTWARE_PREP says MCP + motor-repl first |
| `tools/marengo-pi-mcp` | **Keep** | 6, 7 | SOFTWARE_PREP L11 |
| `tools/marengo-research-mcp` | **Delete** | 4 | ~2.4k LOC research helper |
| `tools/compound-auto-learn` | **Delete** | 4 | README: Cursor API BFF for Consul Auto Learn |
| `tools/limit-sync-local` | **Keep** | 2 | dev helper |
| `sim/` | **Keep** | 6 | MuJoCo smoke |
| `config/` (as-is) | **Rebuild** | 2 | `robot.yaml:23ΓÇô28` `left_elbow`/`left_wrist`; SOFTWARE_PREP L47ΓÇô48 mismatch |

---

## Ranked findings

### Blocker

1. **No drive-parameter preflight on Enable** (tenet 3; left-arm item 3). **Verified:** `enable_targets_inner` (`davout/src/lib.rs:1612ΓÇô1632`) checks grants, identity, disabled posesΓÇönot `INSPECTED_PARAMETERS`. CanTimeout read exists only on `inspect_drive_protocol` / `motor-repl protocol-inspect` (`protocol_inspection.rs:27ΓÇô33`, `AGENTS.md` L74). **Worst case:** power-cycled drive with wrong CanTimeout or run mode enables anyway. **Smallest fix:** call the same read set as `INSPECTED_PARAMETERS` before first Enable frame per drive; refuse enable on mismatch; cache until UID change or power cycle.

2. **Drive CanTimeout ~30 ms on bench, not Γëñ50 ms with read-back policy** (tenet 3). **Verified:** `docs/status.md` L47 (PR #254, CanTimeout=600 Γëê30 ms); `AGENTS.md` L176; tenet 3 / `REV_B_PARTS.md` L114 require Γëñ50 ms and read-back before every enable. **Worst case:** host jitter >30 ms stops drives mid-motion; opposite of PDB ΓÇ£hung PiΓÇ¥ story if timeout too short without coordination. **Smallest fix:** policy module: write Γëñ50 ms (or agreed counts), read-back in preflight, refuse enable; document interaction with host `comm_watchdog_ms: 100` (`config/control.yaml`).

3. **Power-board `WD_FEED` heartbeat not implemented** (tenet 3 hardware layer). **Verified:** requirement in `hardware/STATUS.md` L64, `REV_B_PARTS.md` L113; **no** `WD_FEED` in Rust crates (grep only docs + unbuilt homing GPIO stubs in `davout/reference.rs:404ΓÇô409`). **Worst case:** crashed/hung `marengo-pi` leaves motor power latched when rev B PDB is live. **Smallest fix:** GPIO toggle every 50ΓÇô100 ms from the realtime loop (same thread as CAN), fail-closed if GPIO unavailable on PDB systems.

4. **Tenet 2 violated: no single skeleton YAML** (left-arm config debt). **Verified:** `config/codemap.md` L7ΓÇô12 four files + `assets/urdf/marengo.urdf`; limits intersect URDF Γê⌐ `motors.yaml`; `robot.yaml:23ΓÇô28` wrong left joint names vs `SOFTWARE_PREP.md` L42ΓÇô48. **Worst case:** left arm commissioned on wrong bus/IDs/names. **Smallest fix:** one `skeleton.yaml` for the 10 arm joints; generate URDF for sim only; fix left names/IDs in one commit per STATUS.

5. **Tenet 5 violated: CAN not an isolated, crash-survivable realtime owner** (tenet 5). **Verified:** `marengo-pi/src/main.rs:1746ΓÇô1755` opens SocketCAN and runs 200 Hz control in one process; `AGENTS.md` L30ΓÇô31. **Worst case:** gateway/Consul/Chappe work or gravity preflight slices stall the same thread that must meet drive CanTimeout (~30 ms today). **Smallest fix:** separate `marengo-can` process owning all buses; control talks over one socket API; CAN rewrite target per owner decision.

6. **MCP2515 RX overrun cause B still open** (tenet 5 operational). **Inferred from `docs/status.md` L44ΓÇô45, not re-measured in this review). **Worst case:** `can1` left arm loses frames during DisableΓåÆRun transitions (`davout/codemap.md` Enable staggering for 2-frame FIFO). **Smallest fix:** USB `gs_usb` per bus per `SOFTWARE_PREP.md` L67ΓÇô69 before trusting `can1` on the HAT; soak via `marengo-candump`.

### Major

7. **`davout` + `berthier` mass vs tenets 4 and 7.** Reference journal, virtual owners, degraded hold, dual position laws, enable echo staggeringΓÇömuch is right-arm era complexity. **Smallest fix:** migration list from `physical_reference.rs`, `burst.rs`, `POST_SET_ZERO_*` (`lib.rs:193ΓÇô203`), `total_torque.rs`, `firmware_profile.rs` tests; delete virtual reference production paths on lean stack.

8. **Total torque and `limit_torque` (0x700B)** (tenet 3 gap, partially mitigated). **Verified:** `total_torque.rs:6ΓÇô8` never reads/writes 0x700B; host prediction can be wrong between ticks (`docs/safety.md` L16ΓÇô17). Calibration loop **spec** wants 0x700B written (`CALIBRATION_LOOP.md` L142, not verified implemented). **Smallest fix:** for calibration stages, write/read 0x700B in stage preflight; keep host clamp as backstop.

9. **HIL / simulated drive on vcan missing for item 4** (tenet 5). **Verified:** `robstride` vcan kernel tests; `SimulationBus` for unit tests (`marengo-pi` tests); **no** production-grade simulated RobStride on `vcan` for fault injection (tenet 5 bullet). **Smallest fix:** small sim firmware responding to MIT in `robstride` rewrite, run under `just vcan`.

10. **T1ΓÇôT6 test-stand and calibration-loop tooling not in repo** (tenet 6). **Inferred from `sim-to-real-loop.md` and `CALIBRATION_LOOP.md` (ΓÇ£nothing here is builtΓÇ¥ for circuit runner). Existing: commissioning docs, MCP motion suiteΓÇönot automated T2 luggage-scale runner.

11. **Observability stack is home-grown** (tenet 8). SQLite `marengo-store`, gateway log HTTP, Chappe telemetryΓÇönot OTel/MCAP. **Smallest fix:** log CAN + `RobotState` to MCAP from new CAN process; defer store deletion until parity.

12. **`docs/status.md` stale (2026-10-28)** vs PACKET date (tenet 7). Agents will wrong-foot left-arm bring-up.

13. **Process-local reference grants** (`AGENTS.md` L62ΓÇô63). **Worst case:** `motor-repl set-zero` then separate `marengo-pi` ΓåÆ enable refused. Documented, still wastes bench time. **Smallest fix:** lean stack: one long-lived `marengo-pi` for all bench motion (already MCP pattern).

### Minor

14. **Deprecated proto Jetson messages** still compiled (`marengo.proto`). Delete after `buf breaking` migration.

15. **`compound-auto-learn`, `marengo-research-mcp`**: not needed for left-arm hardware tests.

16. **Sensor/homing GPIO scaffolding** in config/Davout without hardware (`reference.rs:404ΓÇô409`; `docs/safety.md` sensor health deferred).

17. **Credential-free gateway disable** (`docs/safety.md` L195+)ΓÇöacceptable on bench LAN only; document firewall.

18. **Dual position law (`scaled_pd`)** only needed after pitch trial; left arm can run legacy law only until ADR 0039/0040 decided (`berthier/README.md` L12).

---

## 2. Topology

**Verified layout today** (`AGENTS.md` L26ΓÇô32, `docs/architecture.md` L18ΓÇô26):

```
Consul ΓöÇΓöÇHTTP/WebTransportΓöÇΓöÇΓû╢ marengo-gateway (tokio, Chappe listener /run/marengo/chappe.sock)
                                    Γû▓
stdin/MCP ΓöÇΓöÇΓû╢ marengo-pi (sync thread, 200 Hz)
                 Berthier ΓöÇΓû╢ Davout ΓöÇΓû╢ robstride ΓöÇΓû╢ SocketCAN (can0/can1) ΓöÇΓû╢ drives
```

Chappe also carries telemetry, enable, set-zero, testing MIT batches, limit patches (command topic allowlist `AGENTS.md` L43).

**Tenet 5 breaks:** CAN I/O and control share `marengo-pi`; UI path is two extra processes (gateway + Consul); protobuf pub/sub is not ΓÇ£one small interfaceΓÇ¥ for motion-critical path.

**Tenet 4 breaks:** Full Consul + gateway + SQLite logging + reference journal worker for a 5-DOF left-arm bench that SOFTWARE_PREP says can start with config + motor-repl + MCP.

**Smallest topology for one arm, scalable to 23 joints / 5 buses:**

| Process | Owns |
|---------|------|
| `marengo-can` | All SocketCAN/`gs_usb` ifaces; RobStride encode/decode; timestamped frame log; optional vcan sim backends |
| `marengo-ctrl` | Berthier + slim Davout; 200 Hz; stdin + one Unix IPC to CAN service (not full Chappe fan-out) |
| `marengo-gateway` + Consul | Optional; commissioning only |
| `motor-repl` | Bench maintenance when `marengo-ctrl` stopped; never fights socket ownership (`AGENTS.md` L34) |

Five buses = five CAN threads or one epoll multiplexer inside `marengo-can` only; config maps jointΓåÆbus name from skeleton YAML.

---

## 3. Safety inventory

| Mechanism | Layer | Class | Notes |
|-----------|-------|-------|-------|
| Physical E-stop in motor power | HW | (a) required | `docs/safety.md` L8; sense GPIO unwired |
| PDB TPS3431 watchdog | HW | (a) required when rev B | **Missing in code** (blocker #3) |
| Drive CanTimeout Γëñ50 ms + read-back | Drive | (a) required | **Wrong/missing policy** (blocker #2); inspection-only today |
| Drive torque/current/speed caps (firmware) | Drive | (a) | Not fully verified in this review |
| Anti-backdrive damper | Drive | (a) | Hardware doc; not verified in code |
| `require_fault_clear` / latched faults | Host | (b) cheap | `lib.rs:1271ΓÇô1280`; no `encode_fault_clear` in production (good vs tenet 3) |
| Physical reference grants (ADR 0036) | Host | (b) for sign/zero | `lib.rs` module docs |
| Gravity saturation preflight | Host | (b) | `marengo-pi/gravity_preflight.rs`, `AGENTS.md` L73 |
| URDF Γê⌐ bench limits, velocity caps | Host | (b) | Davout filter |
| Total MIT torque prediction clamp | Host | (b) partial | `total_torque.rs`; gap between ticks |
| `comm_watchdog_ms` (100 ms config) | Host | (b) | Not same as drive 50 ms timeout |
| Danger zones (`clamp_velocity`/`clamp_torque`) | Host | (b) | No fault on velocity-only in GravityComp (`docs/safety.md` CS14) |
| Measured position hard-limit fault | Host | (b) | ADR 0009 |
| Enable echo staggering / `BurstPacer` | Host | (b) for MCP2515 | (c) complexity if USB-CAN per bus |
| ADR 0038 shed_subtree on single drive loss | Host | (b) right-arm specific | Offline ╧ä_g admission |
| Reference journal SQLite | Host | (c) | Tenet 4; history Γëá motion grant per ADR |
| Virtual reference / simulation owners | Host | (c) for tests | Large test surface |
| Credential-free HTTP disable | Host | (b) stop | LAN exposure risk |
| Homing three-Hall sensors | Host | (c) unbuilt | Config stubs only |

**Hardware-required gaps:** enable preflight (#1), CanTimeout policy (#2), PDB heartbeat (#3), BMS ΓÇ£refuse enable on charge currentΓÇ¥ (`REV_B_PARTS.md` L13ΓÇöinferred no implementation in Rust grep).

---

## 4. Control modes

**Needed (verified against docs + code):**

| Work | Modes / laws |
|------|----------------|
| Sign/zero (C1, item 3) | TorqueOnly small pulses; reference workflow; GravityComp after sign |
| T1ΓÇôT6 stand | TorqueOnly, velocity/torque steps; MIT logging; likely Position for T1 ramps (**spec** in `sim-to-real-loop.md`; not automated in repo) |
| Calibration C0ΓÇôC8 | C1 sign/zero; C6 reuses gravity/friction suite; C7 raised speedΓÇöneeds Position or Impedance + caps in stage plan (**CALIBRATION_LOOP.md**; runner **not built**) |
| Left-arm initial bench | GravityComp, disable, hold-at/wave optional |

**Not needed on lean left-arm first pass:** `scaled_pd` position law (pitch trial only), ADR 0038 degraded auto-lower (no elevated shed tree until forearm powered), Impedance tuning UI, wave commissioning until playbook demands.

**Bench:** four `ControlMode` values in Berthier (`lib.rs` L10); OperationalMode Disabled/Ready/Active in Davout.

---

## 5. Config and URDF flow

**Verified:** `marengo-pi` loads four YAMLs (`main.rs` imports `load_*`); Davout builds limits from URDF + motors (`davout/lib.rs` L78ΓÇô79); Set Limits can expand URDF on disk (`docs/safety.md` L146ΓÇô156).

**Duplicate facts (tenet 2 violations):**

- Joint names: `robot.yaml` vs `motors.yaml` vs URDF vs `motors_humanoid.yaml` vs `left_arm.urdf` (**SOFTWARE_PREP** vs `robot.yaml:23ΓÇô28`).
- Hard limits: URDF, `motors.yaml` bench, postΓÇôSet Limits URDF union.
- Velocity/torque caps: `control.yaml`, `motors.yaml` types, URDF velocity (explicitly **not** command limit per `docs/safety.md` L14).
- Direction/zero: `motors.yaml`; also implied in URDF axis (runtime uses motors for transform per `davout/lib.rs` L14ΓÇô15).

**Target:** one skeleton YAML; URDF generated for sim/visualization only (tenet 2).

---

## 6. Hard-won knowledge to lift before deletion

| Topic | Where |
|-------|--------|
| Post-SetZero blackout / quiet windows | `davout/src/lib.rs` `POST_SET_ZERO_BLACKOUT_FROM`, `POST_SET_ZERO_QUIET` (~L193ΓÇô203); `davout/codemap.md` |
| MCP2515 2-frame FIFO pacing | `davout/src/burst.rs`, `active_reporting.rs`, `codemap.md` Enable stagger |
| Enable echo / type-24 Off ordering | `davout/codemap.md` L91ΓÇô93; `feedback_consumer.rs` `enable_echo_pending` |
| OWED_ON / host-caused silence | `reference_physical.rs`; `OWED_ON_WRITE_BOUND` exported `lib.rs` |
| Total torque clamp margins | `davout/src/total_torque.rs` L22ΓÇô29 |
| P2C-01 clipped velocity in clamp | `docs/safety.md` L17; `davout/tests/command_admission.rs` (named, not re-read) |
| RobStride MIT encode/decode, receive cap 64 frames | `robstride/src/lib.rs` L19ΓÇô20 |
| RX timestamp mapping | `robstride/src/rx_time.rs` |
| Firmware timing profile | `davout/tests/firmware_profile.rs`, `davout/tests/physical_firmware/` |
| Gravity preflight slice budget | `marengo-pi/src/gravity_preflight.rs`, `AGENTS.md` L73 |
| Motion-owner / stop-on-lag | `marengo-pi/motion_owner.rs`, `docs/safety.md` L172ΓÇô188 |

---

## 7. Deletion plan (after left arm passes on lean stack)

**Migrate first:** reference grant behavior, burst pacing (or USB-CAN removal), total torque clamp, gravity preflight, motor-repl/protocol-inspect or enable preflight, MCP bench workflows, candump tooling, deployment scripts, codemaps for remaining crates.

**Delete outright (if replaced):** bulk of current `davout` virtual/journal paths, `marengo-store` (after MCAP), `consul` features unused on bench, `compound-auto-learn`, `marengo-research-mcp`, `imu-probe`, `marengo-imu`, humanoid template YAMLs, deprecated proto fields, `.agents/skills` impeachable/SDD noise in repo copy, ADR-only Jetson crates (already removed per ADR 0014).

**Do not delete until CAN rewrite lands:** nothing that owns SocketCAN concurrently with motor-repl.

---

## 8. CAN layer (current path and rewrite)

**End-to-end (verified):** `ControlLoop::tick` ΓåÆ `Supervisor::send_mit_batch` ΓåÆ `robstride::MotorBus` ΓåÆ `RuntimeBus::socketcan_from_motors` (`marengo-pi/main.rs:1746`) ΓåÆ kernel SocketCAN ΓåÆ MCP2515 HAT.

**Pacing:** `BurstPacer` 2 ms spacing per interface group (`davout/codemap.md`); enable one target per period on echoing buses.

**Worst failures:**

- RX FIFO overrun after Disable (`status.md` cause B).
- Host stall > drive CanTimeout (~30 ms) ΓåÆ drives stop independently.
- `comm_watchdog_ms` 100 ms host stop slower than drive timeoutΓÇöordering surprises on partial bus loss.
- `motor-repl disable` bypasses Davout (`AGENTS.md` L34)ΓÇöcorrect for estop, breaks unified logging if not captured.

**Rewrite must reproduce:** MIT/lifecycle/param/identity framing; bounded recv; kernel timestamps; bus naming from config; frame logging; vcan test harness; DavoutΓÇÖs ordering constraints (SetZero quiet, Off before Enable) **or** hardware that removes MCP2515 FIFO issue.

**Drop:** embedding CAN in 200 Hz control process; duplicating pacing logic across Davout and driver; unlogged parameter writes scattered in history (centralize in CAN service).

---

## 9. Speculative scaffolding (tenet 4)

**Verified candidates for deletion:** deprecated Jetson proto (`marengo.proto`); `config/motors_humanoid.yaml`, `robot_humanoid.yaml` (test templates); `compound-auto-learn`; `marengo-research-mcp`; ADR 0014 Jetson pipeline text referencing removed bins; homing sensor overrides without hardware (`docs/reviews/2026-10-03-crate-audit/contracts.md` refsΓÇöinferred); Consul simulation route shell; improvable skills under `repo/.agents/` and `repo/.cursor/` (not runtime).

**Not verified as unused:** full Consul Testing panels, limit-persist coalescing (status.md G04ΓÇôG09).

---

## 10. Tests (tenet 6)

**Guard real failures (verified by name/location, not every assertion re-read):**

- `davout/tests/physical_reference.rs` (67 `#[test]` in crate grep)ΓÇöprotocol timing, grants.
- `davout/tests/command_admission.rs`, `drive_loss.rs`, `fault_authority.rs`.
- `robstride` receive bounds, vcan transport, RS03 velocity scale.
- `marengo-pi` gravity preflight, motion_owner, stop_path, enable_gate tests.
- `berthier/position_hold_tests/law_gates.rs`, `bench_replay.rs`ΓÇöposition law regressions.

**Likely coverage / archive weight:** `berthier/tests/cs24_*` includes `cs24_one_code_crawl.rs` marked ΓÇ£Archive-only, unexecuted preparationΓÇ¥ (L1ΓÇô2)ΓÇöaudit before trusting in lean CI.

**Missing tests for real upcoming failures:** enable-time CanTimeout mismatch; PDB heartbeat; left-arm `can1` soak; T1ΓÇôT6 automation; calibration loop gates.

---

## 11. Logging and observability (tenet 8)

**Verified:** `tracing` via `marengo-support`; Chappe `RobotState`/`SafetyState` at 25 Hz (`AGENTS.md` L49ΓÇô50); `marengo-store` SQLite sessions; gateway serves Consul logs; `marengo-candump` for CAN postmortems; host metrics on Chappe.

**Off-the-shelf replacement candidates:** MCAP + Foxglove for time-series; OpenTelemetry logs/metrics to Loki/Prometheus; keep `tracing` as emitter. Home-grown store/gateway query path is tenet 8 negative.

---

## 12. Agent onboarding (tenet 7)

**Strengths (verified):** `AGENTS.md`, per-crate `lib.rs` headers, `codemap.md` files, `docs/architecture.md` mermaid.

**Stale/wrong:** `docs/status.md` date; tenet 2 ΓÇ£single skeletonΓÇ¥ not reflected in `config/codemap.md`; README still ΓÇ£CAD drives softwareΓÇ¥ tone vs tenet 2 ΓÇ£runtime never reads CADΓÇ¥; `robot.yaml` left arm names; PACKET says read `hardware/SOFTWARE_TENETS.md` but software tenets live at `repo/hardware/SOFTWARE_TENETS.md` (PACKET L17 paths OK). **Davout/berthier size defeats ΓÇ£minutes to learn seams.ΓÇ¥**

---

## 13. Off-the-shelf replacements

| Tool | Replaces | Why better | Catch |
|------|----------|------------|-------|
| **SocketCAN `gs_usb` adapters** (e.g. CANable/candleLight firmware) | MCP2515 dual HAT | Larger RX buffers, per-bus isolation | Cost, cabling; still need good grounding |
| **can-utils** (`candump`, `cansniffer`) | Parts of custom candump UX | Standard tooling | Less RobStride decode unless you keep `marengo-candump` enrich |
| **MCAP + Foxglove** | `marengo-store` + Consul log viewer | Robotics-native replay | Migration effort |
| **OpenTelemetry + Grafana** | Ad-hoc Chappe metrics + SQLite | Standard ops stack | Pi resource budget |
| **Rerun** | Custom telemetry panels | Fast visual debug | Another UI |
| **MuJoCo** (already in `sim/`) | Partial internal sim | Industry standard | Not HIL for CAN |
| **pinocchio / kasa** (Rust) | Future whole-body dynamics (ADR 0005) | Proven RNEA | Not needed for 5-DOF arm yet |

No confident public ΓÇ£RobStride Rust driverΓÇ¥ found in this tree; owner rewrite from manuals matches tenet 5.

---

## 14. Left-arm bring-up failure modes / time sinks

1. Enabling on `can1` without preflight (#1, #2) after ID renumbering (`SOFTWARE_PREP` L62ΓÇô63).
2. Wrong joint names in `robot.yaml` blocking Ready aggregation (#4).
3. Assuming `protocol-inspect` once replaces **every** enable checkΓÇöit does not (#1).
4. `motor-repl set-zero` then new `marengo-pi` without `home ΓÇª sign-tested` in same process (`AGENTS.md` L62ΓÇô63).
5. Cause B on shared HAT while bringing `can1` online (#6).
6. Building full Consul/calibration loop before skeleton config and CAN soak (`SOFTWARE_PREP` ordering).
7. Treating **Online** badge as grant liveness (`docs/safety.md` L59ΓÇô62)ΓÇö5 s TTL vs `comm_watchdog_ms`.
8. Status doc saying bench green-ish while PACKET lists unverified open items (RX B, CS10, M02/M03).

---

### Inference summary

**Verified in code:** topology, enable path without param check, bus open site, fault latch without auto-clear frames, config multiplicity, `robot.yaml` left names, `comm_watchdog_ms`, total torque doc, SimulationBus for tests only.

**From docs only (not re-verified on bench):** CanTimeout=600 on drives, cause B measurements, calibration/T-test tooling absence, BMS enable refusal, exact CS10/CS14 behavior under load.
