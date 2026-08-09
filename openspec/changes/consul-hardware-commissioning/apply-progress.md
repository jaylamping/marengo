# Apply Progress: consul-hardware-commissioning

**Mode**: Strict TDD (`strict_tdd: true`; Vitest available)
**Slice**: PR1 / Phase 1 — IA / Chrome + Glossary
**Branch**: `jl/hardware-commissioning-cutover-6ab6`
**Chain strategy**: stacked-to-main
**Updated**: 2026-08-09

## Completed Tasks

- [x] 1.1 `/telemetry` route stub + sidebar Telemetry entry
- [x] 1.2 `/subsystems` → `/telemetry` redirect (`Navigate replace`)
- [x] 1.3 Removed `/actuators` route/page/sidebar + `components/dashboard/actuators/**`; retired `VITE_FEATURE_ACTUATORS`
- [x] 1.4 Master chrome: overview/site-header subtitles; dropped `bench_4dof`/`arm_4dof_right` from bringup preset maps
- [x] 1.5 Folded PR #117 glossary into root `CONTEXT.md`
- [x] 1.6 Vitest IA coverage (nav Telemetry, Actuators absent, master chrome, subsystems redirect)

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| 1.1 | `consul/src/data/__tests__/hardware-commissioning-ia.test.ts` | Unit | ✅ 25/25 related | ✅ Written | ✅ Passed | ✅ nav + route chrome cases | ✅ Clean |
| 1.2 | `consul/src/pages/__tests__/subsystems-redirect.test.tsx` | Integration | N/A (page rewrite) | ✅ Written | ✅ Passed | ✅ redirect + no Inventory chrome | ➖ None needed |
| 1.3 | same IA test file (`/actuators` absent) | Unit | ✅ actuators suite baseline | ✅ Written | ✅ Passed | ✅ titles + urls | ✅ Deleted dead flag/tests |
| 1.4 | IA test master-chrome describe | Unit | ✅ 25/25 | ✅ Written | ✅ Passed | ✅ route + site-header + presets | ✅ Clean |
| 1.5 | N/A (docs fold) | Structural | N/A | ➖ Triangulation skipped: glossary copy only | ✅ CONTEXT.md matches glossary branch | ➖ Single | ➖ None needed |
| 1.6 | both new test files | Unit+Integration | ✅ 25/25 | ✅ Written first | ✅ 14/14 then 239/239 | ✅ multi-case | ✅ Clean |

### Test Summary

- **Total tests written**: 10 new (8 IA + 2 redirect)
- **Total tests passing**: 239 (`cd consul && npm test -- --run`)
- **Layers used**: Unit (8), Integration (2), E2E (0)
- **Approval tests**: None — behavior change, not pure refactor
- **Pure functions created**: 0 (route/nav data + page shell)

## Deviations from Design

None — Phase 1 IA stub only; full Telemetry table deferred to Phase 2 as designed.

## Issues Found

None.

## Remaining Tasks

Phase 2–6 unchecked in `tasks.md` (PR2 Telemetry onward).

## Workload / PR Boundary

- Mode: stacked PR slice (PR1)
- Current work unit: IA/chrome + glossary
- Boundary: Consul routes/nav/chrome + CONTEXT.md glossary; no proto/wire/Hardware facets
- Estimated review budget impact: well under 400 LOC for this slice
