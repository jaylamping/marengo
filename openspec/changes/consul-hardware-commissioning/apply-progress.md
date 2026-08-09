# Apply Progress: consul-hardware-commissioning

**Mode**: Strict TDD (`strict_tdd: true`; cargo test + Vitest available)
**Slice**: PR3 / Phase 3 — Proto / Codegen / Runtime Wire Facets
**Branch**: `jl/hardware-commissioning-cutover-6ab6`
**Chain strategy**: stacked-to-main
**Updated**: 2026-08-09

## Completed Tasks

### Phase 1 (PR1)

- [x] 1.1–1.6 IA/chrome + glossary (prior batch)

### Phase 2 (PR2)

- [x] 2.1–2.4 Telemetry read-only (prior batch)

### Phase 3 (PR3)

- [x] 3.1 Proto `JointHomingState` + `JointState` fields 7–9; buf lint; regen Rust + Consul TS
- [x] 3.2 Homing→proto mapping + OutOfLimits in `marengo-homing` (`commissioning.rs`, registry, verify)
- [x] 3.3 Publish `drive_active` + facets via Davout wire helpers → Berthier `RobotState` → marengo-pi status
- [x] 3.4 Anatomical `limbs` in `config/robot.yaml` (+ humanoid template); Ready aggregation Joint→Limb→Robot
- [x] 3.5 Rust tests: proto round-trip, aggregation, OutOfLimits mapping, UNSPECIFIED gating

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| 3.1 | `armee-proto/src/lib.rs` | Unit | ✅ 6/6 proto | ✅ new fields + ordinals | ✅ 7/7 | ✅ round-trip + ordinals | ✅ Clean |
| 3.2 | `marengo-homing/.../commissioning.rs`, `verify.rs` | Unit | ✅ 9/9 homing | ✅ module missing | ✅ mapping + OOL | ✅ verify OOL vs tolerance | ✅ pure helpers |
| 3.3 | `davout` commissioning_wire + measured_position_fault | Unit | ✅ enable_blocked | ✅ helpers missing | ✅ drive_active + OOL latch | ✅ enable/disable + fault path | ✅ `joint_commissioning_wire` |
| 3.4 | `commissioning.rs` aggregation + `robot_yaml_parses` | Unit | ✅ config parse | ✅ limb_ready/robot_ready missing | ✅ unbuilt non-blocking | ✅ scope≠Ready + OOL block | ✅ `JointFacetInput` |
| 3.5 | (above combined) | Unit | ✅ | ✅ Written first | ✅ All pass | ✅ Multi-case | ✅ Clean |

### Test Summary

- **Total tests written (Phase 3)**: ~14 new (9 commissioning + 2 verify/davout + 1 proto ordinal + config limb asserts)
- **Total tests passing (focused)**: armee-proto 7, marengo-homing 19, davout 52, marengo-config 54, berthier 143, chappe robot_state, consul telemetry-facets 6
- **Layers used**: Unit (all), Integration (0), E2E (0)
- **Approval tests**: None
- **Pure functions created**: `to_proto_homing_state`, `from_proto_homing_state`, `verify_error_is_out_of_limits`, `limb_ready`, `robot_ready`, `wire_homing_is_unspecified`, `JointFacetInput`, `Supervisor::joint_commissioning_wire`

## Deviations from Design

- `drive_active` is global ACTIVE-mode for all configured motors until Phase 5 `active_joints` targeted enable.
- Robot Ready aggregation is a pure library API (`robot_ready`); Enable gating that *consumes* it lands in Phase 5.

## Issues Found

None.

## Remaining Tasks

Phase 4–6 unchecked in `tasks.md` (Hardware facets UI onward). Deploy Phase 3 runtime before Phase 4 facet UI.

## Workload / PR Boundary

- Mode: stacked PR slice (PR3 on cutover branch)
- Current work unit: Proto/codegen/runtime wire facets
- Boundary: proto + codegen + marengo-homing/davout/berthier/pi publish + limbs/aggregation; no Hardware UI / scope CRUD
- Estimated review budget impact: ~400–600 LOC class; keep as PR3 slice
