# Research Robotics Ingest (weekly automation)

## Workspace rules (mandatory)

Honor `.cursor/skills/_shared/workspace-rules.md` before task-specific work (always-apply `.cursor/rules/`, including Explore/Library model routing).

Populate `docs/research/robotics/` with curated humanoid/control/Pi bring-up notes.

## Schedule

Weekly (Sunday evening). Cursor Automation: `research-robotics-ingest`.

## Steps

1. Use `marengo-research` MCP — queries from `tools/marengo-research-mcp` humanoid seeds + Marengo ADRs.
2. Distill 3–7 durable facts (no secrets, no ephemeral chat fluff).
3. Write each note as `docs/research/robotics/{slug}.md` (create the directory if needed).
4. A lightweight model (`gpt-5.4-nano-medium`) is fine for summarization.

## Do not

- Store API keys or bench credentials
- Duplicate existing note slugs without merging content first
