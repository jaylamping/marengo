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

**WSL / Linux / macOS (software home):** mcp.json uses **`bash` + [`run-mcp.sh`](run-mcp.sh)**. Cursor's spawn PATH often lacks mise `node`, so bare `node` fails with `spawn node ENOENT`. The shell launcher finds mise (or `MARENGO_MCP_NODE`).

**Windows CAD session:** prefer [`run-mcp.cmd`](run-mcp.cmd) / [`run-mcp.ps1`](run-mcp.ps1) — Cursor often lacks Git `sh`/`bash`. Those wrappers exec [`dist/launch.js`](src/launch.ts) once `node` is resolved.

Profile / SSH defaults live in [`src/launch.ts`](src/launch.ts) / `run-mcp.sh` — **not** in `mcp.json` `env` (that thrash auto-disables the project MCP; see [ADR 0016](../../docs/decisions/0016-wsl-software-home.md)).

Open the marengo repo root as the Cursor workspace (WSL software session, or Windows UNC CAD session — ADR 0016).

`MARENGO_LOCAL_ROOT` is optional. The server derives the repo root from its install path. Override only for unusual clone layouts.

After clone or pull that touches MCP sources:

```bash
just mcp-build
```

Then restart the marengo-pi MCP server in Cursor.

**Stay enabled:** `.cursor/hooks.json` runs `session-start-marengo.js` on `sessionStart` (`--write --best-effort`) so disabled/unapproved state is scrubbed when a chat opens. If the server still shows as disabled after a config thrash:

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

### One CAN owner

Every `motor-repl` subcommand opens SocketCAN and sends type-24 active-reporting frames while starting up. That includes `status`, `homing-status` and `gravity-preview`. If `marengo-pi` already owns the bus, that extra traffic can latch a persistent Transport fault in `marengo-pi`. So while a `marengo-pi` or `motor-repl` process runs (`pgrep -x`):

- `pi_motor_repl_status`, `pi_gravity_preview` and `pi_can_up` print `… skipped: <name> (pid N) owns CAN` and leave the bus alone.
- `pi_health`, `pi_homing_status` and `pi_sync_bench_config` show per-joint homing from marengo-pi's own `RobotState`, read from the gateway's `/snapshot/robot/state`. They don't run `motor-repl homing-status`.
- `scripts/homing-preflight.sh`, which `install-pi.sh` runs, skips `homing-status` (strict mode exits 1).

Motion tools that open CAN take sole ownership of the bus for the session. These are `pi_motor_enable`, `pi_motor_disable`, `pi_motor_recover`, `pi_set_zero`, `pi_jog`, `pi_hold_on`, `pi_hold_off`, `pi_marengo_pi_script` and `pi_bench_harness`. Each session:

1. Stops `marengo-pi.service` with `sudo -n /usr/local/libexec/marengo/pi-restart-marengo-pi.sh stop`, the same helper `pi_restart_marengo_pi` uses. The service runs as `marengo` with `Restart=always`, so a bare `pkill` either fails or lets systemd start a second owner within 5 s.
2. Kills leftover `marengo-pi` processes owned by the deploy user.
3. Refuses to run (exit 1) if any `marengo-pi` or `motor-repl` is still running.
4. Restarts the unit when the session ends if it was active before, the same way `install-pi.sh` restores state. The unit comes back Disabled. The restart runs on every exit path, including errors and SIGHUP/SIGTERM. `pi_bench_harness` restarts it in a final `restore_marengo_pi_service` step.

To keep control off after a session, run `pi_restart_marengo_pi` with `mode: stop`.

### Reference and zero (no Motor Studio)

A current reference is granted only inside the process that acquires it. When that process exits, the grant ends. Acquiring a reference runs SetZero at the joint's current pose, so put each joint at its mechanical reference first.

- `pi_hold_on` and `pi_bench_harness` acquire references inside their own marengo-pi session. They send `home <joints> sign-tested` on stdin and wait until marengo-pi prints `reference <joint> current pos=…` for every joint. Then they send the plain `home`, `enable` and hold lines, so dwell sleeps begin only once the references are held. If a joint prints `reference <joint> failed|skipped: …` or `home failed: …`, or the wait passes 10 s per joint, the session sends `disable` and `quit` and exits 1.
  `pi_marengo_pi_script` applies the same wait to any `home <joints> sign-tested` line in its script.
- Both tools require `set_zero: true` **and** `at_mechanical_reference: true`. Without them they refuse before contacting the Pi, so a session never re-zeros at an arbitrary pose. `pi_hold_on` references only `joint` when you give one. If you omit it, it references every joint of the bench profile and holds `right_shoulder_pitch`. `pi_bench_harness` references `joints`, or every joint of the profile, once. It then runs all enable-requiring suites in **one** marengo-pi process: grants survive a clean `disable` but not the process, and re-zeroing per suite would accumulate the return-to-0 tracking error. Before each later suite it checks `$LOG` and stops if an earlier suite failed.
- `pi_set_zero` runs `motor-repl set-zero <joint> --sign-tested` and then `homing-status`. That checks SetZero and the readback, but the grant ends when motor-repl exits. It doesn't let a later marengo-pi enable.
- `pi_motor_recover` never acquires a reference and never enables. After a fault the arm isn't attested at the reference, so it disables and reads `fault=` from `status` while Disabled.

```json
{
  "confirm": true,
  "joint": "right_shoulder_pitch",
  "set_zero": true,
  "at_mechanical_reference": true,
  "position_rad": 0.1
}
```

`config_dir` defaults to `/opt/marengo/config`. For a 3-DOF harness run, select the harness profile that exports `MARENGO_JOINT_SUBSET`. Don't point `MARENGO_CONFIG_DIR` at a separate bringup tree.

### `pi_sync_main`

1. Local `git pull --ff-only` on `main` (fails if dirty)
2. `./scripts/deploy-pi.sh joey@marengo.local`
3. Remote `install-pi.sh` → `/opt/marengo`
4. Writes `/opt/marengo/.deploy-rev`

### `pi_restart_marengo_pi`

Stops leftover `marengo-pi` processes and restarts `marengo-pi.service` so Davout reloads boot config (URDF + YAML). Numerical Set Limits Apply hot-reloads live limits and write-behind URDF without restart ([ADR 0012](../../docs/decisions/0012-config-db-overrides.md) / [ADR 0017](../../docs/decisions/0017-bench-set-limits-urdf-expand.md)); use restart for binary deploy, structural wiring, or after a failed persist left disk stale. Requires `confirm: true`. Optional `mode: "stop"` skips systemd start. Does not touch `marengo-gateway`.

Canonical remote body: [`scripts/pi-restart-marengo-pi.sh`](../../scripts/pi-restart-marengo-pi.sh) (also used by `pi-remote.sh restart-marengo-pi` / `stop-marengo-pi`, and Consul `POST /control/restart-marengo-pi`). MCP embeds the local checkout copy so the tool works before the Pi has the new script installed.

```json
{ "confirm": true }
```

### `pi_sync_tree`

Sync the Pi staging checkout (`MARENGO_PI_STAGING_ROOT`, default `~/marengo`) with `origin/main` without building or installing. Fails if that tree is dirty. `pi_git_pull`, `pi_clean_tree` and `pi_build` work in the same checkout. `/opt/marengo` is the root-owned install tree and isn't a git checkout.

1. `git fetch origin`
2. `git checkout main`
3. `git pull --ff-only origin main`

## Session logs

Motion runs tee to `$MARENGO_ROOT/var/log/bench-latest.log`. Read with `pi_logs_tail` / `pi_logs_last_fault`.

### Motor recover (no Motor Studio)

```json
{ "confirm": true }
```

Tool: `pi_motor_recover`. Disable drives, brief `status` with `fault=0x…`, prints `RECOVER_OK` or `RECOVER_FAIL`. Optional: `"config_dir": "/opt/marengo/config"`; omission uses the same master directory.

## Skills

- [`.cursor/skills/marengo-pi-mcp/SKILL.md`](../../.cursor/skills/marengo-pi-mcp/SKILL.md) — log-first bench workflow
- [`.cursor/skills/marengo-pi-sync/SKILL.md`](../../.cursor/skills/marengo-pi-sync/SKILL.md) — sync-with-main deploy

Also [docs/pi-commissioning.md](../../docs/pi-commissioning.md).
