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

Repo [`.cursor/mcp.json`](../../.cursor/mcp.json) uses `${workspaceFolder}` so the same file works on Windows and Mac. Open the marengo repo root as the Cursor workspace (or `marengo.code-workspace`).

**Windows Cursor** launches via [`run-mcp.ps1`](run-mcp.ps1) (PowerShell). **macOS / Linux / WSL Cursor** should use `bash` + [`run-mcp.sh`](run-mcp.sh) instead — System32 `bash.exe` on Windows is the WSL launcher and is not a safe MCP command.

**Do not put `MARENGO_CONFIG_DIR` / `MARENGO_BENCH_PROFILE` / SSH env in `mcp.json`.** Cursor hashes those fields into an approval key; changing them auto-disables the project MCP until you re-enable it. Defaults live in the launchers (and `src/config.ts` fallbacks). Override with shell env only when needed.

`MARENGO_LOCAL_ROOT` is optional. The server derives the repo root from its install path. Override only for unusual clone layouts.

After clone or pull that touches MCP sources:

```bash
just mcp-build
```

Then restart the marengo-pi MCP server in Cursor.

**Stay enabled:** `.cursor/hooks.json` runs `session-start-marengo.ps1` on `sessionStart` (`--write --best-effort`) so disabled/unapproved state is scrubbed when a chat opens. If the server still shows as disabled after a config thrash:

```bash
# Quit Cursor first for a hard write, then:
just mcp-ensure-enabled --write
```

## Tool summary

| Class | Confirm | Examples |
|-------|---------|----------|
| Read-only | No | `pi_logs_tail`, `pi_health`, `pi_motor_repl_status`, `pi_gravity_preview`, `pi_imu_probe` |
| Admin | No | `pi_can_up`, `pi_sync_main`, `pi_sync_tree`, `pi_sync_bench_config`, `pi_sync_bench_urdf`, `pi_git_pull`, `pi_build` |
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
