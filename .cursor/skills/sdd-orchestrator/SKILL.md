---
name: sdd-orchestrator
description: "Orchestrator-only: delegate SDD phases, parallel PRs, context handoff. Not for executors."
model: claude-4.6-sonnet-medium-thinking
disable-model-invocation: true
user-invocable: false
---

## Orchestrator gate

If you loaded this as an **executor** phase agent, ignore this file. If you are the **sdd-orchestrator** agent, follow `.cursor/agents/sdd-orchestrator.md` and `.cursor/rules/gentle-ai-sdd.mdc`.

## Hard rules

- Coordinate only — never multi-file apply in orchestrator thread.
- One branch + one PR per independent backlog item; launch parallel `sdd-apply` when safe.
- Inject `## Skills to load before work` on every delegation.
- **At or after 50%** context (not before): write `.atl/session-handoff.md`, then fresh agent (see `_shared/sdd-phase-common.md` § F).
- `partial` + `next_recommended: session-handoff-resume`: follow `sdd-orchestrator.md` **Handoff Resume Delegation** — invoke fresh phase executor with handoff bootstrap (Read OpenSpec artifact paths (see openspec-convention.md)
- Automatic gatekeeper: same handoff + `resume_pending: true` + fresh `created_at` → PASS routing; then delegate — not DAG drift, not re-run saturated phase in-thread.
- Bootstrap: never auto-resume on OpenSpec hit alone — guards in `sdd-phase-common.md` § F *Resume eligibility*; clear after archive or supersession.

## Output to user

After parallel batch: list PR URLs, branch names, and what remains blocked.
