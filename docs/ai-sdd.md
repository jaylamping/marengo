# Marengo AI + SDD

Gentle-AI SDD (v1.40.2 assets, workspace install) with **OpenSpec files** as the artifact store.

## When to invoke SDD

Use SDD (`/sdd-new`, `/sdd-continue`, or "start SDD for …") when:

- Touching 2+ non-trivial files or a new feature slice
- Design needs ADR-worthy decisions
- Hardware + software co-design

Skip SDD for one-line fixes, typos, and single-file mechanical edits.

## Artifact store

Default mode: **openspec** — artifacts under `openspec/changes/{slug}/`.

| Mode | Behavior |
|------|----------|
| `openspec` | Files in repo (default) |
| `none` | Ephemeral — return inline only |

There is no memory MCP. Session handoff uses `.atl/session-handoff.md`. Skill registry uses `.atl/skill-registry.md`.

Contracts: [`.cursor/skills/_shared/persistence-contract.md`](../.cursor/skills/_shared/persistence-contract.md), [`.cursor/skills/_shared/openspec-convention.md`](../.cursor/skills/_shared/openspec-convention.md).

## Feasibility gate

After `sdd-explore`, before `sdd-propose`, for hardware/CAN/CAD/bench work:

1. Run `feasibility-brief` skill
2. Require a brief with **Go** or **Revise** (write under `openspec/changes/{change}/` or note in the proposal)
3. **No-Go** blocks unless user accepts risk (record acceptance in the proposal / PR)

Pure Rust-only changes skip the gate.

## Marengo gates (always)

- `just check` before finishing apply/verify
- Proto-first API changes
- No `unwrap()` in `crates/*`
- Pi bench: `marengo-pi` MCP (not manual SSH)
- CAD: worktree-safety — no restore/overwrite `.SLDASM` without backup + OK

## Cursor model assignments

Agent `model:` fields in [`.cursor/agents/`](../.cursor/agents/) mirror the orchestrator table in [`.cursor/rules/gentle-ai-sdd.mdc`](../.cursor/rules/gentle-ai-sdd.mdc). Slugs must match Cursor's model picker.

**Credit budget:** Composer 2.5 Fast uses a separate Cursor credit bucket. Prefer Composer when quality is a toss-up; reserve API credits for orchestrator, propose, design, verify, review-risk, review-reliability, and expert-cad.

### Launch orchestrator

- **Agent:** `@sdd-orchestrator` (reads `gentle-ai-sdd.mdc` + persona rule)
- **Meta-commands:** `/sdd-new`, `/sdd-continue`, `/sdd-ff`, `/sdd-status`

### SDD phases

| Agent | Model |
|-------|-------|
| sdd-orchestrator | `claude-4.6-sonnet-medium-thinking` |
| sdd-init | `composer-2.5-fast` |
| sdd-explore | `composer-2.5-fast` |
| sdd-propose | `claude-4.6-sonnet-medium-thinking` |
| sdd-spec | `composer-2.5-fast` |
| sdd-design | `claude-opus-4-8-thinking-high` |
| sdd-tasks | `composer-2.5-fast` |
| sdd-apply | `composer-2.5-fast` |
| sdd-verify | `claude-opus-4-8-thinking-high` |
| sdd-archive | `composer-2.5-fast` |
| sdd-onboard | `composer-2.5-fast` |

### Experts

All `expert-*` agents (except `expert-cad`) use `composer-2.5-fast` for read-only domain review. Orchestrator re-runs with `claude-opus-4-8-thinking-high` when an expert contradicts authoritative sources, invents specs, or issues **No-Go** without evidence.

| Agent | Model |
|-------|-------|
| expert-cad | `gpt-5.3-codex-high` |
| expert-mech | `composer-2.5-fast` |
| expert-ee | `composer-2.5-fast` |
| expert-robotics | `composer-2.5-fast` |
| expert-kinematics | `composer-2.5-fast` |

### Review agents

| Agent | Model |
|-------|-------|
| review-readability | `composer-2.5-fast` |
| review-reliability | `gpt-5.5-high` |
| review-resilience | `composer-2.5-fast` |
| review-risk | `claude-opus-4-8-thinking-high` |

## Gentle-AI install note

Windows workspace `gentle-ai install --scope=workspace` hit a rollback bug (v1.40.2). Assets were copied from upstream v1.40.2 manually; re-run dry-run before upgrading installer.

## Related docs

- [OpenSpec README](../openspec/README.md)
- [Persistence contract](../.cursor/skills/_shared/persistence-contract.md)
