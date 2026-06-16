# Feasibility Brief (Marengo)

Run after `sdd-explore`, before `sdd-propose`, when the change touches hardware, CAD, CAN, motors, or Pi bench bring-up.

## Skip when

Pure Rust/software changes with no hardware implications — orchestrator may skip this skill.

## Steps

1. Identify `{change}` slug and affected domains (mech, cad, ee, robotics).
2. `mem_search` on `research/{domain}/` and `expert/{domain}/` for background.
3. Use `marengo-research` MCP to fill gaps.
4. Delegate readonly experts in parallel: `expert-mech`, `expert-cad`, `expert-ee`, `expert-robotics` (only relevant domains).
5. Write `feasibility/{change}/brief` via `mem_save` with sections:
   - Assumptions
   - Risks
   - Unknowns
   - ExpertVerdicts (per domain)
   - **Go | Revise | No-Go**
6. **No-Go** blocks `sdd-propose` unless user explicitly accepts risk (log acceptance in mem0).

## mem0 topic_key

`feasibility/{change}/brief`
