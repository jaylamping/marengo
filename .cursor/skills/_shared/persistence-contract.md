# Persistence Contract (shared across all SDD skills)

## Mode Resolution

The orchestrator passes `artifact_store.mode` with one of: `openspec | none`.

The orchestrator ASKs the user which mode they want when `/sdd-new`, `/sdd-ff`, or `/sdd-continue` is invoked for the first time in a session. The choice is cached for the session.

Default (if user doesn't specify): `openspec`.

Legacy aliases (removed backends): if a prompt still says `engram` or `hybrid`, treat as `openspec` and warn once that the memory backend was removed.

## Mode Roles

- **`openspec`**: Source of truth. Files in repo, git history, team-shareable, full audit trail.
- **`none`**: Ephemeral. Lost when conversation ends.

### Mode Comparison

| Capability | `openspec` | `none` |
|------------|------------|--------|
| Cross-session recovery | ✅ (via git / files) | ❌ |
| Compaction survival | ✅ (files on disk) | ❌ |
| Shareable with team | ✅ (committed files) | ❌ |
| Full iteration history | ✅ (git history) | ❌ |
| Audit trail (archive) | ✅ (full folder) | ❌ |
| Project files created | Yes | Never |

## Behavior Per Mode

| Mode | Read from | Write to | Project files |
|------|-----------|----------|---------------|
| `openspec` | Filesystem | Filesystem | Yes |
| `none` | Orchestrator prompt context | Nowhere | Never |

## State Persistence (Orchestrator)

The orchestrator persists DAG state after each phase transition to enable SDD recovery after compaction.

| Mode | Persist State | Recover State |
|------|--------------|---------------|
| `openspec` | Write `openspec/changes/{change-name}/state.yaml` | Read `openspec/changes/{change-name}/state.yaml` |
| `none` | Not possible — warn user | Not possible |

## Common Rules

- `none` → do NOT create or modify any project files; return results inline only
- `openspec` → write files ONLY to paths defined in `openspec-convention.md`
- NEVER force `openspec/` creation unless orchestrator explicitly passed `openspec`
- If unsure which mode to use, default to `openspec`

## Sub-Agent Context Rules

Sub-agents launch with a fresh context and NO access to the orchestrator's instructions.

Who reads, who writes:
- Non-SDD (general task): orchestrator summarizes context in the prompt; durable notes go in docs/ADRs/commits — not a memory MCP
- SDD (phase with dependencies): sub-agent reads artifacts from `openspec/changes/{change}/`; sub-agent saves its artifact
- SDD (phase without dependencies, e.g. explore): nobody reads; sub-agent saves its artifact

## Orchestrator Prompt Instructions for Sub-Agents

Non-SDD:
```
PERSISTENCE:
There is no memory MCP. Capture durable decisions in repo docs, ADRs, or the PR description.
Do not call mem_save / mem_search / mem_get_* — those tools are removed.
```

SDD (with dependencies):
```
Artifact store mode: openspec
Read these artifacts before starting (filesystem):
  openspec/changes/{change-name}/proposal.md
  openspec/changes/{change-name}/specs/**/spec.md
  openspec/changes/{change-name}/design.md
  openspec/changes/{change-name}/tasks.md
  (as required by the phase)

PERSISTENCE (MANDATORY — do NOT skip):
After completing your work, write the phase artifact under
  openspec/changes/{change-name}/
per openspec-convention.md. If you return without writing the file, the next phase CANNOT find your artifact and the pipeline BREAKS.
```

SDD (no dependencies):
```
Artifact store mode: openspec

PERSISTENCE (MANDATORY — do NOT skip):
After completing your work, write the phase artifact under
  openspec/changes/{change-name}/
per openspec-convention.md.
```

## Sub-Agent Response Ordering

When a sub-agent persists artifacts (file writes), the persistence MUST happen BEFORE the final text response. The sub-agent's absolute last output must be text, never a tool call.

**Why**: The Task tool returns the sub-agent's final output to the parent. If the sub-agent ends with a tool call, the parent may receive only the tool result — the text analysis is lost. Always: do your work → write files → respond with text envelope.

## Skill Registry

The orchestrator pre-resolves skill paths from the skill registry and injects them as `## Skills to load before work` in your launch prompt. Sub-agents read those exact `SKILL.md` files before task-specific work.

To generate/update: run the `skill-registry` skill, or run `sdd-init`. Registry lives at `.atl/skill-registry.md` only.

Sub-agent skill loading: check for a `## Skills to load before work` block in your prompt — if present, read those exact files. If not present, check for `SKILL: Load` instructions as a fallback. If neither exists, proceed without — this is not an error.

## Detail Level

The orchestrator may pass `detail_level`: `concise | standard | deep`. This controls output verbosity but does NOT affect what gets persisted — always persist the full artifact.
