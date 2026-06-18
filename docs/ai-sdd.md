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
| MCP | `mem0-mcp` → `mem_get_by_topic_key`, `mem_search`, `mem_save`, `mem_get_observation`, `mem_update` |

### topic_key schema

```
sdd/init/{project}
sdd/{change}/{phase}
feasibility/{change}/brief
feasibility/{change}/expert/{domain}
decision/{area}/{slug}
hardware/{subsystem}/{slug}
cad/{assembly}/{slug}
pi/{subsystem}/{slug}
control/{subsystem}/{slug}
software/{crate}/{slug}
research/{domain}/{slug}
expert/{domain}/{slug}
maintenance/skill-registry
maintenance/session-handoff/{project}
maintenance/prune/{date}
```

Repo-wide memory rules: [`.cursor/rules/marengo-memory.mdc`](../.cursor/rules/marengo-memory.mdc). Full convention: [`.cursor/skills/_shared/mem0-convention.md`](../.cursor/skills/_shared/mem0-convention.md).

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

## OpenRouter + Cursor model assignments

Enable OpenRouter in Cursor Settings → Models. Agent `model:` fields in [`.cursor/agents/`](../.cursor/agents/) mirror the orchestrator table in [`.cursor/rules/gentle-ai-sdd.mdc`](../.cursor/rules/gentle-ai-sdd.mdc).

### Launch orchestrator

- **Agent:** `@sdd-orchestrator` (reads `gentle-ai-sdd.mdc` + persona rule)
- **Meta-commands:** `/sdd-new`, `/sdd-continue`, `/sdd-ff`, `/sdd-status`

### SDD phases

| Agent | Model |
|-------|-------|
| sdd-orchestrator | `openrouter/z-ai/glm-5.2:nitro` |
| sdd-init | `openrouter/z-ai/glm-5.2:nitro` |
| sdd-explore | `openrouter/owl-alpha` |
| sdd-propose | `openrouter/z-ai/glm-5.2:nitro` |
| sdd-spec | `composer-2.5-fast` |
| sdd-design | `openrouter/z-ai/glm-5.2:nitro` |
| sdd-tasks | `openrouter/minimax/minimax-m3` |
| sdd-apply | `composer-2.5-fast` |
| sdd-verify | `openrouter/z-ai/glm-5.2:nitro` |
| sdd-archive | `openrouter/nex-agi/nex-n2-pro:free` |
| sdd-onboard | `composer-2.5-fast` |

### Experts (Owl Alpha; GLM escalation on rails)

All `expert-*` agents use `openrouter/owl-alpha` while the OpenRouter promo runs (~30 days). Orchestrator re-runs with GLM 5.2 nitro when an expert contradicts authoritative sources, invents specs, or issues **No-Go** without evidence.

| Agent | mem0 path |
|-------|-----------|
| expert-cad | `feasibility/{change}/expert/cad` |
| expert-mech | `feasibility/{change}/expert/mech` |
| expert-ee | `feasibility/{change}/expert/ee` |
| expert-robotics | `feasibility/{change}/expert/robotics` |
| expert-kinematics | `feasibility/{change}/expert/kinematics` |

Long-lived heuristics: `expert/{domain}/{slug}`.

### Review agents

| Agent | Model |
|-------|-------|
| review-readability | `openrouter/nex-agi/nex-n2-pro:free` |
| review-reliability | `openrouter/minimax/minimax-m3` |
| review-resilience | `openrouter/minimax/minimax-m3` |
| review-risk | `openrouter/z-ai/glm-5.2:nitro` |

Slug strings must match Cursor's model picker after OpenRouter is connected. Adjust agent frontmatter if a slug differs.

## Gentle-AI install note

Windows workspace `gentle-ai install --scope=workspace` hit a rollback bug (v1.40.2). Assets were copied from upstream v1.40.2 manually; re-run dry-run before upgrading installer.

## Related docs

- [mem0 ops runbook](mem0-ops.md)
- [mem0 dashboard custom instructions](mem0-custom-instructions.md)
