# Review: marengo software foundation (left-arm keep/rebuild/delete)

**Scope.** Read-only. I read `PACKET.md`, `hardware/SOFTWARE_TENETS.md`, `README.md`, `AGENTS.md`, `docs/architecture.md`, `docs/safety.md` (full), `docs/status.md`, all `codemap.md` at repo/crate/bin level, `config/*.yaml`, and the hardware docs the packet names (`SOFTWARE_PREP.md`, `STATUS.md`, `CALIBRATION_LOOP.md` §9/§11, `sim-to-real-loop.md` §2/§5, `REV_B_PARTS.md` accepted risks, the PR #254 findings, ADR 0036/0037 excerpts, `robstride-timing-profile.json`). In code I read most of `robstride` (lib, mit, bus, receive, params, lifecycle, motor_type), large parts of `davout/src/lib.rs`, `total_torque.rs`, `burst.rs`, `protocol_inspection.rs`, `berthier/src/lib.rs` and `loop.rs`, `marengo-pi/src/main.rs` (startup, loop, stop path), `gravity_preflight.rs`, the gateway route/codemap level, the store/candump/deploy/kinematics headers, the MCP launch/config/greps, and the Consul codemap/package. I did **not** read the full 35.6k-line Consul source, the full 9.7k-line gateway, the full 10.3k-line store, the full 16k-line MCP, the full 5,951-line `davout/src/lib.rs`, the full 4,361-line `berthier/src/loop.rs`, most ADRs, most `docs/reviews`/`commissioning`, the PDF manuals, or most test bodies. I ran nothing and touched no hardware.

The owner's belief is **half right**: the stack is over-built in reference/journal/virtual-history, gateway/Consul/storage/deploy and config write-back, and under-built exactly where the left-arm tests need it — drive-side preflight, real-time scheduling, a vcan emulator, and a 1 kHz bench runner. The safety layers tenet 3 calls "drives" are largely not implemented.

---

## 1. Per-component verdict (left-arm stack)

| Component | Verdict | Tenet | Evidence / reason |
|---|---|---|---|
| `crates/robstride` | **Rebuild** | 5 | Owner already decided. 7.3k lines + tests; keep `comm.rs`, `mit.rs` scales, `lifecycle.rs`, `receive.rs` bounds, `bus.rs` echo ledger and `tests/*` as the protocol reference, not the code. |
| `crates/davout` | **Keep but slim** | 3, 4 | Sole motor path and the only place limits/caps/watchdog/stop exist (`lib.rs:1499`, `2347`, `3136`, `2667`). Delete `reference_journal*`, `reference_model/codec/urdf_codec`, `simulation*`, virtual-reference machinery; keep limits, caps, comm watchdog, fault authority, `drive_loss`, `burst`, `active_reporting` (if type-24 stays). |
| `crates/berthier` | **Keep but slim** | 3, 4 | Gravity + Position + TorqueOnly are needed. Drop `wave`, the legacy Position law (choose one), friction assist, and the right-arm tuned `scaled_pd` parameters until re-fit. Keep the fuses and degraded hold for elevated unattended runs. |
| `crates/marengo-config` | **Keep but slim** | 2, 4 | YAML loaders are the contract seam. Delete `limit_patch`, `profile_txn`, `urdf_merge` (841 lines), `commissioning_scope`, `bench_joints` if the offline-limb issue is fixed properly. |
| `crates/armee-kinematics` | **Keep but slim** | 2, 3 | Limits and velocity-scaled envelope are used by Davout. Its URDF dependency is the tenet-2 conflict (see B4). |
| `crates/armee-dynamics` | **Keep** | 3 | `gravity_torques(q)` is required; keep the calibration fit for C6. |
| `crates/chappe` | **Delete** | 4, 8 | In-process bus + Unix-socket IPC exists to separate UI from motion. A one-arm bench does not need it; replace with a command socket and a log file. |
| `crates/marengo-store` | **Delete** | 8 | SQLite log/session store (`src/lib.rs:1-10`). Replace with MCAP/Parquet + journald. |
| `crates/marengo-candump` | **Keep but slim** | 6, 8 | Useful for `firmware-timing` and offline analysis; most parsing is `can-utils` + `python-can`/`cantools`. |
| `crates/marengo-deploy` | **Delete** | 4 | Self-update jobs/revisions; use rsync/systemd or an atomic image tool. |
| `crates/marengo-host-metrics` | **Delete** | 4, 8 | `node_exporter` does this. |
| `crates/marengo-imu` | **Delete** | 4 | Not used by the left arm; the torso IMU is not in the left-arm test path. |
| `crates/marengo-support` | **Keep** | 7 | 28 lines, one tracing init. |
| `crates/sim-harness` | **Delete or rebuild** | 4, 6 | Current MuJoCo smoke/URDF-MJCF parity is not the HIL piece the tests need; a vcan drive emulator is. |
| `crates/armee-proto` | **Keep but slim** | 4 | Drop deprecated messages, Jetson/host-metrics types (`marengo.proto` has `deprecated = true` on many). If Chappe/Consul go, keep only the few messages the bench needs. |
| `bins/marengo-pi` | **Keep but slim / rebuild around new CAN** | 5 | Thin runtime is the right shape, but it is not real-time and it carries gateway/Consul coupling. Add drive preflight, RT priority, absolute-deadline pacing. |
| `bins/marengo-gateway` | **Delete** | 4, 8 | HTTP/WebTransport/SQLite for Consul; not needed for the left-arm tests. |
| `bins/marengo-log-cli` | **Delete** | 8 | Archive query CLI; off-the-shelf tools replace it. |
| `bins/motor-repl` | **Keep but slim** | 3, 6 | `disable` and `protocol-inspect` are essential; `set-zero` should become the minimal reference flow. Drop right-arm-only commands. |
| `bins/marengo-limit-sync` | **Delete** | 4 | Dev limit-sync; tied to the Consul Set Limits workflow. |
| `bins/imu-probe` | **Delete** | 4 | No IMU in the left-arm path. |
| `tools/marengo-pi-mcp` | **Keep but slim** | 6 | Automation is the right idea, but 16k lines, right-arm profiles and `can0` hardcoding. Add `can1` id scan / set-id / preflight; drop deploy/Consul plumbing. |
| `tools/marengo-research-mcp` | **Delete** | 4 | Not robot software. |
| `tools/limit-sync-local` | **Delete** | 4 | Consul Set Limits mirror. |
| `tools/compound-auto-learn` | **Delete** | 4 | UI BFF. |
| `consul/` | **Delete** (keep a read-only page later if wanted) | 4, 8 | 35.6k lines of operator UI for one bench arm; the left-arm tests do not need it. |
| `sim/` | **Keep only as HIL harness** | 6 | Current `smoke_test.py` is not the needed vcan emulator. |
| `proto/` | **Keep but slim** | 4 | Remove deprecated messages and Jetson/host-metrics types. |
| `config/` | **Keep but consolidate** | 2 | It is four YAMLs, not one skeleton (see Q5). |
| `assets/urdf` | **Keep but generate** | 2 | The runtime currently depends on it (B4). |
| `scripts/` | **Keep a few** | 6 | `can-up.sh`, `vcan-up.sh`, deploy; delete WSL/daily-audit/legacy. |
| `docs/decisions` | **Keep ~10** | 4 | Delete 0014 (Jetson), 0016 (WSL), 0026–0035 (virtual reference), and speculative roadmap docs. |

---

## Ranked findings

### Blockers

**B1. The drive-side safety layer is absent.** Tenet 3 requires a 50 ms-or-less host-loss timeout, torque/current/speed caps in the drives, and a check of those settings before every enable. The code never writes or reads any of them:

- `Supervisor::enable_targets` goes straight to reference checks and `write_enable` (`crates/davout/src/lib.rs:1499-1538`, `2002-2020`), which sends only type-3 Enable + RunMode MIT.
- No `write_parameter_at` call exists in `davout` outside tests; `ParameterId` (`crates/robstride/src/params.rs:23-43`) does not even define `limit_cur` (0x7018), the damper (0x702A), or VBUS (0x701C), so the preflight in `SOFTWARE_PREP.md:79-90` cannot be expressed today.
- `protocol_inspection.rs:29-36` reads only RunMode, MechPos, MechVel, CanTimeout, ZeroSta, AddOffset.
- The PR #254 note records CanTimeout **0 on all five drives out of the box** (`docs/commissioning/2026-10-02-right-arm-pr254-findings.md:19-26`); 600 (~30 ms) was later written by hand, and whether it survives a power cycle is unknown (`docs/safety.md:590-599`).
- `SOFTWARE_PREP.md:79-90` requires CanTimeout ≤50 ms, limit_torque, limit_cur, limit_spd, damper=0, VBUS 22–26 V and no faults, read back before every enable.

Worst case: a fresh left-arm drive is enabled with CanTimeout=0. The Pi hangs or loses power; the drive keeps executing its last MIT frame (including τ_g) indefinitely. No host layer covers that. **Smallest fix:** implement a read-back preflight for CanTimeout first (refuse enable if 0 or >50 ms), then write/read the torque/current/speed caps and damper; re-run it after any drive power cycle. This is the single most important left-arm software item and it is not in the code.

**B2. RS00 wire scale is wrong for the left forearm.** The left `left_lower_arm_yaw` is an RS00 (`hardware/STATUS.md:39`). `crates/robstride/src/motor_type.rs:21-32` uses velocity ±50 rad/s and torque ±17 Nm, while the code's own comment says every RS00 manual revision gives ±33 rad/s and ±14 Nm (Seeed SDK is the outlier). `hardware/designs/left-arm/urdf/left_arm.urdf:71` also says `velocity="50" effort="14"`, so the URDF and code disagree with the manual and each other. Effect: feedback velocity is scaled 50/33 ≈ 1.5×, torque 17/14 ≈ 1.21×, and commands reach the drive at 33/50 and 14/17 of intent. That corrupts gravity compensation, kd damping and the torque cap on one of the five left joints. **Smallest fix:** set RS00 to 33/14 after one bench check, and update the URDF velocity field; add a test that the RS00 scale matches the manual.

**B3. Adding the left arm to the one skeleton blocks the right arm unless `MARENGO_JOINT_SUBSET` is set.** Tenet 1 says an unbuilt limb is just a limb whose drives are offline and needs no separate code path. Today:

- `ControlLoop::from_repo_inner` takes `joint_names = robot.robot.joints` (`crates/berthier/src/loop.rs:496-499`) and builds the gravity model over all of them.
- `GravitySweep::start` refuses if any of those joints has no motor, policy or **measured position** (`bins/marengo-pi/src/gravity_preflight.rs:131-153`).
- `build_limits` refuses a robot joint with no motor entry (`crates/davout/src/lib.rs:3572-3578`).

So adding the five left joints to `robot.yaml`/`motors.yaml` while their drives are absent makes right-arm enable refuse at the gravity preflight, and the only escape is `MARENGO_JOINT_SUBSET` (`davout/src/lib.rs:665-666`). That is a separate code path, contrary to tenet 1. **Smallest fix:** scope the preflight and enable to the requested/active joints, and treat a modeled joint with no feedback as offline unless it is in the active set; or make the gravity model evaluate only the active set plus explicitly held angles.

**B4. There is no one skeleton, and the runtime reads the URDF.** Tenet 2 says one YAML is the contract and the runtime never reads CAD/design files, with a URDF generated for sim/visualisation. The code reads four YAMLs plus the URDF:

- `config/AGENTS.md` calls `config/{robot,motors,control,homing}.yaml` plus `assets/urdf/marengo.urdf` the master SoT.
- `berthier/src/loop.rs:496-504` loads `robot.yaml`, resolves `assets/urdf/marengo.urdf`, and builds `UrdfGravityModel::from_urdf`.
- `davout/src/lib.rs:3572-3580` takes hard limits as `URDF ∩ motors.bench`, and `3633-3637` takes velocity from `control.yaml` and effort from `URDF ∩ bench`.
- `docs/architecture.md:52` itself says "One master description (`config/` + `assets/urdf/marengo.urdf`)" — already two.

This is not a small duplication: joint limits exist in `motors.yaml` `bench`, the URDF `<limit>`, `control.yaml` soft bounds and the hardware STATUS table; masses exist in the URDF, `mass_overrides.json`, `MEASURED_MASSES.md` and the MJCF. **Smallest fix:** either amend tenet 2 to say "one YAML plus one generated model", or move limits/inertials into the skeleton and generate the URDF/MJCF from it. Do not build the left-arm stack on the current split and then claim tenet 2.

**B5. No watchdog for a hung or crashed Pi.** `CanTimeout` is not written (B1), the hardware E-stop sense is not wired (`Supervisor::set_hardware_estop` has no production caller; `davout/src/lib.rs:1468-1490`; `docs/safety.md:267`), the systemd unit has no `WatchdogSec` (`scripts/systemd/marengo-pi.service:7-25`), and I found no `SCHED_FIFO`/`mlockall` in the code (the only SCHED_FIFO mention is the kernel IRQ thread in `docs/troubleshooting.md:27`). Pacing is `thread::sleep(period - elapsed)` (`bins/marengo-pi/src/main.rs:2313-2315`). Worst case: a hung loop or a killed process leaves drives on the last MIT frame; only `ExecStopPost=motor-repl disable` covers a normal/SIGKILL exit, not a hang. **Smallest fix:** CanTimeout ≤50 ms plus `WatchdogSec`/`sd_notify` from the control loop and RT priority. For the bench, CanTimeout alone is the layer that holds if the Pi dies.

**B6. The test-stand and calibration-loop work is not implementable on this runtime as written.** `SOFTWARE_PREP.md` asks for a vcan drive emulator (item 4), a T1–T6 runner at ~1 kHz command/reply (item 5), and a camera calibration loop (item 7). The code has a 200 Hz loop (`config/control.yaml:3`), no 1 kHz runner, no chirp/multi-sine tool, no vcan RobStride emulator (grep finds only `vcan0` test helpers), and no AprilTag/ChArUco code. `sim/` is a MuJoCo smoke test. This is a blocker for the left-arm test plan even though it is not a defect in the existing code. **Smallest fix:** build a standalone single-drive bench runner that bypasses Berthier and logs command+reply at 1 kHz, and a `vcan` emulator driven by MuJoCo; keep the 200 Hz runtime for the arm.

**B7. First GravityComp on the left arm has no automatic wrong-sign protection.** `wrong_sign_watchdog` is disabled in `config/control.yaml:269-274`, and the left-arm `direction` signs are unknown until the sign check (`SOFTWARE_PREP.md:53`). A wrong `direction` in GravityComp makes τ_g push the arm the wrong way; the Position fuses (`docs/safety.md:216-263`) only arm in Position, so they do not catch it. **Smallest fix:** enable a per-joint overspeed/opposition check for GravityComp during bring-up, or require a TorqueOnly sign pulse (verified against the URDF) before GravityComp is allowed. The operator is currently the only layer.

### Major

**M1. The reference workflow is over-built for the task.** The physical grant is sound in principle (UID, type-2 ack after SetZero, 0x7019 readback), but it is wrapped in a process-local journal, virtual reference, model/codec layers and seven `reference_journal_*` test files (`crates/davout/src/`), plus ADRs 0022–0036. `docs/safety.md:26-27` says the journal is history only and never grants motion. The cost is that every drive power cycle is a re-home, and the 20-cycle soak is the project's hardest test. **Smallest fix:** keep the wire evidence (UID + ack + readback) and delete the journal/virtual layers; a bench SetZero can be a bounded transaction with no durable history.

**M2. The total-torque bound is a host prediction, not a drive cap.** `total_torque.rs:1-13` says Marengo never reads or writes `limit_torque`; `clamp_total_torque` predicts from the latest `q`/`dq` and is documented to understate during a stall, stale pose, or the P2C-01 clipped-velocity band (`total_torque.rs:97-104`; `docs/safety.md:16-17`; CS10 in `docs/status.md:45`). Tenet 3 requires a bound on total torque; the host has one, the drives do not. **Smallest fix:** write `limit_torque` per joint (B1) so the drive clamps even if the host is wrong, and keep the prediction as the second layer.

**M3. Startup on a streaming bus latches Transport and needs manual recovery.** `main.rs:1746` opens the CAN sockets before `ControlLoop::from_repo_with_physical_reference` (1755) parses the URDF/builds the supervisor and `admit_drive_loss` (1770) runs the offline gravity bound. If a previous session left type-24 reporting on, more than 64 frames queue before the first bounded drain, and the first drain latches Transport (`docs/safety.md:373-379`; receive cap `robstride/src/receive.rs:7-8`). The documented recovery is `motor-repl disable` / `pi_protocol_inspect`. **Smallest fix:** open sockets after all startup work, or perform one bounded drain-and-discard before the first judged drain.

**M4. Stops are not scoped to a limb or bus.** `motor-repl disable` and the exit stop read every address from `motors.yaml` (`bins/motor-repl/src/stop.rs:85-95`, `145-178`; `docs/safety.md:556-566`), and `ExecStopPost` runs without an interface argument. Once both arms are in the master config, a left-arm `set-zero` failure or an MCP pre-session disable will disable the right arm too — a stop, but an unexpected drop of an elevated right arm. **Smallest fix:** add `--can-interface`/limb scoping to the exit stop and MCP pre-session disable, and pass it from `ExecStopPost`.

**M5. The "realtime" loop is not real-time and sleeps inside the tick.** `main.rs:2313` paces with `sleep(period - elapsed)` (drift, no catch-up), `BurstPacer::begin_group` sleeps in the tick (`crates/davout/src/burst.rs:34-39`), and `receive::drain` sleeps (`robstride/src/receive.rs:187`). No SCHED_FIFO/mlock. For T4 latency and C7 tracking this matters. **Smallest fix:** absolute-deadline pacing, SCHED_FIFO on the control thread, and move burst pacing out of the tick (or accept a documented latency budget).

**M6. The zero-speed frame in the stop sequence is inert in MIT mode.** `perform_stop_paced` writes `speed_control_at(address, 0.0)` (`davout/src/lib.rs:2734-2741`), which writes `SpeedTarget` (`robstride/src/params.rs:226-236`) but never sets `RunMode=Speed`; `write_enable` sets `RunMode::Mit` (`davout/src/lib.rs:2017`). Inference from vendor semantics: the drive ignores SpeedTarget in MIT mode, so the frame is an extra solicited reply on every stop and adds to the 2-frame FIFO load. **Smallest fix:** drop it or set RunMode first; neutral MIT + Disable is the effective stop.

**M7. Documentation contradicts the code on safety-critical points.** Examples: `docs/safety.md:9` says every joint must be Verified before `Ready`, while `davout/src/lib.rs:9-10` says Enable does not require Ready; `docs/safety.md:354` says sockets open before config, but `main.rs:1694-1753` loads config before opening; `codemap.md:45` advertises a URDF preview while `docs/status.md:19` says there is no URDF render; `docs/status.md:3` says nothing changed since 2026-10-04 while ADR 0040 has a 2026-10-28 amendment. **Smallest fix:** one architecture map, one safety contract, delete stale claims.

**M8. The gateway's credential-free disable is a network hazard if the gateway survives into the lean stack.** `POST /command/disable` needs no credential, no rate limit, and admits clients with no Origin (`docs/safety.md:193-197`; `bins/marengo-gateway/src/http.rs:156`). It cannot move the arm, but it can drop an elevated arm. **Smallest fix:** delete the gateway for the bench, or bind it to loopback and require a token.

### Minor

- `recv_can_id` is stored but never used for routing; only compared in reference continuity (`marengo-config/src/lib.rs:166`; `reference.rs:365`). Duplicate fact, inert.
- `firmware_version` in `motors.yaml` is compared config-to-config, not read from the drive (`reference.rs:366`); the actual version is only read by `protocol-inspect`.
- `var/calibration/zero_registry.yaml` is a retired registry file still tracked (`docs/safety.md:26`).
- `encode_fault_clear` is exported but never called (`robstride/src/lifecycle.rs:23-30`, `lib.rs:80-81`); dead API for a forbidden action.
- `set_hardware_estop` is a dead API (no production caller).
- `config/robot.yaml:23-28` still names `left_elbow`/`left_wrist`; `motors_humanoid.yaml` puts the left arm on `can3` IDs 14–18 (`STATUS.md:47-50`).
- MCP `enable-soak` and `can-up` hardcode `can0` (`tools/marengo-pi-mcp/src/tools/enable-soak.ts:225-231`, `config.ts:84`); `can1` works only in some tools.
- `marengo.proto` carries many `deprecated = true` messages (Jetson, LogSession*, CandumpSummary, etc.).
- `justfile` still has WSL recipes although ADR 0018 retired the WSL checkout (`docs/status.md:59`).
- Chappe log layer caps at 40/s + 10/s (`chappe/codemap.md:9`), so it is a lossy log path; the runtime does not write a candump itself.

---

## 2. Topology

**Actual layout.** `marengo-pi` is one synchronous thread at 200 Hz. It owns every CAN interface named in `motors.yaml` through `SocketCanRouter` (one socket per interface, `robstride/src/bus.rs:1587-1625`), runs Berthier and Davout as in-process libraries, reads stdin on a helper thread, runs the gravity-preflight slice, the actuator overlay, the limit-persist worker, the IMU thread and host-metrics publishing, and connects to Chappe as a client. `marengo-gateway` is a tokio process: Chappe IPC listener at `/run/marengo/chappe.sock`, HTTP 8080/HTTPS, WebTransport 8443, SQLite store (`bins/marengo-gateway/src/main.rs:94-364`). "Consul" is a browser SPA served by the gateway, not a server. `motor-repl` is a separate CLI and the only other process that opens SocketCAN; `disable` deliberately opens unowned sockets (`robstride/src/bus.rs:1346-1348`). `marengo-pi-mcp` is a Node process over SSH that starts `marengo-pi` sessions, runs `candump`, and reads logs. Chappe is an in-process broadcast bus plus a Unix-socket fan-out with latest-value slots and bounded event FIFOs (`crates/chappe/codemap.md`). Davout and Berthier are not processes.

**Where it breaks tenet 5.** The "one realtime process" does own every bus and bus names do come from config (good), but the process is not real-time (M5), and the CAN layer is embedded in a 200 Hz loop that also drains stdin, Chappe command channels, the overlay, persistence and telemetry. `drain_chappe_commands` drains until empty for set-zero and lease channels (`main.rs:752-855`), so a command flood can delay motion. The CAN code is 7.3k lines plus tests, not "small and readable". **Where it breaks tenet 4.** Gateway, Consul, store, deploy, limit-sync, host-metrics and IMU are not needed for one arm's tests; the reference journal and virtual-history stack are not needed for the left arm.

**Smallest topology that meets the tenets for one arm and scales to 23 joints/five buses.**

1. One `marengo-rt` process, SCHED_FIFO, owning N SocketCAN interfaces named in the skeleton; 200 Hz control, with a separate 1 kHz single-drive bench mode for T1–T6.
2. One skeleton YAML (limbs, joints, actuator type, bus, ID, direction, zero, soft/hard limits, mass/inertia) and one generated model for sim/visualisation; runtime reads only the skeleton.
3. One command socket (stdin or a tiny Unix socket) and one optional MCP wrapper; no Chappe.
4. One log sink: MCAP (commands, replies, faults, state, operator actions) plus journald for process logs; Foxglove/Rerun for viewing.
5. Optional read-only `marengo-view` for the operator; no gateway/Consul in the motion path.
6. Scale by adding bus names and joints to the skeleton; add per-bus threads only if measured bus load demands it. The receive cap (64 frames/256 attempts per poll) and the MCP2515 2-frame FIFO are the real limits, not the process count.

---

## 3. Safety inventory

(a) required by the hardware; (b) cheap and useful; (c) breaks a tenet / should go.

| Mechanism | Where | Class | State |
|---|---|---|---|
| Physical E-stop in motor power | hardware | (a) | Authoritative; sense not wired to Pi (`davout/src/lib.rs:1468-1490`; `docs/safety.md:267`). |
| PDB heartbeat watchdog (`WD_FEED`) | `REV_B_PARTS.md:113` | (a) | **Missing.** No GPIO code; the 1 Hz Chappe heartbeat (`main.rs:1324`) is not a watchdog feed. |
| Drive CanTimeout ≤50 ms, read back before enable | `SOFTWARE_PREP.md:83` | (a) | **Missing.** Code never writes/reads 0x7028; drives read 0 out of the box. |
| Drive torque/current/speed caps | `SOFTWARE_PREP.md:84-86` | (a) | **Missing.** `ParameterId` lacks limit_cur/damper/VBUS; no preflight. |
| Anti-backdrive damper read = 0 | `SOFTWARE_PREP.md:87` | (a) | **Missing.** No 0x702A in the driver. |
| Bus voltage 22–26 V check | `SOFTWARE_PREP.md:88` | (a) | **Missing.** No 0x701C. |
| Drive settings checked before every enable | tenet 3 | (a) | **Missing** (B1). |
| Soft limits inside every hard stop | `build_limits`, `davout/src/lib.rs:3602-3622` | (a) | Present. |
| Bound on total commanded torque | `total_torque.rs` | (a) | Present but predictive only (M2). |
| Stop on any fault, never auto-clear | `faults.rs`, `require_fault_clear` | (a) | Present; no fault-clear frame is ever sent. |
| Stop when a drive goes silent | `check_comm_watchdog`, `davout/src/lib.rs:2347-2421` | (a) | Present. |
| Measured position fault (hard + slack) | `armee_kinematics::measured_position_fault` | (a) | Present. |
| Feedback velocity fault (3 trips, cap + 0.5) | `davout/src/lib.rs:375-378` | (b) | Present. |
| Danger zones (`clamp_velocity`, `clamp_torque`) | `control.yaml:256-264`; `davout/src/lib.rs:3366-3410` | (b) | Present; `clamp_velocity` cannot brake at kd=0 (CS14). |
| Wrong-sign watchdog | `control.yaml:269-274` | (b) | **Disabled**; gap for left-arm GravityComp (B7). |
| Position fuses (ascent/return/hold/wave) | `berthier/src/position_hold.rs`; `docs/safety.md:216-263` | (b) | Present; Position only. |
| Gravity saturation preflight | `bins/marengo-pi/src/gravity_preflight.rs` | (b) | Present; blocks offline limbs (B3). |
| Enable completion gate | `bins/marengo-pi/src/enable_gate.rs` | (b) | Present. |
| Physical reference grants | `davout/src/reference_physical.rs`; ADR 0036 | (a/b) | Present but over-built (M1). |
| Persistent fault authority | `davout/src/faults.rs`; ADR 0020 | (a) | Present. |
| Transport latch on error/overflow/foreign writer | `robstride/src/bus.rs:120-176`, `receive.rs` | (b) | Present; strict, causes manual recovery. |
| Single-drive-loss degraded hold | `davout/src/drive_loss.rs`; ADR 0038 | (b) | Present; useful for elevated unattended runs. |
| Motion-owner lease (stdin vs Chappe) | `bins/marengo-pi/src/motion_owner.rs` | (b) | Present; unnecessary in a single-command lean stack. |
| Credential-free disable | gateway `http.rs:156` | (b/c) | Useful stop, but network hazard (M8). |
| Panic/EPIPE stop | `run_with_panic_stop`, `main.rs:494-504` | (a) | Present. |
| Exit reporting Off | `release_reporting_for_exit`; `docs/safety.md:346-379` | (a) | Present; guards a real bench failure. |
| Startup streaming-bus latch | `docs/safety.md:373-379` | (b) | Present but forces manual recovery (M3). |
| Stop on Chappe broadcast lag | `docs/safety.md:188` | (b) | Present. |
| Zero-speed frame in stop | `davout/src/lib.rs:2734-2741` | (c) | Inert in MIT mode (M6); drop. |

**Missing/wrong today:** drive CanTimeout, torque/current/speed caps, damper/VBUS readback, preflight-before-enable, PDB heartbeat, E-stop sense, wrong-sign protection in GravityComp, and a real-time/independent watchdog. Those are the tenet-3 "drives" and "hardware" layers, and they are the ones that hold when the Pi crashes.

---

## 4. Control

| Task | Needed modes / laws | Not needed |
|---|---|---|
| Sign / zero | **TorqueOnly** for the small signed τ pulse (kp=kd=0, τ_ff); **GravityComp** for support; no Position. SetZero needs no motion mode. | Position planner, wave, fuses. |
| T1 friction/inertia | Velocity ramps and constant-speed passes, plus current steps. The code has **no velocity mode command**: `speed_control_at` writes SpeedTarget but never sets `RunMode=Speed` (`params.rs:226-236`; `davout/src/lib.rs:2017`). Need a raw velocity/current bench mode. | Position hold, gravity. |
| T2 torque truth | **TorqueOnly**, τ_ff steps, kp=kd=0. | Position planner. |
| T3 backlash/stiffness | **TorqueOnly** slow ± ramps. | Gravity. |
| T4 latency | MIT step or TorqueOnly step; needs a 1 kHz path. | Position planner. |
| T5 thermal | **TorqueOnly** held torque at stall. | Motion. |
| T6 chirp/multi-sine | **Impedance/Position** with settable kp/kd and a chirp target. | Wave. |
| C0–C8 | **GravityComp** for support, **Position** for holds/moves, **Impedance** optionally; danger zones; position fuses for unattended runs. | Wave until gesture iteration; legacy Position law; friction assist; right-arm `scaled_pd` tuning. |

The current runtime is missing a velocity/current bench mode and a 1 kHz command path, so T1 and T4 cannot be run as specified.

---

## 5. Config and URDF

**Flow.** `robot.yaml` `robot.joints` becomes `ControlLoop::joint_names` (`berthier/src/loop.rs:496-499`) and the gravity model's joint order. `robot.yaml` `urdf` resolves the URDF; `armee-kinematics` reads `<limit>` and `armee-dynamics` reads inertial/COM. `motors.yaml` gives Davout the joint→address map, `direction`, `gear_ratio`, motor type and bench hard bounds. `control.yaml` gives soft bounds, gains, velocity caps, danger zones and drive-loss settings. `homing.yaml` gives reference methods and the journal path. Davout builds hard = `URDF ∩ motors.bench` (`davout/src/lib.rs:3579-3580`), effort = `URDF ∩ bench ∩ robot.bench` (`3634-3638`), velocity from `control.yaml` (`3633`), soft from `control.yaml` (`3602-3607`).

**Same fact in more than one place (tenet 2):**

| Fact | Places |
|---|---|
| Joint list / names | `robot.yaml:8-28`, `motors.yaml`, `control.yaml:57-254`, `homing.yaml:15-24`, URDF, `assets/mjcf/marengo.xml`, `hardware/docs/kinematics.md`, `hardware/STATUS.md:33-40`, `robot_humanoid.yaml`, `motors_humanoid.yaml` |
| Position limits | `motors.yaml` `bench.position_lower/upper`, URDF `<limit>`, `control.yaml` soft bounds, `hardware/STATUS.md` table |
| Torque limits | `motors.yaml` `bench.torque_limit_nm`, `robot.yaml` `bench.max_joint_torque_nm`, `control.yaml` `tau_ff_max_nm`, URDF `effort` |
| Velocity limits | `control.yaml` (the only enforced source, ADR 0010), `motors.yaml` `bench.velocity_limit_rad_s` (ignored), URDF `velocity` |
| Masses/inertia | URDF, `hardware/designs/left-arm/urdf/mass_overrides.json`, `MEASURED_MASSES.md`, MJCF |
| CAN IDs | `motors.yaml` `device_id`, `hardware/docs/decisions/0001-can-and-motors.md`, `hardware/STATUS.md:33-40`, left-shoulder harness docs, `motors_humanoid.yaml` (can3, 14–18) |
| `recv_can_id`, `firmware_version` | `motors.yaml`; not used for routing; firmware not read from the drive |

The left arm also has a **separate URDF** (`hardware/designs/left-arm/urdf/left_arm.urdf`) that the runtime cannot load alongside `assets/urdf/marengo.urdf`; `robot.yaml` has one `urdf` path. To run both arms, the left joints must be merged into the master URDF (the 841-line `urdf_merge.rs` is a preview/apply tool, not a runtime multi-model loader). This is a direct consequence of B4.

---

## 6. Hard-won knowledge a rewrite must keep

Lift these before deleting anything:

1. **Post-SetZero blackout.** 45–61 ms of silence starting 511–543 ms after a SetZero (once 614 ms); no Enable or type-24 write from 450 ms to 800 ms. `POST_SET_ZERO_QUIET` / `POST_SET_ZERO_BLACKOUT_FROM` in `crates/davout/src/lib.rs:177-203`; profile `docs/commissioning/firmware/robstride-timing-profile.json`; guard `crates/davout/tests/firmware_profile.rs`.
2. **Type-24 Off before every Enable, read its echo.** `davout/src/lib.rs:1643-1680`, `issue_target_reporting_offs` at `1970-2000`; the 2026-10-03 14:55:24 Reset failure is in `docs/safety.md:323-345`.
3. **Burst pacing 2 ms per interface group.** `crates/davout/src/burst.rs:23`; the 15-frame/15-reply stop overrun is documented there and in `docs/safety.md:497-506`.
4. **Enable + RunMode MIT as one sequence, staggered one target per interface per period.** `write_enable` at `davout/src/lib.rs:2002-2020`; `issue_due_enable_writes` at `2029-2047`.
5. **RS03 velocity scale ±20 rad/s, not ±50.** `crates/robstride/src/motor_type.rs:40-53`; bench capture `cd-20261003T145133Z`; `docs/safety.md:270-280`.
6. **RS00 scale dispute.** `motor_type.rs:21-32` — do not copy the 50/17 values to the left forearm without a bench check (B2).
7. **CanTimeout facts.** Out-of-box 0, PR #254 wrote 600 (~30 ms), survival across power cycle unknown (`docs/commissioning/2026-10-02-right-arm-pr254-findings.md:19-26`; `docs/safety.md:590-599`).
8. **Own-message echo and foreign-writer detection.** `robstride/src/bus.rs:84-176`, `1208-1228`, `1470-1565`; a host command this socket did not send fails the receive. This is the only way the host orders its writes against drive traffic.
9. **Bounded receive 64 frames / 256 attempts.** `robstride/src/receive.rs:7-8`; the 2026-10-04 startup failure is `docs/safety.md:346-379`.
10. **SetZero evidence and grant revocation.** `reference_physical.rs`; UID, type-2 ack, 0x7019 readback, coordinate epoch. Even if the journal goes, the wire evidence is the hard-won part.
11. **Total-torque clamp behavior.** `total_torque.rs:91-181`; the P2C-01 decision is in `docs/safety.md:16-17` and `command_admission.rs`.
12. **Position-law bench tuning.** `control.yaml:91-156` comments and ADR 0039/0040; this is right-arm-specific and should be re-fit for the left arm, not copied.

---

## 7. Deletion plan

**Migrate first (must survive):**

- Drive preflight (B1) — new code, not a migration.
- Wire evidence for SetZero: UID, type-2 ack, 0x7019 readback, coordinate epoch (`reference_physical.rs`).
- `POST_SET_ZERO_QUIET`/`FROM`, burst spacing, Enable+RunMode order, type-24 Off-before-Enable, echo ledger, receive bounds (Q6).
- RS03/RS02/RS00 scales and the firmware-profile test.
- Comm watchdog, fault authority, measured-position fault, total-torque clamp, stop sequence, drive-loss degraded hold.
- `motor-repl disable` and `protocol-inspect` (independent stop and read-only inspection).
- The minimal config schema (one skeleton) and the gravity model.
- The MCAP/log schema decision (so old sessions remain readable).

**Delete outright once the left arm passes:**

- `crates/chappe`, `bins/marengo-gateway`, `consul/`, `crates/marengo-store`, `bins/marengo-log-cli`, `crates/marengo-deploy`, `crates/marengo-host-metrics`, `crates/marengo-imu`, `bins/imu-probe`, `bins/marengo-limit-sync`, `tools/limit-sync-local`, `tools/compound-auto-learn`, `tools/marengo-research-mcp`.
- `davout/src/reference_journal*`, `reference_model.rs`, `reference_codec.rs`, `reference_urdf_codec.rs`, `simulation*.rs`, and the virtual-reference ADRs 0026–0035.
- `marengo-config/src/{limit_patch,profile_txn,urdf_merge,commissioning_scope}.rs`.
- `berthier` wave, legacy Position law, friction assist, and the right-arm `scaled_pd` block until re-fit.
- `sim-harness`, `sim/` smoke (keep only a new vcan HIL harness).
- `config/robot_humanoid.yaml`, `config/motors_humanoid.yaml`, `var/calibration/zero_registry.yaml`.
- Deprecated proto messages; Jetson/WSL ADRs; `docs/roadmap.md` or trim it to current decisions (tenet 4).
- `justfile` WSL recipes; daily-audit/legacy scripts.

---

## 8. CAN layer

**Path end to end.** `marengo-pi` opens one `SocketCanBus` per configured interface via `SocketCanRouter::open` (`robstride/src/bus.rs:1587-1625`); each socket claims an abstract Unix name `marengo/can-owner/<iface>` (`1280-1298`), enables loopback + own-message echo (`1208-1213`), kernel RX timestamps (`1218-1221`), the drop counter (`1225-1228`) and accept-all error frames (`1372-1374`). TX is one atomic nonblocking write (`1409-1459`); every write is recorded in an `EchoLedger` and matched when it comes back. RX is one `recvmsg` per attempt, bounded by 64 frames/256 attempts (`receive.rs:68-199`), with kernel timestamps mapped onto `Instant` (`rx_time.rs`). Davout drains, decodes status/type-24/fault/identity/parameter replies, projects joint space with `direction`/`gear_ratio`, filters commands, and writes MIT batches. Enable is staggered; type-24 Off precedes each Enable; reference bursts are paced at 2 ms. Stops write speed-zero, neutral MIT, Disable per address, unpaced for faults.

**Worst failure modes.**

- **MCP2515 2-frame FIFO overrun.** Any burst that solicits >2 replies faster than the IRQ thread drains it latches Transport persistently (`docs/safety.md:539-551`; `docs/status.md:44` cause B). Cause B is still open.
- **Startup on a streaming bus** (M3): >64 queued frames latch Transport and require `motor-repl disable`.
- **Lost bus / interface down.** A write fails, Davout latches Transport and attempts `disable_all`; the stop writes also fail. The drives rely on CanTimeout, which is not written (B1). The process stays Disabled; recovery is manual.
- **Foreign writer.** Another `cansend`/`motor-repl`/second owner frame fails the receive and latches Transport (`bus.rs:126-148`). This is deliberate but makes ad-hoc debugging a stop.
- **Drive power cycle.** SetZero persistence is unknown (`ADR 0036:347-350`); the grant is revoked on coordinate discontinuity/UID change, so the joint must be re-homed. CanTimeout must be re-verified (B1).
- **Host stall.** The loop sleeps in the tick and has no RT priority; a stall over the drive's ~30 ms CanTimeout drops the drives (if 600 survives), and the first post-stall batch is computed from a stale pose (`docs/safety.md:478-496`).

**Reproduce in the rewrite:** 29-bit packing and per-type scales; Enable+RunMode; type-24 Off-before-Enable and echo; post-SetZero quiet; 2 ms burst pacing; own-message echo/foreign-writer detection; bounded receive; error-frame latch; stop sequence (neutral MIT + Disable); no type-22, no ZeroSta/AddOffset, no fault clear; addressed per-interface routing. **Drop:** the speed-zero frame (M6), the journal/virtual reference layers, the Chappe coupling, the 7-test-file journal suite, and the right-arm-specific tuning.

---

## 9. Speculative scaffolding (tenet 4)

Delete or demote:

- **Virtual reference / history:** `davout/src/simulation*.rs`, `reference_model.rs`, `reference_codec.rs`, `reference_urdf_codec.rs`, `reference_journal*`, ADRs 0026–0035. `docs/safety.md:26-27` says the journal is history only; the left arm does not need it.
- **Dead APIs:** `encode_fault_clear` (`robstride/src/lifecycle.rs:23-30`), `set_hardware_estop` (no production caller), `solicit_status_feedback` if the Hardware page goes.
- **Config/UI plumbing:** `marengo-config/src/{limit_patch,profile_txn,urdf_merge,commissioning_scope}.rs`; `bins/marengo-limit-sync`; `tools/limit-sync-local`; `tools/compound-auto-learn`; the gateway deploy/restart/limit-patch routes.
- **Templates that do not boot:** `config/robot_humanoid.yaml` (its own comment says it does not boot), `config/motors_humanoid.yaml` (can3/IDs 14–18 vs hardware can1/11–15).
- **Proto:** deprecated `LogSession*`, `CandumpSummary`, `SessionStart*`, `JetsonPlatformMetrics`, `HostNodeRole.JETSON`, and the deprecated scalar fields.
- **Docs:** ADR 0014 (Jetson), ADR 0016 (WSL), `docs/roadmap.md` future milestones, `docs/wsl-setup.md`, `justfile` WSL recipes, `docs/ai-sdd.md`/daily-audit if not used.
- **Leftover files:** `var/calibration/zero_registry.yaml` (retired), `assets/urdf/archive/*` seeds if not used by tests.

---

## 10. Tests

**Guard a real failure (keep):**

- `davout/tests/physical_firmware/` (FirmwareBus: echoes, latency, blackout, 2-buffer FIFO) and `firmware_profile.rs` — these encode the bench timing.
- `davout/tests/command_admission.rs` — total-torque, envelope, P2C-01.
- `davout/tests/drive_loss.rs`, `fault_authority.rs`, `fault_snapshot.rs`, `receive_bounds.rs`, `exit_reporting.rs`.
- `robstride/tests/transport_regressions.rs`, `receive_contract.rs`, `feedback_report.rs`, `rs03_velocity_scale.rs`, `command_validity.rs`.
- `berthier/tests/tick_error_intent.rs`, `feedback_failure_propagation.rs`, `drive_loss_admission.rs`, `hold_wave_admission.rs`, `gain_validity.rs`.
- `marengo-pi/src/stop_path_tests.rs`, `enable_gate_tests.rs`, `gravity_preflight_tests.rs`, `shutdown_tests.rs`.
- `marengo-store/tests/migration_*` guard real crash/recovery behavior, but the store itself should go; migrate the migration tests only if a new store is kept.

**Over-built / coverage-only (delete with their subsystem):**

- The seven `reference_journal_*` test files in `davout/src/` and the `reference_*` integration files for history/virtual continuity. They test a subsystem that should not survive.
- `davout/tests/limit_restore_*` and `reference_model_*` — tied to the journal/Set Limits workflow.
- `berthier/tests/cs24_*` (5 files) and `simulation_*` — tied to the virtual/simulation path and the old control audit.
- `sim-harness` URDF/MJCF parity tests if the sim harness goes.
- Tests that pin exact wording (many `assert!(text.contains("..."))` in stop/enable tests) are contract tests, not pointless, but they will need rework in a rewrite; do not port them blindly.

I did not run any test, so this is a judgment from file names, headers and sampled bodies.

---

## 11. Logging and observability

**Current stack.** `tracing` → `ChappeLogLayer` publishes `LogEvent` on `logs/structured` (40/s, plus 10/s warn/error; `chappe/codemap.md:9`); `marengo-pi` also publishes `SafetyState` at 25 Hz and a 1 Hz heartbeat; `RobotState` comes from Berthier. The gateway writes structured logs and session artifacts to SQLite (`marengo-store`) and streams to Consul over WebTransport/HTTP. `PositionTrace` writes a CSV at up to loop rate when `MARENGO_POSITION_TRACE` is set (`berthier/src/position_trace.rs:38-63`). `candump` is started by the MCP for sessions, not by the runtime. There is no OpenTelemetry, MCAP, Prometheus, Loki or Sentry (grep).

**Replace with off-the-shelf:**

- `tracing-opentelemetry` + OTLP collector → Loki/Tempo/Prometheus/Grafana for logs/metrics/traces; one clock, one query surface.
- MCAP for CAN frames, commands, replies, state and operator actions; Foxglove or Rerun for replay. This replaces `marengo-store`, `marengo-candump` (most of it), `marengo-log-cli` and the gateway log routes.
- journald for process logs and crashes; `systemd-coredump`; Sentry if desired.
- `node_exporter` for host metrics; `chrony`/PTP for clocks (the hardware doc already asks for this).
- Keep a small `firmware-timing` analyzer if it produces a number a loop depends on; port it to Python/`python-can` rather than the Rust candump crate.

---

## 12. Agent onboarding

The docs are voluminous and the layer map is mostly there (`codemap.md`, crate `//!` headers, `docs/architecture.md`), so an agent can learn the nominal path quickly. The problem is that the docs describe a more capable system than the code:

- `docs/safety.md:9` "every configured joint must reach Verified before supervisor Ready" vs `davout/src/lib.rs:9-10` "Ready is a marker ... Enable does not require".
- `docs/safety.md:354` "marengo-pi opens its sockets before loading config" vs `main.rs:1694-1753` (config first, socket at 1746).
- `codemap.md:45` "URDF preview" vs `docs/status.md:19` "There is no URDF render".
- `docs/status.md:3` "nothing changed since 2026-10-04" vs ADR 0040's 2026-10-28 amendment.
- `docs/architecture.md:52` "one master description (config/ + URDF)" vs tenet 2's "one YAML".
- `AGENTS.md` mixes stable contracts with implementation detail and right-arm-specific commands; a new agent cannot tell which is which without reading the code.
- `docs/safety.md` is 603 lines of incident history; it is valuable but not a map. Tenet 7 asks for "one short architecture map, kept current" — the repo has several long ones.

The stale claims that will waste the most time: the drive preflight (docs imply it exists; it does not), the E-stop (docs say not wired, code has a dead API), and the "one realtime process" (it is not RT). Fix: one 1-page map, one safety contract, and move incident history to `docs/reviews`.

---

## 13. What is missing (proven off-the-shelf)

| Tool / project | Replaces | Why better | Catch |
|---|---|---|---|
| **SocketCAN + gs_usb USB-CAN adapters** (CANable 2.0, candleLight, PEAK PCAN-USB) | MCP2515 SPI HAT and its 2-frame FIFO | Real RX buffers, per-bus isolation, hot-plug, no SPI IRQ contention | Buy one per bus; 5 buses is real money; CAN-FD adapters cost more |
| **can-utils** (`candump`, `cansend`, `canbusload`, `cansniffer`, `cangw`) | `marengo-candump`, MCP candump wrappers | Standard, fast, scriptable, already in every Linux distro | Log format/analysis still needs a parser; `candump` is not a real-time logger |
| **python-can + cantools** | Custom candump parsing, ad-hoc decode | Mature, DBC support, easy analysis scripts | Python on the Pi is fine for offline, not for the 200 Hz loop |
| **RobStride vendor Motor Studio / SDK** | Hand-derived protocol constants | Vendor-maintained, can confirm frames/scales | Windows/desktop tool, not a runtime; verify against the manual |
| **MCAP + Foxglove / Rerun** | `marengo-store`, gateway log routes, `marengo-log-cli` | Standard robotics log format, time-synced multi-stream replay, good UI | Another dependency; schema discipline needed |
| **OpenTelemetry + Grafana/Loki/Prometheus/Tempo** | Chappe log layer + SQLite + host metrics | Standard, one clock, alerting, no home-grown retention | Collector/agent footprint; not a hard-real-time path |
| **systemd `WatchdogSec` + `sd_notify`** | Ad-hoc 1 Hz heartbeat | The init system kills a hung process and runs `ExecStopPost` | Needs a `sd_notify` crate and a real watchdog thread |
| **`cyclictest` / `rt-tests`, `perf`, `bpftrace`** | Hand-rolled loop timing counters | Measure the actual jitter instead of guessing | Not a fix by itself; requires RT priority to matter |
| **`chrony` / PTP (linuxptp)** | Ad-hoc timestamps | Shared clock across Pi/Jetson, required by tenet 8 | Network setup; PTP needs hardware timestamping for <1 ms |
| **OpenCV + AprilTag (`pupil-apriltags` / `opencv-contrib` aruco)** | The unbuilt camera calibration service | Proven detectors, pose estimation, calibration | Camera intrinsics and lighting still need care |
| **MuJoCo + a `vcan` RobStride emulator (python-can)** | The unbuilt HIL harness | Test stop rules and faults with no motor at risk | Emulator must match the vendor timing/quirks; it can lie |
| **`ros2_control` hardware-interface pattern** (pattern, not necessarily ROS) | Ad-hoc mode/state machine | Separates hardware lifecycle (configure/activate/deactivate/error) from control; proven on robots | ROS 2 is a large dependency on a Pi; borrow the lifecycle pattern without the framework |
| **`ostree`/`RAUC`/Mender** | `marengo-deploy` self-update | Atomic A/B updates, rollback | Overkill for a bench; useful when the robot leaves the bench |
| **SQLite/Parquet + DuckDB** | Custom store queries | Fast offline analysis of MCAP/CSV | Not a real-time store |

---

## 14. Anything else that will make left-arm bring-up fail or waste time

1. **RS00 scale (B2)** — fix before the first forearm yaw command; the left arm has one.
2. **No drive preflight (B1)** — the first powered left-arm session will run with CanTimeout=0 unless someone writes it by hand.
3. **Offline left arm blocks the right arm (B3)** — do not add the left joints to the master config until enable/preflight is scoped, or you will spend a day on `MARENGO_JOINT_SUBSET`.
4. **One URDF path (B4)** — `left_arm.urdf` cannot coexist with `marengo.urdf`; merging is a hardware-side regeneration, not a software feature.
5. **No vcan emulator (B6)** — the stop rules and preflight cannot be tested safely before power; this is the cheapest risk reduction and it does not exist.
6. **No 1 kHz runner (B6)** — T1/T4 as specified cannot run on a 200 Hz loop.
7. **SetZero persistence is unknown (ADR 0036:347-350)** — every drive power cycle is a re-home; C5's power-cycle repeatability test may fail for software reasons, not hardware.
8. **IDs 14/15 are proposals and old IDs may persist** — there is no "scan all drives / set device ID one at a time" tool; `SOFTWARE_PREP.md:59-64` asks for it. The left drives may still hold 11–14 from earlier builds.
9. **`can1` shares the MCP2515 HAT** — cause B applies; if the soak shows overruns, buy the USB adapter before blaming the arm.
10. **Cross-limb stop (M4)** — a left-arm `set-zero` failure will disable the right arm if both are in `motors.yaml`.
11. **`motor-repl disable` writes type-24 Off to every address** — on a shared config it can stop the right arm's reporting as well; scope it.
12. **No RT priority** — the first T4 latency measurement will be dominated by scheduler jitter, not the drive.
13. **Left-arm masses are placeholders** (hand 200 g, forearm pack 300 g; `SOFTWARE_PREP.md:180-182`) — do not run GravityComp on the printed arm until the URDF is re-exported and weighed; τ_g will be wrong.
14. **Right-arm COM was fitted while the shoulder rubbed** (`hardware/STATUS.md:15`) — do not copy right-arm gravity parameters to the left arm.
15. **The left arm mirrors the right, but `direction` signs are unknown** — a wrong sign plus disabled wrong-sign watchdog (B7) is the most likely way to break a printed part on first power.
16. **Docs say "homing before motion" but the code's gate is a process-local reference grant** — an agent following the docs will try a fresh `home` in a new process and be refused (`docs/safety.md:64-68`); put the actual one-process rule in the architecture map.
17. **`config/robot.yaml` still has `left_elbow`/`left_wrist`** — a rename must happen in one commit across `robot.yaml`, `kinematics.md`, `motors_humanoid.yaml`, the playbook and the URDF; otherwise `validate_safety_config` fails or the wrong joint name reaches the wire.
18. **`docs/status.md` is stale** — the next agent will believe the 2026-10-04 checkpoint and the "nothing changed" line; update it before the left-arm work starts.

**Bottom line.** Keep and slim Berthier/Davout/robstride/config/kinematics/dynamics and `motor-repl`; rebuild the CAN layer as tenet 5 says; delete Chappe, the gateway, Consul, the store, deploy, host-metrics, IMU, limit-sync, research/compound tools and the virtual-reference/journal stack. Before any of that, implement the drive preflight and fix the RS00 scale, or the left arm will be tested on a stack that cannot protect it when the Pi is not watching.