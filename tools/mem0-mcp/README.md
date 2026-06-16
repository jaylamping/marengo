# mem0-mcp

Engram-compatible MCP server for Marengo self-hosted mem0.

## Tools

| Tool | mem0 API |
|------|----------|
| `mem_search` | `POST /search` |
| `mem_save` | `POST /memories` (`infer: false`) |
| `mem_get_observation` | `GET /memories/{id}` + `/history` |

## Env

```bash
MEM0_API_URL=https://joey-pc.tail0b414.ts.net:8888
MEM0_API_KEY=m0sk_...
MEM0_USER_ID=marengo-joey
```

Set `MEM0_API_KEY` in user environment (not committed). Registered in `.cursor/mcp.json`.

## Build

```bash
cd tools/mem0-mcp
npm install
npm run build
npm test
```

## Prune CLI

```bash
node dist/prune.js --dry-run
MEM0_PRUNE_TARGET_MAX=400 node dist/prune.js
```

## topic_key validation

Must match `^(sdd|feasibility|research|expert|maintenance)/…` — see `src/schema.ts`.
