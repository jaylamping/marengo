**Do not carry this snapshot unchanged into powered left-arm testing.** The main blockers are missing drive preflight, an incomplete total-torque bound, ineffective protection near limits in torque-only modes, and a silence policy that conflicts with the new tenets.

Reviewed snapshot: `614371d3ae804eb16693f05d5b94fcd1af3b2457`. No files were changed, no builds or tests were run, and no hardware was accessed. Paths below are relative to `repo/`. **Code** means inspected implementation; **docs** means a recorded claim that I did not independently reproduce.

**1. Per-component verdict**

These are migration decisions for the left-arm stack, not certifications of uninspected code. “Delete” means remove from that stack after the replacements and evidence migrations below.

| Component | Verdict | Tenets and evidence |
|---|---|---|
| `crates/robstride` | **Rebuild** | T5 explicitly requires it. Preserve protocol fixtures; resolve RS00 scales. `src/{bus,receive,motor_type,params}.rs`. |
| `crates/davout` | **Rebuild** | T3: missing drive preflight; torque/silence exceptions; excessive reference machinery. `src/lib.rs:1612`, `src/total_torque.rs`, `src/drive_loss.rs`. |
| `crates/berthier` | **Keep but slim** | T4/T6: retain bounded trajectories, torque/gravity composition and measured-progress checks; select one tested position law. `src/{loop,position_hold,position_law,mit_feedforward}.rs`. |
| `crates/marengo-config` | **Rebuild** | T1/T2: four YAMLs, runtime URDF, scopes and multiple writers instead of one skeleton. `src/lib.rs`, `src/{profile_txn,commissioning_scope}.rs`. |
| `crates/armee-dynamics` | **Keep but slim** | T2/T6: retain gravity and fitting mathematics; replace runtime URDF loading with skeleton-supplied model facts. `src/lib.rs`, `tests/exact_gravity_oracle.rs`. |
| `crates/armee-kinematics` | **Keep but slim** | T2/T3: retain validated bounds and necessary geometry utilities; remove runtime URDF authority and expand-only hard-limit editing. `src/{lib,limits,expand}.rs`. |
| `crates/armee-proto` | **Keep but slim** | T4/T5: generated types can serve one fixed interface; prune obsolete schema. `src/lib.rs`; `proto/marengo/v1/marengo.proto`. |
| `crates/chappe` | **Rebuild** | T5/T8: replace custom topic/framing/log transport with a small owner API and standard telemetry export. `src/{ipc,ipc_outbox,tracing_layer}.rs`. |
| `crates/marengo-store` | **Delete** | T8: custom logging, retention, archive and recovery stack. Export existing evidence first. `src/{store,retention,migrations,recovery}.rs`. |
| `crates/marengo-candump` | **Keep but slim** | T5/T6: preserve RobStride enrichment and capture analysis; remove storage/UI paging coupling. `src/lib.rs`. |
| `crates/marengo-deploy` | **Delete** | T4: self-update jobs and upstream-version machinery are unnecessary for these tests. Keep a reproducible external deployment procedure. `src/lib.rs`. |
| `crates/marengo-host-metrics` | **Delete** | T8: replace generic machine telemetry with a standard exporter; retain robot-specific counters elsewhere. `src/lib.rs`. |
| `crates/marengo-imu` | **Delete from left-arm stack** | T4: none of the specified stationary-arm tests requires torso orientation. `src/lib.rs`; `bins/marengo-pi/src/imu.rs`. |
| `crates/marengo-support` | **Keep as-is** | T7/T8: small wrapper around standard `tracing-subscriber`, without robot policy. `src/lib.rs`. |
| `crates/sim-harness` | **Rebuild** | T5/T6: XML parity helpers are not a simulated drive running the real owner over `vcan`. `src/lib.rs`. |
| `bins/marengo-pi` | **Rebuild** | T3/T5: retain one CAN owner, but replace dispatch, configuration, safety integration and tick-side file I/O. `src/main.rs:2123`; `gravity_preflight.rs`. |
| `bins/marengo-gateway` | **Keep but slim** | T4/T5/T8: retain optional authenticated browser adapter; remove logging backend, model editing and self-update orchestration. `src/{http,config,logs,webtransport}.rs`. |
| `bins/motor-repl` | **Rebuild** | T5: make diagnostics/reference commands clients of the owner. Preserve independent-stop evidence during migration. `src/{main,stop}.rs`. |
| `bins/marengo-log-cli` | **Keep but slim** | T6/T8: extract `firmware-timing` and `gravity-fit`; delete SQL/archive/recovery commands after export. `src/{main,firmware_timing,gravity_fit}.rs`. |
| `bins/marengo-limit-sync` | **Delete** | T2: mirrors a second mutable hardware description and widens URDF limits. `src/main.rs:1`. |
| `bins/imu-probe` | **Delete from left-arm stack** | T4: unrelated to the specified tests. `src/main.rs:1`. |
| `tools/marengo-pi-mcp` | **Keep but slim** | T5/T6: retain test planning/scoring; replace SSH-launched owners and stdout parsing with owner API calls. Fix bus assumptions. `src/tools/{motion,enable-soak,calibration-session}.ts`. |
| `tools/marengo-research-mcp` | **Delete from left-arm stack** | T4: research aggregation is not required by a bring-up test. `README.md`. |
| `tools/limit-sync-local` | **Delete** | T2/T4: another service synchronizing another hardware copy. `README.md`. |
| `tools/compound-auto-learn` | **Delete** | T4/T6: separate agent service for taught gesture proposals precedes the required measured calibration loop. `src/server.ts`; `README.md`. |
| `tools/cad` | **Keep as-is, design side only** | T2: CAD-tree checking is outside runtime hardware interpretation. `check_cad_layout.py:1`. |
| `consul` | **Keep but slim** | T2/T4/T5/T8: retain commands, status and results; remove hardware facts, browser-owned test execution and placeholder dashboards. `src/hooks/use-compound-playback.ts`; `src/data/`. |
| `sim/` | **Keep but slim** | T6: retain MuJoCo/model loading; replace “500 steps ran” as meaningful qualification with replay and fault scenarios. `scripts/smoke_test.py:16`. |

**2. Actual topology and minimum replacement**

**Code:** Berthier, Davout and robstride are libraries inside `marengo-pi`, not separate motor-control processes. Chappe is an in-process bus plus IPC workers. The gateway owns the Unix-socket listener; the Pi connects to it. Consul reaches the gateway through HTTP/WebTransport. MCP commonly stops the installed service, launches another `marengo-pi` session over SSH/stdin, parses its output, then restores the service.

Evidence: `bins/marengo-pi/src/main.rs:1779`, `crates/chappe/src/ipc.rs:113`, `tools/marengo-pi-mcp/src/tools/motion.ts:283`.

The runtime already routes configured interfaces through one `SocketCanRouter`; it is not limited to one bus internally (`crates/robstride/src/bus.rs:1587`). The topology violation is the alternative CLI ownership and scripting/control interfaces, plus browser-owned sequencing. Most unnecessary complexity surrounds the owner rather than being extra realtime processes.

Smallest topology:

```text
UI / MCP / test-planning CLI
            │ one versioned command/status interface
            ▼
Pi owner: fixed joint table → stage executor → safety checks → SocketCAN
            │
            └─ bounded telemetry handoff → recorder/exporter
Camera service ── timestamped observations ──► owner
```

Use one owner for one arm or five buses. Missing drives/interfaces appear offline; they do not require different config profiles. A selected test must refuse unavailable required joints. Load complete stage plans into the owner so browser, MCP and gateway loss cannot strand execution between waypoints.

**Major — command dispatch can starve control.** The stdin loop drains until empty before `tick`; there is no per-tick work budget (`main.rs:2143–2191`). Continued input can delay CAN servicing. **Smallest fix:** bounded command processing, a priority stop latch, and deadline checking before generating another motion frame. T3/T5.

**3. Safety inventory and ranked defects**

**Blocker — drive protection is not established before enable.**  
**Code:** `enable_targets_inner` checks identity, reference and poses, then schedules enable. `write_enable` sets MIT mode and enables; it does not configure/read back timeout, torque/current/speed caps or damper. Reference acquisition itself sends Enable at `reference_transaction.rs:1093`, so protecting only the normal enable API would still leave a bypass. `params.rs:23` lacks current-limit, damper, VBUS and current-feedback entries.

A previous right-arm readback of 600 counts is **docs-only evidence**, not a left-drive guarantee or a power-cycle guarantee.

**Smallest fix:** one shared, bounded preflight before **every energizing path**, including reference acquisition: write applicable caps/timeout, read them back, read damper and voltage, require fault-free replies, and invalidate admission after reset/silence. Prove that the selected firmware enforces caps in the selected mode. T3/T5.

**Blocker — “total torque bound” does not guarantee its stated bound.**  
**Code:** `davout/src/lib.rs:3259–3268` deliberately clips measured velocity before prediction and permits excess braking torque. `total_torque.rs` also permits a result whose recorded `bounded_nm` exceeds `cap_nm` when margins/feed-forward cannot be made safe by moving setpoints toward feedback. The regression `tests/command_admission.rs:723` explicitly expects over-cap damping.

**Smallest fix:** remove the over-cap exception; validate the final encoded command against actual fresh feedback and uncertainty, reducing gains/feed-forward or refusing when necessary. Independently verify drive-side total-output limiting. A fixed two-tick prediction cannot cover an arbitrary host stall. T3.

**Blocker — soft-limit protection is ineffective for some active laws.**  
**Code:** position-envelope filtering changes `q_des`; danger-zone velocity filtering changes `dq_des`. Neither produces corrective torque when the respective gain is zero. GravityComp and TorqueOnly can therefore continue applying feed-forward near a stop. The measured guard trips beyond hard limits plus slack, not at the soft boundary. Evidence: `davout/src/lib.rs:3189`, `:3381`; `armee-kinematics/src/limits.rs:228`; `config/control.yaml` sets measured slack to `0.03`.

The default taught inset is `0.027 rad` (`marengo-config/src/limit_patch.rs:8`), below the unattended plan’s five-degree requirement.

**Smallest fix:** explicit stage soft limits and a measured-state guard effective in every mode, with a defined stop on outward motion/torque near the boundary. Do not represent a zero-gain setpoint clamp as braking. T3.

**Blocker — lost-drive behavior violates the required stop policy.**  
**Code:** watchdog configuration is 100 ms (`config/control.yaml:5`). `pose_is_current` excuses host silence when no reply is outstanding (`davout/src/lib.rs:2325`). Selected drive losses retain proximal motion, hold for five seconds, then lower using frozen shed-joint angles (`drive_loss.rs`; `berthier/src/degraded.rs`).

**Smallest fix:** enforce absolute feedback age and a loop deadline; stop and latch on a required drive’s silence. Remove degraded hold/lower for this supported bench arm. The accepted catch-pad plan already defines the consequence of an unhealthy stop. T3/T4.

**Blocker for unattended calibration — required run guards are absent.**  
I did not find implementation of the camera freshness/residual guards, stage thermal thresholds, VBUS window, or attended-pass identity gate in the searched control/runtime/tool paths. `CALIBRATION_LOOP.md` explicitly describes unbuilt work.

**Smallest fix:** implement these in the host stage executor and fault-inject them before unattended C3–C8. UI checks or MCP promises do not satisfy this requirement. T3/T6.

The mechanism inventory below groups related checks; classification applies to their role, not to every detail of the present implementation:

| Mechanism located | Classification and disposition |
|---|---|
| Hardware E-stop; PDB watchdog/fuses/breakers | **(a) Required.** Physical implementation is docs-only here. No production caller of `set_hardware_estop` was found. PDB integration is required before PDB operation, not before supply-powered bench work. |
| Drive timeout, torque/current/speed caps, damper verification | **(a) Required, missing/incomplete.** See blocker above. |
| Fault authority retaining status flags, detailed faults, malformed evidence and transport errors | **(a) Required.** Preserve stop/latch behavior; simplify representation. `davout/src/{faults,feedback_consumer}.rs`. |
| Fault-clear encoding | An encoder exists, but repository Rust search found no production invocation. Do **not** report automatic drive fault clearing as an existing defect. `robstride/src/lifecycle.rs:23`. |
| Address/type checks, finite-value checks, gain bounds, coordinate transforms, whole-batch admission | **(a)/(b).** Preserve protocol validity and fixed-joint safety checks. `davout/src/lib.rs`; `robstride/src/command.rs`. |
| Soft/hard envelopes, total-torque prediction, feed-forward caps/slew, measured velocity guard | **(a)** for limits/caps; **(b)** for measured smoothing/slew. Present enforcement has the blockers above. |
| Global wrong-sign watchdog | **(c) Current formulation.** Disabled in config; one global angle/sign rule does not describe coupled gravity. Replace with measured commissioning results. `lib.rs:3305`; `control.yaml:269`. |
| Named danger zones | **(b)** only with a demonstrated physical failure and effective action; current zero-damping velocity clamp is ineffective. |
| Reference grants, UID checks, coordinate epochs, SetZero acknowledgment/readback | **(b), supporting (a).** Preserve knowing the coordinate/sign and detecting reset. Condense to a small per-joint lifecycle. |
| Durable virtual-reference selection, model stamps, retained stages, journal-driven permission | **(c) Excess machinery for this fixed robot.** Preserve audit evidence without making a database transaction the core motion-permission model. `reference*.rs`, `simulation_reference.rs`. |
| Enable completion gate, neutral bootstrap, explicit-enable latch | **(b).** Preserve refusal to move before fresh post-enable feedback and refusal to resurrect motion after operator stop. `berthier/src/loop.rs`; Pi `enable_gate.rs`. |
| Gravity sweep before enable | **(b)** as stage feasibility checking; **(c)** as the current exhaustive whole-model grid. See Q14. |
| Motion-owner lease; stop-on-command-channel-lag | **(b).** Preserve exclusive intent and stop priority in the replacement API. Pi `motion_owner.rs`, `main.rs:708`. |
| Type-24 reporting leases, expiry, reassertion and Off/echo gates | **(b)** for firmware sequencing; **(c)** for UI-shaped lease complexity. Preserve the wire requirements, simplify ownership. `active_reporting.rs`. |
| AscentStall, ReturnStall, HoldTracking, WaveStall; controller invariant latches | **(b).** Named jam/model failures; consolidate around measured progress and stage requirements. `berthier/src/position_hold.rs`. |
| Single-drive degraded hold/lower and deadline | **(c)** for this bring-up; violates the selected stop policy. |
| Signal, EOF, output-failure and panic stops; stop-before-persistence; systemd exit stop | **(b).** Preserve best-effort cleanup, but none replaces independent drive protection against hang/power loss. |
| Config CAS/locks, active-change refusals, persistence/deploy/restart gates | **(b)** for preventing inconsistent changes; much becomes unnecessary with one immutable active skeleton. |
| MCP confirmations, weighted-motion checks, reference attestation, UI qualification flags | **(b)** procedural gates. They cannot prove physical reference or enforce safety after the client dies. |
| Gateway credentials/origin checks | **(b).** Preserve access control; it is separate from motor safety. `bins/marengo-gateway/src/access.rs`. |

**4. Control modes and laws actually needed**

| Test | Required capability |
|---|---|
| Sign/zero; C1 | Supported manual reference, bounded small torque pulses or slow position moves, measured sign confirmation, fresh zero/readback. |
| T1 | Velocity ramps and torque/current steps; log actual current and acceleration. Existing TorqueOnly is not automatically an accurate current-control experiment. |
| T2/T3/T5 | Bounded torque commands, timed holds/ramps, temperature monitoring, external force measurements. Apply stall ratings to stalled tests. |
| T4 | Small position/torque steps with command and reply timestamps. |
| T6 | MIT position/velocity/gain trajectories for chirp/multisine excitation. |
| C0 | No motor control required. |
| C2–C4 | Slow position trajectories, static holds, bidirectional approaches, synchronized camera observations. |
| C5 | Attended power-cycle, renewed preflight/reference, repeatability measurement. |
| C6 | Gravity compensation, torque experiments and measured friction fitting. |
| C7/C8 | Host-executed waypoint plans, tracking/settling gates, rest periods and runtime stop rules. |

Keep Disabled, bounded TorqueOnly, gravity compensation and one position/impedance primitive. Separate named modes need not duplicate command construction.

**Code:** current Impedance sets desired position to measured position every tick; its `kp` has no restoring error at composition time (`berthier/src/mit_feedforward.rs:55`). Do not mistake it for holding a fixed equilibrium.

Delete parallel legacy/scaled-PD implementations after selecting and qualifying one. Do not copy the pitch trial’s integral/friction settings into left joints without measurement. Full-body balance, IK planners, RL and semantic gestures are not prerequisites. T4/T6.

**5. Config and URDF flow**

**Code flow:**

- `robot.yaml` supplies joint order and URDF path.
- `motors.yaml` supplies addresses, model, direction, gearing and bench limits.
- `control.yaml` repeats joint/model entries and supplies gains, soft limits and velocity policy.
- `homing.yaml` supplies reference configuration.
- Davout loads all of these and the URDF (`lib.rs:662–679`).
- Hard limits are the intersection of URDF and motor bench limits; torque limits combine URDF effort, motor bench, robot bench and type defaults (`lib.rs:3565`).
- Berthier separately loads the URDF into the gravity model (`loop.rs:494–505`).

I verified runtime dependence on **asset URDF**. I did not find a runtime reader of SolidWorks files in the inspected paths.

**Major — hardware authority is duplicated and mutable.** Joint names/limits also occur in `consul/src/data/robot-inventory.ts:151`, compound presets and humanoid templates. Left names and routes disagree: `robot.yaml` has `left_elbow`/`left_wrist`; `motors_humanoid.yaml:192` assigns can3/IDs 14–18.

**Smallest fix:** one agreed skeleton with names, routing, sign/zero, hard/soft bounds and the model facts needed by gravity computation. Runtime consumes that contract; UI inventory and simulation descriptions derive from it. A separate tuning file may contain gains, but must not redefine hardware facts. T1/T2.

**Major — multi-file writes are not crash-atomic.** `profile_txn.rs:126` writes files sequentially and rolls back only after a returned error. Power loss between writes can leave mixed generations. **Smallest fix:** replace the hardware transaction with one atomic skeleton replacement, including revision identity. T2/T3.

**6. Bench knowledge to extract before deletion**

Preserve these implementations, tests and their underlying captures as a compact protocol qualification package:

- **RS03 velocity scale ±20 rad/s:** `robstride/src/motor_type.rs:42`; actual captured frames in `tests/rs03_velocity_scale.rs`.
- **RS00 remains unresolved:** `motor_type.rs:22` uses ±50 rad/s and ±17 Nm despite the bundled manual’s ±33 and ±14. **Blocker for RS00 motion/model fitting:** qualify its actual firmware and fix encoding, decoding and fixtures together. T5/T6.
- **Post-SetZero blackout:** preserve the measured profile, `POST_SET_ZERO_QUIET`/blackout handling, and `davout/tests/firmware_profile.rs`. The documented 800 ms quiet is a bench-derived margin, not a universal vendor guarantee.
- **MCP2515 receive capacity/pacing:** `davout/src/burst.rs`, `active_reporting.rs`, and `tests/physical_firmware/mod.rs`. Preserve per-interface pacing and Off/echo ordering while the HAT remains.
- **Reporting survives process exit:** Disable alone does not turn type-24 reporting off. Preserve exit Off behavior and its tests: `bins/motor-repl/src/stop.rs:1`; `davout/tests/exit_reporting.rs`.
- **Stop is not fault clear:** type-4 byte 0 remains zero. `robstride/src/lifecycle.rs:19`.
- **Receive evidence:** exact DLC/class validation, ordered fault inspection before latest-pose replacement, kernel timestamps, bounded drain and per-bus fairness. `robstride/src/{receive,feedback,bus,rx_time}.rs`.
- **Mechanical/model failures:** measured-progress fuses and fit provenance. Preserve evidence that right-arm COM fits were obtained while the shoulder rubbed; do not transplant them as left-arm truth.

Vendor basis inspected: timeout/damper and zero-calibration sections in :codex-file-citation{path="C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/hardware/docs/robstride/RS03_User_Manual_261013.pdf" purpose="source"}, and operation-control scales in :codex-file-citation{path="C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/hardware/docs/robstride/RS00_User_Manual_260713.pdf" purpose="source"}. Register readback alone does not establish how every cap behaves in MIT mode.

**7. Deletion order**

1. Preserve raw captures, firmware identities, timing profile, calibration inputs/results, protocol fixtures and scoring definitions.
2. Bring up the new owner against `vcan`; qualify preflight, silence, reset, fault, stale-command and stop behavior.
3. Qualify loose left drives, then the stand and supported arm. Re-run relevant tests against the deployed code/config identity.
4. Migrate UI/MCP to the owner interface and export historical logs into the replacement recorder.
5. Delete old CAN ownership, Davout reference/degraded machinery, duplicate config writers, custom log backend, self-update subsystem, virtual-authority scaffolding and placeholder UI.
6. Migrate the right arm before deleting operational tooling it still needs.

Do not delete `firmware-timing`, gravity fitting, captured-frame tests or failure evidence merely because they currently live inside a component marked Delete. T5/T6.

**8. CAN path and worst failures**

Current output path:

`marengo-pi → Berthier → Davout admission/filter/transform → MotorBus → protocol encoders → SocketCanRouter → nonblocking socket writes`.

Input reverses through the bounded raw receive engine, frame classification, ordered hazard inspection, pose selection and joint-space conversion.

**Major — a queued write is not a timely delivered command.** `scripts/can-up.sh:11` defaults TX queue length to 1000 and bus-off restart to 100 ms. `robstride/src/bus.rs:1442` records successful kernel writes; stops use the same transport. **Inference:** a stalled/recovering bus can retain old motion ahead of stop traffic; the inspected path provides no command-age guarantee at physical transmission.

**Smallest fix:** a measured, bounded TX backlog; detect missing transmission completion; latch bus failure; explicitly discard stale queued traffic during controlled recovery and require fresh admission. Do not auto-resume motion after bus recovery. T3/T5.

Other worst cases:

- Lost bus: Disable cannot reach the drives; only verified drive timeout or power removal remains.
- Process killed/Pi hung: graceful cleanup cannot be the primary protection.
- Drive reset: parameters and zero may change; same UID alone is insufficient evidence.
- RX overflow: an omitted frame could have contained a fault. Keep the latch until a replacement policy is justified.
- One absent interface: current router construction fails the whole open (`bus.rs:1615`), contradicting the fixed offline-limb model.

The rewrite must reproduce wire encoding, model-specific scales, safe lifecycle sequencing, timestamped TX/RX, bounded/fair servicing, honest stop outcomes and reset/fault detection. Drop generic virtual permission backends, browser reporting leases, alternate control owners and URDF-dependent motor admission.

`vcan` must exercise the actual production SocketCAN path. Existing in-memory firmware tests and SocketCAN transport tests are not a complete Pi-plus-simulated-drive HIL implementation. T5/T6.

**9. Speculative scaffolding to remove**

**Minor individually, major collectively — T4/T7:**

- `docs/decisions/0014-jetson-perception-semantic-motion.md`: unimplemented semantic planner, transport and catalog design; its opening also describes already removed scaffolds.
- Unstarted D2/future-regression portions of ADR 0003; keep the implemented MuJoCo description.
- `config/{motors,robot}_humanoid.yaml`: replace competing templates with the agreed skeleton’s fixed offline entries.
- `consul/src/data/simulation.ts`: explicitly dummy Isaac sessions, mug-picking and locomotion scenarios.
- Dummy power/battery data in `consul/src/data/{power-system,power-metrics,battery-metrics}.ts`; replace with “unavailable” until real PDB telemetry exists.
- `tools/compound-auto-learn` and its UI surfaces.
- Obsolete virtual-reference acquisition/history-selection APIs and their ADR descriptions after the replacement testing path exists.
- Deprecated Jetson fields/messages at `marengo.proto:235`, `:268`, `:442`; reserve removed field numbers.
- Superseded WSL instructions and duplicate historical architecture contracts.

Do not delete every deprecated protobuf field solely because it is marked deprecated: some still have compatibility consumers. Likewise, ADR 0039/0040 describe partially implemented trials, so first extract the actual behavior and evidence.

**10. Tests**

**Keep tests that guard identifiable failures:**

- Captured RS03 scaling; malformed/RTR/short frames; timestamp mapping; receive budgets and fairness.
- Fault followed by healthy feedback in the same drain; failed stop on one address while others still receive attempts.
- Enable/Off/echo/blackout sequencing and measured firmware-profile margins.
- NaN/invalid commands, mode isolation, operator-disable persistence, missing feedback and measured-progress fuses.
- Exact gravity oracle and calibration input/provenance checks.
- UI first-click stop, frozen Dry Run, stale configuration and playback cleanup regressions.

Evidence: `robstride/tests/`; `davout/tests/{fault_authority,physical_reference,firmware_profile,command_admission}.rs`; `armee-dynamics/tests/exact_gravity_oracle.rs`; Consul testing/playback tests.

**Major — some tests preserve behavior the new tenets reject.** The over-cap damping test, host-silence exemption test and degraded-drive-loss tests must change with policy, not be treated as vetoes against simplification.

**Minor — weak tests should be replaced or removed.** `sim/scripts/smoke_test.py` proves loading/stepping, not safe control. `sim-harness`’s renamed-joint test checks its modified fixture rather than invoking a production parity refusal. Replace such checks with scenarios that fail the real boundary.

I cannot establish author intent as “coverage only” from these samples. Small enum/encoding tests can guard wire contracts. Delete tests of removed behavior with that behavior; do not delete safety tests based on size.

**11. Logging and observability**

**Code:** logging currently spans `tracing → Chappe LogEvent → IPC → gateway writer → SQLite`, plus optional position CSVs, MCP candump sessions, archive/session CLI commands and browser buffers/charts.

**Major — recording can block motion.** `berthier/src/loop.rs:2137` calls trace recording from the control tick; `position_trace.rs:173` writes to `BufWriter<File>`. Filling that buffer causes synchronous file I/O even without an explicit flush.

**Smallest fix:** a bounded nonblocking handoff to a recorder worker/process; no filesystem writer on the control thread. Test slow/full storage while checking loop deadlines. T5/T8.

**Major — the current stack does not establish “every frame/every session recorded.”** Chappe caps normal/urgent log publication at 40/10 events per second; its outbox coalesces/drops; gateway inserts can drop; CSV recording is optional and stops at its session cap. Evidence: `tracing_layer.rs:16`, `ipc_outbox.rs:11`, gateway `logs.rs:229`, `position_trace.rs:28`.

Use MCAP for timestamped raw frames, commands, state and camera observations; standard metrics/log exporters for operations; preserve robot-specific decoders and scorers. Record sequence gaps and invalidate incomplete experimental results. Collector failure must not block control. T6/T8.

**12. Agent onboarding**

**Minor — documentation contradicts inspected code and the new contract.**

- Root README says there is a URDF viewer; status and Consul guides identify a schematic picker.
- `docs/safety.md` says panic is unhandled; `main.rs:499` has a panic-stop wrapper.
- `docs/status.md` leaves stale-IMU publication open; `marengo-imu/src/driver.rs:108` returns only newly received samples.
- The two-click stop, Dry Run and playback issues have implemented fixes in the inspected UI.
- Root/config guides still declare YAML plus URDF the hardware source of truth.
- Codename maps and many overlapping ADRs make a new agent reconstruct historical authority rules.

**Smallest fix:** one current architecture page, descriptive module names, one safety table mapping failure → detection → action → test, and per-module ownership summaries. Link measured evidence separately from current behavior. T7.

**13. Off-the-shelf replacements**

These are named projects verified against primary sources; suitability for this robot remains an engineering choice.

| Project/tool | Replaces or improves | Catch |
|---|---|---|
| [candleLight firmware / gs_usb](https://github.com/candle-usb/candleLight_fw) | USB SocketCAN path can remove dependence on MCP2515’s tiny receive FIFO. | Select actual isolated hardware separately; qualify USB latency, timestamps and unplug behavior. |
| [can-utils](https://github.com/linux-can/can-utils) | Capture, replay and CAN diagnostics instead of custom acquisition tooling. | Preserve RobStride interpretation; replay must be confined to simulation during development. |
| [MCAP](https://mcap.dev/) | Custom multimodal log storage, indexing and schema packaging. | Still needs a nonblocking recorder, retention and robot-specific channels. |
| [OpenTelemetry](https://opentelemetry.io/docs/what-is-opentelemetry/) | Custom service log/metric transport and correlation. | Exporters are outside the realtime path; it is not the high-rate robotics replay format. |
| [Prometheus node_exporter](https://github.com/prometheus/node_exporter) | Much of `marengo-host-metrics` and host dashboard plumbing. | Add CAN/loop-specific metrics; generic host health does not prove motor safety. |
| [AprilTag](https://github.com/AprilRobotics/apriltag) + [OpenCV ChArUco](https://docs.opencv.org/4.x/da/d13/tutorial_aruco_calibration.html) | Marker detection and camera calibration. | C0 must establish actual precision, visibility and latency. |
| [chrony](https://chrony-project.org/) | Clock synchronization and monitoring. | Preserve acquisition timestamps and monotonic deadlines; synchronized clocks do not eliminate camera latency. |
| [ros2_control](https://control.ros.org/rolling/doc/getting_started/getting_started.html) patterns | Established read/update/write and lifecycle separation. | Borrow the separation now; importing its plugin/framework surface would conflict with T1/T4 without a concrete need. |

I would not select an existing RobStride SDK as protocol authority without model/firmware qualification: this repository already records a scale disagreement with a third-party SDK. T5.

**14. Other bring-up failures and wasted work**

- **Major — the soak watches the wrong bus.** `tools/marengo-pi-mcp/src/tools/enable-soak.ts:225` explicitly reads can0 counters and parses can0 results. Other capture helpers enumerate can0–can2. **Fix:** derive interfaces from the selected skeleton joints and score each bus independently. T5/T6.
- **Major — full-body scaling fails before motion.** `gravity_preflight.rs:175–179` computes `5^n` samples and rejects above one million. Nine modeled joints already exceed that limit; two complete arms cannot pass this unchanged. **Fix:** stage/chain-specific feasibility analysis rather than a whole-body Cartesian grid. T1/T4.
- **Major — stand telemetry is incomplete.** The implemented parameter catalog lacks `iqf` and VBUS, and normal type-2 feedback is not a separate current measurement. **Fix:** specify and qualify current sampling/bandwidth before claiming T1/T2/T6 results. T5/T6.
- **Major — source requirements conflict.** `CALIBRATION_LOOP.md` §9 says 0.1–0.2 s drive timeout; tenet 3 and SOFTWARE_PREP require ≤50 ms. T5’s rotating-rated-torque wording is later restricted by stall ratings. **Fix:** one stage contract using the stricter current requirement; remove contradictory numbers. T2/T3.
- **Blocker before battery/PDB operation:** heartbeat GPIO, INA237 readings, port faults, startup self-test, braking-energy limits and brown-out policy were not found in the searched runtime paths. **Fix:** implement and test the actual rev-B5 interface before connecting that power system; do not impose its 45 V startup condition on the separate 24 V bench profile. T3/T4.
- **Major — left identity/sign commissioning remains work.** IDs 14/15 are proposals in hardware status; direction is not verified. I found identity reads, but no completed one-drive ID-assignment flow in the inspected CLI. **Fix:** explicit commissioning state that refuses normal motion until routing/sign/reference are measured. T1/T3/T6.

My deep sampling concentrated on Davout/robstride admission, receive, stop and torque paths; the Pi loop; Berthier composition/tracing; config loading/writes; selected MCP and UI execution paths; and representative regression tests. I read the tenets in full, the architecture/safety/status material, the specified test requirements, and relevant RS00/RS03 manual sections. I did **not** read every line of the large crates, every ADR/test, all gateway handlers, most UI presentation code, research-provider implementations, or RS02/RS04 manuals in full. Hardware performance, actual drive settings, cap enforcement by firmware and the recorded bench timings remain unverified on this review.