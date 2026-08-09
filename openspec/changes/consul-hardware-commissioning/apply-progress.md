# Apply Progress: consul-hardware-commissioning

**Mode**: Strict TDD (`strict_tdd: true`; Vitest available)
**Slice**: PR2 / Phase 2 — Telemetry Read-Only (on top of PR1)
**Branch**: `jl/hardware-commissioning-cutover-6ab6`
**Chain strategy**: stacked-to-main
**Updated**: 2026-08-09

## Completed Tasks

### Phase 1 (PR1)

- [x] 1.1 `/telemetry` route stub + sidebar Telemetry entry
- [x] 1.2 `/subsystems` → `/telemetry` redirect (`Navigate replace`)
- [x] 1.3 Removed `/actuators` route/page/sidebar + `components/dashboard/actuators/**`; retired `VITE_FEATURE_ACTUATORS`
- [x] 1.4 Master chrome: overview/site-header subtitles; dropped `bench_4dof`/`arm_4dof_right` from bringup preset maps
- [x] 1.5 Folded PR #117 glossary into root `CONTEXT.md`
- [x] 1.6 Vitest IA coverage (nav Telemetry, Actuators absent, master chrome, subsystems redirect)

### Phase 2 (PR2)

- [x] 2.1 Telemetry page + `components/dashboard/telemetry/**` from Inventory table (live enrichment)
- [x] 2.2 Stripped Set Limits/Enable/Home/go-to-zero from Inventory/Telemetry modals; read-only limits + Hardware redirect
- [x] 2.3 Wire-gated Reference facet (`resolveTelemetryReferenceFacet`) — Unknown when `homing_state` absent; ignores localStorage zeroed
- [x] 2.4 Vitest: Telemetry render, `/subsystems` redirect, no commissioning actions, read-only limits

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| 1.1–1.6 | (prior apply-progress) | Unit+Integration | ✅ | ✅ | ✅ | ✅ | ✅ |
| 2.1 | `telemetry-overview.test.tsx`, `telemetry-page.test.tsx` | Integration | ✅ 14 related baseline | ✅ stub still present | ✅ overview + table | ✅ page + overview shells | ✅ eager table under DeferredLazyBody |
| 2.2 | `telemetry-page.test.tsx`, `inventory-panel-shell.test.tsx`, `inventory-limits-readonly.test.tsx` | Integration | ✅ panel-shell 14/14 | ✅ Home still present | ✅ Actions/Tests stripped | ✅ modal + row-menu + limits | ✅ redirect copy wording |
| 2.3 | `telemetry-facets.test.ts`, overview facet case | Unit+Integration | N/A (new) | ✅ module missing | ✅ Unknown gate | ✅ absent/UNSPECIFIED/Verified/localStorage ignore | ✅ pure helper + badge |
| 2.4 | all above | Unit+Integration | ✅ full suite | ✅ written first | ✅ 251/251 | ✅ multi-case | ✅ Clean |

### Test Summary

- **Total tests written (Phase 2)**: 12 new (6 facets + 2 overview + 4 page/modal/menu; panel-shell assertions updated)
- **Total tests passing**: 251 (`cd consul && npm test -- --run`)
- **Layers used**: Unit (6), Integration (6+), E2E (0)
- **Approval tests**: None
- **Pure functions created**: `resolveTelemetryReferenceFacet`, `telemetryReferenceFacetLabel`

## Deviations from Design

None — Telemetry reuses Inventory table with commands stripped; facet placeholders gate on absent wire until PR3 codegen.

## Issues Found

None.

## Remaining Tasks

Phase 3–6 unchecked in `tasks.md` (PR3 wire facets onward).

## Workload / PR Boundary

- Mode: stacked PR slice (PR2 on PR1 branch until orchestrator splits)
- Current work unit: Telemetry read-only
- Boundary: Consul Telemetry page/components + Inventory modal/command strip + wire-gated facet helper; no proto/runtime
- Estimated review budget impact: ~400 LOC class; keep as PR2 slice
