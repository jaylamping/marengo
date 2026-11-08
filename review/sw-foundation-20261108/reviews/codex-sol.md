Review of snapshot `614371d3ae804eb16693f05d5b94fcd1af3b2457`.

**The current stack should not be carried over unchanged to powered left-arm testing.** The owner’s concern about excess machinery is supported, particularly in reference authority, configuration persistence, telemetry storage, and tool-driven process ownership. However, several complicated CAN paths encode actual bench failures; deleting them without extracting their contracts would repeat those failures.

This was a static, read-only review. I ran text searches and PDF text extraction only—no builds, tests, repository scripts, or hardware operations. “Verified” below means verified by reading code, not demonstrated on a drive.

**1. Per-component verdict**

“Delete” means exclude from the lean robot stack and remove the old implementation after its necessary evidence or functionality has migrated. These are component-level decisions based on the sampling described under question 12, not exhaustive certifications.

| Component | Verdict | Tenets | Evidence and disposition |
|---|---|---|---|
| `crates/robstride` | **Rebuild** | 3, 5 | Mandatory under T5. Extract protocol contracts from [mit.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/mit.rs), timing lessons and receive semantics; resolve disputed scales in [motor_type.rs:26](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/motor_type.rs:26). |
| `crates/davout` | **Rebuild** | 1–5 | Replace the supervisor/reference/journal/degraded-mode machinery with the fixed host checks. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs), [reference_transaction.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/reference_transaction.rs), [drive_loss.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/drive_loss.rs). |
| `crates/berthier` | **Keep but slim** | 3, 4, 6 | Retain trajectory, torque/position composition and measured-progress contracts. Remove degraded hold and competing position-law lifecycle machinery after choosing and measuring one law. [src/codemap.md](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/codemap.md), [position_law.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/position_law.rs). |
| `crates/marengo-config` | **Rebuild** | 1, 2 | Four YAMLs, runtime URDF, subsets, commissioning scope and multi-file write-behind conflict with the skeleton contract. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-config/src/lib.rs), [profile_txn.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-config/src/profile_txn.rs). |
| `crates/armee-dynamics` | **Keep but slim** | 2, 4, 6 | Preserve gravity/calibration mathematics and independent oracles; replace runtime URDF ingestion with parameters in the skeleton. Remove subtree-loss analysis. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/armee-dynamics/src/lib.rs), [drive_loss.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/armee-dynamics/src/drive_loss.rs). |
| `crates/armee-kinematics` | **Keep but slim** | 2–4 | Keep finite-bound validation and necessary geometry helpers. Remove runtime URDF authority and expand-only hard-limit editing. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/armee-kinematics/src/lib.rs), [expand.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/armee-kinematics/src/expand.rs). |
| `crates/armee-proto` | **Keep but slim** | 4, 7 | A generated wire contract remains useful. Prune unused/deprecated interfaces during the coordinated cutover. [marengo.proto](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/proto/marengo/v1/marengo.proto). |
| `crates/chappe` | **Rebuild** | 4, 5, 8 | Replace general topic fanout and home-grown log transport with one small runtime interface and separate recording. Preserve bounded queues and explicit loss counters. [ipc.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/chappe/src/ipc.rs), [ipc_outbox.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/chappe/src/ipc_outbox.rs). |
| `crates/marengo-store` | **Delete** | 4, 8 | SQLite log/session/archive/migration/recovery infrastructure should not survive merely to support its own UI. Export evidence first. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-store/src/lib.rs), [src/codemap.md](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-store/src/codemap.md). |
| `crates/marengo-candump` | **Keep but slim** | 5, 6 | Retain offline RobStride enrichment, capture chronology and firmware timing support. Reassess paging/archive features when Store disappears. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-candump/src/lib.rs). |
| `crates/marengo-deploy` | **Delete** | 4 | Self-update jobs, upstream polling and version-status machinery are not needed to run these tests. Use an explicit stopped deployment procedure. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-deploy/src/lib.rs). |
| `crates/marengo-host-metrics` | **Delete** | 4, 8 | Replace generic host sampling with an exporter; retain domain-specific loop/CAN metrics in the runtime. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-host-metrics/src/lib.rs). |
| `crates/marengo-imu` | **Delete from this stack** | 4 | No listed arm test needs torso orientation. Its fresh-sample handling should not be described as currently broken; see question 12. [driver.rs:108](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-imu/src/driver.rs:108). |
| `crates/marengo-support` | **Keep as-is for offline CLIs** | 4, 8 | Small wrapper around an existing tracing library. It is not the runtime recorder. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-support/src/lib.rs). |
| `crates/sim-harness` | **Rebuild** | 5, 6 | Existing XML parity tests are not the required unchanged-binary `vcan` HIL system. Retain necessary parity assertions. [lib.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/sim-harness/src/lib.rs). |
| `bins/marengo-pi` | **Rebuild** | 2–5, 8 | Keep one owner process, but remove shell grammar as the automation protocol, synchronous output and duplicated dispatch/gates. [main.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-pi/src/main.rs). |
| `bins/marengo-gateway` | **Keep but slim** | 4, 5, 8 | Optional HTTP/static adapter to the one runtime interface. Remove config authority, URDF activation, deployment and log-store ownership. [http.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-gateway/src/http.rs). |
| `bins/motor-repl` | **Rebuild** | 3, 5 | Make normal diagnostics/reference clients of the owner. Preserve a minimal independent stop fallback until hardware timeout/watchdog acceptance exists. [stop.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/motor-repl/src/stop.rs). |
| `bins/marengo-log-cli` | **Keep but slim** | 6, 8 | Preserve `firmware-timing`, gravity fitting and capture analysis; remove Store administration after export. [src/codemap.md](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-log-cli/src/codemap.md). |
| `bins/marengo-limit-sync` | **Delete** | 2, 4 | Exists to mirror a distributed, multi-file source of truth that should disappear. [main.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-limit-sync/src/main.rs). |
| `bins/imu-probe` | **Delete from this stack** | 4 | No current arm-test dependency. [main.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/imu-probe/src/main.rs). |
| `tools/marengo-pi-mcp` | **Rebuild** | 1, 4–6 | Retain test plans/scoring, replace SSH-generated sessions and service takeover with runtime API calls; derive joints from the skeleton. [bench-profiles.ts](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/tools/marengo-pi-mcp/src/bench-profiles.ts), [can-owner.ts](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/tools/marengo-pi-mcp/src/can-owner.ts). |
| `tools/marengo-research-mcp` | **Delete from robot software** | 4 | General research tooling, not an arm-test dependency. Move elsewhere if wanted. [README.md](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/tools/marengo-research-mcp/README.md). |
| `tools/compound-auto-learn` | **Delete** | 4, 6 | Agent-generated teach landmarks are not the specified measured camera calibration/scoring loop. [README.md](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/tools/compound-auto-learn/README.md), [agent.ts](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/tools/compound-auto-learn/src/agent.ts). |
| `tools/limit-sync-local` | **Delete** | 2, 4 | Laptop HTTP service exists to maintain the second limits copy. [README.md](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/tools/limit-sync-local/README.md). |
| `tools/cad/check_cad_layout.py` | **Keep as-is, hardware tooling only** | 2, 7 | Checks CAD directory conventions; should remain outside runtime dependencies. [source](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/tools/cad/check_cad_layout.py). |
| `consul` | **Keep but slim** | 1, 2, 4, 8 | Keep online/offline, faults, enable/stop, stage control and results. Remove static robot facts, local limit mirroring, Auto Learn and browser-owned test execution. [robot-inventory.ts](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/consul/src/data/robot-inventory.ts), [use-compound-playback.ts](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/consul/src/hooks/use-compound-playback.ts). |
| `sim/` | **Rebuild** | 5, 6 | Current smoke advances MuJoCo without running the production CAN/controller path. [smoke_test.py](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/sim/scripts/smoke_test.py). |
| Supporting `scripts/` | **Keep but slim** | 4–6 | Preserve deployment, capture and bench-measurement essentials; regenerate bus setup from skeleton and remove obsolete workflows. [can-up.sh](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/scripts/can-up.sh), [check-sim.sh](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/scripts/check-sim.sh). |

**Ranked findings**

Severity is relative to the intended test: “blocker—unattended” does not prohibit unpowered C0 work.

| Rank | Finding, evidence and worst case | Smallest fix |
|---|---|---|
| **Blocker—powered motion** | **Required drive preflight does not exist.** Enable verifies identity/reference and pose, then schedules Enable; it does not write/read timeout, torque/current/speed caps or damper. The parameter enum lacks current limit, damper and VBUS. Standalone inspection reads timeout but not the complete protection set. Verified: [davout/lib.rs:1612](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:1612), [params.rs:23](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/params.rs:23), [protocol_inspection.rs:29](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/protocol_inspection.rs:29). A fresh or power-cycled left drive can enable with timeout disabled. **T3.** | One bounded, owner-run preflight before **every** energizing path, including reference acquisition’s `ArmTarget`; write allowed protection settings, read back, refuse mismatches, invalidate on reboot. Never write damper off. |
| **Blocker—printed parts** | **“Total torque bound” is not a total bound.** The filter deliberately clips measured velocity before predicting torque; its comment admits excess damping torque. A test explicitly requires actual torque above the cap. Separately, `clamp_total_torque` can leave an over-cap prediction unchanged when no reachable scaling helps. Verified: [lib.rs:3254](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:3254), [total_torque.rs:153](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/total_torque.rs:153), [command_admission.rs:741](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/tests/command_admission.rs:741). Braking torque still loads printed parts. **T3.** | Use conservative, age-aware feedback bounds and refuse an output whose final prediction exceeds the cap. Verify the drive’s actual MIT torque/current limiting behavior; do not treat parameter names as proof. |
| **Blocker—shutdown** | **Trace flush precedes Disable.** `finish_owner_shutdown` inhibits intent, flushes CSV, then stops drives. Inhibiting intent sends no stop frame. A blocked filesystem can therefore block graceful stop while the last MIT command remains active. Verified: [main.rs:1976](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-pi/src/main.rs:1976), [loop.rs:1453](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/loop.rs:1453), [loop.rs:560](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/loop.rs:560). **T3, T5.** | Move all stop attempts ahead of trace flush and logging/storage waits. |
| **Blocker—unattended / thermal stand tests** | **The specified stop inputs are missing.** I found no implemented camera/tag freshness, residual-jump, 22–26 V bench interlock, or 60/75 °C stage policy in the searched runtime/tools. VBUS and `iqf` are absent from the parameter API. Existing feedback carries temperature but is not the calibration policy. Requirements: [CALIBRATION_LOOP.md:149](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md:149). **T3, T4, T6.** | Implement only the needed stage guards in the owner, with simulated sensor inputs and explicit stale-data deadlines, before authorizing those stages. |
| **Major** | **Subset bring-up fails construction.** Berthier reads all `robot.joints`; Davout filters its motors through `MARENGO_JOINT_SUBSET`; Berthier then requests a progress threshold for every original joint, and Davout returns `UnknownJoint` for excluded joints. Verified static call chain: [loop.rs:496](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/loop.rs:496), [davout/lib.rs:665](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:665), [lib.rs:791](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:791). The test in Pi main checks only motor selection. **T1, T6.** | Load one consistent runtime joint view; keep absent fixed limbs offline. Add a constructor-level regression for a proper subset while the old workflow exists. |
| **Major** | **Silence does not reliably mean stop.** Active pose freshness depends on unanswered solicit time, not an absolute sample-age limit; an old pose can authorize the first post-stall batch. Qualifying drive loss instead disables a subtree and keeps proximal drives moving. Verified: [lib.rs:2320](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:2320), [drive_loss.rs:248](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/drive_loss.rs:248). Current watchdog is 100 ms, versus calibration’s 50 ms missing-feedback rule. **T3.** | Enforce absolute acquisition-age and loop-gap deadlines; stop the tested arm on loss. Remove automatic degraded hold/lower for this catch-pad workflow. |
| **Major** | **Other software can delay the motor loop.** Command drains run until empty; CSV `write_all`, operator output and tracing stdout execute synchronously. A command flood or slow output consumer can delay RX and motion updates. Verified: [main.rs:2143](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-pi/src/main.rs:2143), [position_trace.rs:173](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/position_trace.rs:173), [operator_output.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-pi/src/operator_output.rs), [tracing_layer.rs:288](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/chappe/src/tracing_layer.rs:288). **T5, T8.** | Bound command work per tick, give stop priority, and hand records to a bounded nonblocking worker. Add an actual loop-deadline stop. |
| **Major** | **Fixed hardware facts have conflicting owners.** Names, limits, motor types and membership occur across YAML, URDF, UI inventory and MCP constants; left mappings already disagree. Verified examples under question 5. **T1, T2.** | Replace them with the skeleton and generated views. |
| **Major** | **Multi-file “atomic” configuration is not crash-atomic.** Writers replace files sequentially and roll back only when an error returns. SIGKILL/power loss between replacements bypasses rollback; a mixed generation can remain. Verified: [profile_txn.rs:139](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-config/src/profile_txn.rs:139), [urdf_expand.rs:80](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-config/src/urdf_expand.rs:80). **T2.** | Replace one skeleton atomically, validate it completely at startup, and record its hash with each session. |
| **Major—RS00 bring-up** | **RS00 wire scales remain disputed.** Code uses velocity ±50 and torque ±17 while its own comment says the manuals specify ±33 and ±14, with no captured RS00 motion settling the discrepancy. Verified: [motor_type.rs:26](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/motor_type.rs:26). I confirmed those manual ranges in the supplied RS00 PDF. **T5, T6.** | Qualify the actual firmware’s encode/decode scales at low energy before forearm tests. Do not copy either table blindly. |
| **Major** | **The all-joint gravity preflight cannot scale as documented.** It evaluates `5^n`, capped at 1,000,000 samples; ten modeled joints already exceed that cap. Missing feedback from modeled peers also blocks control. Verified: [gravity_preflight.rs:31](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-pi/src/gravity_preflight.rs:31), [loop.rs:1833](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/loop.rs:1833). **T1, T4.** | Validate actual stage paths and physically coupled chains; do not sweep the Cartesian product of every limb or require absent limbs’ feedback. |
| **Major** | **The record is insufficient for complete replay.** Normal CAN traces omit payload bytes; RobotState is decimated; logs and queues can drop; position tracing is optional. Verified sources under question 11. **T5, T6, T8.** | Always record raw frames, applied commands, acquisition times, configuration and gaps independently of the UI/gateway. |
| **Minor** | **Onboarding describes incompatible and stale contracts.** Examples under question 12. **T7.** | Replace competing guidance with one current map and reconcile the status page against code. |

**2. Actual topology and the smallest replacement**

Verified process layout:

```text
Browser Consul
   ↕ HTTP / WebTransport
marengo-gateway — Chappe IPC listener, snapshots, config/deploy/log APIs
   ↕ Unix socket
marengo-pi — Chappe client, command dispatch, Berthier, Davout, robstride
   ↕ SocketCAN router
configured CAN interfaces → drives

Dev-host MCP → SSH → stop installed service → launch temporary owners/tools
```

Berthier, Davout and Chappe are libraries, not additional control processes. Normal operation already has one motor-owning process across configured buses. The router uses `(interface, device_id)` and rotates receive sources; those facts refute a blanket claim that the process topology itself is fundamentally wrong. Evidence: [architecture.md](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/docs/architecture.md), [bus.rs:1578](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/bus.rs:1578).

The T5 departures are ordinary tools opening CAN, MCP replacing the installed owner, multiple command surfaces, synchronous output, and the absence of an unchanged-production-binary simulated drive environment. Library seams and a fake-bus test seam are not themselves violations of T1.

The smallest replacement is:

- One persistent Pi process owns all configured buses, fixed joint state, control, drive preflight and the fixed stop checks.
- One bounded local request/state interface serves UI, CLI and test runners. Stop bypasses ordinary request work.
- An optional gateway serves the browser. A recording worker handles disk and telemetry export.
- A separate simulated drive process speaks the same protocol on `vcan`; the owner binary is unchanged.
- Five buses mean five routes in the same owner, not five runtime instances. Offline limbs remain inventory; their missing interfaces must not prevent unrelated arm bring-up.

**3. Safety inventory**

This is the inventory found in the inspected control, transport, management and tool paths. **A** means hardware-required behavior; **B** means cheap, useful protection with a named failure; **C** means the current mechanism conflicts with the tenets. Several mechanisms contain both necessary behavior and excess implementation.

| Mechanism | Class and judgment |
|---|---|
| Physical E-stop, fuses, port breakers | **A**, hardware-owned. Their physical operation was not verified. |
| `hardware_estop` supervisor gate | **A** for observing hardware state; production GPIO wiring is absent. Default false is not evidence that power is available. [lib.rs:689](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:689). |
| Drive timeout, torque/current/speed caps, damper verification | **A**, incomplete/missing; first blocker. |
| Persistent vendor-fault, drive-mode and transport latches | **A** for fault stop; **B** for preserving malformed/drop evidence. [feedback_consumer.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/feedback_consumer.rs), [faults.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/faults.rs). |
| Ordinary Disable versus fault clear | **A**. Disable encodes byte zero; fault-clear encoder exists, but searches found no production caller automatically sending it. [lifecycle.rs:19](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/lifecycle.rs:19). |
| Communication watchdog, session-pose/bootstrap gate | **A**, current age policy is wrong for the required stop-on-silence behavior. |
| Hard-position feedback guard, soft envelope, velocity guards | **A** for protecting stops; **B** for velocity anomaly corroboration. Current calibration margins/policies are not established for the left arm. |
| Feed-forward caps/slew, gain validation, transform checks, whole-batch numeric admission | **A/B**; retain the contracts. They do not establish total torque or drive-local protection. |
| Predicted total MIT torque clamp | **A**, presently insufficient; second blocker. |
| Danger zones | **C** as a credited safety layer. Current velocity clipping does nothing to torque at `kd=0`; see [lib.rs:3366](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:3366). Replace with an effective fixed limit or stage guard. |
| Wrong-sign watchdog | **C** in its current global-sign form; disabled in master config. Sign verification is still necessary. [control.yaml](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/config/control.yaml). |
| Manual reference, identity/coordinate continuity, acknowledgement/readback | **B**, protects against wrong frame/zero and reboot. |
| Realm-bound grants, virtual-reference selection, journal-dependent permission and model stamps | **C** as the carried-over architecture. Keep reference evidence and invalidation; remove the authority framework and its historical compatibility machinery. |
| Reference queue, interference gates and enable-completion timeout | **B**, prevents command interleaving and motion before pose exists. Collapse into the one owner’s operation state. |
| Explicit-enable latch, motion-source ownership, stop-on-lag | **B**, protects against competing writers and lost stop commands. The “motion lease” is a process-lifetime source selector, not a renewable operator heartbeat. [motion_owner.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-pi/src/motion_owner.rs). |
| Active-reporting leases, Off/echo gate and burst pacing | **B** for MCP2515 load/sequencing; lease architecture is unnecessary if fixed observation scheduling replaces it. |
| Gravity saturation sweep | **B** intent; **C** all-joint combinatorial implementation. A sampled grid is not a mathematical bound. |
| Ascent/Return/HoldTracking/WaveStall fuses | **B**, guarding measured non-progress/model failure. Retain one stage-appropriate progress/deadline rule rather than all historical variants. [position_hold.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/position_hold.rs). |
| Subtree shed, degraded hold, automatic lower and deadline | **C** for this stack: contradict stop-on-silence and the accepted catch-pad response. |
| Independent stop, signal cleanup, panic catch, systemd `ExecStopPost` | **B**. They cannot stop through a lost bus or reliably cover a hung owner. [stop.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/motor-repl/src/stop.rs), [main.rs:499](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-pi/src/main.rs:499), [service](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/scripts/systemd/marengo-pi.service). |
| Config validation/CAS, Active-edit refusal, persist-degraded management gate | **B** while multiple mutable copies exist; most disappear with one skeleton and stopped activation. |
| Gateway credentials/origin/rate limits, MCP confirmations/weighted gates, gravity gate, CAN-settle checks | **B** at the operational boundary; they are not substitutes for owner/drive protections. |
| Camera, voltage, thermal and residual stage guards | **A for the specified unattended tests**, missing. |

The PDB requirements are also absent from the searched software: loop-driven `WD_FEED`, port-fault decoding, INA237 metering/integration, startup self-test, braking/energy budget, BMS charge/staleness/brown-out interlocks and aggregate drive-current admission. These are **blockers before PDB/battery use**, not prerequisites for unpowered work or the separate 24 V bench. Exact requirements: [REV_B_PARTS.md:108](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/hardware/electrical/pdb/REV_B_PARTS.md:108).

**4. Control needed now**

| Test | Required control |
|---|---|
| Sign/zero, C1 | Explicit reference operation plus small bounded torque pulses/position moves. SetZero acknowledgement verifies a coordinate operation, not physical alignment. |
| T1 | Speed ramps and current/torque steps, both directions, with speed/current measurement. Implement firmware speed/current modes only if those test procedures actually use them. |
| T2/T3/T5 | Bounded torque/current steps and holds, with test deadlines, temperature guards and external force evidence. |
| T4/T6 | MIT position/torque steps, chirps and multi-sine trajectories; full-rate command/reply timing. |
| C0 | No motor control. |
| C2–C4 | Slow position trajectories and measured settling. |
| C5 | Attended power cycle, fresh preflight/reference, then position targets. |
| C6 | Gravity compensation, torque experiments and bounded position waves; offline friction/gravity fitting. |
| C7/C8 | Owner-executed waypoint trajectories, stage guards, scoring and rest periods. |

Current mode composition is verified in [mit_feedforward.rs:45](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/mit_feedforward.rs:45): TorqueOnly zeros gains; GravityComp uses gravity feed-forward; Impedance continually uses measured position as `q_des`. That Impedance mode is not a separate fixed-target stiffness controller.

Choose one measured MIT position law. The pitch-only `scaled_pd` implementation includes reference governing, friction/inertia feed-forward, filtered host damping and integral behavior; it is not merely “move PD into the drive.” [position_law.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/position_law.rs).

Neither locomotion, Cartesian planning, semantic gestures, automatic degraded lowering nor parallel legacy/new law frameworks are required. T6 does require varying Kp/Kd; it does not require a general controller plugin system.

**5. Config and URDF flow**

Verified flow:

1. `robot.yaml` selects joint order and URDF.
2. `motors.yaml` supplies bus/ID/type/direction/gearing and bench bounds.
3. `control.yaml` repeats motor types, supplies soft limits, caps, groups and gains.
4. `homing.yaml` supplies reference policy and offsets.
5. Davout loads URDF and forms hard bounds from **URDF ∩ bench**; effort is the minimum of URDF, motor bench and robot bench caps. Velocity follows separate control precedence.
6. Berthier independently loads URDF for gravity and reference inertia.
7. Set Limits can expand URDF, alter motors/control and persist those copies.

Evidence: [davout/lib.rs:657](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:657), [build_limits:3565](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:3565), [berthier/loop.rs:496](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/loop.rs:496), [limit_envelope.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/limit_envelope.rs).

I found runtime URDF dependency; I did **not** find production runtime code directly loading SolidWorks/CAD or `mass_overrides.json`. T2 is still violated by the multiple hardware contracts.

Concrete duplication/drift:

- `robot.yaml` lists `left_elbow`/`left_wrist`; hardware specifies `left_elbow_pitch`/`left_lower_arm_yaw`.
- `motors_humanoid.yaml` places the left arm on `can3`, IDs 14–18; hardware specifies `can1`, IDs 11–15, with 14/15 still proposals.
- UI inventory duplicates planned joints, ranges and device descriptions.
- MCP hardcodes the five right-arm joints and profile prefixes.
- URDF hard limits, motor hard limits and control soft limits are independently editable.

Sources: [robot.yaml](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/config/robot.yaml), [motors_humanoid.yaml](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/config/motors_humanoid.yaml), [robot-inventory.ts](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/consul/src/data/robot-inventory.ts), [bench-profiles.ts:20](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/tools/marengo-pi-mcp/src/bench-profiles.ts:20), [hardware/STATUS.md:25](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/hardware/STATUS.md:25).

Masses/COMs reside in runtime URDF. Its header explicitly says right-arm COMs were fitted while the shoulder rubbed. That is recorded evidence about provenance, not a verified left-arm gravity model. Put calibrated runtime geometry/gravity parameters in the skeleton; generate visualization/sim artifacts separately.

**6. Bench knowledge to extract before deletion**

- **RS03 velocity ±20 rad/s:** literal capture regression, not just an SDK table. Preserve [rs03_velocity_scale.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/tests/rs03_velocity_scale.rs).
- **Post-SetZero delayed blackout:** 800 ms quiet and 450 ms reporting-write hold, with measured-profile margin tests. [davout/lib.rs:175](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:175), [firmware_profile.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/tests/firmware_profile.rs). These measurements concern recorded firmware; qualify the left units.
- **MCP2515 burst exposure:** 2 ms group spacing, staggered Enables and suppressed bootstrap solicits to unwritten targets. [burst.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/burst.rs), [lib.rs:1643](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:1643).
- **Reporting survives owner processes:** Disable alone does not turn reporting off; exit sends type-24 Off. [motor-repl/stop.rs:1](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/motor-repl/src/stop.rs:1).
- **Wire ordering:** own TX echo, Off settling, old-feedback drain and fresh session pose; an echo is not an application acknowledgement. [lib.rs:1801](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:1801), [bus.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/bus.rs).
- **Protocol distinctions:** ordinary Disable versus fault clear; firmware-version responses must not become pose; type-0 identity replies differ from ordinary status addressing. [lifecycle.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/lifecycle.rs), [identity.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/identity.rs), [bus.rs:789](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/bus.rs:789).
- **Receive truthfulness:** malformed/RTR frames never refresh pose, drops remain faults, queued fault evidence is not overwritten by a later healthy pose, receive work is bounded. [receive.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/receive.rs), [feedback_consumer.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/feedback_consumer.rs).
- **Quantization and progress:** credit new encoder levels, not repeated commands/retargets. [lib.rs:791](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:791), [position_hold.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/position_hold.rs).

Do not preserve the host-silence exemption, RS00 disputed constants or over-cap damping exception as requirements merely because tests pin them.

**7. Deletion plan**

Migrate first: raw captures, firmware timing profiles, calibration datasets and provenance, protocol fixtures, sign/transform facts, independent math oracles, test scoring, and stop/reference sequencing contracts. Export Store-held evidence before deleting its schema/recovery machinery.

Then prove the lean owner against `vcan` faults and the attended left-arm stages. Acceptance must include killed/hung owner, missing drive, bus loss, power cycle, malformed/drop evidence, blocked recorder and repeated start/stop.

After acceptance, delete the replaced CAN/supervisor implementation, virtual-reference authority/history framework, subtree-loss machinery, multi-file config/URDF activation and mirroring, Store/log administration, self-update UI/domain code, Auto Learn, right-arm profile constants and old MCP owner-takeover scripts.

The right arm should migrate to the same skeleton/interface before its old implementation disappears. Keep historical experimental evidence; remove obsolete operating instructions so agents cannot execute them.

**8. CAN path and rewrite contract**

Current path is:

`Pi dispatch → Berthier tick → Davout reference/fault/filter/transform → MotorBus batch validation → MIT encoder → addressed SocketCAN router → nonblocking socket writes`.

Receive reverses through kernel timestamp/drop handling, bounded round-robin reads, raw evidence classification, Davout fault inspection and pose projection. [bus.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/bus.rs), [receive.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/receive.rs).

Worst cases:

- **Owner hung:** its watchdog cannot execute. `ExecStopPost` does not run until the process exits; existing right-arm timeout settings are a documented bench fact, not a left-arm guarantee.
- **Bus lost:** successful socket enqueue cannot prove drive receipt. Stop delivery may fail; local timeout and motor-power hardware must cover it.
- **Drive power cycle:** UID can remain unchanged while volatile zero/settings disappear. Identity alone is insufficient; fresh protection/readback and reference are required.
- **Burst/backlog:** overflow latches transport and stops; missing fault identity prevents assuming a dropped frame was harmless.
- **Queued TX after reconnection:** `can-up.sh` defaults to `txqueuelen=1000` and bus-off restart after 100 ms. Stale queued commands reaching a recovered bus are a credible failure scenario, not something I reproduced. [can-up.sh:8](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/scripts/can-up.sh:8).

Reproduce: validated bytes/scales, correct routes, bounded fair receive, original timestamps and raw evidence, honest stop results, ordinary non-clearing Disable, qualified zero sequence, required HAT pacing, fresh enable completion and all-drive fault response.

Add: complete protection preflight, absolute freshness/loop deadlines, bounded TX age, complete timestamped recording and unchanged-binary `vcan` acceptance.

Drop: runtime URDF authority, general reference realms/grants/history selection, tool-owned CAN sessions, active-reporting lease complexity, host-stall exemptions and degraded continuation. The new transport need not preserve old Rust APIs.

**9. Speculative or obsolete material to remove**

Concrete candidates:

- `config/robot_humanoid.yaml` and `motors_humanoid.yaml`: separate future templates, not the fixed skeleton.
- Hall-sensor configuration/search schema while only manual reference exists: [marengo-config/lib.rs:1123](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-config/src/lib.rs:1123).
- ADR 0014’s unimplemented TCP bridge, semantic pipeline, primitive catalogs and proposed messages. `NavigatorIntent` and `PerceptionFrame` are in that ADR, **not** in the current proto. [ADR 0014](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/docs/decisions/0014-jetson-perception-semantic-motion.md).
- ADR 0019’s unimplemented `MotionSession`/`ConfigAuthority` architecture. Preserve implemented invariants in current documentation instead. [ADR 0019](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/docs/decisions/0019-runtime-authority-and-verification.md).
- ADR 0003’s unstarted D2 architecture/future hashed-physics proposals; retain actual sim decisions. [ADR 0003](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/docs/decisions/0003-simulation-testing.md).
- Superseded WSL operating instructions and retired reference/config architecture after cutover.
- Proto `HomingComplete`, Jetson telemetry surface and unused deprecated log/session message families. Searches found these schemas without production consumers; coordinate removal and reserve wire numbers. Do **not** delete every deprecated field blindly—some candump compatibility fields remain represented.
- Static future-device catalog entries and unused robot-software research/Auto Learn integrations.

The supplied hardware test specifications are concrete requested work, not grounds for deleting the specifications under T4. Nor does an implemented feature become “unused” just because I recommend removing it.

**10. Tests**

Retain contracts guarding named failures:

- Captured RS03 scaling and measured firmware timing.
- Short/RTR/malformed feedback, transport errors, drop counters, bus fairness and competing owners.
- Whole-batch rejection, transform correctness, fault persistence and stop attempts despite peer failures.
- Zero acknowledgement/readback ordering and reboot invalidation.
- Independent gravity oracle and model parity that measured calculations depend on.
- Measured encoder progress, mode isolation, explicit enable and enable-completion behavior.
- Stop-before-persistence and UI first-click stop/dry-run isolation.

Examples: [receive_contract.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/tests/receive_contract.rs), [transport_boundary_vcan.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/tests/transport_boundary_vcan.rs), [exact_gravity_oracle.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/armee-dynamics/tests/exact_gravity_oracle.rs), [shutdown_tests.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-pi/src/shutdown_tests.rs).

Remove tests with deleted behavior: virtual-reference authority variants, old profile transactions/mirrors, Store recovery, speculative inventory and unused interfaces. The over-cap damping test must change because its specified behavior violates T3.

I cannot establish author motivation or call tests “coverage-only” from names. One concrete redundancy candidate is `sim-harness::default_fixture_exists`: subsequent tests already load that fixture. The 500-step MuJoCo smoke checks dependency/model loading, not closed-loop safety.

Missing meaningful tests include blocked trace flush before stop, actual controller construction with a subset, every protection mismatch before **any** Enable, timeout after drive power cycle, and unchanged-binary HIL fault cases. None was executed here.

**11. Logging and observability**

Current verified stack:

- `tracing` writes stdout and publishes Chappe `LogEvent`.
- Chappe limits ordinary events to 40/s and urgent events to 10/s; IPC has bounded/coalescing queues.
- Gateway queues log inserts, batches them into SQLite and counts queue drops.
- Store manages structured logs, sessions, artifacts, gzip archives, retention and recovery.
- log-cli queries/imports/archives and performs domain analyses.
- Consul displays snapshots, charts, logs and capture pages.
- Optional PositionTrace writes CSV; normal RobotState publication is configured at 25 Hz.

Evidence: [tracing_layer.rs:17](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/chappe/src/tracing_layer.rs:17), [gateway/logs.rs:20](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-gateway/src/logs.rs:20), [store.rs](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-store/src/store.rs), [position_trace.rs:38](C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/position_trace.rs:38).

This is not a complete motion recorder. SocketCAN TX/RX trace fields show IDs/lengths, not payload bytes, and service logging defaults suppress trace-level events. Rate-limited diagnostic logs cannot satisfy “every command/reply.”

Use MCAP or an equivalent robotics recorder for full-rate motion/sensor evidence, with explicit gaps and original acquisition times. Use OpenTelemetry/Grafana for operational events and metrics. Keep firmware-timing analysis and calibration fitting; replace custom retention/query/chart infrastructure. Clock-status sampling via `timedatectl` does not establish Pi/Jetson synchronization.

**12. Agent onboarding and review coverage**

The docs expose seams, but an agent cannot safely treat them as a consistent operating contract:

- Root README says there is a URDF viewer; status says there is no URDF render.
- `robstride/README.md` says bins never call it; `motor-repl/stop.rs` deliberately does.
- “Thin bins” is contradicted by the packet’s Pi/gateway size totals and the amount of policy in main/overlay/hardware handlers.
- Status still lists IMU stale republishing and old UI defects. Current code yields only fresh IMU samples, posts stop on first click and captures Dry Run for a running hold.
- Safety docs describe stop-before-persistence, but trace flush precedes stop.
- Hardware prep recommends extending four YAMLs/runtime URDF, contrary to the new skeleton tenet.
- Calibration specifies 100–200 ms drive timeout, contrary to T3’s ≤50 ms.

Replace these with one current map, plain module responsibilities, one wire contract and a short table distinguishing implemented, simulated and physically qualified behavior.

I prioritized the packet’s largest safety-critical areas: selected Davout and Berthier paths, robstride protocol/transport, Pi dispatch/enable/shutdown, config construction/persistence, gateway management/logging, UI controls/playback and MCP ownership/profile logic. I read PACKET and the tenets first, the requested hardware briefs, architecture/status material, ADR index and selected relevant ADRs; I sampled vendor PDF sections and regression tests.

I did **not** read every line of the large crates/apps, every ADR/review, the complete Store/research/deployment implementations, every UI screen/test, all simulator assets, or all four manuals cover to cover. Security/authentication, complete numerical-controller stability and physical timing were not exhaustively reviewed.

**13. Named replacements worth evaluating**

These are implementation options, not a proposal to add every stack.

| Project/tool | Replaces or accelerates | Catch |
|---|---|---|
| [candleLight / gs_usb](https://github.com/candle-usb/candleLight_fw) on an isolated USB-CAN adapter | Removes dependence on MCP2515’s particular RX bottleneck while retaining SocketCAN routing | Firmware support is not proof that an adapter is isolated or has adequate buffering/timestamps; qualify the purchased hardware. |
| [can-utils](https://github.com/linux-can/can-utils) | Capture, inspection and reproducible traffic generation/replay | Raw replay is not a stateful simulated drive or a safe motion runner. |
| Linux [SocketCAN/vcan](https://docs.kernel.org/networking/can.html) | Same production transport API for simulated-drive fault injection | `vcan` does not reproduce arbitration, physical errors or HAT FIFO limits. Model those separately. |
| [MCAP](https://mcap.dev/) and [Foxglove](https://www.foxglove.dev/product/mcap) | Store artifacts, custom replay plumbing and much of Consul telemetry/log viewing | Still need robot schemas, acquisition timestamps and a nonblocking recorder boundary. |
| [OpenTelemetry Collector](https://opentelemetry.io/docs/collector/) / [Grafana Alloy](https://grafana.com/docs/opentelemetry/collector/opentelemetry-collector/), [node_exporter](https://prometheus.io/docs/guides/node-exporter/) | Home-grown log transport and generic host dashboards | Keep collectors and exports outside the control thread; these do not replace raw motion recording. |
| [AprilTag](https://github.com/AprilRobotics/apriltag/blob/master/README.md) and [OpenCV ChArUco](https://docs.opencv.org/5.0/tutorials/objdetect/charuco_detection/charuco_detection.html) | Custom marker detection and camera calibration | Exposure, frame convention, physical tag size and timestamp validation remain project work. |
| [chrony](https://chrony-project.org/doc/4.9/chronyd.html) | Ad-hoc cross-host clock assumptions | Measure offset/error; retain monotonic time for deadlines. |
| [ros2_control’s read/update/write pattern](https://control.ros.org/master/doc/getting_started/getting_started.html) | Clarifies the owner’s hardware/control seam | Borrow the pattern; adopting its plugin/resource framework would add abstractions T1 does not require. |
| [Seeed RobStride_Control](https://wiki.seeedstudio.com/robstride_control/) | Independent comparison implementation for protocol experiments | Existing scale disagreements make it unsuitable as unqualified truth or a drop-in safety foundation. |

A digital luggage scale is already a specified external torque reference. Use it to close T2 before trusting drive-reported torque; neither a second software decoder nor a fitted gravity model supplies that independent measurement.

**14. Other bring-up failures and wasted work**

- **The current “bare motor” profile still references all five master joints.** It is not the required one-actuator stand runner. MCP joint schemas are right-arm-only. Rebuild selection from the skeleton.
- **Current caps prevent the full stand experiment.** Davout takes the minimum including the robot-wide 5 Nm cap and motor-type feed-forward caps. Stage-specific stand limits need explicit configuration; copying the right-arm configuration cannot execute the stated RS03 torque tests.
- **Type-2 feedback does not contain current.** The implementation exposes position, velocity, torque and temperature; `iqf` needs parameter reads. Budget those frames before claiming 1 kHz current recording.
- **Stand documentation conflicts internally on stalled versus rotating rated torque.** The no-spares restriction and stall limits must determine T2/T5 plans; do not blindly run the earlier “100% rated” wording.
- **Left directions, proposed IDs 14/15, provisional limits and placeholder masses are not qualified facts.** Do not turn them into defaults that permit motion.
- **Retain the catch-pad stop assumption explicitly.** The current degraded-hold design solves a different accepted-risk choice.
- **PDB scope must be staged.** Its missing watchdog/current/braking/BMS protections become mandatory before battery/PDB operation; they should not expand the initial bench runtime prematurely.
- **Require evidence from the exact accepted build/config.** Existing soak and control results in status.md concern earlier right-arm builds. They do not qualify this snapshot, the rewrite or the left arm.