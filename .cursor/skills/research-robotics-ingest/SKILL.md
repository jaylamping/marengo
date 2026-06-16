# Research Robotics Ingest (weekly automation)

Populate `research/robotics/` in mem0 with curated humanoid/control/Pi bring-up notes.

## Schedule

Weekly (Sunday evening). Cursor Automation: `research-robotics-ingest`.

## Steps

1. Use `marengo-research` MCP — queries from `tools/marengo-research-mcp` humanoid seeds + Marengo ADRs.
2. Distill 3–7 durable facts (no secrets, no ephemeral chat fluff).
3. `mem_save` each with `topic_key: research/robotics/{slug}` and `type: discovery`.
4. Cheap OpenRouter model is fine for summarization.

## Do not

- Store API keys or bench credentials
- Duplicate existing `research/robotics/` topic_keys without merging content first
