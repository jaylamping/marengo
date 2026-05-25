---
name: marengo-pi-mcp
description: SSH Marengo Pi bench control via MCP — log-first investigation, confirm-gated motion, pi_bench_harness. Use when debugging CAN/motors/gravity on marengo.local or when the user mentions Pi bench, motor-repl, marengo-pi, or shoulder pitch bring-up.
---

# Marengo Pi MCP

MCP server: `tools/marengo-pi-mcp/` (configure in `.cursor/mcp.json`).

Host: **`marengo.local`** user **`joey`**. Pi root: **`/opt/marengo`**.

## Log-first (mandatory — no permission ask)

1. On any question or failure: **`pi_logs_last_fault`** → **`pi_logs_tail`** → **`pi_logs_grep`** — call immediately.
2. Never ask the user to paste logs available in `var/log/bench-latest.log`.
3. Never ask *“may I SSH?”* for read-only tools — pre-authorized.

## Read-only (no confirm)

`pi_health`, `pi_can_status`, `pi_motor_repl_status`, `pi_gravity_preview`, `pi_logs_*`, `pi_journal`, `pi_candump_once`, `pi_read_file`

## Admin (no confirm)

`pi_can_up`, **`pi_sync_main`** — see [marengo-pi-sync](../marengo-pi-sync/SKILL.md)

## Motion (confirm required)

| Profile | Params |
|---------|--------|
| `bare_motor` | `confirm: true` |
| `weighted_single_arm`, `arm_attached` | `confirm: true` + `confirm_weighted_motion: true` (after **two** chat approvals) |

Prefer **`pi_bench_harness`** for debug sessions. Sustained control uses **`pi_marengo_pi_script`** (not `pi_motor_enable` alone).

**Encoder zero (replaces Motor Studio):** position arm at mechanical zero → **`pi_set_zero`** with `confirm: true`. Default joint `right_shoulder_pitch`; use `config_dir: .../shoulder_pitch_right_only` for isolated right bench. `verify: true` (default) prints feedback position — expect |pos| < 0.05 rad.

**Backdrive:** use **`gravity-on`** in `pi_marengo_pi_script` with **`timeout_sec: 60`** (default). Script `["home","enable bench","gravity-on"]`, `config_dir: .../shoulder_pitch_right_only`. Stay within ±1.0 rad during manual moves. Optional **`impedance-on`** if it feels too loose later.

**Weighted proposals:** if logs suggest load/model issues, ask user to run weighted bench now; if deferred, append [docs/bench-test-backlog.md](../../docs/bench-test-backlog.md).

## Bench workflow

1. Check `MARENGO_BENCH_PROFILE`.
2. `pi_health` + `pi_can_up` before motion.
3. **`bare_motor`:** one user OK → `pi_bench_harness` with `confirm: true`.
4. **Weighted:** ask twice in chat → harness with both confirm flags.
5. After harness: `pi_logs_last_fault` + read JSON in response; link log path in issues.

Commissioned map: right `can0`/id **2**, left `can1`/id **12**. Default dual config: `shoulder_pitch_dual`; right-only bench: `shoulder_pitch_right_only`.

Docs: [pi-commissioning.md](../../docs/pi-commissioning.md), [tools/marengo-pi-mcp/README.md](../../tools/marengo-pi-mcp/README.md).
