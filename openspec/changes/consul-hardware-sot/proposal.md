# Proposal: Consul Hardware Source of Truth

## Intent
Give the sole operator one Hardware workflow for keeping runtime configuration and geometry complete, durable, and recoverable. Today, bringup trees, slice URDFs, profiles, deploy tools, and Inventory edits can disagree with Pi memory.

## Proposal Question Round
Waived by request: issue #110 locks the choices. Closed questions: incompleteness never blocks; Accept activates resolved fields; durability lives only in master files; test subsets stay ephemeral.

## Scope
### In Scope
- Cut over once to `/opt/marengo/config/` and `/opt/marengo/assets/urdf/marengo.urdf`; promote `arm_4dof_right.urdf`, archive old/slice URDFs, delete bringup SoT, and retire `/config/profiles*`.
- Compute v1 completeness in gateway/shared Rust for mass/COM, kinematics, hard limits, and config coverage.
- Provide authenticated URDF read/upload/activate plus archive list/fetch/restore using `staging/` and `archive/<upload-id>/manifest.json`.
- Add `/hardware`: table-first, optional vanilla Three.js picker, settings sheet, warn badges, and joint-name/field resolve with explicit kinematics-critical picks.
- Move durable Set Limits to Hardware; make deploy/sync/MCP and callers use ephemeral master subsets.

### Out of Scope
Table polish, offline-actuator pruning, 3D as default, Postgres SoT, hard blocking, alternate URDF/config SoTs, and motor-path changes.

## Capabilities
### New Capabilities
- `hardware-description-sot`: Master config/URDF lifecycle, completeness, ADR 0012 write-behind, and ADR 0017 expansion.
- `hardware-management-api`: URDF staging, resolution, activation, archive, restore, and subset contracts.
- `hardware-operator-workspace`: Hardware table/3D/sheet/import and exclusive durable Set Limits workflow.

### Modified Capabilities
None; existing specs cover only log API errors.

## Approach
Deliver vertical PRs: (1) SoT cutover, (2) API/completeness, (3) Hardware UI, (4) Inventory/deploy/sync/MCP cutover. Preserve Berthier → Davout → robstride and memory-first ACK/write-behind.

## Affected Areas
| Area | Impact |
|---|---|
| `config/`, `assets/urdf/` | Master seed, archive, bringup removal |
| `crates/marengo-config/`, `bins/marengo-gateway/` | Validation, merge/archive API, profile retirement |
| `consul/src/` | Hardware workspace; Inventory becomes non-durable |
| `bins/marengo-pi/`, `scripts/`, `tools/marengo-pi-mcp/` | Runtime, deploy, sync, subset cutover |

## Risks
- Split-brain/deploy clobber: require durable ACKs, atomic writes, checksums, and coherent rollout.
- Bad merge changes kinematics: stage first, require critical picks, archive every accepted contributor, support restore.
- Completeness false positives: remain warn-only.

## Rollback Plan
Before cutover, snapshot Pi state and archive checksummed YAML/URDF inputs. Roll back gateway, Consul, runtime paths, and sync tools together; restore prior YAML/profile pointers and archived URDF, restart, then verify the 4-DOF bench. Never mix old/new writers.

## Dependencies
- A `feasibility-brief` **Go** is required before apply; none is persisted yet because this is software plus bench config/URDF co-design.

## Success Criteria
- [ ] Pi, gateway, Consul, deploy, and MCP use one master config and URDF.
- [ ] Completeness warns without blocking; upload/resolve/activate/archive/restore work end-to-end.
- [ ] Hardware exclusively owns durable Set Limits; subsets create no alternate SoT.
