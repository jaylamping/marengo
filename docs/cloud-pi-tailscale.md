# Cloud Pi access via Tailscale

Cloud agents can reach the Marengo bench over **Tailscale userspace networking** (required in Cursor VMs), then use **`pi-remote.sh`** for the same log-first workflow as local `marengo-pi` MCP.

## One-time: Cursor secrets

Add these in [Cursor → Cloud Agents → Secrets](https://cursor.com/dashboard/cloud-agents) for your Marengo environment.

| Secret | Type | Value |
|--------|------|-------|
| `TAILSCALE_AUTH_KEY` | **Runtime Secret** | Ephemeral or reusable auth key from [Tailscale admin → Settings → Keys](https://login.tailscale.com/admin/settings/keys). Tag the key with the same ACL tag as the Pi if you use tagged nodes. |
| `MARENGO_PI_SSH_PRIVATE_KEY_B64` | **Runtime Secret** | Base64 of the `id_ed25519_marengo` private key authorized on the Pi. **macOS:** `base64 -i ~/.ssh/id_ed25519_marengo \| tr -d '\\n'`. **Linux:** `base64 -w0 ~/.ssh/id_ed25519_marengo`. |
| `MARENGO_PI_HOST` | Environment Variable | `joey-robot.tail0b414.ts.net` (or your Pi MagicDNS name). |
| `MARENGO_PI_USER` | Environment Variable | `joey` |
| `MARENGO_GATEWAY_LOG_TOKEN` | Runtime Secret (optional) | Only if `MARENGO_GATEWAY_LOG_TOKEN` is set on the Pi gateway. |

Do **not** commit private keys or auth keys to the repo.

### Tailscale ACL (recommended)

Ensure your tailnet allows the cloud agent hostname (`cursor-cloud-*`) to reach the Pi on:

- TCP 22 (SSH)
- TCP 8080 (gateway HTTP / log API)
- TCP 8443 / 8444 (Consul / WebTransport — optional for UI)

## How it works

1. **`.cursor/environment.json`** `install` runs `setup-cloud.sh` then `setup-cloud-pi.sh --prepare`:
   - Installs **Node.js ≥ 24.16** (NodeSource) before Consul `npm ci` — base cloud images ship Node 22, which fails Consul `engines`
   - Installs protoc / cargo tools, bootstraps the workspace, Tailscale, cross-GCC, and builds `marengo-pi-mcp`
2. **`start`** runs `setup-cloud-pi.sh --start-daemon` then `--connect`:
   - Starts `tailscaled` with **userspace networking** ([Cursor docs](https://cursor.com/docs/cloud-agent/setup#running-tailscale))
   - If Tailscale was never installed (failed install snapshot), start re-runs prepare
   - Connects with `TAILSCALE_AUTH_KEY` and writes SSH config under `~/.marengo-cloud-ssh/`
3. Agents use **`./scripts/pi-remote.sh`** instead of MCP when `marengo-pi` MCP is unavailable.

## Verify after secrets are set

```bash
./scripts/setup-cloud-pi.sh --verify
./scripts/pi-remote.sh health
./scripts/pi-remote.sh logs-list
```

Expected: `SSH_OK`, gateway `/health`, and JSON from `/logs/sessions`.

## Log-first workflow (cloud)

Same order as [marengo-pi MCP skill](../.cursor/skills/marengo-pi-mcp/SKILL.md):

```bash
./scripts/pi-remote.sh logs-last-fault
./scripts/pi-remote.sh logs-tail 200
./scripts/pi-remote.sh logs-grep 'fault|ERROR'
./scripts/pi-remote.sh candump-summary
```

HTTP-only (no SSH) when gateway is up:

```bash
./scripts/pi-remote.sh logs-sessions 15
./scripts/pi-remote.sh logs-structured 100
```

## Deploy from cloud

Cross-build and rsync over Tailscale SSH:

```bash
./scripts/pi-remote.sh deploy --install
```

Or:

```bash
export MARENGO_SSH_DIR="$HOME/.marengo-cloud-ssh"
export MARENGO_PI_HOST=joey-robot.tail0b414.ts.net
./scripts/deploy-pi.sh --install "joey@${MARENGO_PI_HOST}"
```

## Logging architecture

See [ADR 0011](decisions/0011-log-retention-and-archive.md) and [bench-position-tuning.md](bench-position-tuning.md#log-retention).

| Layer | Hot file | Cloud command |
|-------|----------|---------------|
| Text | `bench-latest.log` | `pi-remote.sh logs-tail` |
| Planner | `position-trace-latest.csv` | `pi-remote.sh ssh -- tail /opt/marengo/var/log/position-trace-latest.csv` |
| Wire | `candump-latest.log` | `pi-remote.sh candump-summary` |
| Archive | SQLite + `var/log/blobs/` | `pi-remote.sh logs-sessions` / `logs-list` |

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Environment setup / install fails with `EBADENGINE` / Node v22 | Cursor images ship Node 22 ahead of PATH (`/exec-daemon/node`, nvm). Latest `setup-cloud.sh` installs Node ≥24.16 and shims it into `/usr/local/cargo/bin`. Re-run install or trigger a new environment build. |
| `tailscaled: No such file or directory` | Install never finished — re-run `./scripts/setup-cloud.sh && ./scripts/setup-cloud-pi.sh --verify` |
| `TAILSCALE_AUTH_KEY not set` | Add secret; restart cloud agent |
| `Could not resolve hostname` | Tailscale not connected — check `/tmp/tailscaled.log`, re-run `--connect` |
| `Permission denied (publickey)` | Wrong or missing `MARENGO_PI_SSH_PRIVATE_KEY_B64` |
| `GET /logs/sessions` 401 | Set `MARENGO_GATEWAY_LOG_TOKEN` secret to match Pi |
| SSH works, rsync deploy fails | `export MARENGO_SSH_DIR=$HOME/.marengo-cloud-ssh` before `deploy-pi.sh` |
| `marengo-pi` MCP missing in cloud | Expected — use `pi-remote.sh` (MCP runs on local Cursor only today) |

## Local dev (unchanged)

Mac/Windows with LAN or Tailscale: keep using `marengo-pi` MCP and `MARENGO_PI_HOST` in `.cursor/mcp.json`.
