# mem0 MCP — DISABLED

**Status: disabled (2026-07-14).** Prefer local handoffs under `.omo/`.

## Do not use

- Do not start the `mem0` MCP server
- Do not call `mem_search`, `mem_save`, `mem_get_*`, or `mem_update`
- `tools/mem0-mcp/` remains in-tree for possible future re-enable; leave it idle

## Replacement

| Was (mem0) | Use instead |
|------------|-------------|
| `maintenance/session-handoff/marengo` | `.omo/session-handoff.md` |
| SDD `sdd/{change}/{phase}` | `openspec/changes/{change}/…` |
| Skill registry in mem0 | `.atl/skill-registry.md` (local) |
| Agent bootstrap | [`.cursor/rules/marengo-memory.mdc`](../../rules/marengo-memory.mdc) |

Consul `/memory` (mem0 observatory) is unrelated UI code — out of scope unless re-enabling the stack.
