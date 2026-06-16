# mem0 MCP (Marengo)

Persistent memory via self-hosted mem0 at `https://joey-pc.tail0b414.ts.net:8888`.

## When to use

- SDD phase artifacts (`mem_save` with `topic_key`)
- Feasibility briefs and expert reviews
- Research ingest and prune audits

## Setup

1. Set user env `MEM0_API_KEY` (m0sk_… from mem0 dashboard)
2. Build: `cd tools/mem0-mcp && npm run build`
3. MCP registered as `mem0` in `.cursor/mcp.json`

## Never

Store secrets in `mem_save` — server rejects key-like patterns.
