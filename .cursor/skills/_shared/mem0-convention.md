# mem0 Artifact Convention (Marengo)

Marengo uses **self-hosted mem0** via `mem0-mcp` (Engram-compatible tool names).

## Naming Rules

ALL persisted artifacts MUST use:

```
topic_key: {namespace}/{...}
metadata.topic_key: same as topic_key
user_id: marengo-joey (via MCP env)
```

### Namespaces

| Prefix | Purpose |
|--------|---------|
| `sdd/init/{project}` | SDD project context (from sdd-init) |
| `sdd/{change}/{phase}` | SDD phase artifacts |
| `maintenance/skill-registry` | Skill registry index |
| `feasibility/{change}/brief` | Feasibility gate output |
| `feasibility/{change}/expert/{domain}` | Expert review |
| `research/{domain}/…` | Scheduled research ingest |
| `expert/{domain}/…` | Curated heuristics |
| `maintenance/prune/{date}` | Prune audit summaries |

### SDD artifact types

`explore`, `proposal`, `spec`, `design`, `tasks`, `apply-progress`, `verify-report`, `archive-report`, `state`

## Recovery (2 steps)

```
mem_search(query: "sdd/{change}/{artifact}") → id
mem_get_observation(id) → full content (required)
```

## Writes

```
mem_save(
  title: "sdd/{change}/{artifact}",
  topic_key: "sdd/{change}/{artifact}",
  type: "architecture",
  project: "marengo",
  content: "{full markdown}"
)
```

`mem_save` rejects invalid `topic_key` patterns and content containing secrets.

## History

mem0 stores revision history; Consul `/memory` detail sheet shows `/history` timeline.
