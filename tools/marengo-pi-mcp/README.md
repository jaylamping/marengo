# Marengo Pi MCP

MCP server for Marengo bench control on a Raspberry Pi over SSH. Runs on your **dev Mac**; the Pi is only an SSH target.

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

```sudoers
# Marengo MCP — passwordless sudo for bench scripts only (visudo -f /etc/sudoers.d/marengo-joey)
joey ALL=(ALL) NOPASSWD: /opt/marengo/scripts/can-up.sh *
joey ALL=(ALL) NOPASSWD: /opt/marengo/scripts/install-pi.sh
# If you use deploy-pi.sh staging under ~/marengo:
# joey ALL=(ALL) NOPASSWD: /home/joey/marengo/scripts/install-pi.sh
```

Verify:

```bash
sudo -n /opt/marengo/scripts/can-up.sh can0 can1
sudo -n /opt/marengo/scripts/install-pi.sh
```

Do **not** prefix `MARENGO_INSTALL_ROOT=` on the install command — default is `/opt/marengo` and sudoers blocks arbitrary env vars.

### Build MCP server

```bash
cd tools/marengo-pi-mcp
npm install
npm run build
```

### Cursor `mcp.json`

Copy [`mcp.json.example`](mcp.json.example) into `.cursor/mcp.json` and adjust paths.

## Tool summary

| Class | Confirm | Examples |
|-------|---------|----------|
| Read-only | No | `pi_logs_tail`, `pi_health`, `pi_motor_repl_status`, `pi_gravity_preview` |
| Admin | No | `pi_can_up`, `pi_sync_main`, `pi_git_pull`, `pi_build` |
| Motion | Yes | `pi_set_zero`, `pi_bench_harness`, `pi_marengo_pi_script`, `pi_jog` |

Weighted profile (`weighted_single_arm`, `arm_attached`) requires **`confirm: true`** and **`confirm_weighted_motion: true`**.

### Encoder zero (no Motor Studio)

1. Position shaft at mechanical zero (arm down).
2. **`pi_set_zero`** with `confirm: true`, `joint: "right_shoulder_pitch"`, optional `config_dir` for right-only bench.
3. Check readback: `pos=` should be within ±0.05 rad when `verify: true` (default).

```json
{
  "confirm": true,
  "joint": "right_shoulder_pitch",
  "config_dir": "/opt/marengo/config/bringup/shoulder_pitch_right_only"
}
```

1. Local `git pull --ff-only` on `main` (fails if dirty)
2. `./scripts/deploy-pi.sh joey@marengo.local`
3. Remote `install-pi.sh` → `/opt/marengo`
4. Writes `/opt/marengo/.deploy-rev`

## Session logs

Motion runs tee to `$MARENGO_ROOT/var/log/bench-latest.log`. Read with `pi_logs_tail` / `pi_logs_last_fault`.

## Skills

- [`.cursor/skills/marengo-pi-mcp/SKILL.md`](../../.cursor/skills/marengo-pi-mcp/SKILL.md) — log-first bench workflow
- [`.cursor/skills/marengo-pi-sync/SKILL.md`](../../.cursor/skills/marengo-pi-sync/SKILL.md) — sync-with-main deploy

See also [docs/pi-commissioning.md](../../docs/pi-commissioning.md).
