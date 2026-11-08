# Proposed edits — Roark `TENETS.md` / `PLAN.md` sharpening

Ranking: **must-fix** = blocks a first agent or risks a drive under a fault/crash/power-cycle; **should-fix** = causes rework or two-way reads; **nice-to-have** = polish. Each item: (a) where, (b) current text, (c) exact replacement/new text, (d) why. Facts cite files actually read.

---

## MUST-FIX

### M1 — Q2/Q5 Safety preflight has no place in the build order; first enable can precede it
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) § Start here, steps 4–6; §6
(b) `4. Build the new CAN layer and its simulated drive together (items 3 and 4), on vcan first. 5. Buy one USB-CAN adapter, confirm it, and bring the left drives up on it loose on the bench (item 5). 6. Then the actuator test stand tooling (item 7), and the camera loop (item 9).` — item 6 (safety) is never named in Start here.
(c) Replace steps 4–6 with:
> 4. Build the new CAN layer and its simulated drive together (items 3 and 4), on `vcan` first. No real drive is enabled in this step.
> 5. Build item 6's drive preflight and host stop rules against the sim drive, and pass the item 6 acceptance tests on `vcan`, including injected silence, fault bits and bus-off.
> 6. Buy one USB-CAN adapter, confirm it (item 5), scan and set IDs with drives unpowered-loaded (clamped, no link attached), then run preflight before the first enable.
> 7. Then the actuator test stand tooling (item 7), and the camera loop (item 9).
(d) Why: [marengo/review/sw-foundation-20261108/RECONCILIATION.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/review/sw-foundation-20261108/RECONCILIATION.md) Blocker 1 states the drive layer does not exist in legacy code; enabling before it exists repeats that hole.

### M2 — Q2 No drive/limb state machine; crashed process, rebooted drive and power cycle are undefined
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) new §3A after §3; [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §3
(b) *(none)*
(c) Insert new §3A:
> ## 3A. Drive and limb states (normative)
> Per drive: `UNKNOWN → IDENTIFIED → PREFLIGHTED → ENABLED → RUNNING`, plus latched `FAULT` and `SILENT`. Per limb: `OFFLINE` (no drive answering), `READY` (all drives `PREFLIGHTED`), `ACTIVE` (all drives `RUNNING`), `STOPPED-LATCHED`.
> Transitions: scan success moves `UNKNOWN→IDENTIFIED` only. Preflight (§6) success moves `IDENTIFIED→PREFLIGHTED`. Enable sequence (§3B) moves `PREFLIGHTED→ENABLED→RUNNING`. Any fault bit, mode ≠ run, 3 missed replies, bus-off, or VBUS out of range moves the drive to `FAULT`/`SILENT` and its limb to `STOPPED-LATCHED` in the same tick; no automatic exit from `FAULT`/`SILENT`/`STOPPED-LATCHED` — an operator clear plus a fresh preflight is required.
> A drive that reboots (uptime/mode returns as reset, ID re-appears after silence, or type-2 mode bits change without a host command — [marengo/hardware/docs/research/can-stack-for-marengo.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/research/can-stack-for-marengo.md) § protocol) is treated as a power cycle: it returns to `UNKNOWN`, the limb latches stopped, and §6 re-verification runs before any re-enable. The runtime never assumes a rebooted drive kept its limits, zero or run mode.
(d) Why: Worst case is a single drive browning out mid-hold; without this, one agent re-enables on stale preflight and another latches forever.

### M3 — Q2 Exact enable / disable / stop sequences missing, including firmware quiet times
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) new §3B
(b) *(none; §2 only lists `enable and stop order` as knowledge to lift)*
(c) Insert:
> ## 3B. Enable, disable and stop sequences (normative, per drive, in skeleton order)
> Enable: (1) confirm active reporting off and wait for its echo; (2) run §6 preflight reads; (3) Enable; (4) RunMode; (5) confirm type-2 mode = run before the first motion command. After any Set Zero, wait 800 ms with no commands, and expect a 450 ms reply blackout ([marengo/review/sw-foundation-20261108/RECONCILIATION.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/review/sw-foundation-20261108/RECONCILIATION.md) Hard-won knowledge).
> Controlled stop: SpeedTarget 0, neutral MIT frame, Disable. Fault/silence/bus-off stop: Disable at once, no controlled ramp. Process exit (clean or crash): systemd `ExecStopPost` runs the independent `disable` (§3, sole exception to single-transmitter); a hung realtime process is killed by systemd watchdog expiry first, then the same `disable` runs. The independent `disable` must tolerate the bus already being bus-off and any subset of drives not answering, and must be safe to run twice.
(d) Why: A crashed/hung process is the tenet-3 case the drive timeout exists for; `ExecStopPost`-only handling in the reference verdict leaves the hang uncovered.

### M4 — Q1/Q3 `canTimeout` rule is ambiguous and inconsistent on the refusal bound
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §6 Drive row 1; [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §3 item 2
(b) PLAN: `Write 1000 (= 50 ms; 20000 = 1 s) and read back before every enable; refuse on 0 or above.` TENETS: `CAN host-loss timeout of 50 ms or less`.
(c) PLAN replacement: `Write 1000 (= 50 ms; 20000 = 1 s, [marengo/hardware/docs/research/can-stack-for-marengo.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/research/can-stack-for-marengo.md)) and read back before every enable. Refuse enable if the read-back is 0 (disabled) or > 1000 (> 50 ms). Re-check after any drive power cycle, reset report or mode change (§3A).` TENETS replacement: `CAN host-loss timeout set to 50 ms (value 1000) and read back before every enable (see PLAN §6)`.
(d) Why: “Above” has no referent; [marengo/hardware/STATUS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/STATUS.md) Power board section states the testable form: refuse if any drive reads 0 or above the limit.

### M5 — Q3 Silence deadline contradicts the calibration spec (12 ms vs 50 ms)
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §6 Host/Silence; cross-ref [marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md) §9
(b) PLAN: `Stop the limb after 3 missed replies (well inside 50 ms)`. CALIBRATION_LOOP §9: `Drive feedback missing > 50 ms | Stop.`
(c) PLAN replacement: `Stop the limb after 3 consecutive missed replies per drive (12 ms at 250 Hz). This is the only silence deadline; it replaces the 50 ms figure in CALIBRATION_LOOP §9, which is the drive-side canTimeout, not a host deadline. One missed reply increments a per-drive counter and is logged; the counter resets only on an on-time reply from that same drive.`
(d) Why: At 250 Hz the two texts differ 4×; an agent implementing the calibration runner from §9 builds the looser, wrong one.

### M6 — Q2 Skeleton schema, units and zero/offset convention are unspecified
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §1
(b) `every joint with its actuator type, bus, device ID, direction, zero, soft and hard limits, and torque, current and speed caps` — no fields, units, angle origin, or zero semantics.
(c) Append to §1:
> Schema (v1, normative). Top level: `name`, `schema_version`, `limbs[]`. Per limb: `name`, `bus`, `joints[]`. Per joint: `name`, `actuator` (`RS00|RS02|RS03|RS04`), `device_id`, `direction` (`+1|-1`, joint-positive sense per [marengo/hardware/STATUS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/STATUS.md): pitch raises forward, roll abducts, yaw rotates outward, elbow flexes), `zero_offset_rad` (added to the drive-reported angle after `direction` is applied; the drive's own Set-Zero is a commissioning act, never a runtime write), `hard_lower_rad`, `hard_upper_rad`, `soft_lower_rad`, `soft_upper_rad` (all rad, skeleton frame, `soft` strictly inside `hard`), `torque_cap_nm`, `current_cap_a`, `speed_cap_rad_s`. All angles rad, torques N·m, currents A. The loader rejects: unknown actuator, duplicate `(bus, device_id)`, duplicate joint name, `soft` not inside `hard`, any cap above the actuator peak in PLAN §6 note, and a limb with zero joints. Left-arm `hard_*` initial values are the provisional URDF limits in STATUS (shoulder pitch −0.87…3.14, shoulder roll −0.26…3.14, upper-arm yaw −1.57…1.57, elbow 0…2.164, lower-arm yaw −1.05…1.05 rad); `direction` stays `null` (loader rejects enable, allows scan) until the §6 sign pulse records it.
(d) Why: Without units/origin/null-handling, two agents write incompatible loaders and a placeholder direction silently becomes `+1`.

### M7 — Q3/Q4 Left-arm joint table in PLAN omits the limits STATUS/URDF already fix; elbow ID status differs
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §1
(b) `Left arm: can1, IDs 11 to 15, joint names left_shoulder_pitch, left_shoulder_roll, left_upper_arm_yaw, left_elbow_pitch, left_lower_arm_yaw.`
(c) Replacement:
> Left arm: `can1`, IDs 11–15, joints (actuator, hard limits rad, peak N·m): `left_shoulder_pitch` (RS03, −0.87…3.14, 60), `left_shoulder_roll` (RS03, −0.26…3.14, 60), `left_upper_arm_yaw` (RS02, −1.57…1.57, 17), `left_elbow_pitch` (RS02, 0…2.164, 17), `left_lower_arm_yaw` (RS00, −1.05…1.05, 14) — [marengo/hardware/STATUS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/STATUS.md) Left arm table and [left_arm.urdf](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/designs/left-arm/urdf/left_arm.urdf); IDs 14–15 were “proposal” in STATUS, adopted here; ID 16 is reserved for a future hand and must not be assigned to a joint. URDF `velocity` is the MIT protocol range, not a rated speed (STATUS), and must not be copied into `speed_cap_rad_s`.
(d) Why: PLAN is the handoff; forcing the agent to mine STATUS/URDF for the same five rows invites copying the `velocity="50"` RS00 MIT range as a speed cap (URDF line 71).

### M8 — Q4 “Rated torque” vs peak vs stall are conflated; §8 elbow cap contradicts “never exceed rated”
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §7, §8; [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §3
(b) §8: `Never | run into a hard stop, hold a stall, exceed rated torque`. §8 elbow cap `≤ 17 N·m`. §7: `stalled holds stop at the stall rating (RS03 13 N·m, RS02 6 N·m; check RS00 and RS04)`.
(c) Add a definitions box at the top of §6 and fix §8:
> Definitions (vendor, [marengo/hardware/docs/sim-to-real-loop.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/sim-to-real-loop.md) §2.2): rated (continuous rotating) RS00 5, RS02 6, RS03 20, RS04 40 N·m; peak RS00 14, RS02 17, RS03 60, RS04 120 N·m; stall (held, T2/T5 only) RS03 13, RS02 6 N·m, RS00/RS04 unconfirmed — no stalled hold on RS00/RS04 until measured. §8 “Never exceed rated torque” means: continuous command ≤ rated; brief commands ≤ the joint cap in §8; the elbow's 17 N·m is a printed-stop design load equal to RS02 peak, so it is a never-exceed ceiling, not an operating target.
(d) Why: As written, 17 N·m both is and is not allowed on the elbow; stalled-hold worst case is exactly where the confusion overheats a drive with no spare.

### M9 — Q2 Realtime-process interface (`api`) names no transport, message set, or motion authority
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) crate table (`api`, `runtime`, `ui/`, `mcp/`); [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §5 last paragraph
(b) `Everything else … talks to the realtime process through one interface, and can crash without affecting motion.`
(c) Append after the crate table:
> `api` contract (v1): Unix-domain socket on the Pi, length-prefixed messages, no network listener. Message set: `GetSnapshot`, `CommandMotion` (per-joint target + mode + per-command torque ceiling), `Stop` (controlled | immediate), `ClearLatch` (operator only), `GetHealth`. Only one client may hold the motion lease at a time; `CommandMotion` without the lease, with a stale snapshot (> 100 ms), or for a limb not `ACTIVE` (§3A) is rejected and logged. `robot-cli disable`, UI and MCP tools use this same socket except the systemd independent `disable` (§3B). A client crash never stops motion by itself; loss of the lease holder for > 1 s commands a controlled stop. Jetson processes are clients like any other and get no second path to CAN.
(d) Why: “One interface” without authority rules lets a crashed UI, an MCP tool and a test runner command motion concurrently — the multi-writer fault the reconciliation flags in legacy code.

### M10 — Q2 Fault matrix missing: limb vs whole-robot stop, bus-off, lost bus, non-realtime crash
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §6, add table
(b) Scattered rows: `Bus-off | Stop the whole robot`, `Silence | Stop the limb`, `Hang | systemd WatchdogSec`.
(c) Insert:
> Fault scope (normative): single-drive fault/silence/VBUS → stop that limb, other limbs keep state but accept no new motion until the operator acknowledges. Whole-bus silence, bus-off, or adapter unplug (`gs_usb` device gone) → stop the whole robot; no automatic bus restart while any limb is `ACTIVE` (bench-only `restart-ms` must be off in the shipped unit files). Realtime-process crash/hang → §3B independent disable; drives then rely on the 50 ms `canTimeout` as last resort (reset mode = limp, [can-stack report](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/research/can-stack-for-marengo.md)). Non-realtime crash (UI, logger, MCP, camera) → motion unaffected; logger loss > 5 s during an attended powered run stops the run (no log, no motion). Power-board `PORT_FLT_N` low (milestone 4+) → stop that limb and name the port per [REV_B_PARTS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/electrical/pdb/REV_B_PARTS.md) fault decode.
(d) Why: Worst cases differ in blast radius; today an agent can reasonably implement any of limb-stop or robot-stop for a lost bus.

### M11 — Q2/Q4 Power-board section omits the hard interlocks STATUS/REV_B already impose
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §10
(b) `heartbeat toggled … every 50 to 100 ms, port-fault reporting, INA237 meters … self-test, brake clamp caps (30 W average over 60 s until the heat test, 3 kJ per burst), brown-out budget, E-stop input debounce. Can be built against the sim now.`
(c) Replacement:
> Heartbeat: toggle `WD_FEED` from the control thread itself (not a PWM peripheral, not a helper thread) every 50–100 ms; loss of a falling edge for 270–355 ms drops the motor latch until manual reset ([marengo/hardware/electrical/pdb/REV_B_PARTS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/electrical/pdb/REV_B_PARTS.md) Accepted risks). The systemd watchdog is fed only by the same control-thread iteration that toggles `WD_FEED`, so a hung loop starves both. Interlocks: refuse enable while BMS shows charge current or BMS telemetry is stale; shut compute down at min cell ≤ 3.0 V, filtered pack ≤ 38 V, or BMS telemetry lost > 5 s; total drive current budget = (V_rest − 35 V) / R_pack_measured, and the sum of all drives that can move at once ≤ 90 A read back at startup ([marengo/hardware/STATUS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/STATUS.md)). Self-test only with VBUS < 3 V, banks open, ≥ 5 s after the last stop, pack ≥ 40 V (REV_B_PARTS). Bench (milestones 1–3) substitutes: 24 V supply, VBUS window 22–26 V (§6), physical E-stop; none of the pack thresholds apply on the bench supply.
(d) Why: Item 10 as written is buildable “against the sim” into a board that latches off or enables during charging; the numbers already exist and are safety requirements, not research.

### M12 — Q2 Milestones have no acceptance criteria; “passes its tests” is untestable
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) § Milestones
(b) `Each milestone is "these drives come online and pass their tests".`
(c) Append per-milestone gates:
> Milestone 1 done when: skeleton loads the whole robot with unbuilt limbs `OFFLINE`; on `vcan`, injected silence/fault/bus-off stop per §6/§3A; on the bench, scan finds exactly the wired drives, §6 preflight passes, sign recorded per joint, 10 min soak with zero error frames at 250 Hz, and one attended single-joint move per drive stays inside soft limits. Milestone 2 done when: the right arm runs the same binary/config shape on `can0` (IDs 1–5) with its taught signs/limits carried into the skeleton, both arms pass preflight in one process, and only then legacy code is deleted (Before-deleting checklist). Milestone 3 done when: waist `waist_yaw` (RS03, [kinematics.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/kinematics.md)) repeats milestone 1's soak on its bus with two other buses live. Milestone 4 done when: §10 interlocks pass on the sim, then heartbeat-loss drops the latch in 270–355 ms on the real board. Milestone 5 done when: five buses at full skeleton load hold 250 Hz within the §5 jitter budget with parameter traffic in slack only.
(d) Why: Tenet 6 requires a logged test per claim; “pass their tests” names none, so milestone 2's deletion gate is currently a judgement call.

### M13 — Q1/Q2 “Total commanded torque bounded” has no definition, owner, or behaviour on violation
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §3 item 3; [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §6 Host/Limits
(b) TENETS: `a bound on total commanded torque (not only feed-forward)`. PLAN: `total commanded torque bounded, backed by the drive's own limit`.
(c) PLAN replacement: `Total commanded torque: per joint, |τ_ff + Kp·e + Kd·ė| (the full MIT command) is clamped to the joint's skeleton torque cap before sending; clamping is counted and logged, and clamping on > 10 consecutive ticks stops the limb. The clamp is defence-in-depth only — the drive's `limit_torque` (§6) is the authority, because the host estimate has known gaps ([RECONCILIATION.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/review/sw-foundation-20261108/RECONCILIATION.md) Blocker 4).`
(d) Why: The reconciliation explicitly found the legacy “total bound” was a prediction with gaps; without the formula an agent re-implements exactly that.

### M14 — Q5 Research start-order contradicts PLAN start-order on codec vs adapter purchase
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) § Start here / §5; [marengo/hardware/docs/research/can-stack-for-marengo.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/research/can-stack-for-marengo.md) start-here list
(b) PLAN step 5 buys the adapter after CAN+sim; the research list buys it first (“Two things need fixing before any money is spent”, step 1 buy one adapter).
(c) Add to PLAN §5, first bullet: `Buy one adapter first among the hardware steps (it is the long-lead item), but do not power a drive from it until M1 step 6. Confirm `dmesg` shows `gs_usb` (not `slcan`/ttyACM); a board that enumerates as `slcan` is the wrong board (STM32G431 clone class) — return it, do not work around it in software. Flash mainline candleLight if the board ships with `slcan` firmware.`
(d) Why: The G431/`slcan` trap is the research report's Risk 1; PLAN's “confirm it” doesn't say what failure looks like or that it's a purchasing error, not a code task.

---

## SHOULD-FIX

### S1 — Q1 “Registered and addressed; when it doesn't, it shows as offline. Nothing else.” hides partial-bus states
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §1
(b) `When a drive answers it is registered and addressed; when it doesn't, it shows as offline. Nothing else.`
(c) `When a drive answers the scan with the expected ID and firmware family it is IDENTIFIED (§3A in PLAN); when it doesn't answer it shows as offline. A drive that answers with an unexpected ID, duplicates another drive's ID, or answers then goes silent is a fault state (§3A), not “offline”, and blocks enable of its limb.`
(d) Why: Duplicate/old IDs are the expected bench case (§3: drives may hold 12 on `can1`, 11–14 on `can0`); “nothing else” forbids representing it.

### S2 — Q1 Tenet 4 “Code exists only if a test we're actually running needs it” conflicts with building safety/sim first
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §4
(b) `Code exists only if a test we're actually running needs it. No scaffolding for features that may come later.`
(c) `Code exists only if a test we're actually running needs it, or a tenet-3 safety rule requires it before the next powered step. Safety, preflight and the sim drive are built ahead of the motion that needs them (PLAN Start here steps 4–5); that is “needed now”, not scaffolding.`
(d) Why: Read literally, tenet 4 licenses deleting the preflight until after first motion — the reverse of PLAN §6 “builds it first”.

### S3 — Q1/Q6 Tenet 8 candidate list pre-decides an observability research project; PLAN already decided most of it
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §8; [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) § Observability
(b) TENETS lists OpenTelemetry/Grafana/Sentry/Foxglove/Rerun as “candidates to evaluate”; PLAN: `A wider observability stack choice … is still to be researched.`
(c) TENETS replacement: `Use the off-the-shelf stack fixed in PLAN Observability (MCAP + candump + Prometheus/Grafana). Proposing a different stack needs a measured failure of that stack, not an evaluation project.` PLAN replacement: delete `A wider observability stack choice … is still to be researched.` and add: `Sentry/OpenTelemetry/Rerun are not adopted; crashes are systemd coredumps plus the MCAP/candump black box. Revisit only if a logged incident shows a gap.`
(d) Why: Tenet 4 forbids the speculative evaluation both texts currently commission; the CAN research already settled viewers (Foxglove/Rerun/PlotJuggler open MCAP).

### S4 — Q2 Repo layout, toolchain, CI and Pi deploy are entirely absent
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) after crate table
(b) *(none)*
(c) Insert:
> Workspace and delivery: one Cargo workspace at the repo root; crates as in the table, bins under `bins/`. Pinned Rust toolchain in `rust-toolchain.toml`; `cargo test` (codec vectors + skeleton loader) and `cargo clippy -- -D warnings` run in CI on every PR; no test is added for coverage (tenet 6). Pi deploy: cross-build a release `runtime` + `robot-cli` (+ `sim-drive` for bench Pis), install as a versioned directory with a systemd unit (`WatchdogSec` set, `ExecStopPost` = independent disable, restart on-failure off while any limb is ACTIVE). Config (`config/skeleton.yaml`) deploys separately from binaries; a binary refuses to enable on a `schema_version` it doesn't know. Rollback = repoint the unit at the previous versioned directory; skeleton changes roll back by git revert, never by runtime write.
(d) Why: A first agent otherwise invents layout/CI/deploy, and “Work on branches and open PRs” ([roark/CLAUDE.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/CLAUDE.md)) has no checks to open a PR against.

### S5 — Q2 Sim drive: “driven by MuJoCo” is undefined (model source, stepping, fault list, rate)
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §4
(b) `one sim-drive process that speaks the RobStride protocol through the same codec, driven by MuJoCo, with fault injection built in (silent drive, fault bits, VBUS out of range, reboot, dropped and delayed frames).`
(c) Replacement:
> `vcan0`–`vcan4` plus one `sim-drive` process speaking the RobStride protocol through the same `robstride` codec. It implements types 1/3/4/17/18, simulates `canTimeout` (limp after 50 ms without commands), and takes a fault script + seed: per-drive silence, fault bits, VBUS out of range, reboot (mode returns as reset), dropped/delayed frames. Physics, where a test needs it, comes from the MJCF generated for sim (PLAN §1); logic/safety tests run with a trivial hold-last-pose model and must not depend on MuJoCo. `vcan` has no bit timing, so sim proves logic and stop behaviour, never bus load or jitter ([can-stack report](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/research/can-stack-for-marengo.md)). The realtime binary, its unit files (minus hardware watchdog) and its `api` clients run unchanged against it.
(d) Why: “Driven by MuJoCo” makes every vcan safety test appear to need a dynamics model; the reference sim spec separates the protocol emulator from physics.

### S6 — Q2 Test-stand 1 kHz mode conflicts with the 250 Hz runtime and is unexplained
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §7 vs §3 (250 Hz)
(b) §7: `one drive alone on its bus, command and reply at about 1 kHz`. §3: `250 Hz default`.
(c) Append to §7: `The ~1 kHz rate is a test-stand mode of the same runtime (one drive on the bus, all other limbs offline), not a second control loop; it exists because a lone drive has the bus headroom ([sim-to-real-loop.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/sim-to-real-loop.md) §2.3). The skeleton/`api` path is identical; only the tick period differs, set by config for that run and recorded in the log header. T5 still stops at 60 °C and stalled holds still stop at the §6 stall ratings.`
(d) Why: Otherwise an agent either caps the stand at 250 Hz (losing T4 latency resolution) or builds a second, unreviewed loop.

### S7 — Q2 Logging schema, file layout and clock sync named as goals, not specified
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) § Observability; [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §8
(b) `Every frame in and out goes to MCAP … with kernel timestamps; candump -L runs per bus … health metrics … to Prometheus/Grafana.`
(c) Append:
> MCAP channels: one raw channel per bus with records `{bus, id, dlc, data, t_kernel, t_mono, dir}` ([can-stack report](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/research/can-stack-for-marengo.md) Logging), one decoded joint channel per limb, one injected-fault channel (sim only). One session = one directory: MCAP, per-bus `candump -L` logs, skeleton copy + git hashes, stage/run plan if any. Pi and Jetson sync by chrony over their direct Ethernet link (PTP only if a replay shows > 1 ms error, [sim-to-real-loop.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/sim-to-real-loop.md) §4); every record carries the shared wall clock plus a monotonic timestamp. Disk-full or logger-backpressure must never block the control thread (drop-and-count, §6 logger rule in M10).
(d) Why: Tenet 8's “one shared clock” and replay loop are unbuildable without the channel/record definition the research already drafted.

### S8 — Q2/Q5 Right-arm migration: what carries over (signs, taught limits, fits) and in what order
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) Milestone 2; § Before the old stack is deleted
(b) `Right arm moved to the new stack, then the old code deleted`. Checklist mentions `gravity-fit`/`firmware-timing` tools only.
(c) Replacement for the Before-deleting section:
> Before the old stack is deleted: (1) add the right arm to the skeleton on `can0` IDs 1–5 with its measured signs (all `direction: -1` in legacy [motors.yaml](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/config/motors.yaml)), taught bench limits and friction/gravity fits re-expressed as skeleton values or `dynamics` data — no value is copied from `control.yaml` soft limits without noting they were runtime values under a different convention ([kinematics.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/kinematics.md) Live bench arm); (2) pass milestone 2's gate (M12); (3) lift item 2's knowledge and keep the `gravity-fit` and `firmware-timing` tools and commissioning docs. Only then delete per the reconciled table. If the right arm fails the gate, legacy stays and milestone 3 does not start.
(d) Why: Deletion is irreversible in the old repo (PACKET owner intent); the current text lets an agent delete on “moved” without proving the taught envelope survived.

### S9 — Q3 Skeleton ownership contradiction: hardware edits it vs software owns it
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §2; [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §1 first paragraph
(b) TENETS: `When the hardware changes, the hardware side edits the skeleton`. PLAN: `the software project owns its schema and loader; the hardware side supplies and edits the values, as tenet 2 says`.
(c) Make TENETS read: `When the hardware changes, the hardware side edits the skeleton's values, in one commit, and says so in STATUS.md. The software project owns the schema and loader (PLAN §1); a schema change is a software commit that names the hardware values it invalidates.` (PLAN text already matches; keep it.)
(d) Why: As written, tenet 2 authorises hardware to change the schema mid-build; “one fact in one place” needs the schema/values split stated once.

### S10 — Q3 Five-limb / five-bus accounting vs waist, neck and hand reservations
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §1; [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) intro + Milestones
(b) `five limbs: two arms, two legs and the waist` … `on five buses`. No mention of neck/head or the reserved hand ID.
(c) Add to PLAN intro: `The five buses are: left arm, right arm, left leg, right leg, waist (1 joint, `waist_yaw` RS03). Neck/head servos (parked, [STATUS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/STATUS.md)) are not RobStride CAN joints, are not in the skeleton v1, and get no bus. The reserved hand ID (`can1` ID 16) is likewise not a skeleton joint until a hand exists (tenet 4). Total skeleton joints at the finished robot: 23.`
(d) Why: A fixed-robot skeleton (tenet 1) that silently includes or excludes parked hardware produces either dead code paths or a later schema break.

### S11 — Q1 Tenet 1 “No code, doc, file or folder is named after the current step” vs `robot-cli`, milestone docs, `left-arm` references throughout PLAN
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §1 last bullet; [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) crate table
(b) `No code, doc, file or folder is named after the current step ("right arm", "torso", "left arm") unless it is truly limb-specific hardware data.`
(c) Append: `Milestone names in PLAN, bench procedure docs, and skeleton limb names (`left_arm`, …) are hardware data / schedule, not code structure, and are allowed. The rule binds crate, module, type, config-key and service names: e.g. `robot-cli` is the operator CLI for the whole robot (not a limb name) and stays; a `left-arm-tool` binary would violate this rule.`
(d) Why: Without the carve-out the rule reads as forbidding the skeleton's own limb names and PLAN's milestone titles, so agents will either violate it silently or rename hardware data.

### S12 — Q4 RS00 MIT scales stated as an “open fact” but skeleton defaults could still ship the wrong values
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §2 Open fact; §3 Codec
(b) `Code uses ±50 rad/s / ±17 N·m, every RS00 manual says ±33 / ±14. Capture a loose RS00 before any forearm motion.` Codec: `per-model scales from the skeleton`.
(c) Append to §3: `Until that capture lands, the skeleton's RS00 scales are the manual values (±33 rad/s, ±14 N·m — [can-stack report](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/research/can-stack-for-marengo.md) scale table) marked provisional, and the loader refuses RS00 motion commands (scan/preflight allowed) while the provisional flag is set. RS03 velocity scale is the measured ±20 rad/s, not a manual value (item 2).`
(d) Why: Wrong velocity/torque scales mis-encode every MIT frame; “capture before motion” needs a loader-level block, worst case being the first forearm move.

### S13 — Q1 “Refuse on any mismatch” / “stop on any fault” lack latch-clear semantics (who, how, logged?)
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §3 item 3; [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §6 Host/Faults
(b) `stop on any fault and never auto-clear it` / `Stop on any fault bit; never auto-clear and retry`.
(c) Append to PLAN §6: `A latched stop is cleared only by an operator `ClearLatch` (§ api contract) after the fault source reads clear on a fresh preflight; the clear, its operator/source, and the fault snapshot are logged. Re-enabling uses the full §3B sequence. There is no retry counter and no timed auto-clear, including after a drive power cycle (§3A).`
(d) Why: “Never auto-clear” still permits an agent to add a UI button that clears blind; the power-cycle case is where blind clears do damage.

### S14 — Q2 Camera loop ordering omits its own attended/unattended and correction limits in PLAN's summary
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §9
(b) `camera service and stage C0 (arm unpowered), then the stage runner with section 9's stop rules tested on vcan, then C1 and C2 attended with Joey.`
(c) Replacement:
> Order: camera service + C0 (arm unpowered); stage runner with [CALIBRATION_LOOP.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md) §9 stop rules proven on `vcan`; C1–C2 attended; C3+ unattended only after the same stage passed attended with the same code/config/hardware (CALIBRATION_LOOP §9). Software may auto-apply only: zero offsets ≤ 2°, friction parameters, URDF origin corrections ≤ 2 mm / 1° (CALIBRATION_LOOP §10); anything larger, a twice-failed gate, or any physical anomaly stops for the owner. A stop with all drives healthy goes to the nearest rest pose then disables; any unhealthy drive disables at once onto the catch pad (CALIBRATION_LOOP §9).
(d) Why: PLAN §9 is what the building agent reads; the unattended-run preconditions live only in the reference doc it may skim.

---

## NICE-TO-HAVE

### N1 — Q6 Crate table over-specifies internal splits before any test needs them (`control` vs `dynamics`, `telemetry` contents)
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) crate table + `Final names are the software project's call`
(b) 12-row table presented as “Set up the new crate layout” (Start here step 2).
(c) Change Start here step 2 to: `Set up the workspace with `skeleton`, `robstride`, `can`, `safety`, `runtime`, `sim-drive`, `robot-cli` first; split out `control`, `dynamics`, `telemetry`, `api` when a second consumer exists. The table below is the target shape, not a day-one checklist.` And in the table mark `control`/`dynamics` rows `(split out when needed)`.
(d) Why: Tenet 4; seven crates prove the milestone-1 stack, five speculative splits are scaffolding the tenets say to delete.

### N2 — Q6 “About 10 crates in place of 15 crates and 6 bins” is a count target, not a requirement
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) § Names and crate layout
(b) `Proposed layout, about 10 crates in place of 15 crates and 6 bins.`
(c) `Proposed layout (12 rows including `ui/` and `mcp/`). The count is not a target; merge or split rows where tenet 7's “know within minutes what every layer does” reads better.`
(d) Why: The table lists 10 crates + 2 bins + `ui/`+`mcp/` (= 14 units); the “about 10” figure is both wrong and the kind of quota an agent will contort the design to hit.

### N3 — Q1 Tenet 7 “within minutes” and tenet 6 “beats debugging by hand every time” are untestable slogans
(a) [roark/TENETS.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/TENETS.md) §6, §7
(b) `An agent (or a person) new to the repo should know within minutes …` / `A feedback loop beats debugging by hand every time.`
(c) §7: `The repo root carries one architecture map (≤ 1 page) naming every crate/app, its owner seam and its one interface; each crate's first doc lines state what it is for, what it must never do, and what it talks to. A change that moves a seam updates the map in the same PR.` §6: delete `A feedback loop beats debugging by hand every time.` (the preceding bullet already states the rule testably).
(d) Why: “Minutes” has no check; the map-in-same-PR wording (already implied by the next bullet) is the enforceable version.

### N4 — Q1 PLAN “confirm it” / “check RS00 and RS04” / “lower if the rig fails low” delegate numbers without owners or defaults
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §5, §7, §8
(b) `Buy one adapter; confirm it` · `check RS00 and RS04` · `lower if the rig fails low`.
(c) §5 → see M14. §7 → `RS00/RS04 stall ratings: unconfirmed — treat as “no stalled hold permitted” (M8) until a manual or bench capture supplies the number; the test-stand runner must refuse a stalled-hold test profile for those types.` §8 → `Elbow torque cap: ≤ 17 N·m, and ≤ 0.8 × the dummy-rig measured printed-stop failure torque ([ELBOW_DUMMY_OUTPUT_TEST.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/designs/left-arm/testing/ELBOW_DUMMY_OUTPUT_TEST.md) Test 5) once that number exists; the lower value wins and is written into the skeleton by hand commit.`
(d) Why: Each is a two-way read at the exact moment a drive is under load; the rig's proposed pass (failure ≥ 25.5 N·m) makes 17 N·m the binding cap today, which the text should just say.

### N5 — Q3 `CLAUDE.md` “Nothing in this repo depends on or copies its structure” vs PLAN lifting test vectors/tools by name
(a) [roark/CLAUDE.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/CLAUDE.md) item 3; [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) §2, Before-deleting
(b) CLAUDE: `Nothing in this repo depends on or copies its structure.` PLAN: copy bench lessons “into the new codec's docs and test vectors”, keep `gravity-fit`/`firmware-timing`.
(c) CLAUDE replacement: `Nothing in this repo depends on the legacy repo at build or run time, and no legacy structure is copied. Facts, test vectors and the two named tools in PLAN §2/Before-deleting are lifted deliberately and become this repo's own code and docs.`
(d) Why: A literal agent reading CLAUDE.md first refuses the PLAN §2 lift that must happen before any deletion.

### N6 — Q4 Numbers audit trail: state which PLAN numbers were verified and which are provisional
(a) [roark/PLAN.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/roark/PLAN.md) new footnote in § Rules that hold for every item
(b) `Hardware facts change: check STATUS.md before acting on a number here.`
(c) Append: `Verified 2026-11-08 against the reference repo: 250 Hz / 50 ms timeout / scale table ([can-stack report](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/research/can-stack-for-marengo.md)), left-arm IDs/limits (STATUS + URDF, M7), stall ratings RS03 13 / RS02 6 N·m ([sim-to-real-loop.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/sim-to-real-loop.md) §2.3), elbow stops 0°/124° and 17 N·m design load ([ELBOW_DUMMY_OUTPUT_TEST.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/designs/left-arm/testing/ELBOW_DUMMY_OUTPUT_TEST.md)), bench fusing 3 A first power / 10 A running ([left-shoulder-bench-harness.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/electrical/wiring/left-shoulder-bench-harness.md)). Provisional until measured: RS00 scales (S12), RS00/RS04 stall ratings (N4), all left-arm directions (§1). Not verified from the PDFs in this pass: any RobStride manual value not quoted in the can-stack report — check the manual before relying on it.`
(d) Why: §4 of the packet asks for checked facts; the building agent needs to know which numbers are load-bearing vs placeholders without re-deriving the audit. (RS02 stall “6 vs 7” rotating figure in the reference is internally inconsistent — rated 6 in §2.2 table vs “6 vs 7” in §2.3 text of sim-to-real-loop.md; flagged here, not resolved — owner to confirm against the RS02 manual.)

---

## Q7 — Questions only the owner can answer (with recommended defaults so work is not blocked)

1. **Q7 Skeleton `direction: null` handling** — Default adopted in M6: loader allows scan/preflight, refuses enable. Confirm, or should an unmeasured direction block even preflight? *(Blocks §1 loader.)*
2. **Q7 RS02 rated torque: 6 or 7 N·m rotating?** — [sim-to-real-loop.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/sim-to-real-loop.md) §2.2 says rated 6; §2.3 says “RS02 6 vs 7”. Default: 6 N·m (the lower, and the §2.2 vendor table) until the RS02 manual page is cited. *(Blocks M8 caps.)*
3. **Q7 Whole-robot vs limb stop on bus-off when only one limb is moving** — Default in M10: whole robot. Confirm you don't want “moving limbs stop, idle limbs unaffected”. *(Blocks §6 implementation choice, low cost to change later.)*
4. **Q7 May the independent `disable` (systemd) be run manually by an operator while the realtime process is alive?** — Default: no; it refuses if the runtime holds the bus, because two transmitters violate §3. Manual override requires stopping the runtime first. *(Blocks `robot-cli` design.)*
5. **Q7 Is the waist a “limb” for stop-scope purposes** (a waist fault stops legs?) — Default: waist is its own limb/bus; a waist stop latches the waist only, but blocks new leg motion until acknowledged once legs exist. *(Blocks M10 matrix for milestone 5.)*
6. **Q7 Camera/calibration data retention** — Sessions include camera frames at holds ([CALIBRATION_LOOP.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/designs/left-arm/testing/CALIBRATION_LOOP.md) §10); disk budget on the Pi is finite ([sim-to-real-loop.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/sim-to-real-loop.md) §4: nightly offload). Default: keep all MCAP/candump, keep hold frames only, offload nightly, never delete a failed run's data automatically. *(Blocks S7 file-layout sizing.)*
7. **Q7 Who may `ClearLatch` / run Set Zero** — Default in S13/M6: interactive operator only (not MCP/agent tools); Set Zero only in the attended reference workflow (CALIBRATION_LOOP §9), never unattended. Confirm agent tools get no latch-clear path even in sim-to-bench parity. *(Blocks `api`/`mcp` permissions.)*

**Not verified in this pass:** RobStride PDF internals beyond the can-stack report's extraction (manuals are PDFs under [marengo/hardware/docs/robstride/](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/hardware/docs/robstride/README.md), read via its README only); legacy crate internals beyond [RECONCILIATION.md](/C:/Temp/REVIEW_PACKET_ROARK_DOCS_20261108/marengo/review/sw-foundation-20261108/RECONCILIATION.md) citations; `reviews/` panel transcripts (not opened — reconciliation majority verdicts taken as the record).
