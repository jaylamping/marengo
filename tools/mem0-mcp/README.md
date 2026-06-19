# mem0-mcp

MCP server for Marengo self-hosted mem0 (Engram-compatible).

## Tools

| Tool | mem0 API |
|------|----------|
| `mem_search` | `POST /search` (+ client-side `project` filter) |
| `mem_get_by_topic_key` | `GET /memories?top_k=10000` + exact metadata match; search fallback |
| `mem_save` | `POST /memories` or `PUT /memories/{id}` with metadata (upsert by `topic_key`) |
| `mem_get_observation` | `GET /memories/{id}` + `/history` |
| `mem_update` | `PUT /memories/{id}` — preserves existing metadata; prefer `mem_save` upsert |

## Env

```bash
MEM0_API_URL=https://joey-pc.tail0b414.ts.net:8888
MEM0_API_KEY=m0sk_...
MEM0_USER_ID=marengo-joey
```

Set `MEM0_API_KEY` in your environment, not in git. Registered in `.cursor/mcp.json`.

## Build

```bash
cd tools/mem0-mcp
npm install
npm run build
npm test
```

After build, reconnect the mem0 MCP server in Cursor so tool descriptors refresh.

## Prune CLI

```bash
node dist/prune.js --dry-run
MEM0_PRUNE_TARGET_MAX=400 node dist/prune.js
```

## Repair corrupted topic_key rows

After text-only PUT updates that dropped metadata:

```bash
node scripts/repair-topic-keys.mjs --dry-run
node scripts/repair-topic-keys.mjs
```

Protection rules: `src/prune-policy.ts`.

## topic_key validation

Must match allowed namespaces in `src/schema.ts` (`sdd`, `feasibility`, `research`, `expert`, `maintenance`, `decision`, `hardware`, `cad`, `pi`, `control`, `software`).
