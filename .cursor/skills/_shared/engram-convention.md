# Engram / memory backend — removed

The mem0 MCP and Engram-style `mem_*` persistence path were removed from Marengo.

**Use OpenSpec files instead** — see `openspec-convention.md` and `persistence-contract.md`.

If a skill or agent still mentions `mem_save`, `mem_search`, `mem_get_observation`, `mem_get_by_topic_key`, `mem_update`, or mode `engram` / `hybrid`:

1. Treat the mode as `openspec`
2. Read/write artifacts under `openspec/changes/{change-name}/`
3. For session handoff, write `.atl/session-handoff.md` (see `sdd-phase-common.md` § F)
