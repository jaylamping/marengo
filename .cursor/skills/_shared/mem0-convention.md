> **DISABLED (2026-07-14):** mem0 agent memory is off. Use `.omo/session-handoff.md` + OpenSpec. Kept for re-enable reference.

# mem0 Artifact Convention (Marengo)

Marengo uses **self-hosted mem0** via `mem0-mcp` (Engram-compatible tool names).

## Naming Rules

ALL persisted artifacts MUST use:

```
topic_key: {namespace}/{...}
metadata.topic_key: same as topic_key
metadata.project: marengo
user_id: marengo-joey (via MCP env)
```

## Namespaces

| Prefix | Purpose |
|--------|---------|
| `sdd/init/{project}` | SDD project context (from sdd-init) |
| `sdd/{change}/{phase}` | SDD phase artifacts |
| `feasibility/{change}/brief` | Feasibility gate output |
| `feasibility/{change}/expert/{domain}` | Expert review for a change |
| `decision/{area}/{slug}` | Durable cross-cutting decisions (not full ADRs) |
| `hardware/{subsystem}/{slug}` | Mechanical/electrical build facts and constraints |
| `cad/{assembly}/{slug}` | SolidWorks assembly, mate, and frame knowledge |
| `pi/{subsystem}/{slug}` | Pi deployment, systemd, CAN, and bench operations |
| `control/{subsystem}/{slug}` | Motor path, safety, gravity, kinematics |
| `software/{crate}/{slug}` | Rust, Consul, proto, CI patterns |
| `research/{domain}/{slug}` | Cited research distillations |
| `expert/{domain}/{slug}` | Stable curated heuristics |
| `maintenance/skill-registry` | Skill registry index |
| `maintenance/session-handoff/{project}` | Context saturation handoff |
| `maintenance/prune/{date}` | Prune audit summaries |

### SDD artifact types

`explore`, `proposal`, `spec`, `design`, `tasks`, `apply-progress`, `verify-report`, `archive-report`, `state`

## Classification (conflict rules)

| If the fact is about… | Use | Not |
|----------------------|-----|-----|
| SDD phase deliverable | `sdd/{change}/{phase}` | `decision/` |
| Go/no-go for hardware change | `feasibility/{change}/brief` | `expert/` |
| Stable rule reused across tasks | `expert/{domain}/{slug}` | `pi/` or `control/` |
| Pi systemd, deploy, CAN iface | `pi/{subsystem}/{slug}` | `control/` |
| Motor limits, Davout, gravity | `control/{subsystem}/{slug}` | `pi/` |
| Part loads, wiring, E-stop | `hardware/{subsystem}/{slug}` | `cad/` |
| SolidWorks mates, frames | `cad/{assembly}/{slug}` | `hardware/` |
| Rust crate or Consul pattern | `software/{crate}/{slug}` | `decision/` |

Promote to `expert/` only after a lesson survives two+ sessions and is not tied to one commit/deploy.

## Recovery

**Preferred (exact):**

```
mem_get_by_topic_key(topic_key: "sdd/{change}/{artifact}", project: "marengo")
```

**Exploratory (semantic):**

```
mem_search(query: "sdd/{change}/{artifact}", project: "marengo") → id
mem_get_observation(id) → full content (required)
```

## Writes

```
mem_save(
  title: "sdd/{change}/{artifact}",
  topic_key: "sdd/{change}/{artifact}",
  type: "architecture",
  project: "marengo",
  capture_prompt: false,
  content: "{full markdown}"
)
```

Same `topic_key` **upserts** (updates existing observation). Use `mem_save`, not `mem_update`, for SDD artifacts.

`mem_update(id, content)` only replaces text. The MCP server re-sends existing metadata so `topic_key` is preserved, but mem0 OSS still drops custom metadata if you call the REST API without it. After any update, verify with `mem_get_by_topic_key`.

`mem_get_by_topic_key` lists up to 10k memories, then falls back to semantic search with an exact `metadata.topic_key` filter. If lookup fails immediately after create, retry once or use `mem_search` → `mem_get_observation`.

Operational memories (`pi/`, `cad/`, live bench) should include in content:

```
- observed_at: {ISO-8601 UTC}
- source_ref: {tool, file, or command}
- git_sha: {when applicable}
- valid_until: {optional}
```

`mem_save` rejects invalid `topic_key` patterns, secrets, oversized content, and raw log dumps.

## History

mem0 stores revision history; Consul `/memory` detail sheet shows `/history` timeline.
