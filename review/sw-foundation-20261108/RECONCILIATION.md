# Software foundation review: reconciled verdicts

2026-11-08. Six reviewers (Codex Sol, Codex Astra, Opus high, Composer 2.5 Fast, Muse Wonder, DeepSeek V4.1 Flash) read the
software at snapshot `614371d3` against the eight tenets in [hardware/SOFTWARE_TENETS.md](../../hardware/SOFTWARE_TENETS.md).
All six delivered in 31.5 min of wall time and all passed the on-target check (86–94 % of cited names found). Reviews:
[reviews/](reviews/). Packet and questions: [PACKET.md](PACKET.md).

How this was reconciled: a verdict below is the majority of the six; splits are named. Composer was the outlier (it kept almost
everything) and is still on trial, so it doesn't break ties. The load-bearing blockers were re-checked in the code by the owning
thread (marked **checked**).

## Bottom line

The owner's read holds for most of the stack. Every reviewer found it over-built in the same places: the reference/grant/
journal machinery, degraded modes, limits held in five places, about 20k lines of home-grown logging and storage, and tools that
take over the CAN bus. But the one thing tenet 3 says must exist, the **drive** safety layer, doesn't exist at all. And the left
arm can't simply be added to the current stack: the enable gate refuses it.

## Verdict per component (majority of six)

| Component | Verdict | Votes / split | Notes |
|---|---|---|---|
| crates/robstride | **Rebuild** (tenet 5) | 6 of 6 | Lift the codec, scale table, TX allowlist, socket options first (list below) |
| crates/davout (36k) | **Rebuild, small** | 5 of 6 (DeepSeek: slim) | Keep fault latch, envelope, UID check ideas. Drop reference/grant/journal (~6k), leases, `drive_loss`, danger zones, wrong-sign |
| crates/berthier (22k) | **Keep but slim** | 5 of 6 (Opus: rebuild smaller) | One position law, not two; drop degraded hold and workarounds; not hard-wired to 200 Hz |
| crates/marengo-config | **Rebuild as the skeleton loader** | 5 of 6 | One YAML; runtime never writes hardware facts |
| crates/armee-kinematics, armee-dynamics | **Keep but slim** | 6 of 6 | Limits from the skeleton, not URDF; keep gravity fits for calibration C6 |
| crates/armee-proto + proto/ | **Keep but slim** | 6 of 6 | 12 unreferenced messages, PR-5 stubs, ~20 deprecated fields go |
| crates/chappe | **Replace** with the one runtime interface | 5 of 6 (Composer: slim) | Two ingress paths; log layer drops WARN/ERROR past 10/s |
| crates/marengo-store (SQLite logs) | **Delete** | 6 of 6 | Replaced by MCAP + Grafana stack (tenet 8) |
| crates/marengo-candump | **Delete; keep a small decode table** | split 3/3 slim vs delete | can-utils + MCAP replace it |
| crates/marengo-deploy, marengo-host-metrics | **Delete** | 5 of 6 | node_exporter for host metrics |
| crates/marengo-imu, bins/imu-probe | **Delete for now** (git history) | 6 of 6 | No arm test needs an IMU |
| crates/sim-harness | **Rebuild** as the vcan sim drive | split rebuild/delete | Not the unchanged-binary HIL tenet 5 needs |
| bins/marengo-pi | **Rebuild** as the one realtime process | 5 of 6 | Not realtime today (sleep-paced, no SCHED_FIFO) |
| bins/marengo-gateway | **Thin UI bridge only**, or delete | split | Must not write config, URDF, deploy or own logs |
| bins/motor-repl | **Slim to the independent `disable`** | 5 of 6 | Keep as `ExecStopPost`; nothing else opens the bus |
| bins/marengo-log-cli | **Delete after lifting `gravity-fit` and `firmware-timing`** | split | Rest is store plumbing |
| bins/marengo-limit-sync, tools/limit-sync-local | **Delete** | 5 of 6 | Second writer of skeleton facts |
| tools/marengo-pi-mcp | **Rebuild as a thin client** of the runtime | split rebuild/slim | Today it stops the service and launches its own CAN owner |
| tools/compound-auto-learn, tools/marengo-research-mcp | **Delete** | 6 of 6 | |
| tools/cad | **Move to hardware/** | | CAD tooling, not runtime |
| consul | **Keep but slim** | 4 of 6 (DeepSeek: delete) | Keep stop, live state, preflight, results. Drop auto-learn, Logs page, URDF wizard |
| sim/ | **Rebuild around replay + the vcan sim drive** | 6 of 6 | |
| config/ | **Rebuild into one skeleton YAML** | 6 of 6 | Delete the humanoid templates |

## Blockers (fix in the new stack before powered motion)

1. **The drive safety layer doesn't exist.** No code writes or checks the CAN timeout (0x7028), torque (0x700B) or speed
   (0x7017) limits before enable; current limit, damper and VBUS aren't even defined (`crates/robstride/src/params.rs`).
   0x7028 is only read by an offline inspection tool. **Checked.** (Opus, Sol, Astra, DeepSeek)
2. **A hung process has no backstop.** No systemd watchdog, no heartbeat; drives keep their last command, gravity feed-forward
   included, until an unmanaged timeout fires. (Opus, DeepSeek)
3. **The left arm can't join the current stack.** The gravity preflight sweeps 5^n poses over all joints and refuses above
   1,000,000 (ten joints is 9.8M), and it refuses when any joint is offline, which inverts tenet 1. **Checked**
   (`bins/marengo-pi/src/gravity_preflight.rs:31`). Subset bring-up also fails construction (Sol).
4. **"Total torque bound" isn't a total bound.** It's a host prediction with known gaps (clipped-velocity damping, stale poses),
   with no drive-side clamp behind it. The printed elbow stop (17 N·m) needs the drive's own limit. (Opus, Sol)
5. **Shutdown flushes trace files before disabling drives**, so a slow disk can delay a stop. (Sol)

## Major

- **RS00 scales are unsettled**: code uses ±50 rad/s and ±17 N·m, every RS00 manual says ±33 and ±14. The left forearm is
  RS00. Settle with a capture on a loose RS00 before motion. **Checked** (`motor_type.rs:21-32`).
- Degraded hold keeps drives powered 5–15 s after a drive goes silent; tenet 3 says stop.
- Silence stop is 100 ms (calibration loop needs 50 ms); no 60/75 °C or VBUS stops.
- No tool to scan or set drive IDs (needed for old IDs on the left drives).
- Limits live in five places with three different values for the same joint.
- Several processes can write to the bus (motor-repl, MCP-launched owner).
- No per-frame log by default; no shared clock.
- Soft limits ~1.5° inside hard stops; the plans need 2–3° supervised, ≥5° unattended.

## Hard-won knowledge to lift before deleting anything

From Opus Q6 and Sol, confirmed in shape by the others. The rewrite must carry these over:

- Codec facts: ID layout, host 0xFD / identity 0xFE, version reply `00 C4 56` is not a pose, refuse out-of-range rather than
  clamp, mode and fault bits (`crates/robstride/src/comm.rs`, `identity.rs`, `mit.rs`, `feedback.rs`).
- MIT scale table with RS03's measured ±20 rad/s fix (`motor_type.rs`, `tests/rs03_velocity_scale.rs`).
- TX allowlist: never send types 7/22/23/25 casually, never write 0x7029/0x702B (`bus.rs:45-53`, `params.rs`).
- Firmware timing: 800 ms quiet after Set Zero, 450 ms blackout (`davout/src/lib.rs:193-203`,
  `docs/commissioning/firmware/robstride-firmware-behavior.md`).
- Enable order: active reporting off, wait for its echo, Enable, then RunMode; stop sequence SpeedTarget 0, neutral MIT, Disable.
- Active reporting persists across processes, so turn it off at exit.
- Socket options: `RECV_OWN_MSGS`, `SO_TIMESTAMPNS`, `SO_RXQ_OVFL`, error subscription, transmit-only stop with ENOBUFS retry
  (`bus.rs`, `motor-repl/src/stop.rs`).
- The drive model for the vcan emulator: `davout/tests/physical_firmware/mod.rs` (echoes, blackouts, reboots).
- MCP2515 pacing rules, needed only while any bus is still on the HAT.
- Right-arm signs, taught limits and friction fits now in config.

## Before deleting the old stack

Move the right arm into the skeleton as a limb on `can0` first, or deleting the old crates strands it. Lift the knowledge above,
the `gravity-fit` and `firmware-timing` tools, and the commissioning docs (limb playbook, bench ledger, firmware behaviour).
Then delete per the table. Speculative ADRs, proto, docs and repo clutter to delete are listed in Opus Q9.

## Reviewer notes for the scorecard

- Opus cost $17.54 (subagent fan-out), against the recipe's ~$2 estimate; its JSON `result` held only a closing note, the review
  came from the transcript. It also used PowerShell once, outside its allowed tools (hashes unchanged). Tighten before next run.
- DeepSeek's first run hit its 32k reasoning budget with no review; the retry delivered.
- Composer was lenient (kept most components) against five reviewers; no false blocker found in a first read.
