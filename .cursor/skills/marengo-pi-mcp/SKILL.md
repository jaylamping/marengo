---
name: marengo-pi-mcp
description: SSH Marengo Pi bench control via MCP — log-first investigation, confirm-gated motion, pi_bench_harness. Use when debugging CAN/motors/gravity on marengo.local or when the user mentions Pi bench, motor-repl, marengo-pi, or shoulder pitch bring-up.
---

## Workspace rules (mandatory)

Honor `.cursor/skills/_shared/workspace-rules.md` before task-specific work (always-apply `.cursor/rules/`, including Explore/Library model routing).


# Marengo Pi MCP

MCP server: `tools/marengo-pi-mcp/` (configure in `.cursor/mcp.json`).

**Rebuild after tool changes:** `just mcp-build` from repo root, then **restart** the `marengo-pi` MCP server in Cursor.

Host: **`marengo.local`** user **`joey`**. Pi root: **`/opt/marengo`**.

## Log-first (mandatory — no permission ask)

1. On any question or failure: **`pi_logs_last_fault`** → **`pi_logs_tail`** → **`pi_logs_grep`** — call immediately.
2. After any **motion** session (`pi_hold_on`, `pi_bench_harness`, `pi_marengo_pi_script`): read **`pi_candump_summary`** and correlate with **`position-trace-latest.csv`** (via `pi_read_file` tail or analyze script). CAN is the wire truth; trace is planner/code truth.
3. Never ask the user to paste logs available in `var/log/bench-latest.log`, `position-trace-latest.csv`, or **`candump-latest.log`**.
4. Never ask *“may I SSH?”* for read-only tools — pre-authorized.
5. Never ask the user to run Pi commands, paste command output, deploy files, or verify software state when an MCP tool can do it. Use the MCP tool yourself and report the result.
6. If a needed Pi action is missing from MCP, prefer adding/fixing the MCP tool and rebuilding it over falling back to user-run commands.

## Read-only (no confirm)

`pi_health`, `pi_can_status`, `pi_motor_repl_status`, `pi_gravity_preview`, `pi_logs_*`, **`pi_logs_archive_list`**, **`pi_candump_summary`**, `pi_journal`, `pi_candump_once`, `pi_imu_probe`, `pi_read_file`

## Admin (no confirm)

`pi_can_up`, **`pi_sync_main`**, `pi_sync_bench_config`, `pi_sync_bench_urdf`, `pi_wait_deploy`, `pi_install_staging`, `pi_git_pull`, `pi_build` — see [marengo-pi-sync](../marengo-pi-sync/SKILL.md)

**Config sync:** after editing `config/bringup/*/control.yaml` (or motors/robot) on the Mac, run **`pi_sync_bench_config`** with `profile: shoulder_pitch_right_only` or `shoulder_pitch_left_only`, `install_to_opt: true`; do not ask the user to run rsync/deploy manually. After editing bench URDF assets, run **`pi_sync_bench_urdf`** with the relevant `assets` list, `install_to_opt: true`; use the default asset list for right weighted/right-only COM calibration, or pass `shoulder_pitch_left_bare.urdf` for left-bench. If `/opt` install fails, run **`pi_install_staging`** (or passwordless `install-pi.sh`) and retry — do not trust stale `/opt` inertials. After URDF sync, verify gravity (`pi_gravity_preview` / hold) before treating COM edits as live.

## Admin (confirm required)

- **`pi_restart_marengo_pi`** — `{ "confirm": true }` (optional `mode: "stop"`). Reloads Davout hard limits from `motors.yaml` after Consul Set Limits Apply. Motors go limp — support elevated arms. Does not restart gateway.
- **`pi_clean_tree`** — stash/reset Pi git tree when sync is blocked by dirt.

## Motion (confirm required)

Ask the user only for physical actions or required safety confirmations: support the arm, keep hands off, power-cycle, plug hardware, enter sudo password, use Motor Studio UI, or approve weighted/live motion. After confirmation, run the MCP motion tool yourself.

**Motor fault (no Motor Studio — do not ask user to paste SSH):**

1. **`pi_motor_recover`** with `{ "confirm": true }` — full recover script (disable → status → `RECOVER_OK` / `RECOVER_FAIL`); logs to `var/log/bench-latest.log`.
2. Read result via **`pi_logs_last_fault`** / tail — do not ask user to paste terminal output.
3. `RECOVER_FAIL` or arm still limp → user **power-cycles drive**, then run **`pi_motor_recover`** again.
4. Optional first step: **`pi_hold_off`** (confirm) if `marengo-pi` was running.

Motion tools pick **`config_dir`** from **`joint`** when omitted: `right_shoulder_pitch` → `shoulder_pitch_right_only`, `left_shoulder_pitch` → `shoulder_pitch_left_only`. Otherwise default right-only. Set `MARENGO_PI_HOST` to Pi IP if `.local` fails.

| Profile | Params |
|---------|--------|
| `bare_motor` | `confirm: true` |
| `weighted_single_arm`, `arm_attached` | `confirm: true` + `confirm_weighted_motion: true` (after **two** chat approvals) |

Prefer **`pi_bench_harness`** for debug sessions. Sustained control uses **`pi_marengo_pi_script`** (not `pi_motor_enable` alone).

**Encoder zero (replaces Motor Studio):** position arm at mechanical zero → **`pi_set_zero`** with `confirm: true`. Then **`pi_homing_status`**; all joints must be `Verified` before **`home`** / enable. `pi_hold_on` defaults `set_zero: false` — only set true when intentionally recalibrating.

**Backdrive:** use **`gravity-on`** in `pi_marengo_pi_script` with **`timeout_sec: 15`** (default). Script `["home","enable bench","gravity-on"]`, `joint: left_shoulder_pitch` or right equivalent. Stay within ±1.0 rad during manual moves. Bump timeout only when the move needs it (e.g. gravity assist to arm-down: `timeout_sec: 60`).

**Position hold:** **`pi_hold_on`** (`confirm: true`, `joint: left_shoulder_pitch` for left bench) — set-zero, `hold-on` or `position_rad` for `hold-at`, **15 s default**, logs to `var/log`. Pass **`timeout_sec`** explicitly for long traverses (π rad at 0.25 rad/s ≈ 20 s → use ≥ 30). **`pi_hold_off`** with same `joint` to stop. Current right-only bench tuning: **kp=12, kd=2.0**, slew **0.10 rad/s**, max_lead **0.03**, trim **0.0** after set-zero at arm-down; feedback velocity guard disables on sustained overspeed above the effective bench cap. Operator limits are **[-0.872665, 3.141593] rad** (-50° to +180°); hard bench/URDF envelope is **[-0.9, 3.17] rad**. Sync profile to Pi before hold tests if YAML changed locally. **Hands off** during scripted round trips.

**Left bench round trip (hands off):** `pi_marengo_pi_script` with `joint: left_shoulder_pitch`, script `home` → `enable bench` → `hold-at 0` (pause) → `hold-at 1.570796` → `hold-at 0` → `disable`, `timeout_sec` ≥ 30 for full round trip (default 15 is for short probes only).

**Weighted proposals:** if logs suggest load/model issues, ask user to run weighted bench now; if deferred, append [docs/bench-test-backlog.md](../../docs/bench-test-backlog.md).

## Bench workflow

1. Check `MARENGO_BENCH_PROFILE`.
2. `pi_health` + `pi_can_up` before motion.
3. **`bare_motor`:** one user OK → `pi_bench_harness` with `confirm: true`.
4. **Weighted:** ask twice in chat → harness with both confirm flags.
5. After harness: `pi_logs_last_fault` + **`pi_candump_summary`** + position trace JSON in response; link log paths in issues.
6. Session history: **`pi_logs_list`** / **`pi_logs_archive_list`** (gateway SQL) before raw file greps when gateway is up.

### Bench telemetry triad (motion troubleshooting)

Every `pi_hold_on` / harness run auto-records:

| Layer | File | What it shows |
|-------|------|----------------|
| Text | `bench-latest.log` | 1 Hz diagnostics, faults |
| Code | `position-trace-latest.csv` | 200 Hz planner/MIT command |
| **Wire** | **`candump-latest.log`** | raw CAN frames (TX/RX rate) |

Compare: trace smooth + candump jerky → firmware/mechanical; both jerky → control tuning; candump rate ≠ ~400/s/motor → comm/scheduling issue.

Commissioned map: right `can0`/id **2**, left `can1`/id **12**. Bench profiles: `shoulder_pitch_right_only`, `shoulder_pitch_left_only` (mirrored tuning); dual: `shoulder_pitch_dual`.

Docs: [pi-commissioning.md](../../docs/pi-commissioning.md), [tools/marengo-pi-mcp/README.md](../../tools/marengo-pi-mcp/README.md).
