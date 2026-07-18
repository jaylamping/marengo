# SDD Phase — Common Protocol

Boilerplate identical across all SDD phase skills. Sub-agents MUST load this alongside their phase-specific SKILL.md.

Executor boundary: every SDD phase agent is an EXECUTOR, not an orchestrator. Do the phase work yourself. Do NOT launch sub-agents, do NOT call `delegate`/`task`, and do NOT bounce work back unless the phase skill explicitly says to stop and report a blocker.

## A. Skill Loading

1. Check if the orchestrator injected a `## Skills to load before work` block in your launch prompt. If yes, read those exact `SKILL.md` files before task-specific work.
2. If no skills block was provided, check for `SKILL: Load` instructions. If present, load those exact skill files.
3. If neither was provided, read `.atl/skill-registry.md` from the project root if it exists. From the registry's skills index, match triggers to your task and read the exact listed `SKILL.md` paths.
4. If no registry exists, proceed with your phase skill only.

NOTE: the preferred path is (1) — exact skill paths selected by the orchestrator. Paths (2) and (3) are fallbacks. Searching the registry is SKILL LOADING, not delegation. If `## Skills to load before work` is present, IGNORE redundant `SKILL: Load` instructions.

## B. Artifact Retrieval (OpenSpec)

Read phase dependencies from the filesystem per `openspec-convention.md`:

```
openspec/changes/{change-name}/proposal.md
openspec/changes/{change-name}/specs/**/spec.md
openspec/changes/{change-name}/design.md
openspec/changes/{change-name}/tasks.md
openspec/changes/{change-name}/verify-report.md
openspec/changes/{change-name}/state.yaml
```

Legacy prompts that say `mem_search` / `mem_get_*` for SDD artifacts: ignore those calls and read the OpenSpec paths above instead.

## C. Artifact Persistence

Every phase that produces an artifact MUST persist it. Skipping this BREAKS the pipeline — downstream phases will not find your output.

### OpenSpec mode (default)

Write the artifact file during the phase's main step per `openspec-convention.md`. No additional action needed.

### None mode

Return result inline only. Do not write any files.

Legacy `engram` / `hybrid` / `mem_save`: treat as OpenSpec — write files only. Do not call removed `mem_*` tools.

## D. Return Envelope

> **CRITICAL — Response ordering**: Your FINAL output MUST be text (the return envelope), NOT a tool call. If you need to write files, do that BEFORE your final text response. **Why**: When a sub-agent's last action is a tool call, the parent agent may receive only the tool result — your text response (the actual analysis) is lost.

Every phase MUST return a structured envelope to the orchestrator:

- `status`: `success`, `partial`, or `blocked`
- `executive_summary`: 1-3 sentence summary of what was done
- `detailed_report`: (optional) full phase output, or omit if already inline
- `artifacts`: list of artifact paths written
- `next_recommended`: the next SDD phase to run, `none`, or `session-handoff-resume` (context saturation only — with `status: partial` after writing handoff)
- `risks`: risks discovered, or "None"
- `skill_resolution`: how skills were loaded — `paths-injected` (received exact skill paths from orchestrator), `fallback-registry` (self-loaded paths from registry), `fallback-path` (loaded via SKILL: Load path), or `none` (no skills loaded)

### Field-name translation (orchestrator)

Phase envelopes use **snake_case** (`next_recommended`, `executive_summary`, `skill_resolution`). Structured status from `sdd-status-contract.md` uses **camelCase** (`nextRecommended`, `blockedReasons`, `applyState`). The orchestrator MUST translate between them — never treat them as different routing semantics.

| Phase envelope | Status contract |
|----------------|-----------------|
| `next_recommended` | `nextRecommended` |
| `executive_summary` | (human summary in status output) |
| `status: blocked` | non-empty `blockedReasons` |
| `status: partial` + `session-handoff-resume` | handoff routing — not a DAG `nextRecommended` advance |

Valid `next_recommended` / `nextRecommended` tokens: `sdd-init`, `sdd-explore`, `sdd-propose`, `sdd-spec`, `sdd-design`, `sdd-tasks`, `sdd-apply`, `sdd-verify`, `sdd-archive`, `none`, `session-handoff-resume`, `select-change`, `resolve-blockers`, `sdd-new`.

Example:

```markdown
**Status**: success
**Summary**: Proposal created for `{change-name}`. Defined scope, approach, and rollback plan.
**Artifacts**: `openspec/changes/{change-name}/proposal.md`
**Next**: sdd-spec or sdd-design
**Risks**: None
**Skill Resolution**: paths-injected — 3 skills
(other values: `fallback-registry`, `fallback-path`, or `none — no registry found`)
```

## E. Review Workload Guard

SDD must protect reviewer cognitive load, not only generate tasks.

- The default PR review budget is **400 changed lines** (`additions + deletions`).
- The orchestrator MUST cache a delivery strategy at session start: `ask-on-risk` (default), `auto-chain`, `single-pr`, or `exception-ok`.
- The orchestrator MUST pass `delivery_strategy` to `sdd-tasks` and the resolved decision to `sdd-apply`.
- `sdd-tasks` MUST forecast whether the planned work may exceed that budget.
- The forecast MUST include exact plain-text guard lines: `Decision needed before apply: Yes|No`, `Chained PRs recommended: Yes|No`, and `400-line budget risk: Low|Medium|High`.
- If the forecast is high, `sdd-tasks` MUST recommend chained or stacked PRs using deliverable work units.
- `sdd-apply` MUST NOT start oversized work unless the delivery strategy resolves to chained/stacked PR slices or explicitly accepted `size:exception`.
- Each chained PR slice must have a clear start, clear finish, autonomous scope, verification, and reasonable rollback.
- In a Feature Branch Chain, PR #1 targets the feature/tracker branch and later child PRs target the immediate previous PR branch; if GitHub shows previous slices in a child diff, retarget/rebase until the diff is clean.

This guard exists to reduce reviewer burnout and keep implementation delivery safe. Do not treat it as optional process noise.

## F. Context Saturation Handoff (MANDATORY — all SDD agents)

**Applies to:** orchestrator and every `sdd-*` phase executor. Context quality degrades at or above ~50% saturation — hand off before continuing heavy work in-thread.

### When to hand off

**Do NOT hand off before 50%.** Continue working while context is below 50% — no preemptive session splits.

When context is **at or after 50%** (UI context meter ≥ 50%, or a reliable estimate crosses 50%):

1. **Finish the atomic step in progress** — complete the current file edit, test run, or return envelope; do not stop mid-edit.
2. **Persist handoff to** `.atl/session-handoff.md` (concise; target ≤800 tokens). Ensure `.atl/` is gitignored. Required fields:

```markdown
## Session handoff
- resume_pending: true
- created_at: {ISO-8601 UTC}
- change: {slug}
- phase: {phase}
- branch: {branch}
- done: ...
- next: ...
- open_prs: ...
- blockers: ...
- questions: ...
- files_touched: ...
```

Also update `openspec/changes/{change}/state.yaml` if a named change is active.

3. **Stop taking new work** in this session.
4. **Orchestrator:** on executor `partial` + `session-handoff-resume`, delegate per `sdd-orchestrator.md` **Handoff Resume Delegation** — invoke a **fresh** phase executor (new context). First message MUST include: read `.atl/session-handoff.md` and the OpenSpec artifacts listed under the active change; continue from handoff `next`. Do not stop at a user-facing progress summary.
5. **Executor:** return `status: partial` with `next_recommended: session-handoff-resume` — do not continue the phase in-thread.

### Resume eligibility (orchestrator bootstrap)

A handoff file at `.atl/session-handoff.md` is **not** automatically actionable. Resume ONLY when **all** hold:

1. **`resume_pending: true`** in the file. Missing or `false` → do not resume.
2. **`created_at` within 72 hours** (ISO-8601 UTC). Missing or expired → do not resume; clear the record (see below).
3. **User intent does not supersede** — do not resume when the user's message is:
   - `/sdd-new`, `/sdd-ff`, `/sdd-status`, `/sdd-onboard`, or unrelated non-SDD work; or
   - `/sdd-new {other}` / `/sdd-ff {other}` where `{other}` ≠ handoff `change`.
4. **Resume trigger present** — auto-resume only on `/sdd-continue` (optional change name must match handoff `change` if provided), explicit "resume SDD handoff", or Automatic mode mid-chain handoff routing (`partial` + `session-handoff-resume`). Opening a fresh chat without an SDD continue command → **offer** resume in Interactive mode; do not silently override.

When guards fail but a handoff file exists, **clear** it by rewriting with `resume_pending: false` and a `cleared_reason`.

**Clear handoff** after: successful resume (`resume-consumed`), archive complete (`archive-complete`), superseded by new change (`superseded`), or TTL expiry (`stale`).

### Questioning rule

Before apply, **read** `openspec/changes/{change}/exploration.md` (if present), `openspec/config.yaml`, and `.atl/session-handoff.md` when present. If the task contradicts stored priorities (e.g. full-robot not built, enhancement vs gremlin), return **`status: blocked`** with evidence — do not silently proceed.

### Orchestrator-only

- Prefer **parallel** `sdd-apply` invocations on **independent branches** (one PR per item).
- Never implement multi-file fixes in the orchestrator thread — delegate.
- After each subagent completes: commit + push + `gh pr create` before starting dependent work.
