# Marengo status

**As of:** 2026-10-28. The last code and bench work was on 2026-10-04; nothing changed in between. Update this page whenever a milestone, bench result or open decision changes. It is the entry point for "where are we"; history lives in the [reviews](reviews/) and [commissioning records](commissioning/).

## Goal

A personal, full-size, walking humanoid in the Unitree G1 class: 1524 mm, 38–42 kg, 23 actuated DOF (legs 6+6, arms 5+5, waist 1), Robstride RS00–RS04 actuators, a Raspberry Pi 5 for realtime control and a Jetson for perception later. The spec is in [`hardware/docs/kinematics.md`](../hardware/docs/kinematics.md) and the milestone plan in [`roadmap.md`](roadmap.md). Everything is open to better ideas; record changes as ADRs.

The **right 5-DOF arm** on the bench is a deliberate first slice to prove the control, safety and tooling stack on real joints. It is not the goal. Everything built for it must scale to the full body without a separate code path.

## What exists

| Area | State |
|------|-------|
| Mechanical | 2020 upper-torso cage committed (125 × 150 × 380 mm). Right arm built; its URDF geometry and inertials come from CAD (2026-10-03). Pelvis, legs, shell, battery bays and head are draft. CAD is local-only on Joey's PC. |
| Electrical | Pi 5 + Waveshare 2-CH MCP2515 CAN HAT (`can0` in use, `can1` free) + BNO085 IMU on I2C. Right arm drives on `can0`, ids 1–5. E-stop GPIO not wired (physical E-stop in motor power is the authority). PDB KiCad project is an empty stub. |
| Runtime | Rust workspace (15 crates, 6 bins): `marengo-pi` 200 Hz loop, Berthier control, Davout safety supervisor (sole motor path), robstride CAN driver, Chappe bus, `marengo-gateway`. Physical Set Zero with process-local grants (ADR 0036), motion-owner lease, enable completion gate, gravity preflight, single-drive-loss degraded hold (ADR 0038), Davout total-torque bound. |
| Control | GravityComp, Impedance, TorqueOnly and Position modes. Position uses the ADR 0007 law on four joints; `right_shoulder_pitch` runs the ADR 0039/0040 `scaled_pd` law as a reduced-speed bench trial. |
| Operator UI | Consul (Vite + React): Overview, read-only Telemetry, Hardware page (master URDF/YAML import, a schematic 3D joint picker, Set Limits, Set Zero, Ready facets, commissioning scope), Testing panels and Logs. There is no URDF render in the UI. |
| Tooling | `marengo-pi-mcp` bench tools (deploy, logs, candump, calibration, motion suite, enable soak), research MCP, daily audit, Docker CI gate (`just check`), MuJoCo smoke sim. |

## Bench checkpoint (last recorded 2026-10-04)

- **Limb playbook** ([limb-playbook.md](commissioning/limb-playbook.md), issue #170): Reference, sign and limits passed historically; arm-down G-comp passed with notes; elevated/wave-pose G-comp is not green; the 25 % Position ladder passed; the 50 % rung was not scored; `WAVE_POSE_GCOMP_SIGNED` is still false (issue #176). Treat all older passes as needing a fresh reference after any power or config change.
- **Enable soak** (`pi_enable_soak`, 20 fresh-process cycles): best 20/20 at `aa773418`; the last recorded soak was 18/20 at `9b1b3f8d` (RX overrun cause A, now fixed, and cause B, open). No soak is recorded on the final 2026-10-04 code.
- **Shoulder pitch motion suite** (`pi_motion_suite`, [bench ledger](commissioning/bench-ledger.md)): with the friction error assist, long moves passed 13/13, but short moves, gravity extremes and repeatability failed on stop overshoot (10.5–14.2 mrad vs 10 mrad bar) and the motion felt like it "fights itself". The [slow-speed control audit](reviews/2026-10-04-control-audit-pitch-slow-speed.md) led to ADR 0040, committed as `07375aa` but **not yet run on the bench**.
- **Gravity model:** pitch phase-1 wave fit applied (A 2.661, B 0.038; fc 0.353). The full multi-joint gravity calibration sweep is still to do.

## CI

`main` CI was red at `07375aa` (run 921) and both causes are fixed on the 2026-10-28 docs branch:

- The Davout test `physical_reference::a_drive_that_died_while_the_host_held_its_stream_off_loses_its_grant` anchored its deadline on the post-SetZero quiet only, but the supervisor (correctly) also excuses silence during owner homing. On a loaded runner homing outlasts the quiet, so revocation landed after the deadline. The test now uses the later of the two; the safety rule is unchanged.
- Consul's `npm audit --audit-level=high` began failing on advisories published after 2026-10-04 (`vite`, `undici`, `source-map-js`, `zod`). Lockfile bumps clear them, and the unused `glinui` CLI devDependency (which pinned a vulnerable `zod` 3.x) was dropped; run it through `npx`.
- `cargo deny` began failing on RustSec advisories published after 2026-10-04 in `quinn-proto`, `rustls`, `rustls-webpki`, `webpki-root-certs` and `tokio`; `Cargo.lock` bumps clear them.
- Still open, below the CI gate: two moderate `react-router` advisories whose fix is the 7.x major, and `unsafe-libyaml` (RUSTSEC-2024-0449, unmaintained, no upgrade) under the deprecated `serde_yaml`, ignored in `deny.toml` until `marengo-config`, Davout and the gateway move to a maintained YAML crate.
- `marengo-gateway`'s real-QUIC tests bind `[::]:0` and fail in sandboxes without IPv6 (they pass on GitHub runners).

## Open issues (most important first)

Detailed per-finding status: [prior-review-status.md](reviews/2026-10-03-crate-audit/prior-review-status.md) (as of `a2b55b3`) and the Phase B reports in [`reviews/2026-10-03-crate-audit/phase-b/`](reviews/2026-10-03-crate-audit/phase-b/). Several Phase B packages fixed items after that table was written (for example G16 store/deploy blocking I/O and CS16), so the table needs reconciling before it is trusted line by line.

**Safety and runtime**
- RX overrun **cause B**: a 3.8–5 ms receive blackout after Disabling a Run-mode drive overflows the MCP2515's 2-frame FIFO and latches the transport fault. Next steps need Joey: measure CANH–CANL with power off (expect ~60 Ω) and report can0 ground/shield bonding; then scope CANH/CANL and INT ([handoff](commissioning/handoff-2026-10-04-liveness-hardening.md)).
- **CS10**: bench caps bound feed-forward only; PD torque can exceed them. Needs a decision (Davout now bounds the *predicted* total MIT torque, which covers part of this).
- **CS14**: danger-zone velocity clipping cannot brake when `kd = 0` (GravityComp). Needs a decision.
- **M02 / M03**: drive-side CAN timeout (PR #254 left CanTimeout = 600, ~30 ms, on all five drives) and the fault-clear frame are not owned by any policy; hardware E-stop sense is not wired.
- RS03-tuned values from before the ±50 → ±20 rad/s MIT velocity-scale fix still need bench re-checks (pitch/roll velocity, accel, kd, slew, friction, the `elevated_shoulder_pitch_fall` danger zone).
- IMU: CS17 (stale orientation republished with fresh timestamps) open; CS18 (12- vs 14-byte rotation report) needs a raw SHTP capture.

**Gateway and tooling**
- G04/G05/G06/G08/G09/G21: limit-persist coalescing, persistence flags, stale-telemetry restart gate, disk no-op "Durable", URDF activation race and HTTP 500 after promotion. Some were confirmed and partly fixed in WP-K; reconcile.
- Consul F01–F04, F07 (playback timer, Dry Run switch, Hold Stop, stale PID target, two-click E-STOP) were not re-checked in the crate audit.
- Proto types marked deprecated by prune batch B13 are still on the wire (removal needs a `buf breaking` exception).

**Platform**
- Local `just check` on Apple Silicon runs the amd64 image under Rosetta and Berthier wall-clock tests time out. Options: treat CI + native `cargo test` as the gate on the Mac, add an arm64 dev image, or make those tests clock-independent.
- Native Windows cannot build the full workspace (Chappe IPC is Unix-only; CS22/G11).
- `justfile` still has `setup-wsl` / `deploy-pi-wsl` recipes although ADR 0018 retired the WSL checkout.

## Open design decisions

These change the robot, so they are Joey's call; most want an ADR.

1. **Full-body CAN plan.** `config/motors_humanoid.yaml` puts 23 joints on five buses (`can0`–`can4`, ids 1–23), which disagrees with the ID blocks in [hardware ADR 0001](../hardware/docs/decisions/0001-can-and-motors.md) (right arm 1–10, …) and with the live arm (`can0` ids 1–5). Five buses also exceed the 2-CH HAT, and the MCP2515's 2-frame FIFO is already the bench's tightest constraint. Decide channel count and CAN interface hardware (e.g. a multi-channel SPI or USB CAN-FD adapter) before M5.
2. **Arm naming and the fifth joint.** The built right arm has `right_elbow_pitch` and `right_lower_arm_yaw` (RS00 about the forearm); the left-arm template still says `left_elbow` and `left_wrist` (wrist pitch). Decide whether the left arm mirrors the right, then rename everywhere at once.
3. **Position law.** Accept, revise or roll back ADR 0039/0040 after the pitch bench trial, then decide whether the other joints move to `scaled_pd`.
4. **Whole-body dynamics.** ADR 0005's gravity-only model is not enough for balance or walking (M8). Choose a rigid-body dynamics approach (pure-Rust RNEA, `k`, or a Pinocchio exception) and a floating-base model.
5. **Locomotion approach.** Model-based (MPC / whole-body control) vs learned policies (RL in Isaac Lab or MuJoCo, ONNX on the Jetson, matching `models/`). ADR 0003's D2 tier and ADR 0014's Jetson role both depend on this.
6. **Power system.** 48 V pack chemistry, capacity and BMS; PDB design; E-stop and pre-charge.

## Next steps (proposed order)

1. Cause-B physical checks (termination, ground), then an enable soak on current `main`.
2. Bench-run the ADR 0040 pitch trial with `pi_motion_suite`; record it in the ledger; decide on 0039/0040.
3. Gravity calibration sweep, RS03 value re-checks, then resume the limb playbook at the 50 % Position rung.
4. In parallel, mechanical M2/M3: torso layout refs, pelvis and one leg in CAD, with decisions 1 and 2 above settled first.
