# mem0 operations (Marengo)

> **Agent note (2026-07-14):** Cursor agent mem0 MCP is **disabled**. Use `.omo/session-handoff.md` and OpenSpec instead. This doc remains for the self-hosted stack / Consul observatory if you re-enable later.

Self-hosted stack: `C:\mem0\server` (fork: [jaylamping/mem0](https://github.com/jaylamping/mem0)).

## URLs (Tailscale Serve on joey-pc)

| Service | URL |
|---------|-----|
| Dashboard | `https://joey-pc.tail0b414.ts.net` |
| API | `https://joey-pc.tail0b414.ts.net:8888` |

After reboot:

```powershell
cd C:\mem0\server
docker compose up -d
.\scripts\tailscale-https.ps1
```

## API keys (scoped)

| Use | Scope | Where stored |
|-----|-------|--------------|
| Cursor `mem0-mcp` | read/write memories | User env `MEM0_API_KEY` → `.cursor/mcp.json` `${env:MEM0_API_KEY}` |
| Consul Vite proxy | list/search/history | `consul/.env.local` `MEM0_API_KEY` (not `VITE_*`) |
| Automations | write + delete (prune) | Cursor Automation secrets |

Create keys in mem0 dashboard → Settings → API Keys. Prefer separate read-only key for Consul if mem0 adds scoped keys later.

## Postgres backup

Memories live in Docker volume `server_postgres_data` (see `docker-compose.yaml`).

```powershell
docker compose -f C:\mem0\server\docker-compose.yaml exec postgres pg_dump -U mem0 mem0 > mem0-backup.sql
```

## Prune

Evening automation + manual:

```powershell
$env:MEM0_API_URL="https://joey-pc.tail0b414.ts.net:8888"
$env:MEM0_API_KEY="m0sk_..."
$env:MEM0_USER_ID="marengo-joey"
node c:\code\marengo\tools\mem0-mcp\dist\prune.js --dry-run
```

Target ~200–400 memories (`MEM0_PRUNE_TARGET_MAX`).

## Cross-repo

- mem0 infra / CORS / Serve → `jaylamping/mem0`
- MCP, Consul UI, SDD rules → `marengo`
