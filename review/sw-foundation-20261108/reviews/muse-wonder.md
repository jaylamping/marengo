Adversarial review — marengo software foundation (read-only; nothing built, run, or edited)

Owner over-built claim: verified for topology, safety sprawl, config, and logging. Refuted for the RobStride protocol core — those quirks are bench-earned.

Method: `PACKET.md` + tenets read in full. Six parallel code reads (inventory/topology, safety, CAN/control, config/speculative, tests/logging/docs, hard-won/left-arm), one skeptic pass, one gap follow-up. Line cites below are from bodies actually opened unless marked INFERRED.

Not read: vendor RobStride PDFs (binary only), full submodule bodies of `davout` (36k LOC) and `berthier` (22k LOC), Consul page internals beyond codemaps/APIs, generated prost/TS bindings. No tests were run — pass/fail is unproven everywhere.

## Q1 — Per-component verdict (lean left-arm stack)

| Component | Verdict | Tenet | Evidence (short) |
|---|---|---|---|
| robstride | rebuild | T5 | [params.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/params.rs:8) |
| davout | rebuild | T3/T4 | [lib.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:24) |
| berthier | keep-slim | T4 | [loop.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/berthier/src/loop.rs:128) |
| armee-dynamics | keep-slim | T2 | [urdf_gravity.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/armee-dynamics/src/urdf_gravity.rs:48) |
| armee-kinematics | rebuild | T2 | [lib.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/armee-kinematics/src/lib.rs:102) |
| marengo-config | rebuild | T2 | [lib.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-config/src/lib.rs:11) |
| chappe | keep-slim | T5 | [lib.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/chappe/src/lib.rs:1) |
| armee-proto | keep-slim | T4 | [marengo.proto](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/proto/marengo/v1/marengo.proto:235) |
| marengo-candump | keep | T5/T6 | [lib.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-candump/src/lib.rs:1) |
| marengo-support | keep | — | trivial tracing init |
| marengo-store | delete→OTS | T8 | [lib.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-store/src/lib.rs:1) |
| marengo-host-metrics | delete→OTS | T8 | [lib.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/marengo-host-metrics/src/lib.rs:15) |
| marengo-imu | delete (lean) | T4 | no left test needs it |
| marengo-deploy | delete (bench) | T4 | bench has no deploy need |
| sim-harness | keep-slim | T6 | parity only; rest T4 |
| marengo-pi | rebuild | T5 | monolith main.rs |
| marengo-gateway | keep-slim | T5 | crash-isolated, off motion path |
| motor-repl | keep | T6 | [stop.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/motor-repl/src/stop.rs:1) |
| marengo-log-cli | keep-slim | T6 | [main.rs](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/bins/marengo-log-cli/src/main.rs:78) |
| marengo-limit-sync | delete | T2 | dual writer of limits |
| imu-probe | delete | T4 | probe only |
| marengo-pi-mcp | keep-slim | T6 | bench automation only |
| marengo-research-mcp | delete | T4 | not robot software |
| compound-auto-learn | delete | T4 | not in left-arm needs |
| limit-sync-local | delete | T2 | with limit-sync |
| tools/cad | delete (sw) | T4 | CAD-side checker |
| consul | keep-slim, deferred | T4 | 35k LOC; not proven for first tests |
| proto/ | keep-slim | T4 | prune deprecated/Jetson |
| config/ | rebuild | T2 | one skeleton.yaml |
| sim/ | keep-slim | T7 | empty codemap |

Davout split noted: one angle said keep-slim (supervisor core only). Rebuild wins — the header itself ([lib.rs:24-44](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:24)) documents journal/virtual/grant owners inside a 36k-LOC crate. Extract supervisor + `total_torque.rs` + watchdog + enable sequence; do not carry the crate.

## Ranked findings

Blocker

- B1 — No drive preflight before enable (T3). VERIFIED: `enable_targets_inner`/`write_enable` writes only Enable + MIT mode ([lib.rs:1612](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:1612), [lib.rs:2002](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:2002)). `CanTimeout 0x7028`, `LimitTorque 0x700B`, `LimitSpeed 0x7017` are never written or read back on the enable path; read-back exists only disabled in [protocol_inspection.rs:27](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/protocol_inspection.rs:27). [safety.md:590](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/docs/safety.md:590) admits it. Left drives may hold `CanTimeout=0` (off). Smallest fix: extend `params.rs` (missing `0x7018/0x701C/0x702A`), then write+read-back CanTimeout ≤50 ms, `0x700B/7018/7017`, damper=0, VBUS 22–26 V before every enable and after every drive power cycle; refuse on mismatch. Note doc conflict: CALIBRATION_LOOP asks 0.1–0.2 s; tenet ≤50 ms wins.
- B2 — Left arm unbootable as configured (T2). VERIFIED: [robot.yaml:8](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/config/robot.yaml:8) is right-only plus stale `left_elbow/left_wrist`; motors/control/homing are right-only; humanoid template puts left on `can3` IDs 14–18 vs settled `can1` IDs 11–15 (`left_elbow_pitch/left_lower_arm_yaw`). Smallest fix: one `skeleton.yaml` with `can1`/11–15; delete templates.
- B3 — Hung or killed Pi holds last MIT torque incl. gravity feed-forward (T3/T5). VERIFIED: no watchdog outside the 200 Hz thread ([safety.md:591](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/docs/safety.md:591)); panic wrapper + `ExecStopPost` cover panic/SIGTERM only. Smallest fix: B1's drive CanTimeout. PDB heartbeat is later.
- B4 — T1 velocity/current has no production path (test gap). VERIFIED: Speed/Current encoders ([params.rs:226](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/params.rs:226)) have zero production callers; production write is MIT-only. Smallest fix: MIT velocity-field law or a real Speed mode before T1 runs.

Major

- Drive torque backstop missing. Host clamp exists ([total_torque.rs:110](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/total_torque.rs:110)) — the packet's "feed-forward only (CS10)" claim is outdated. But `bounded_nm > cap` case ([total_torque.rs:39](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/total_torque.rs:39)), clipped-dq excess `kd·(|dq|−cap)`, and inter-tick/stall torque are unguarded; the 17 Nm elbow stop is at risk. Fix: write drive `0x700B`.
- Two named protections are inert. Danger-zone `clamp_velocity` changes no torque at `kd=0` (header admits it, [lib.rs:16](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:16)); wrong-sign check is disabled ([control.yaml:269](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/config/control.yaml:269)). Fix: delete, or convert to `clamp_torque`/fault. Stop counting them as protection.
- E-stop software mirror is dead. `set_hardware_estop` ([lib.rs:1468](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/davout/src/lib.rs:1468)) has test-only callers; GPIO unwired ([safety.md:267](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/docs/safety.md:267)). Hardware cut is unaffected; software cannot detect/log/cancel. Fix: wire GPIO or delete the mirror and say so.
- Bus-off split is wrong in prior notes. Kernel `restart-ms=100` ([can-up.sh:25](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/scripts/can-up.sh:25)) does restart — "no recovery" was corrected by the skeptic pass. The app layer still latches Transport on any error frame, and stop on a dead bus can latch `StopDelivery`. Fix: rewrite must specify kernel-vs-app behaviour separately.
- MCP2515 RX overrun is fail-closed. `BurstPacer` 2 ms, type-24 one/iface/5 ms, drain caps 64/256 leave a `WorkLimit` suffix unread. Cause B still open. Fix: keep pacing (Q6); resolve cause B on hardware, not by widening caps.
- Partial CAN batch. Frames before a transport failure are already on the wire ([bus.rs:491](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/crates/robstride/src/bus.rs:491)). `ENOBUFS` retry exists only in motor-repl stop, none on the MIT path. Fix: retry/backoff on the MIT path; document non-atomicity.
- Degraded single-drive shed contradicts T3 stop-on-silent. Needs an explicit tenet exception or deletion.
- Grant/journal empire protects zero integrity but is not on the T3 fixed list; `recovery_available=false` always. Fix: slim to SetZero + identity + 800 ms quiet workflow.
- Non-atomic 3-file Set Limits write — power loss mid-write leaves mixed generations. Fix: single skeleton write + atomic rename.
- Duplicate facts (T2): hard limits ×5, soft ×3, velocity ×4, effort ×4 (`min()`-ed), names ×7+ with differing values; MJCF COM ≠ URDF COM despite a "mirror" claim; `right_lower_arm_yaw` axis wrong (Y vs Z). Fix: Q5 skeleton.
- PDB/BMS software absent (WD_FEED 50–100 ms, INA237, brake integrator, BMS). Blocker only when the Rev B board/battery arrives; bench 24 V bypasses it today.

Minor

- `comm_watchdog` 100 ms vs calibration's 50 ms; 60/75 °C host gate not found; deprecated proto kept "never delete"; `gear_ratio: 1.0` everywhere; stale README `cad/models` dirs; PositionTrace CSV split from session store; `LogEvent session_id` env-only.

## Q2 — Topology

VERIFIED actual layout: Consul → HTTP 8080 / WebTransport 8443 → gateway (UDS `/run/marengo/chappe.sock`) → Chappe → marengo-pi (Berthier 200 Hz → Davout → robstride → SocketCAN `can0`, plus IMU/metrics/preflight in the same process; [architecture.md:18](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/docs/architecture.md:18)).

Breaks T5: CAN + control + publish in one process; no verified vcan drive path; frame log is `tracing::trace` only (dropped at info). Breaks T4: gateway/Consul/MCP layers exist ahead of left-arm need.

Smallest tenet-clean topology: on the Pi, one RT `marengo-can` owning every SocketCAN/gs_usb bus (codec, timestamped frame log, vcan backend, bus names from skeleton); one slim `marengo-ctrl` doing host T3 checks over a single UDS request/reply + state stream; gateway/Consul optional and crash-isolated, never in the motion path; motor-repl only while ctrl is stopped. Scales to 23 joints / 5 buses by adding a skeleton entry and a CAN thread — no per-limb code (T1).

## Q3 — Safety inventory

Present (a, hardware-required): soft-inside-hard envelope, host total-torque bound, fault latch never-auto-clear (fault-clear encoder has zero production callers), comm watchdog, enable sequence with echo + stagger.

Missing (a): B1 preflight, drive-side torque/speed/current caps as backstop, PDB suite, independent watchdog.

Keep (b, cheap/useful): gravity preflight `enable_gate` (fail-closed), motion lease, Berthier fuses, feedback velocity/position guards, burst pacing, motor-repl stop, gateway disable-only endpoint (note: LAN-exposed).

Prune (c, tenet-breaking): grant/journal empire, degraded shed, danger-zone `clamp_velocity`, wrong-sign check, active-reporting leases (diagnostic, not safety).

## Q4 — Control

Needed: sign/zero via Position MIT ±5° + SetZero (have); T2/T3/T5 TorqueOnly (have); T4 MIT steps + kernel `rx_time` (have); C0 type-24 status; C1–C4/C7/C8 Position; C5 reference workflow; C6 GravityComp + friction (have).

Gaps: T1 Speed/Current end-to-end (B4). T6 chirp generator: UNRESOLVED — existence not verified in code.

Not needed: Impedance as a distinct mode (degenerate), drive-side Position/Speed/Current scaffolding, the second position law (`position_hold` vs `position_law` — keep one), PositionWave.

## Q5 — Config and URDF

VERIFIED trace: `robot.yaml` → `motors.yaml` → marengo-config loaders → Davout `build_limits` (URDF ∩ motors) → `MotorAddress` → `encode_mit` device low byte. Masses flow URDF → dynamics → berthier. The runtime reads a CAD-derived file — T2 inverted. No skeleton exists.

Fix: one `skeleton.yaml` (names, bus, ID, direction, zero, limits, masses). Generate URDF/MJCF from it. Runtime reads the skeleton only. Every duplicate in the Major list collapses to that file.

## Q6 — Hard-won knowledge (lift verbatim before any deletion)

- MIT packing: torque feed-forward in extended-ID extra data; big-endian payload vs little-endian params.
- Per-type scales (RS03 velocity bench-fit; RS00 scale DISPUTED — code vs manual disagree, unresolved on bench).
- Host ID `0xFD` asymmetry.
- SetZero: 800 ms quiet / 450 ms blackout; drives go silent 45–61 ms at 511–543 ms after.
- `BurstPacer` 2 ms + type-24 5 ms pacing, 1 s heartbeat.
- Off-echo-before-Enable, one-per-interface-per-period enable stagger.
- EchoLedger foreign-writer refusal.
- Total-torque clamp + horizon.
- Reboot Run→Reset handling; drain caps; firmware-timing JSON guard.

Dropping the pacing/quiet constants is the most likely week-1 left-arm fault source.

## Q7 — Deletion plan

Migrate first: the Q6 list, the `protocol_inspection` preflight seed, candump archives + timing profile, store sessions (export to MCAP), MCP motion gates.

Delete after the left arm passes C1–C6 + soak: humanoid templates, `left_elbow/left_wrist` aliases, right-arm fitted COM/friction as left defaults, unused davout/berthier scaffolding, store/host-metrics/imu/deploy/research-mcp/auto-learn/limit-sync, Consul leaf codemaps and panels. Never delete the stop path or preflight until the replacement is proven on `can1`.

## Q8 — CAN layer

VERIFIED path: Berthier tick → Davout filter → `mit_control_all_at` validate-then-send → nonblocking SocketCAN → router; RX drain → feedback consumer. Transport is SocketCAN only; the MCP2515 is a kernel HAT (1 Mbit, `restart-ms=100`).

Worst failures: B1/B3 (no timeout written; hung host holds torque), fail-closed RX overrun, partial batch on the wire, no MIT-path retry, app-layer Transport latch on any error frame.

Rewrite must reproduce: everything in Q6, kernel timestamps (`rx_time`), single-owner bus claim. Drop: drive Position/Speed/Current modes, SimulationBus in the production path, trace-only frame logging (T5 needs a real frame log).

## Q9 — Speculative scaffolding (delete)

Humanoid YAMLs (test-only), proto deprecated/Jetson blocks kept under "never delete", ADR0014 (Jetson) + Proposed 0039/0040, archive seed URDFs, URDF-merge gateway endpoints (CAD inside software, T2), hardcoded 23-DOF `robot-inventory.ts`, sim-harness reserved plant, Rs04 inside templates (keep Rs04 in robstride for the stand), `MARENGO_JOINT_SUBSET`/profile machinery, `recv_can_id`/firmware identity-only dead fields, `driver: 'robstride'` fake plugin field (T1).

## Q10 — Tests

Guards a real failure — keep: `firmware_profile` (SetZero blackout margin), `rs03_velocity_scale` (bench candump frames), `drive_loss`, `safety_validation`, store migration/retention, chappe outage/quota, MCP safety gates, cs24 period, `torque_output_contract`.

Coverage only — delete: Consul layout/card tests asserting CSS class strings. They violate the repo's own rule ([AGENTS.md:249](/C:/Temp/REVIEW_PACKET_SW_FOUNDATION_20261108/repo/AGENTS.md:249): don't pin wording/wiring). Rule for the lean stack: every retained test names the failure it catches. INFERRED: counts (~1350 `#[test]` occurrences) are static; nothing was executed.

## Q11 — Logging and observability

VERIFIED current stack: `tracing` → ChappeLogLayer (publish error ignored via `let _ =`; quota drops counted only in-process) → proto `LogEvent` → gateway batch writer (100 events / 500 ms) → SQLite + FTS5 with custom retention/recovery (~41k `recovery.rs`). Clock is `SystemTime` with NTP-synced boolean only — no shared/monotonic clock, so the T8 "one clock" requirement is unmet. Power loss can drop the last gateway batch (WAL + `synchronous=NORMAL`, 500 ms flush).

Off-the-shelf replaces: logs → Loki/OpenTelemetry or journald+Grafana; PositionTrace + candump → MCAP with Foxglove/Rerun; HostMetrics → node_exporter/Prometheus (keep a thin CAN-health exporter). Keep the log-cli analyzers (firmware-timing, gravity-fit) — they close the calibration loop and consume captures, not the store.

## Q12 — Agent onboarding

`AGENTS.md` + `architecture.md` + `lib.rs` headers do teach the Pi→Berthier→Davout→robstride seam in minutes. Against that: 86 `codemap.md` files with duplicated crate/src pairs; `marengo-store` misdescribed in the crate map; a phantom `SyntheticBus` (0 code hits); the "Davout sole path" claim contradicted by the motor-repl bypass (safety-relevant — a new agent would assume disable goes through Davout); docs still teach URDF-as-source-of-truth against T2. Fix: keep three maps, generate crate maps from `lib.rs` headers, correct those three items, delete Consul leaf codemaps.

## Q13 — What is missing (off-the-shelf)

Evidence here is THIN: no angle bench-verified a candidate against this code. Treat each row as a named, real project with an unassessed catch, not a verified replacement.

- gs_usb USB-CAN adapter (SocketCAN) — replaces the SPI MCP2515 HAT and its 2-frame FIFO overrun class. Catch: INFERRED config-only swap only if skeleton bus names are the sole bus source; timing on `can1` unmeasured.
- can-utils (`candump`, `cansend`, `canplayer`) — replaces home-grown capture/replay tooling. Catch: no RobStride decoding; keep `marengo-candump` enrichment.
- MCAP + Foxglove/Rerun — replaces SQLite session store, PositionTrace CSV, candump text. Catch: writer must be added on the Pi; old sessions need export first (Q7).
- OpenTelemetry + Grafana/Loki, Prometheus + node_exporter, Sentry — replaces ChappeLogLayer/SQLite logs, HostMetrics, and absent crash reporting. Catch: Pi resource budget and offline operation unassessed; tenet 1 requires working offline.

## Q14 — Anything else

- Left IDs 14/15 in existing material are unconfirmed proposals; possible stale IDs already on the buses. Scan before assigning.
- Direction signs are unknown until C1 — do not pre-fill them from the right arm.
- `can1` inherits MCP2515 cause B. Resolve it before soak, not during.
- Elbow needs its dummy rig before a powered forearm is attached.
- Left masses are placeholders (hand 200 g / pack 300 g). Do not copy right-arm fitted COM — the right shoulder rubbed; its URDF values encode that fault.
- Not in the repo at all: vcan emulator, camera calibration circuit, T-stand runner (SOFTWARE_PREP items 4, 5, 7 unbuilt). These are new builds on the critical path, not deletions.
- The CALIBRATION_LOOP timeout conflict (0.1–0.2 s vs tenet ≤50 ms) must be settled in the doc before C0, or the preflight will be configured twice.

Bottom line for the left arm: build B1 preflight + skeleton config + the Q6 protocol lifts first. Nothing else on this list blocks first motion; everything in Blocker does.
