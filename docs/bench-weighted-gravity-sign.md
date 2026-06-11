# Bench: weighted gravity sign test (M4)

Validates `tau_g` sign on **real hardware** before repeating elevated-pose gravity comp. Targets the queued item in [bench-test-backlog.md](bench-test-backlog.md).

**Hypothesis:** Left bare-motor runaway was a sign/zero issue. Loading the **right** side with a measured fixture and running `weighted_single_arm` isolates model sign from left-side calibration.

## Prerequisites

- [safety.md](safety.md) read; E-stop reachable
- Both shoulder pitch motors commissioned (left can1/id 12, right can0/id 2)
- Left set-zero fixed (host `0xFD` + payload `0x01` per backlog)
- Pi deployed (`pi_sync_main`) and CAN up (`pi_can_up`)
- **Measured** loaded-side mass in `shoulder_pitch_weighted.urdf` (see below)

## Step 1 — Weigh the loaded fixture

1. Attach the **right** bench fixture (dowel + any weight plate) as it will sit on the motor.
2. Weigh on a scale (kg).
3. Update the URDF:

```bash
./scripts/bench-set-weighted-mass.sh <mass_kg> right
git diff assets/urdf/shoulder_pitch_weighted.urdf   # verify
```

4. Sync to Pi: `pi_sync_bench_config` with `profile: shoulder_pitch_weighted`, `install_to_opt: true`.

Unloaded (left) side stays at **0.083 kg** (measured dowel) unless you add mass there too.

## Step 2 — Preview torques (read-only)

On Pi with weighted config:

```bash
export MARENGO_CONFIG_DIR=config/bringup/shoulder_pitch_weighted
bin/motor-repl gravity-preview 0 0
bin/motor-repl gravity-preview 0 0.3
bin/motor-repl gravity-preview 0 -0.3
```

Or via MCP: `pi_gravity_preview` with `config_dir: shoulder_pitch_weighted`.

**Expect:** `right_shoulder_pitch` |tau_g| larger than left at `q_right=0.3`; signs stable across angles.

## Step 3 — Sign pulse (before gravity-on)

Per [safety.md](safety.md): small `torque_ff` pulse per joint; verify direction matches URDF. Fix `direction` in `motors.yaml` if inverted.

## Step 4 — Weighted harness (live motion)

**Physical:** Support the **right** (loaded) arm for first enable. Hands clear. Two operator approvals required for weighted motion.

Via MCP:

```text
pi_bench_harness
  profile: weighted_single_arm
  loaded_joint: right_shoulder_pitch
  gravity_angles: [0, 0.3, -0.3]
  confirm: true
  confirm_weighted_motion: true
```

Harness uses `config/bringup/shoulder_pitch_weighted` automatically.

**Pass criteria:**

1. All harness steps PASS; no faults in `var/log/bench-latest.log`
2. `gravity-on` at q≈0: right arm **backdrivable** (no runaway)
3. Upright-pose test: slowly release support; loaded arm must not free-fall
4. `disable` clean; `fault=0x0000`

## Step 5 — Left side (after right passes)

Repeat with `loaded_joint: left_shoulder_pitch` only after right sign is confirmed and left zero is verified (`pi_homing_status`).

Update left mass in URDF if left carries extra fixture:

```bash
./scripts/bench-set-weighted-mass.sh <mass_kg> left
```

## Step 6 — Close the backlog item

Mark the 2026-05-24 entry in [bench-test-backlog.md](bench-test-backlog.md) `done` with log path and result summary.

## Troubleshooting

| Symptom | Action |
|---------|--------|
| Runaway on gravity-on | `disable` / E-stop; check `direction` in `motors.yaml`; re-run sign pulse |
| Loaded arm feels “heavy” or “floaty” | Mass/COM wrong — re-weigh; check URDF matches physical fixture |
| Harness uses wrong URDF | Ensure profile is `weighted_single_arm` (selects `shoulder_pitch_weighted` config) |
| Left still fails after right passes | Left-specific zero or `direction`; use `shoulder_pitch_left_only` profile |

## Related

- [pi-commissioning.md](pi-commissioning.md) Phase 5
- [ADR 0004](decisions/0004-control-modes-and-mit.md) — GravityComp mode
- MCP skill: `.cursor/skills/marengo-pi-mcp/SKILL.md`
