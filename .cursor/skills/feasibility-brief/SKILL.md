# Feasibility Brief (Marengo)

Run after `sdd-explore`, before `sdd-propose`, when the change touches hardware, CAD, CAN, motors, or Pi bench bring-up.

## Skip when

Pure Rust/software changes with no hardware implications — orchestrator may skip this skill.

## Steps

1. Identify `{change}` slug and affected domains (mech, cad, ee, robotics, kinematics).
2. Read `docs/research/` and prior expert notes when present.
3. Use `marengo-research` MCP to fill gaps.
4. Delegate readonly experts in parallel: `expert-mech`, `expert-cad`, `expert-ee`, `expert-robotics`, `expert-kinematics` (only relevant domains). Include `expert-kinematics` when URDF, joint config, `armee-kinematics`, or arm/torso layout is involved.
5. Write `openspec/changes/{change}/feasibility-brief.md` with sections:
   - Assumptions
   - Risks
   - Unknowns
   - ExpertVerdicts (per domain)
   - **Go | Revise | No-Go**
6. **No-Go** blocks `sdd-propose` unless user explicitly accepts risk (record acceptance in the proposal / PR).
