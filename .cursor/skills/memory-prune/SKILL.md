# Memory Prune (Marengo evening automation)

Keeps mem0 lean for search latency. Read-only from Consul; destructive only via this controlled workflow.

## Schedule

Daily evening (after research ingest). Cursor Automation: `memory-prune-evening`.

## Protect (never delete)

- `maintenance/skill-registry`
- `maintenance/session-handoff/*`
- All `expert/{domain}/…` heuristics
- All `feasibility/{change}/brief` entries
- All `research/{domain}/…` distillations
- Active SDD artifacts (`sdd/{slug}/…` updated within 14 days)
- Recent subsystem memories (`decision/`, `hardware/`, `cad/`, `pi/`, `control/`, `software/`) updated within 30 days

## Prune targets

- Stale duplicate research superseded by newer consolidated memory
- Verbose session dumps already distilled into SDD artifacts
- Subsystem memories older than 30 days that are superseded
- Old `maintenance/prune/` audits beyond 30 days

## Target size

~200–400 memories (`MEM0_PRUNE_TARGET_MAX`, default 400).

## Execution

1. Run CLI: `node tools/mem0-mcp/dist/prune.js --dry-run` then without flag.
2. Use a lightweight model (`gpt-5.4-nano-medium`) to merge duplicate clusters before delete (optional agent step).
3. Save audit via `mem_save` → `maintenance/prune/{YYYY-MM-DD}` with before/after counts and deleted topic_keys sample.

Protection logic lives in `tools/mem0-mcp/src/prune-policy.ts` — update code and tests when retention rules change.

## Env

- `MEM0_API_URL`, `MEM0_API_KEY`, `MEM0_USER_ID=marengo-joey`
- Write-capable API key required for deletes
