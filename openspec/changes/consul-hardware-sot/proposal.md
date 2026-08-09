# Proposal: Consul Hardware Source of Truth

## Intent

Operators today reconcile bringup profile trees (`config/bringup/*`), gateway `/config/profiles*`, slice URDFs, Inventory preset edits, and Pi `MARENGO_CONFIG_DIR` against live in-memory limits (ADR 0012) and Enable faults (`URDF ∩ motors.yaml` bench, ADR 0017). The sole operator needs one Hardware workflow where master YAML and `marengo.urdf` are the only durable description, completeness gaps warn but never block, import Accept makes resolved fields active, and durable Set Limits lives only on Hardware.

Tickets: #96 (Hardware SoT), grilling #110 (locked decisions), #111 (tasks), research #108 (URDF archive layout), prototype #100 (UI).

## Proposal Question Round

Waived by request: #110 locks business choices. Assumptions: warn-only completeness; Accept = Active; master files only durable; testing limb subsets ephemeral against master.

## Scope

### In Scope

- **SoT cutover:** Root `config/{robot,motors,control,homing}.yaml`; collapse `bringup/arm_4dof_right` into root; delete bringup as runtime SoT; Pi durable tree `/opt/marengo/config/`; day-1 promote `arm_4dof_right.urdf` → `assets/urdf/marengo.urdf`; archive placeholder + slice URDFs; retire gateway `/config/profiles*`.
- **Completeness v1:** Compute in `marengo-gateway` / `marengo-config` (mass/COM, kinematics, hard limits, config coverage); Consul renders warnings only — never hard-block.
- **URDF API v1:** Authenticated read, upload, activate, archive list/fetch/restore; on-disk layout `staging/`, `archive/<upload-id>/` + `manifest.json` (#108).
- **Hardware UI:** Table-first workspace with optional Table/3D toggle; unified settings sheet; import wizard with full field-level resolve and explicit kinematics-critical picks.
- **Set Limits:** Durable path only on Hardware; remove Inventory durable Set Limits; preserve Berthier → Davout → robstride, ADR 0012 ACK/write-behind, ADR 0017 expand-only URDF hard envelope on Apply.
- **Subsets:** On-the-fly ephemeral limb slices against master — never alternate URDF or config SoTs.
- **Phase 4:** Strip Inventory callers; update deploy scripts and marengo-pi MCP sync to master paths.

### Out of Scope (non-goals)

Table polish, offline actuator pruning, 3D-as-default, Postgres SoT, completeness hard gates, alternate URDF/config SoTs, motor-path or CAN shortcuts, gateway profile registry retention.

## Capabilities

### New Capabilities

- `hardware-description-sot`: Master config/URDF lifecycle, bringup retirement, completeness rules, ADR 0012/0017 limit semantics.
- `hardware-management-api`: URDF staging, field resolution, activation, archive, restore, ephemeral subset contracts.
- `hardware-operator-workspace`: Hardware table/3D/sheet/import UI and exclusive durable Set Limits workflow.

### Modified Capabilities

None — existing `openspec/specs/` covers only `log-api-error-model`.

## Approach

One OpenSpec change delivered as **vertical PRs** in order: **SoT → API → UI → inventory/deploy**. Proto-first if URDF/completeness wire types need new messages; regenerate Rust + TS. No Berthier CAN shortcuts.

| Phase | Deliverable | Gate |
|-------|-------------|------|
| 1 SoT | Root YAML seed, URDF promote/archive, env defaults, bringup SoT removal | Bench boots from master paths |
| 2 API | Completeness endpoints, URDF v1 routes, profile route retirement | Gateway + marengo-config tests |
| 3 UI | `/hardware` workspace, import wizard, Set Limits move, warn badges | Consul vitest |
| 4 inventory/deploy | Inventory strip, `scripts/`, MCP `pi_sync_*` cutover | Bench sync smoke |

**Feasibility gate:** `feasibility-brief` (software + bench) **Go** required before apply; record verdict in change folder.

## Affected Areas (impact surface)

| Area | Impact | Description |
|------|--------|-------------|
| `config/`, `assets/urdf/` | Modified | Master seed, URDF promote/archive, bringup removal |
| `crates/marengo-config/` | Modified | Validation, completeness, profile txn retirement |
| `bins/marengo-gateway/` | Modified | URDF API, completeness, retire `profiles.rs` routes |
| `proto/` | Maybe | URDF/completeness wire types if not HTTP-only |
| `consul/src/` | Modified | Hardware UI; Inventory non-durable limits |
| `bins/marengo-pi/`, `crates/davout/` | Modified | Master paths; limit patch + URDF expand |
| `scripts/`, `tools/marengo-pi-mcp/` | Modified | Deploy/sync/MCP master cutover |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Split-brain during cutover | High | Single coherent release; atomic writes; checksums; no mixed old/new writers |
| Deploy clobbers Pi URDF expand (ADR 0017) | Med | Durable-gated sync; archive restore; pull-before-deploy |
| Bad merge alters kinematics | Med | Stage-first; critical field picks; archive every accepted contributor |
| Completeness false positives annoy operators | Low | v1 warn-only; tune rules in gateway |

## Rollback Plan (SoT cutover)

Before cutover: snapshot Pi in-memory limit state and durable YAML/URDF with checksum manifest (archive API or dated copy). On failure: revert gateway, Consul, marengo-pi, and sync tools as **one release**; restore prior YAML pointers and URDF via archive restore or git; if emergency, temporarily point `MARENGO_CONFIG_DIR` at pre-cutover layout; `pi_restart_marengo_pi`; verify 4-DOF bench Enable, homing, and gravity. Never run legacy profile writers against new master paths in parallel.

## Dependencies

- `openspec/changes/consul-hardware-sot/feasibility-brief.md` with **Go** before apply.
- Locked decisions from grilling #110 — do not re-litigate in spec/design.

## Success Criteria

- [ ] Pi, gateway, Consul, deploy, and MCP use one master config tree and `marengo.urdf`
- [ ] Completeness v1 surfaces warnings only; no blocking HTTP or UI gates
- [ ] URDF upload → field resolve → activate → archive list/fetch/restore works E2E
- [ ] Hardware exclusively owns durable Set Limits; Inventory durable path removed
- [ ] Limb subsets are ephemeral; no alternate URDF SoT created
- [ ] `/config/profiles*` retired; bringup not runtime SoT
- [ ] `cargo test --workspace` and `cd consul && npm test` pass per merged phase
