# Workspace Rules Contract

Every Marengo project skill executor and every delegated sub-agent MUST honor
the always-apply Cursor rules under `.cursor/rules/` before task-specific work.
This file is the index; each `.mdc` remains the source of truth — read the
relevant rule file when the task touches its domain.

## Always honor

| Rule | When |
|------|------|
| `gentle-ai-persona.mdc` | Chat replies (not artifact voice) |
| `marengo.mdc` + `AGENTS.md` | Any Marengo software / proto / control work |
| `no-composer-2.5-fast.mdc` | Model selection (default: do **not** use `-fast`) |
| `explore-library-models.mdc` | Explore / Exploration / Library tasks |
| `worktree-safety.mdc` | Before edits, CAD, or destructive git |
| `windows-shell.mdc` | Shell commands (host-aware) |
| `pi-mcp-first.mdc` | Pi / CAN / motors / deploy / bench |
| `solidworks-mcp.mdc` | SolidWorks / CAD MCP |
| `gentle-ai-sdd.mdc` | SDD orchestration |

## Model routing (summary)

- **Default:** do not select `composer-2.5-fast` unless a skill frontmatter,
  rule, human, or agent definition explicitly pins it
  (`no-composer-2.5-fast.mdc`).
- **Explore / Exploration / Library** (including `subagent_type="explore"`):
  - First Party pool → `composer-2.5-fast`
  - API pool → `gpt-5.4-nano-medium`
  (`explore-library-models.mdc` — allowed exception to the no-fast default)
- **Skill / agent `model:` frontmatter** wins for that skill's executor when set
  (also an allowed exception). Do not override a pinned phase model with
  Explore/Library defaults.

## How skills must apply this

1. Before task work, treat this contract as loaded (via Section A, skill-resolver
   injection, or a pointer in the skill itself).
2. Read the specific `.cursor/rules/*.mdc` files that match the task domain —
   do not invent shortcuts that violate them.
3. When delegating, inject this file's path so sub-agents start with the same
   contract (see `skill-resolver.md`).
