# Marengo AI + SDD + mem0

Gentle-AI SDD (v1.40.2 assets, workspace install) with **self-hosted mem0** instead of Engram.

## When to invoke SDD

Use SDD (`/sdd-new`, `/sdd-continue`, or "start SDD for …") when:

- Touching 2+ non-trivial files or a new feature slice
- Design needs ADR-worthy decisions
- Hardware + software co-design

Skip SDD for one-line fixes, typos, and single-file mechanical edits.

## Pilot change (first e2e)

Suggested slug: `consul-memory-observatory` — documents what we built; safe software-only verify path.

Branch naming: `sdd/{slug}` or `feat/{slug}`.

## Memory backend (mem0)

| Setting | Value |
|---------|--------|
| API | `https://joey-pc.tail0b414.ts.net:8888` |
| Dashboard | `https://joey-pc.tail0b414.ts.net` |
| User ID | `marengo-joey` |
| MCP | `mem0-mcp` → `mem_search`, `mem_save`, `mem_get_observation` |

### topic_key schema

```
sdd/{change}/{phase}
feasibility/{change}/brief
feasibility/{change}/expert/{domain}
research/{domain}/{slug}
expert/{domain}/{slug}
maintenance/prune/{date}
```

Enforced in `tools/mem0-mcp` on `mem_save`. See `.cursor/skills/_shared/mem0-convention.md`.

### Artifact store modes

- **mem0** (default): artifact store mode `engram` in SDD skills maps to mem0
- **hybrid**: also mirror to `openspec/changes/{slug}/` when user requests
- **openspec**: files only
- **none**: ephemeral

## Feasibility gate

After `sdd-explore`, before `sdd-propose`, for hardware/CAN/CAD/bench work:

1. Run `feasibility-brief` skill
2. Require `feasibility/{change}/brief` with **Go** or **Revise**
3. **No-Go** blocks unless user accepts risk (log in mem0)

Pure Rust-only changes skip the gate.

## Marengo gates (always)

- `just check` before finishing apply/verify
- Proto-first API changes
- No `unwrap()` in `crates/*`
- Pi bench: `marengo-pi` MCP (not manual SSH)
- CAD: worktree-safety — no restore/overwrite `.SLDASM` without backup + OK

## Consul Memory Observatory

Dev setup (`consul/.env.local` from `.env.example`):

```bash
MEM0_API_URL=https://joey-pc.tail0b414.ts.net:8888
MEM0_API_KEY=m0sk_...          # server-side via Vite proxy only
VITE_MEM0_USER_ID=marengo-joey
VITE_MEM0_POLL_MS=30000
```

Run `cd consul && npm run dev` → open `/memory`.

Pi-hosted Consul (`marengo.local:8444`) shows offline banner until gateway proxy exists.

## OpenRouter model mapping (manual in Cursor UI)

| Role | Suggested model tier |
|------|---------------------|
| Orchestrator | Composer |
| sdd-explore, archive, automations | Cheaper OpenRouter (e.g. Gemini Flash, gpt-4.1-mini) |
| sdd-propose, design, verify, experts | Stronger (Claude Sonnet, gpt-4.1) |
| sdd-apply | Composer or trusted mid-tier |

Assign in Cursor agent settings per `.cursor/agents/sdd-*.md`.

## Gentle-AI install note

Windows workspace `gentle-ai install --scope=workspace` hit a rollback bug (v1.40.2). Assets were copied from upstream v1.40.2 manually; re-run dry-run before upgrading installer.

## Related docs

- [mem0 ops runbook](mem0-ops.md)
- [mem0 dashboard custom instructions](mem0-custom-instructions.md)
