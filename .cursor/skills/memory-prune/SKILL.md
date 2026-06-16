# Memory Prune (Marengo evening automation)

Keeps mem0 lean for search latency. Read-only from Consul; destructive only via this controlled workflow.

## Schedule

Daily evening (after research ingest). Cursor Automation: `memory-prune-evening`.

## Protect (never delete)

- Active SDD artifacts (`sdd/{slug}/…` updated within 14 days)
- All `feasibility/{change}/brief` entries
- All `expert/{domain}/…` heuristics
- Latest consolidated `research/{domain}/…` per topic cluster

## Prune targets

- Stale duplicate research superseded by newer consolidated memory
- Verbose session dumps already distilled into SDD artifacts
- Old `maintenance/prune/` audits beyond 30 days

## Target size

~200–400 memories (`MEM0_PRUNE_TARGET_MAX`, default 400).

## Execution

1. Run CLI: `node tools/mem0-mcp/dist/prune.js --dry-run` then without flag.
2. Use cheap OpenRouter model to merge duplicate clusters before delete (optional agent step).
3. Save audit via `mem_save` → `maintenance/prune/{YYYY-MM-DD}` with before/after counts and deleted topic_keys sample.

## Env

- `MEM0_API_URL`, `MEM0_API_KEY`, `MEM0_USER_ID=marengo-joey`
- Write-capable API key required for deletes
