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

If mDNS fails, set `MARENGO_PI_HOST` to the Pi IP in `mcp.json`.

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

`MARENGO_LOCAL_ROOT` is optional. The server derives the repo root from its install path. Override only for unusual clone layouts.

After clone or pull that touches MCP sources:

```bash
just mcp-build
```

Then restart the marengo-pi MCP server in Cursor.

## Tool summary

| Class | Confirm | Examples |
|-------|---------|----------|
| Read-only | No | `pi_logs_tail`, `pi_health`, `pi_motor_repl_status`, `pi_gravity_preview`, `pi_imu_probe` |
| Admin | No | `pi_can_up`, `pi_sync_main`, `pi_sync_tree`, `pi_sync_bench_config`, `pi_git_pull`, `pi_build` |
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
  "config_dir": "/opt/marengo/config/bringup/shoulder_pitch_right_only"
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

Tool: `pi_motor_recover`. Disable drives, brief `status` with `fault=0x…`, prints `RECOVER_OK` or `RECOVER_FAIL`. Optional: `"config_dir": "/opt/marengo/config/bringup/shoulder_pitch_right_only"`.

## Skills

- [`.cursor/skills/marengo-pi-mcp/SKILL.md`](../../.cursor/skills/marengo-pi-mcp/SKILL.md) — log-first bench workflow
- [`.cursor/skills/marengo-pi-sync/SKILL.md`](../../.cursor/skills/marengo-pi-sync/SKILL.md) — sync-with-main deploy

Also [docs/pi-commissioning.md](../../docs/pi-commissioning.md).
