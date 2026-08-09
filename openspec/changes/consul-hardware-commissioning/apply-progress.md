# Apply Progress: consul-hardware-commissioning

**Mode**: Strict TDD (`strict_tdd: true`; cargo test + Vitest available)
**Slice**: PR5 / Phase 5 — Scope Persistence + Targeted Enable
**Branch**: `jl/hardware-commissioning-cutover-6ab6`
**Chain strategy**: stacked-to-main
**Updated**: 2026-08-09

## Completed Tasks

### Phase 1 (PR1)

- [x] 1.1–1.6 IA/chrome + glossary (prior batch)

### Phase 2 (PR2)

- [x] 2.1–2.4 Telemetry read-only (prior batch)

### Phase 3 (PR3)

- [x] 3.1–3.5 Proto/codegen/runtime wire facets (prior batch)

### Phase 4 (PR4)

- [x] 4.1–4.5 Hardware facets/commands UI (prior batch; preserved `[x]`)

### Phase 5 (PR5)

- [x] 5.1 `marengo-config` commissioning_scope: versioned YAML, ceiling ∩, atomic rename
- [x] 5.2 Gateway `GET/PUT/DELETE /hardware/commissioning-scope` + `confirm_widen` on widen
- [x] 5.3 Consul `gateway-api` scope client + `commissioning-scope` module (`scopeWidens`); full Hardware scope editor UI deferred as stub (API + pure helper only — avoid Phase 4 UI overlap)
- [x] 5.4 Pi Enable: reload scope, `select_enable_targets` (Verified in-scope excl. Fault/OOL; no-scope → Robot Ready)
- [x] 5.5 Davout `active_joints` + `enable_targets` + inactive MIT reject + `disable_all` on partial failure
- [x] 5.6 Remove enable-time `set_homing_complete`; ignore Chappe HomingComplete; `POST /command/home` → 410 Gone
- [x] 5.7 Rust/gateway/vitest coverage for parse/intersect, unknown reject, widen, partial-fail all-off, inactive command

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| 5.1 | `marengo-config/.../commissioning_scope.rs` | Unit | ✅ 54 config | ✅ module missing | ✅ 9 scope tests | ✅ ceiling/widen/atomic/version | ✅ normalize helpers |
| 5.2 | `marengo-gateway` hardware_tests | Integration | ✅ hardware auth | ✅ routes missing | ✅ CRUD+widen+401 | ✅ unknown joint + DELETE | ✅ shared scope_response |
| 5.3 | `consul/.../commissioning-scope.test.ts` | Unit | ✅ Phase4 commissioning | ✅ module missing | ✅ scopeWidens | ✅ narrow vs widen | ✅ types in module |
| 5.4 | `marengo-homing` select_enable_* | Unit | ✅ robot_ready | ✅ fn missing | ✅ 4 filter cases | ✅ scoped vs Robot Ready | ✅ `is_enable_eligible` |
| 5.5 | `davout` enable_targets_* / InactiveJoint | Unit | ✅ 52 davout | ✅ API missing | ✅ targeted+partial+reject | ✅ empty reject | ✅ `enable_targets_inner` |
| 5.6 | gateway `command_home_is_gone` | Integration | ✅ | ✅ still published | ✅ 410 Gone | ✅ Pi enable no set_homing_complete | ✅ drain-ignore HomingComplete |
| 5.7 | (combined above) | Unit+Int | ✅ | ✅ Written first | ✅ All pass | ✅ Multi-case | ✅ Clean |

### Test Summary

- marengo-config: 63 pass (incl. 9 scope)
- marengo-homing: 23 pass (incl. select_enable_*)
- davout: 56 pass (incl. enable_targets / inactive MIT)
- marengo-gateway: commissioning_scope_crud + command_home_is_gone
- consul vitest: commissioning-scope 2 pass
- marengo-pi: `cargo check` clean

## Deviations from Design

- Consul Hardware **scope editor UI** is a stub: `commissioning-scope.ts` + gateway-api CRUD only (orchestrator-allowed to avoid Phase 4 UI clash). Widen confirm is enforced server-side (`confirm_widen`) and client helper `scopeWidens` is ready for a later editor surface.
- Testing page may still call deprecated `postHomeCommand()` until Phase 6 removes Enable/Home UI; client now throws and gateway returns 410.

## Issues Found

- Parallel agent briefly stashed Phase 5 WIP during Phase 4 commit; restored from `stash` before finalizing. No design change.

## Remaining Tasks

Phase 6 Testing/defaults cleanup unchecked in `tasks.md`.

## Workload / PR Boundary

- Mode: stacked PR slice (PR5 on cutover branch)
- Current work unit: Scope persistence + targeted Enable
- Boundary: scope file/API/Davout active_joints/Pi enable gate; no Phase 6 Testing cleanup
- Estimated review budget impact: ~700–900 LOC class; keep as PR5 slice (`size:exception` not claimed — stacked chain)
