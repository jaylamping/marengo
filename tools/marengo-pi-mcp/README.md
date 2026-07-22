# Marengo Pi MCP

MCP server for Marengo bench control on a Raspberry Pi over SSH. Runs on your dev machine; the Pi is only an SSH target.

## Setup

### SSH

```text
# ~/.ssh/config
Host marengo.local
  Hostname marengo.local
  User joey
  IdentityFile ~/.ssh/id_ed25519
```

Verify: `ssh joey@marengo.local 'echo ok'`

If mDNS fails, set `MARENGO_PI_HOST` in the environment before Cursor starts the MCP (or export it in your shell profile). Do not add it to `.cursor/mcp.json` — that env is hashed into Cursor’s MCP approval key and thrashing it auto-disables the server.

### Passwordless sudo (Pi)

`scripts/install-pi.sh` provisions these rules when run as root. For manual repair:

```sudoers
# Marengo MCP — passwordless sudo for bench scripts only (visudo -f /etc/sudoers.d/marengo-joey)
joey ALL=(root) NOPASSWD: /opt/marengo/scripts/can-up.sh *
joey ALL=(root) NOPASSWD: /opt/marengo/scripts/install-pi.sh
joey ALL=(root) NOPASSWD: /home/joey/marengo/scripts/install-pi.sh
```

Not full passwordless sudo. Only these paths.

Verify:

```bash
sudo -n /opt/marengo/scripts/can-up.sh can0 can1
sudo -n /home/joey/marengo/scripts/install-pi.sh   # staging → /opt (use this after deploy)
```

`sudo -n true` fails by design. After editing files under `~/marengo`, run `pi_install_staging` (MCP) or the staging install command above. Do not run `/opt/marengo/scripts/install-pi.sh` alone; it re-copies stale `/opt` content.

### Build MCP server

From repo root:

```bash
just mcp-build
```

Or:

```bash
cd tools/marengo-pi-mcp
npm install
npm run build
```

Restart the marengo-pi MCP server in Cursor after rebuilding.

If a tool still shows old behavior after `npm run build`, the Cursor MCP process is stale. Restart marengo-pi before retrying.

### Cursor `mcp.json`

Repo [`.cursor/mcp.json`](../../.cursor/mcp.json) uses `${workspaceFolder}` so the same file works on **Windows, macOS, and Linux/WSL**.

**WSL / Linux / macOS (software home):** mcp.json uses **`bash` + [`run-mcp.sh`](run-mcp.sh)**. Cursor's spawn PATH often lacks mise `node`, so bare `node` + `launch.mjs` fails with `spawn node ENOENT`. The shell launcher finds mise (or `MARENGO_MCP_NODE`).

**Windows CAD session:** prefer [`run-mcp.cmd`](run-mcp.cmd) / [`run-mcp.ps1`](run-mcp.ps1) — Cursor often lacks Git `sh`/`bash`. [`launch.mjs`](launch.mjs) is the Node entry those wrappers exec once `node` is resolved.

Profile / SSH defaults live in `launch.mjs` / `run-mcp.sh` / `run-mcp.ps1` — **not** in `mcp.json` `env` (that thrash auto-disables the project MCP; see [ADR 0016](../../docs/decisions/0016-wsl-software-home.md)).

Open the marengo repo root as the Cursor workspace (WSL software session, or Windows UNC CAD session — ADR 0016).

`MARENGO_LOCAL_ROOT` is optional. The server derives the repo root from its install path. Override only for unusual clone layouts.

After clone or pull that touches MCP sources:

```bash
just mcp-build
```

Then restart the marengo-pi MCP server in Cursor.

**Stay enabled:** `.cursor/hooks.json` runs `session-start-marengo.mjs` on `sessionStart` (`--write --best-effort`) so disabled/unapproved state is scrubbed when a chat opens. If the server still shows as disabled after a config thrash:

```bash
# Quit Cursor first for a hard write, then:
just mcp-ensure-enabled --write
```

## Tool summary

| Class | Confirm | Examples |
|-------|---------|----------|
| Read-only | No | `pi_logs_tail`, `pi_health`, `pi_motor_repl_status`, `pi_gravity_preview`, `pi_imu_probe` |
| Admin | No | `pi_can_up`, `pi_sync_main`, `pi_sync_tree`, `pi_sync_bench_config`, `pi_sync_bench_urdf`, `pi_wait_deploy`, `pi_install_staging`, `pi_git_pull`, `pi_build` |
| Admin | Yes | `pi_restart_marengo_pi`, `pi_clean_tree` |
| Motion | Yes | `pi_motor_recover`, `pi_motor_disable`, `pi_set_zero`, `pi_homing_status`, `pi_hold_on`, `pi_hold_off`, `pi_bench_harness`, `pi_marengo_pi_script`, `pi_jog` |

Weighted profile (`weighted_single_arm`, `arm_attached`) needs `confirm: true` and `confirm_weighted_motion: true`.

### Encoder zero (no Motor Studio)

1. Position shaft at mechanical zero (arm down).
2. `pi_set_zero` with `confirm: true`. Verifies |pos| < 0.05 rad, writes calibration record.
3. `pi_homing_status`. Confirm all configured joints show `Verified` before enable/hold.
4. `motor-repl home` / marengo-pi `home`. Supervisor Ready when all joints verified.

```json
{
  "confirm": true,
  "joint": "right_shoulder_pitch",
"config_dir": "/opt/marengo/config/bringup/arm_3dof_right"
}
```

### `pi_sync_main`

1. Local `git pull --ff-only` on `main` (fails if dirty)
2. `./scripts/deploy-pi.sh joey@marengo.local`
3. Remote `install-pi.sh` → `/opt/marengo`
4. Writes `/opt/marengo/.deploy-rev`

### `pi_restart_marengo_pi`

Stops leftover `marengo-pi` processes and restarts `marengo-pi.service` so Davout reloads hard position limits from `motors.yaml` (e.g. after Consul Set Limits). Requires `confirm: true`. Optional `mode: "stop"` skips systemd start. Does not touch `marengo-gateway`.

Canonical remote body: [`scripts/pi-restart-marengo-pi.sh`](../../scripts/pi-restart-marengo-pi.sh) (also used by `pi-remote.sh restart-marengo-pi` / `stop-marengo-pi`, and Consul `POST /control/restart-marengo-pi`). MCP embeds the local checkout copy so the tool works before the Pi has the new script installed.

```json
{ "confirm": true }
```

### `pi_sync_tree`

Sync the Pi Marengo repo with `origin/main` without building or installing. Fails if the Pi working tree is dirty.

1. `git fetch origin`
2. `git checkout main`
3. `git pull --ff-only origin main`

## Session logs

Motion runs tee to `$MARENGO_ROOT/var/log/bench-latest.log`. Read with `pi_logs_tail` / `pi_logs_last_fault`.

### Motor recover (no Motor Studio)

```json
{ "confirm": true }
```

Tool: `pi_motor_recover`. Disable drives, brief `status` with `fault=0x…`, prints `RECOVER_OK` or `RECOVER_FAIL`. Optional: `"config_dir": "/opt/marengo/config/bringup/arm_3dof_right"`.

## Skills

- [`.cursor/skills/marengo-pi-mcp/SKILL.md`](../../.cursor/skills/marengo-pi-mcp/SKILL.md) — log-first bench workflow
- [`.cursor/skills/marengo-pi-sync/SKILL.md`](../../.cursor/skills/marengo-pi-sync/SKILL.md) — sync-with-main deploy

Also [docs/pi-commissioning.md](../../docs/pi-commissioning.md).
