# mem0 MCP (Marengo)

Persistent memory via self-hosted mem0. **Canonical URL:** `tools/mem0-mcp/src/defaults.ts` (`https://joey-pc.tail0b414.ts.net:8888`). Do not discover or hardcode elsewhere — import or rely on MCP defaults.

## When to use

Use mem0 for **all Marengo repo work**, not only SDD:

- SDD phase artifacts (`sdd/{change}/{phase}`)
- Feasibility briefs and expert reviews
- Engineering decisions, hardware/CAD/Pi/control/software lessons
- Research ingest and prune audits
- Session handoffs and skill registry

See [`.cursor/rules/marengo-memory.mdc`](../../rules/marengo-memory.mdc) and [`.cursor/skills/_shared/mem0-convention.md`](../_shared/mem0-convention.md).

## Tools

| Tool | Purpose |
|------|---------|
| `mem_get_by_topic_key` | Exact lookup by `topic_key` (preferred for SDD/bootstrap) |
| `mem_search` | Semantic search; optional `project` filter |
| `mem_get_observation` | Full content + history by observation ID |
| `mem_save` | Upsert by `topic_key` (always prefer for SDD artifacts). Pass `content` as a plain markdown string, not a content-block array. |
| `mem_update` | Replace content by observation ID; MCP preserves metadata, but prefer `mem_save` |

After changing tools: `cd tools/mem0-mcp && npm run build`, then reconnect the mem0 MCP server in Cursor.

## Setup

1. Set user env `MEM0_API_KEY` (m0sk_… from mem0 dashboard). URL and user ID default from `tools/mem0-mcp/src/defaults.ts`.
2. Build: `cd tools/mem0-mcp && npm run build`
3. MCP registered as `mem0` in `.cursor/mcp.json`

## Never

Store secrets in `mem_save` — server rejects key-like patterns. Distill logs before saving.
