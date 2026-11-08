# Marengo software foundation review against the eight tenets (read-only, snapshot 614371d3)

**Bottom line.** The owner is right that the stack is over-built, but only in part. The host side has far too much code: reference grants and journal, leases, degraded modes, five sources of truth for limits, and about 20k lines of home-grown logging. The drive safety layer (tenet 3, layer 2) is missing entirely: no drive timeout, torque, current or speed limit is ever written or checked before enable. Adding the left arm to the current stack is not "mostly configuration", as SOFTWARE_PREP says: the enable gate refuses 10 joints, and it refuses any offline joint (finding B3).

**How to read this.** **[V]** means verified in code. **[D]** means taken from docs or names only. I read the packet, the tenets, the hardware docs it lists, the core docs and the config files myself. The code was read by five parallel read-only passes (CAN path; safety; config, URDF and control; topology and logging; tools, tests and docs). I re-checked the load-bearing [V] claims myself: the CanTimeout, limit and fault-clear greps, `params.rs`, the preflight caps, the torque-cap composition, `set_hardware_estop` callers, systemd units, `can-up.sh`, the tracing caps, the panic stop, the RS00 scale and the burst pacer.

---

## Q1. Verdict per component (left-arm stack)

| Component | Verdict | Tenet | Evidence |
|---|---|---|---|
| crates/robstride (7.3k) | **Rebuild**; lift the codec first | 5 | Good codec: `comm.rs:81`, `mit.rs:111-176`, `motor_type.rs:19-61`, `feedback.rs:44-53`, `version.rs`, `identity.rs`. Missing params 0x7018, 0x702A, 0x701C, 0x701A (`params.rs:23-43`). Fault-clear encoder is never used (`lifecycle.rs:55`). No ID scan or set-ID: type 7 is refused (`bus.rs:45-53`). |
| crates/davout (36k, ~18k tests) | **Rebuild** | 3, 4, 5 | Fault latch, envelope, torque prediction and UID check are sound. About 9k production lines are class (c): reference/grant/journal stack ~6k, AR leases ~500, `drive_loss.rs` 457, danger zones, wrong-sign. Layer-2 checks are absent from `enable_targets_inner` (`lib.rs:1612-1715`). |
| crates/berthier (22k) | **Rebuild, much smaller** | 4 | Two Position laws coexist: `position_hold.rs` ~2k plus `position_law.rs` 458, selected by `control.yaml:111`. Ascent-recovery/breakaway workarounds, `degraded.rs`, `gain_runtime.rs` 764. Hard-wired to 200 Hz (`gain_runtime.rs:468`, `position_law.rs:45`). |
| crates/chappe (3k) | **Delete**; replace with the one RT interface | 5, 8 | Two ingress paths (stdin and 6 IPC topics, `topics.rs:45`). Wall-clock command freshness (`ipc.rs:300-324`). Log layer caps WARN/ERROR at 10/s (`tracing_layer.rs:16-20`). |
| crates/armee-proto (85) + proto/ | **Keep but slim** | 4 | 12 messages with zero refs, 5 oneof variants rejected "until PR-5" (`gateway actuator.rs:62-68`), ~20 deprecated fields. |
| crates/armee-kinematics (1.2k) | **Keep but slim** | 2 | Envelope logic is useful. It reads limits from URDF `<limit>`/`<safety_controller>` (`lib.rs:150-154`); limits must come from the skeleton. |
| crates/armee-dynamics (4k) | **Keep but slim** | 2, 4 | Gravity τ_g and calibration fits are needed for C6. Drop `drive_loss.rs` (255). Feed it a generated model, not the runtime config. |
| crates/marengo-config (6.8k) | **Rebuild** as the skeleton loader | 1, 2 | `urdf_expand.rs:91-145` and `urdf_merge.rs` (841) write config and URDF. Kit machinery: `bench_joints.rs:83-196` (subset and alias table), `commissioning_scope.rs`, three-tier caps. |
| crates/marengo-store (10.3k, 6.9k tests) | **Delete** | 8, 4 | SQLite log store with ADR 0029/0030 recovery (`recovery.rs` 1,185 lines plus 1,354 lines of tests). |
| crates/marengo-candump (1.9k) | **Delete**; keep a ~50-line decode table | 8 | Re-parses candump text; replaced by MCAP or can-utils. |
| crates/marengo-deploy (1.8k) | **Delete** | 4 | Self-update job system for a one-user bench. |
| crates/marengo-host-metrics (1.6k) | **Delete** | 8, 4 | node_exporter covers it. Jetson role arms have no producer (`lib.rs:15,162`). |
| crates/marengo-imu (1.2k) + bins/imu-probe | **Delete** (git history) | 4 | No arm test uses an IMU. Open bugs CS17/CS18. |
| crates/marengo-support (28) | **Keep as-is**, fold into telemetry init | 8 | Trivial. |
| crates/sim-harness (0.5k) | **Delete** | 4 | No dependents; its header says "reserved home" (`lib.rs:6-8`). |
| bins/marengo-pi (14.4k) | **Rebuild** as the one RT process | 5 | Not RT-scheduled; loop paced by `thread::sleep` (`main.rs:2314`). Two motion owners, a stdin grammar, and a 5^n preflight (`gravity_preflight.rs`). |
| bins/marengo-gateway (9.8k) | **Rebuild thin** (UI bridge only) or delete | 4, 5, 2 | About 37 routes. Writes limits and URDF. Has its own safety gates (`restart.rs:49-60`, `deploy.rs:101-110`), a demo publisher and a SQLite writer. |
| bins/motor-repl (1.8k) | **Keep but slim**: keep only `disable`, fold `protocol-inspect` into the preflight | 5 | Second CAN owner that bypasses Davout (`Cargo.toml:27`). `disable` on an unowned socket is the right exit stop (`stop.rs:110-178`). |
| bins/marengo-log-cli (8.7k) | **Delete**, after extracting `gravity-fit` (4.6k) and `firmware-timing` (1.2k) as scripts | 8 | The rest is archive/purge/store plumbing. |
| bins/marengo-limit-sync + tools/limit-sync-local | **Delete** | 2 | Mirrors Pi-taught limits back into git: a second writer of skeleton facts. |
| tools/marengo-pi-mcp (15.6k) | **Rebuild** as a thin client | 5, 6 | About 26 bash builders. Kills the unit and launches a private marengo-pi (`can-owner.ts:120-160`, `env.ts:18`). Safety gates live in the MCP. Joint names and `can0 can1` hard-coded (`bench-profiles.ts:21-25`, `config.ts:84`). |
| tools/marengo-research-mcp | **Delete** from the repo | 4 | Web and arXiv search; no robot code. |
| tools/compound-auto-learn | **Delete** | 4, 3 | An LLM, via `CURSOR_API_KEY`, proposes motion waypoints (`src/prompt.ts`). |
| tools/cad | **Move** to hardware/ | 7 | Lints the SolidWorks tree. |
| consul (35k) | **Keep but slim** (only surveyed) | 4, 8 | Keep stop, live state, preflight and limit views. Delete auto-learn, simulation, the Logs page (use Grafana/Foxglove) and the URDF import wizard. |
| sim/ | **Keep but slim** | 6 | MuJoCo smoke test; rebuild around replay and the vcan emulator. |
| config/ | **Rebuild** into one skeleton YAML | 2 | Five files hold overlapping facts (Q5). |

---

## Ranked findings

### Blockers

**B1. Drive safety layer (tenet 3, layer 2) does not exist. [V]**
- No production code writes CanTimeout 0x7028, `limit_torque` 0x700B or `limit_spd` 0x7017.
- `limit_cur` 0x7018, the damper 0x702A and VBUS 0x701C are not even in `ParameterId` (`crates/robstride/src/params.rs:23-43`).
- 0x7028 is read only by `motor-repl protocol-inspect`, which runs with marengo-pi stopped (`bins/motor-repl/src/main.rs:432` → `davout/src/protocol_inspection.rs:29-36`). Enable never checks it (`davout/src/lib.rs:1612-1715`).
- [D] The right arm's 600 counts (~30 ms) were left by PR #254 and no policy owns them. Whether the value survives a power cycle is unknown (`docs/safety.md:590-599`).
- **Fix:** add the four params. In the preflight, write CanTimeout (≤1000 counts = 50 ms) and the per-joint caps from the skeleton. Read all six back before every Enable and refuse on any mismatch. Re-run after any Reset-mode report.

**B2. A hung loop has no backstop. [V]**
- `marengo-pi.service` has no `WatchdogSec` or `sd_notify`, and there are no grep hits for either.
- `ExecStopPost=motor-repl disable` only runs when the process exits.
- A hang therefore leaves every drive on its last MIT frame, τ_g feed-forward included, until an unmanaged CanTimeout fires.
- **Fix:** add B1, plus `WatchdogSec` with `sd_notify` from the control thread. Later, add the WD_FEED toggle the power board requires (`REV_B_PARTS.md`). No GPIO code exists today [V].

**B3. Adding the left arm to the current stack refuses Enable. [V]**
- The gravity preflight sweeps 5^n over all `robot.joints` and refuses above 1,000,000 samples (`bins/marengo-pi/src/gravity_preflight.rs:29-31,172-185`). Ten joints is 9.77M.
- It also refuses if any joint has no feedback (`:146-152`), so an offline limb blocks Enable. That inverts tenet 1.
- Berthier needs every joint in one URDF with mass (`berthier/src/loop.rs:495-499`), but `left_arm.urdf` is a separate tree.
- **Fix:** on the lean stack, run the preflight per limb chain over online joints only, or make it an offline check per stage plan. Do not try to bolt the left arm onto the old stack.

### Major

**M1. The total-torque "bound" is a host-side prediction, with no drive clamp behind it. [V]**
- `davout/src/total_torque.rs:94-176` predicts the drive's torque and scales q_des/dq_des toward measured q/dq when the prediction exceeds the cap. The cap is the minimum of four sources (`lib.rs:3240-3244`).
- The actual torque can exceed the cap in three ways:
  - the clipped-velocity excess `kd·(|dq|−cap)`, which was accepted (`safety.md:17`);
  - a pose up to `comm_watchdog_ms` = 100 ms old is still accepted while the prediction horizon is about 10 ms (`lib.rs:2325-2337`);
  - between ticks and during stalls.
- Today that leaves CS10 open in practice.
- **Fix:** write `limit_torque` per joint (the elbow ≤ 17 N·m printed-stop rating) and keep the host prediction as the second line.

**M2. The ADR 0038 degraded hold keeps drives energised up to 5 s + 10 s after a drive goes silent. [V]**
- Code: `davout/src/drive_loss.rs:340-372`. Armed for three joints in `config/control.yaml` (`on_drive_loss: shed_subtree`).
- This contradicts tenet 3 ("stop when a drive goes silent") and CALIBRATION_LOOP §9 ("disable at once, fall onto the pad").
- **Fix:** delete the three `on_drive_loss` lines now, then delete about 850 lines.

**M3. Several hardware-required host rules are missing or too loose. [V]**
- Silence stop is `comm_watchdog_ms: 100` (`control.yaml`); §9 requires 50 ms.
- No temperature pause or stop at 60/75 °C. Temperature is only checked for being finite (`davout lib.rs:3477`).
- No VBUS 22–26 V stop; VBUS can't even be read.
- No per-drive fault-bit read before Enable is written.
- **Fix:** 50 ms in config; add temperature and VBUS rules to the RT loop.

**M4. Tenet 2 is broken: the runtime reads and writes its own hardware description. [V]**
- Davout builds limits from the URDF intersected with `motors.yaml` (`lib.rs:675-679, 3579-3607`). Effort caps come from four sources (`:3634-3638`).
- Set Limits rewrites `motors.yaml`, `control.yaml` and the URDF (`marengo-config/src/urdf_expand.rs:91-145`).
- Deploy restores the Pi's taught limits over git (`scripts/install-pi.sh:189-229`, `preserve-taught-limits.py`). [D] ADR 0012 makes the Pi the source of truth.
- Right shoulder pitch "hard" lower limit has three different values in config and models: −1.115 (`motors.yaml:20`), −1.431 (URDF:103) and −0.9 (MJCF:13). `kinematics.md` and Consul each carry their own value too.
- **Fix:** the skeleton holds limits, direction, IDs and buses. Set Limits produces a proposed diff for a commit. The URDF supplies only inertials, for dynamics and sim.

**M5. More than one process writes to the CAN buses, and there are two launch modes. [V]**
- `motor-repl` opens CAN as owner, and also through an unowned socket.
- The MCP stops the unit and starts a private marengo-pi with `MARENGO_MOTION_OWNER=stdin`.
- `candump` runs beside it.
- The owner lock is an advisory abstract socket (`robstride/src/bus.rs:1270-1298`).
- **Fix:** in the rewrite, only the RT process transmits. Keep the unowned `disable` only as `ExecStopPost`.

**M6. No in-process frame log, and no shared clock. [V]**
- Per-frame records exist only at `robstride=trace` (`bus.rs:473,1412,1538`), which the unit's `RUST_LOG` (`robstride=info`) turns off.
- `candump -t z` (zero-based timestamps) runs only inside MCP sessions (`tools/marengo-pi-mcp/src/tools/motion.ts:139`).
- WARN/ERROR events are capped at 10/s (`chappe/src/tracing_layer.rs:16-20`), which contradicts `docs/logging-taxonomy.md:31,38` ("always pass", "never dropped").
- ActionEvent audit events are not persisted.
- **Fix:** write an MCAP file in the RT process: every TX with its write time, every RX with its kernel timestamp, every command, fault and state change.

**M7. RS00 MIT scales are unsettled, and the left forearm is RS00. [V]**
- `motor_type.rs:21-32` uses V ±50 / T ±17, while every manual revision says ±33 / ±14.
- If the manuals are right, commands reach the drive at 0.66× velocity and 0.82× torque, and feedback velocity reads 1.5× high, which risks false velocity faults.
- **Fix:** run a capture on the loose RS00 before any motion (same method as the RS03 fit).

**M8. No tool to scan or set device IDs. [V]**
- Type 7 is refused (`bus.rs:45-53`), and only configured addresses are probed.
- SOFTWARE_PREP item 2 warns that the left drives may hold old IDs (11–14 on can0, 12 on can1).
- **Fix:** a one-drive-at-a-time ID scan and set-ID tool in the new CAN layer.

**M9. The control process is not realtime. [V]**
- No SCHED_FIFO, no mlockall.
- The loop sleeps to pace itself (`main.rs:2314`).
- Inside the tick, `BurstPacer` blocks for 2 ms (`davout/src/burst.rs:34-42`).
- Drives with a ~30 ms CanTimeout drop on any stall longer than that.
- The 1 kHz single-drive runs T1–T6 need are not demonstrated.

**M10. Soft limits sit too close to the hard stops. [V]**
- The default inset is 0.027 rad, about 1.5° (`marengo-config/src/limit_patch.rs:10`). Right roll's soft lower limit is 27 mrad inside the `motors.yaml` hard limit.
- Required: 2–3° supervised (SOFTWARE_PREP item 6) and ≥5° unattended (CALIBRATION_LOOP §9).

### Minor

- **Dead safety code [V].**
  - `set_hardware_estop` has no production caller (`davout lib.rs:1468`).
  - The wrong-sign watchdog is `enabled: false`.
  - The danger zone does nothing while the wire kd is 0 (`safety.md:92`, CS14).
  - Hall homing survives in code (`marengo-config lib.rs:1126`) although `homing.md` says it is retired.
- **Kernel TX queue [V config; queueing effect inferred, not verified].** `can-up.sh` sets `txqueuelen 1000` and `restart-ms 100`. When the bus can't transmit, stop frames queue behind stale MIT frames. Use a short queue (≈16) for the RT socket.
- **Wrong or contradictory docs [V].**
  - `safety.md:594` says a panic is not handled; `main.rs:498-521` does handle it.
  - ADR 0014 says Accepted in its header and "not implemented" in the index.
  - `crates/codemap.md` names `SyntheticBus`, which doesn't exist, and omits marengo-candump.
- **Template config collides with the live arm [V].** `motors_humanoid.yaml` puts the left leg on can0 IDs 1–6, the same bus and IDs as the live right arm. Its `left_elbow` upper limit is 2.5 rad, past the 124° (2.164 rad) printed stop.
- **Order to check against the manual [V order, D whether it matters].** RunMode is written *after* Enable (`davout lib.rs:2016-2017`).
- **Stale MCP install (observed, not code).** The MCP server in this environment still exposes `pi_jog` and `pi_motor_enable`. `AGENTS.md:20` says both were removed.
- **Restart behaviour [V].** `Restart=always` brings up a fresh process. It never auto-enables, but it does reset the in-process fault latch. Drive fault bits stay, because no clear frame is ever sent.

---

## Q2. Topology

**Actual [V]:**

```
Consul (browser) ──HTTPS:8444 / HTTP:8080 / WebTransport:8443──> marengo-gateway (tokio; SQLite; TLS; sudo restart/deploy)
                                                   │ Chappe Unix socket /run/marengo/chappe.sock (6 cmd topics in, 9 telemetry out)
                                                   v
marengo-pi (systemd, Restart=always, --no-stdin-ctl; NOT RT) ── main thread: Berthier→Davout→robstride ── can0, can1
   threads: stdin, IPC r/w, persist worker, journal worker, metrics, IMU
   ExecStopPost: motor-repl disable (unowned TX socket)
Outside the "one interface": motor-repl (owner or unowned), candump, MCP over ssh (stops the unit,
pkill, launches its own marengo-pi as stdin owner), self-update (git+build+install-pi.sh restarts units)
```

Davout is a crate inside marengo-pi, not a separate process.

**Where it breaks tenet 5:**
- Several CAN writers (M5).
- Two launch modes of the "one" process.
- Five ingress channels: stdin grammar, IPC topics, YAML written by the persist worker, signals, and ssh scripts parsing stdout.
- Safety gates that live outside the RT process: the gateway's restart/deploy gates and the MCP's gravity gate.

**Where it breaks tenet 4:** dead proto messages, the unwired tuning path, the "PR-5" rate bucket, the demo publisher, three transports in one gateway, and the store recovery machinery.

**Smallest topology that meets the tenets:**
- **One RT process, `marengo-rt`.** SCHED_FIFO plus mlockall. It loads the skeleton and owns every bus named there, one non-blocking socket per bus polled with epoll.
- **Inside `marengo-rt`:**
  - drive preflight
  - the short safety list
  - control modes
  - the WD_FEED toggle
  - an MCAP recorder fed through a lock-free ring to a non-RT thread
- **One interface:** a single protobuf-over-Unix-socket (or gRPC) endpoint, plus a read-only Foxglove WebSocket for live data.
- **Clients:** a static UI server, a thin MCP, and scripts. None of them is in the stop path.
- **Stop paths:** E-stop, drive CanTimeout, the heartbeat, and `ExecStopPost disable`.
- **Scaling to 23 joints on 5 buses:** five gs_usb adapters, one per bus. Rough load: about 6 drives × 2 frames × 200–500 Hz per bus, roughly 30–60% at 1 Mbit/s. Same process, same code; a new limb is a skeleton edit.

---

## Q3. Safety inventory

Classes: (a) required by the hardware or tenet 3, (b) cheap and useful, (c) breaks a tenet.

| Mechanism | Where [V] | Class |
|---|---|---|
| Fault latch, no auto-clear (ADR 0020) | `davout faults.rs`, `lib.rs:1271` | (a) keep |
| Drive fault bits / Reset-mode → stop | `feedback_consumer.rs:450,493,534` | (a) keep |
| Comm watchdog (silence) | `lib.rs:2325-2347`; 100 ms | (a), but set it to 50 ms |
| Silence revokes reference grant | `reference.rs`, `lib.rs:1031` | (c) duplicates the watchdog |
| Total MIT torque prediction | `total_torque.rs` | (a) as a second line behind the drive's `limit_torque` |
| τ_ff cap + 60 N·m/s rate limit; kp/kd max | `lib.rs:3163,3240-3251` | (b) |
| Velocity cap on dq_des | `lib.rs:3234,3269` | (c) belongs in drive `limit_spd` |
| Measured-velocity fault | `feedback_consumer.rs:840` | (b) |
| Soft-limit envelope (ADR 0009) | `armee-kinematics limits.rs:123-182` | (a) soft limits; (b) the velocity-scaled margin |
| Measured position guard | `feedback_consumer.rs:954` | (b) |
| Danger zone | `lib.rs:3366` | (c) does nothing at kd = 0 |
| Wrong-sign watchdog | `lib.rs:3305` | (c) disabled |
| Degraded hold, ADR 0038 | `drive_loss.rs` etc. (~850 lines) | (c) contradicts tenet 3 (M2) |
| Berthier stall fuses ×4 | `position_hold.rs:204-300` | (b) "never hold a stall"; collapse to one rule |
| Gravity preflight 5^n + enable gate | `gravity_preflight.rs`, `enable_gate.rs` (~750 lines) | (c) too complex; blocks B3 |
| Reference grants + private authority + journal | `reference*.rs` (~6k lines) | (c). Keep the core idea: a zero is valid only if made in this session and the UID matches |
| MCU UID check at Enable | `lib.rs:2064-2166` | (b) |
| Active Reporting leases | `active_reporting.rs` | (c) not safety |
| Motion-owner lock | `motion_owner.rs` | (b); moot with one interface |
| `stop_on_lag`, `forbid_implicit_enable` | `main.rs:693-737`; `berthier loop.rs:1092-1128` | (b). Better: remove implicit enable altogether |
| Credential-free disable route | `gateway http.rs:245,453` | (b) |
| Gateway restart/deploy gates | `restart.rs`, `deploy.rs` | (c) wrong layer |
| `set_hardware_estop` | `lib.rs:1468` | (c) dead |
| Panic stop, SIGTERM stop, `ExecStopPost` disable, StopDelivery latch | `main.rs:498-521,1975-2030`; unit file | (b) |
| Deploy limit preservation | `install-pi.sh:220-227` | (c) tenet 2 |
| Protocol inspection | `protocol_inspection.rs` | Half of an (a): it reads 0x7028 but never gates Enable |
| Post-SetZero quiet, burst pacing | `lib.rs:193-203`, `burst.rs` | Correctness rather than safety; keep the facts |

**Missing or wrong, required by the hardware:**
- CanTimeout written and read back before Enable (B1).
- Drive torque, current and speed caps, plus the damper read (B1, M1).
- A watchdog for a hung process (B2).
- A 50 ms silence stop, temperature and VBUS rules, and a fault-bit read before Enable (M3).
- Soft-limit insets (M10).
- E-stop input and WD_FEED heartbeat. Not needed on the bench; required before the power board.
- The sum-of-drive-current ≤ 90 A read-back at startup (REV_B_PARTS); absent.

---

## Q4. Control

[V] for what exists; the needs are taken from the test docs, which my readers read.

| Need | Modes and laws |
|---|---|
| Sign / zero (playbook ch 1–3) | Limp feedback (type-24 or type-4 poll). In-session SetZero with type-2 ack and 0x7019 readback. TorqueOnly pulse (MIT kp = kd = 0, small τ_ff). Optional slow hold-at. |
| Stand T1–T6 | Raw per-tick MIT (q, dq, kp, kd, τ_ff from a script or chirp generator) at ~1 kHz on one drive. Velocity mode (run_mode 2) for T1 ramps. Current steps (run_mode 3, or τ_ff with kp = kd = 0). `iqf` 0x701A and temperature logging. Caps at test maximum +10%, speed ≤ 1 rad/s. |
| Calibration C0–C8 | Disabled (C0). One Position law with a trapezoid planner and a speed fraction (C1–C4, C7). GravityComp (C6). The reference workflow (C5). |

- **What exists for T1–T6:** run-mode encoders are present (`robstride/src/params.rs:8-13,217-262`) but unused. Davout forces MIT at enable (`lib.rs:2017`). The stand's RS03 is in no URDF, and Berthier refuses joints without mass. **None of the current loop serves T1–T6.**
- **Not needed:** Impedance; wave (until gesture work after C7); the second Position law (pick one of legacy or `scaled_pd`); ascent recovery, breakaway latches and the friction "assist" (right-shoulder rub workarounds); integral variants; degraded hold; Testing-batch gain overrides.

---

## Q5. Config and URDF

**Flow [V]:**
- `robot.yaml` gives the joint list and the URDF path.
- `motors.yaml` gives bus, ID, direction, gear ratio, bench hard limits and torque.
- `control.yaml` gives soft limits, caps, gains and friction.
- `homing.yaml` gives the reference-journal path.
- The URDF is parsed at startup for limits, effort (Davout) and gravity/inertia (Berthier).
- Set Limits and deploy write these files back (M4).

**Same fact in more than one place:**
- **Joint names: about 20 places.** The 4 YAMLs, URDF, MJCF, both humanoid templates and `left_arm.urdf`. In docs: hardware `STATUS.md`, `kinematics.md` and hardware ADR 0001. In Consul: `commissioning.ts:11-24`, `robot-inventory.ts`, `compound-tests.ts`, `bench-arm-scene.ts`. In tools and code: MCP `bench-profiles.ts`, the `bench_joints.rs:185-196` aliases, `gravity_fit.rs:91`.
- **Hard limits:** `motors.yaml`, URDF, MJCF, `kinematics.md` (two values) and Consul. They disagree (M4).
- **Soft limits:** `control.yaml` and URDF `safety_controller`.
- **Torque caps: four sources.** URDF effort, `motors bench.torque_limit_nm`, `robot.bench.max_joint_torque_nm`, `tau_ff_max_nm`.
- **Velocity: four sources.** Two of them are dead: `motors bench.velocity_limit_rad_s` and URDF velocity.
- **Motor type:** `motors.yaml` and `control.yaml`.
- **Device ID:** plus `recv_can_id`, which is derivable from the ID.
- **`firmware_version`:** never checked against the drive.
- **Masses:** URDF, a hand-made MJCF copy, the reference journal's URDF copy, and for the left arm `mass_overrides.json`.
- **Left-arm mismatches:**
  - `robot.yaml:22-28` lists roll before pitch, plus `left_elbow` and `left_wrist`.
  - `motors_humanoid.yaml` puts the left arm on can3 with IDs 14–18.
  - Consul's `MASTER_LIMBS` uses the old names.
  - Hardware says can1, IDs 11–15, with names `left_elbow_pitch` and `left_lower_arm_yaw`.

---

## Q6. Hard-won knowledge to lift before deleting anything [V]

- **Codec.**
  - ID layout and inbound device-id bits: `comm.rs:68-81`.
  - Host 0xFD, identity marker 0xFE: `identity.rs:12`.
  - Version reply `00 C4 56` is not a pose: `bus.rs:791`.
  - 8-byte data-only rule; refuse out-of-range values rather than clamp: `mit.rs:28-60`.
  - Mode and fault bits: `feedback.rs:44-53`.
- **MIT scale table:** `motor_type.rs:19-61`. RS03 V = ±20 with its bench evidence (`tests/rs03_velocity_scale.rs`). RS00 is still open.
- **TX allowlist:** refuse types 7, 22, 23 and 25, and never write 0x7029/0x702B (`bus.rs:45-53`, `params.rs:135`).
- **Firmware timing:**
  - `POST_SET_ZERO_QUIET` 800 ms and `POST_SET_ZERO_BLACKOUT_FROM` 450 ms (`davout lib.rs:193-203`).
  - Evidence: `docs/commissioning/firmware/robstride-firmware-behavior.md`, its profile JSON, and `tests/firmware_profile.rs`.
- **Enable order** (`lib.rs:1612-2020`):
  1. Type-24 Off.
  2. Wait until its echo has been read at least one period earlier.
  3. Enable, then RunMode.
  4. Expect Run only after the Enable echo (`feedback_consumer.rs:356-368,534-552`).
- **Reporting:** streams persist across processes, so write Off at exit (`lib.rs:2971-3021`) and in `motor-repl disable` (`stop.rs`).
- **MCP2515 pacing:** 2 RX buffers. Keep received traffic ≤ ~1.5 frames/ms, stop groups 2 ms apart (`burst.rs:23`), one Enable per bus per tick, type-24 writes 5 ms apart (`active_reporting.rs:29`).
- **Stop sequence:** SpeedTarget = 0, then neutral MIT, then Disable (`lib.rs:2701-2810`).
- **Reference sequence:** `reference_transaction.rs:66-86`.
- **Socket options:**
  - `RECV_OWN_MSGS`, `SO_TIMESTAMPNS`, `SO_RXQ_OVFL`, error subscription (`bus.rs:1208-1225,1350-1390`).
  - Kernel-stamp sanity check (`rx_time.rs`).
  - Bounded drain (`receive.rs`).
  - Transmit-only stop with ENOBUFS retry (`motor-repl/src/stop.rs:110-178`).
- **Drive model to port to the vcan emulator:** `davout/tests/physical_firmware/mod.rs` (807 lines). It models echoes, blackouts, reboots and the 2-buffer `RxFifo`.
- **Bench data:** `var/motion-suite/20261004T202431Z` and the candumps behind `physical_reference.rs:661,1192`. Also the right-arm signs, taught limits and friction fits now in config.

---

## Q7. Deletion plan

**Migrate first:**
1. Everything in Q6.
2. The right arm into the skeleton, as a limb on can0. Otherwise deleting Davout and Berthier strands the right arm.
3. `gravity-fit` and `firmware-timing` as scripts.
4. The intent of the enable-soak and motion-suite loops.
5. Commissioning docs: the limb playbook, the bench ledger and the firmware behaviour doc.
6. An export of the reference journal.
7. `ExecStopPost disable`.

**Then delete outright:**
- **Crates:** davout, berthier, chappe, marengo-store, marengo-candump, marengo-deploy, marengo-host-metrics, marengo-imu, sim-harness.
- **Bins:** marengo-gateway (or keep a thin rebuild), marengo-log-cli, marengo-limit-sync, imu-probe.
- **Tools:** research-mcp, compound-auto-learn, limit-sync-local, the old MCP.
- **Code to keep but cut down:** robstride, after it has been rewritten.
- **Config:** the humanoid templates.
- **Docs and repo clutter:** the Q9 docs, `openspec/`, `review/`, `docs/reviews/` (681 files), `SYNTHESIS.md`, `autoresearch.sh`.

---

## Q8. CAN layer

**Current path [V]:**
1. Berthier produces an MIT batch.
2. Davout filters it (`lib.rs:3163-3290`) and maps joint to motor space.
3. `RuntimeBus` and `SocketCanRouter` send it as one nonblocking `write` per frame.
4. Each frame's echo is matched in a 1024-entry `EchoLedger` (`bus.rs:72-176`).
5. Receive is round-robin, capped at 64 frames and 256 reads per poll (`receive.rs:7-8`).
6. Any error frame, `SO_RXQ_OVFL` drop, foreign writer or cap overrun latches a Transport fault and stops everything.
- Bus names come from `motors.yaml`; `can0 can1` are hard-coded in `can-up.sh`, the systemd unit and the MCP.

**Worst failure modes:**
- A hang with an unmanaged CanTimeout (B2).
- CanTimeout or caps reverting after a drive power cycle, never re-checked (B1).
- Blocking sleeps inside the tick (M9).
- One stray error frame drops the arm.
- [D] RX overrun cause B (a 3.8–5 ms receive blackout after Disable) is still open.

**The rewrite must reproduce:** everything in Q6.

**It should add:**
- Preflight writes and read-backs.
- An MCAP frame log.
- A vcan emulator.
- Kernel CAN ID filters (there are none today).
- An ID scan and set-ID tool.
- Bus names from the skeleton.

**It should drop:**
- The roughly ten echo-ordering state sets, owed-On excuses and solicit ledgers, in favour of one explicit state machine per drive.
- Foreign-writer detection, which is moot with a single writer.
- Unpaced, ad-hoc pacing spread across modules: put it in one per-bus TX scheduler.

---

## Q9. Speculative scaffolding to delete [V unless marked]

- **ADRs:** 0003, 0005, 0014 (Jetson), 0016 (WSL), 0019 (a repair roadmap), 0028–0030 (store machinery). 0039/0040 are proposed for pitch only; decide them or drop them. 0006, 0007, 0009 and 0035 are partly superseded.
- **Proto:** 12 messages with zero refs, the PR-5 oneof variants, the unwired Tuning messages, and about 20 deprecated fields.
- **Code:** Hall homing (`marengo-config lib.rs:1126`, `davout reference.rs:404-409`); `set_hardware_estop`; the wrong-sign watchdog; Jetson roles in host-metrics; the gateway demo publisher; the MOTION rate bucket; sim-harness.
- **Config:** `robot_humanoid.yaml` and `motors_humanoid.yaml`, which say "does not boot"; the dead `velocity_limit_rad_s`; `recv_can_id`; `firmware_version`.
- **Docs:** `roadmap.md` M5–M8, `ai-sdd.md`, `daily-audit*`, `position-hold-control-review.md`, `position-slew-audit.md`, `wsl-setup.md`, `cloud-pi-tailscale.md`, `test-timing.md`, `docs/agents/`.
- **Repo clutter:** `openspec/` (stuck since 2026-08-09), `.cursor/agents` (24 agents), `.agents/skills` (32 skills), WSL scripts and justfile recipes.
- **README entries for things that don't exist:** `models/` (ONNX) and `assets/meshes/`. `cad/` is absent from this packet; the README says it is local-only, so check the real repo.

---

## Q10. Tests

About 1,530 Rust tests, 416 Consul and 326 MCP.

**Guard real failures (keep):**
- `robstride/tests/rs03_velocity_scale.rs` (candump `cd-20261003T145133Z`).
- `berthier/src/position_hold_tests/bench_replay.rs:987` (the 2026-10-04 stall) and `hold_tracking.rs` (the 2026-10-03 pitch trip).
- `davout/tests/physical_reference.rs:661,1192`.
- `transport_regressions.rs`; the robstride vcan tests.
- `firmware_profile.rs`; `service_unit_contract.rs` (RT-02 restart loop).
- `enable-soak.test.ts:134-150` (no motion verb is ever sent).
- `machine-state-safety.test.ts`.

**Coverage or agent artefacts (cut):**
- 22+ files with agent-pipeline headers rather than a guarded failure, e.g. `berthier/tests/cs24_constructor_period.rs:1-2` ("Unexecuted preparation…"), `fuse_audit.rs` and `numeric_contract.rs`.
- marengo-store's ~6.9k lines of recovery and competing-writer tests.
- `reference_clock_boundary.rs` (deadlines near `Duration::MAX`).
- A test that validates the retired Hall method (`safety_validation.rs:639-686`).
- Consul styling tests (`button.test.tsx`).
- About 212 MCP assertions that regex-match generated shell strings.
- `config_joint_coverage.rs`, which string-splices the live `control.yaml`.

**Fragile:** 216 wall-clock sleeps and `now()` calls in tests. The docs admit Berthier tests time out under emulation.

---

## Q11. Logging and observability [V]

**What exists:**
- tracing → Chappe `LogEvent`, capped at 40/s with WARN/ERROR at 10/s → gateway → SQLite with FTS (marengo-store).
- marengo-log-cli: archive, purge, journal import, candump.
- Consul Logs page; host metrics into protobuf; a 37-column position-trace CSV behind an env flag.

**Gaps:**
- No frame record in production.
- Timestamps are `SystemTime` milliseconds. Clock sync is only observed (`host-metrics lib.rs:479-493`): no chrony or PTP setup.
- No OTel, MCAP, Foxglove, Rerun, Prometheus, Loki or Sentry anywhere.

**What off-the-shelf tools replace:** about 20k lines. Store, generic CLI, candump parser, host-metrics, tracing layer, gateway log routes, the Consul Logs UI and the MCP recording shell all go to MCAP/Foxglove plus Loki/Prometheus.

**Keep:** gravity-fit and firmware-timing as scripts that read MCAP.

---

## Q12. Agent onboarding

**No, an agent cannot learn the layers quickly from today's docs.** The basic chain (Consul → gateway → Chappe → marengo-pi → Berthier → Davout → robstride) is easy to find. But there are 86 codemaps, 7 AGENTS.md files, a 25 KB AGENTS.md and a 12 KB CONTEXT.md, and they overlap and disagree.

**Wrong or stale [V]:**
- "Davout is the sole path to robstride" is false: motor-repl depends on robstride directly (`bins/motor-repl/Cargo.toml:27`).
- "Thin bins" is false: marengo-pi is 14.4k lines.
- `crates/codemap.md` names `SyntheticBus` (doesn't exist), gets dependencies wrong and misdescribes marengo-store.
- Crate headers run to 82 lines (davout) instead of "a few lines".
- The left arm is absent from AGENTS.md, README, codemap and `architecture.md`.
- Two status files disagree: `docs/status.md` (as of 2026-10-28) and hardware `STATUS.md` (2026-11-07).
- The `safety.md` panic claim and the logging-taxonomy claims are wrong (above).
- WSL leftovers remain after ADR 0018.

---

## Q13. Proven off-the-shelf tools that would replace home-grown code

| Tool | Replaces | Why better | Catch |
|---|---|---|---|
| gs_usb adapters (CANable 2.0 / candleLight, PEAK PCAN-USB) | MCP2515 SPI HAT | Deeper buffers end the 2-frame-FIFO pacing and cause B; plain SocketCAN | USB latency and jitter (~0.1–1 ms); buy isolated units; one per bus |
| Waveshare 2-CH CAN FD HAT (MCP2518FD) | MCP2515 HAT | Larger RX FIFO, same SPI form factor | Still SPI and shared; only 2 channels |
| can-utils (`candump -l`, `canbusload`, `cangen`) | marengo-candump, soak reports | Standard, absolute kernel timestamps | Text logs; not on your data model |
| python-can + cantools | Analysis and parsing in log-cli | Mature, scriptable | Python, off the RT path |
| ros2_control pattern (read → update → write, hardware interface, controller switching) | Berthier/Davout coupling | A known seam layout agents already understand | Full ROS 2 on a Pi is heavy; copy the pattern, not the stack |
| MCAP + Foxglove (app and SDK, live WebSocket) | marengo-store, Logs UI, trace CSV, much of the gateway telemetry | Records your existing protobuf schemas; replay and plots for free | Foxglove app licensing and tiers; MCAP itself is open |
| Rerun | Ad-hoc plots for calibration and camera poses | Strong for 3D, camera and time-series together | Fast-moving API |
| OpenTelemetry + Grafana (Loki, Prometheus) + node_exporter | Chappe log layer, host-metrics | Off-the-shelf, as tenet 8 asks | Run the backends on the PC, not the Pi |
| Sentry (Rust SDK) | Hand-rolled panic reporting | Crash context | Needs network, or a self-hosted instance |
| systemd `WatchdogSec` + `sd_notify`; Pi `/dev/watchdog` | Nothing (gap, B2) | Catches hangs | Needs a notify call from the loop thread |
| PREEMPT_RT kernel, SCHED_FIFO, mlockall | `thread::sleep` loop | Bounded jitter, needed for 1 kHz | Kernel build and maintenance on the Pi |
| chrony (linuxptp if <1 ms is needed) | Observed-only NTP | One clock across Pi and Jetson | PTP hardware timestamping depends on the NIC |
| Existing RobStride drivers: vendor sample code, the Seeed SDK (already cited in `motor_type.rs`), K-Scale `actuator` crate | Parts of the codec | Cross-check for scales and frames | The Seeed SDK is already known to be wrong on RS03 V; verify license and maintenance before relying on any of them |
| OpenCV ArUco/ChArUco, AprilTag 3 (`pupil-apriltags`) | Calibration-loop camera service | Proven detectors | Getting a timestamp on the CAN clock is your job |

---

## Q14. Anything else that will fail or waste time

1. **Following SOFTWARE_PREP item 1 literally fails at Enable.** Adding five joints to the current config hits B3 (the 5^n preflight and the offline-joint refusal) and the separate-URDF requirement.
2. **Deploys can silently override a hand-edited skeleton.** `preserve-taught-limits.py` takes the union of the Pi's taught limits and git's, unless `MARENGO_REPLACE_LIMITS=1` is set.
3. **The MCP hard-codes the right arm and `can0 can1`.** The enable soak monitors can0 only (`enable-soak.ts:225-508`), so left-arm soaks on can1 would be blind.
4. **No ID scan (M8), and RS00 scales are unknown (M7).** Both bite on the very first left-arm session.
5. **CI cost.** [D] `test-timing.md` reports a 437 s rerun across 110 test binaries after touching robstride. That slows every CAN-layer iteration until the old crates are gone.
6. **The installed MCP is stale.** In this session it still exposes `pi_jog` and `pi_motor_enable`; rebuild it before any bench session.

---

## What I did not read

- The RobStride PDFs: claims about the manuals come from code comments and docs.
- Most of Consul (surveyed at file level and with greps only).
- `docs/reviews/` (681 files) and `openspec/`.
- The bodies of Davout's `reference_*` modules (~6k lines) and of `enable_gate.rs`.
- Berthier `tick_inner` and position-law internals in detail.
- Most test bodies (sampled only).
- The research-mcp Python code.
- Most ADR bodies (headers and index only).
- The limb playbook beyond chapters 1–3.
- The Pi's actual on-disk config: no hardware or MCP access, by rule.