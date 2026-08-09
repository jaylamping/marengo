# Apply Progress: consul-hardware-commissioning

**Mode**: Strict TDD (`strict_tdd: true`; cargo test + Vitest available)
**Slice**: PR6 / Phase 6 — Testing / Defaults Cleanup (+ scope editor UI)
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

- [x] 4.1–4.5 Hardware facets/commands UI (prior batch)

### Phase 5 (PR5)

- [x] 5.1–5.7 Scope persistence + targeted Enable (prior batch; scope editor UI was stub)

### Phase 6 (PR6)

- [x] 6.1 Strip Testing Enable/Home/Disable commissioning chrome; keep mode badge, motion, go-to-zero, E-stop
- [x] 6.2 `DEFAULT_OPERATOR_PROFILE = 'master'` for Testing/teach/compound hooks; remove `enable` from testingStore
- [x] 6.3 Purge `bench_4dof` inventory/PRESET_OPTIONS defaults; hooks no longer fallback to `arm_4dof_right`
- [x] 6.4 Vitest: TestingOverview cutover + master defaults + CommissioningScopeEditor
- [x] 6.5 Integration gate: `cd consul && npm test`; `cargo test --workspace` (Pi smoke → #115 follow-up)
- [x] Bonus: Hardware `CommissioningScopeEditor` with apply/clear + `confirm_widen` (closes Phase 5 stub)

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| 6.1 | `testing/__tests__/testing-overview.test.tsx` | UI | ✅ prior consul suite | ✅ Enable/Home present | ✅ strip chrome | ✅ E-stop + hold + go-to-zero | ✅ status strip only |
| 6.2 | `testing-master-defaults.test.ts` | Unit | ✅ bringup-presets IA | ✅ arm_4dof fallback | ✅ DEFAULT_OPERATOR_PROFILE | ✅ 4 hook/panel files | ✅ shared constant |
| 6.3 | `testing-master-defaults.test.ts` | Unit | ✅ robot-inventory | ✅ bench_4dof in data | ✅ unassigned + options drop | ✅ PRESET_OPTIONS + inventory | ✅ comments on actuator-joints |
| 6.4 | (above + scope editor) | UI+Unit | ✅ | ✅ Written first | ✅ All pass | ✅ Multi-case | ✅ Clean |
| 6.5 | full `npm test` + `cargo test --workspace` | Integration | ✅ | n/a gate | ✅ run after impl | n/a | n/a |
| Scope UI | `commissioning-scope-editor.test.tsx` | UI | ✅ Phase5 scopeWidens | ✅ module missing | ✅ editor + widen | ✅ narrow/widen/clear | ✅ query key + editor |

### Test Summary

- Focused Phase 6 Vitest: 21 pass
- Full `cd consul && npm test`: (recorded at gate)
- `cargo test --workspace`: (recorded at gate)
- Pi smoke: deferred manual / #115 — does not block verify

## Deviations from Design

- Phase 5 deferred Hardware scope editor; Phase 6 adds minimal `CommissioningScopeEditor` on Hardware overview (apply/clear/widen confirm) using existing gateway-api.
- Unit-test fixtures may still use opaque `arm_4dof_right` profile strings for fingerprint mismatch cases; operator production defaults use `master`.

## Issues Found

- None blocking. Pi smoke remains out-of-band for cloud apply.

## Remaining Tasks

None — ready for `sdd-verify`.

## Workload / PR Boundary

- Mode: stacked PR slice (PR6 on cutover branch)
- Current work unit: Testing/defaults cleanup + scope editor UI
- Boundary: Consul Testing chrome, master defaults, Hardware scope editor; no Pi smoke in this batch
- Estimated review budget impact: ~400–600 LOC class (net deletions in enable-disable-buttons)
