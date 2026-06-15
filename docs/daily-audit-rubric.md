# Daily audit rubric

Single source of truth for Marengo **deterministic** checks and **qualitative** daily review.

## Marengo invariants (deterministic)

From [AGENTS.md](../AGENTS.md), [rust-patterns.md](rust-patterns.md), [safety.md](safety.md), [architecture.md](architecture.md):

| ID | Rule | Severity if violated |
|----|------|----------------------|
| R1 | No `unwrap()` / `expect()` in `crates/*` library code | critical |
| R2 | Motor commands must flow Berthier → Davout → robstride | critical |
| R3 | No hand-edits under `consul/src/gen/` (except `.checksum`) | critical |
| R4 | API wire changes start in `proto/` | warn |
| R5 | No `unsafe` without ADR | critical |
| R6 | Berthier must not touch CAN; Davout must not plan trajectories | critical |
| R7 | Safety-path edits must consider enable FSM / watchdog | warn |
| R8 | New joints must appear in kinematics SSOT + config templates | warn |
| R9 | SolidWorks MCP: no destructive CAD without confirm + backup | critical |
| R10 | Hardware/config coupling: `config/motors*.yaml` or kinematics changes need paired doc updates | warn |
| R11 | Motor-path diffs (`davout/`, `robstride/`) should include test changes | warn |
| R12 | Sim harness changes (`sim/`, `crates/sim-harness/`) should include test updates | info |

## Industry / humanoid checkpoints (qualitative)

Use `research_humanoid` or `audit-research` CLI. Each finding needs a **project rule** and/or **external URL**.

| Area | Compare against |
|------|-----------------|
| Actuator control | Vendor MIT docs ([Robstride ADR](../hardware/docs/decisions/0002-robstride-protocol.md)), impedance/gravity-comp literature |
| Locomotion / WBC | Recent CoRL/RSS papers, open humanoid repos (Unitree, OpenLoong, Berkeley Humanoid) |
| Software stack | ROS2 patterns (Discourse), Marengo architecture boundaries |
| Bench bring-up | [pi-commissioning.md](pi-commissioning.md), [safety.md](safety.md) |
| Standards (beyond bench) | ISO 13482, ISO 10218, IEC 62443 summaries via `search_standards` |
| Academic vs bench | Prefer vendor docs + Marengo safety over paper-only motor recommendations |

## Severity policy

| Level | Action |
|-------|--------|
| info | Log in report only |
| warn | Include in GitHub issue |
| critical | GitHub issue + PR comment if overlapping open PR |

## Blindspot mitigations

- Scan **open PRs** fully, not only last-24h commits on `main`
- Flag PRs open >7 days touching safety paths
- Run `cargo audit` when advisory-db available
- Map changed crates to [decisions/](decisions/) ADRs
- Note sibling repo `solidworks-mcp` when CAD automation changes
- Prefer `search_vendor_docs` for motor/CAN questions
