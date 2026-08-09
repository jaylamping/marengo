# Apply Progress: consul-hardware-commissioning

**Mode**: Strict TDD (`strict_tdd: true`; cargo test + Vitest available)
**Slice**: PR4 / Phase 4 — Hardware Facets / Commands UI
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

- [x] 4.1 Facet derivation + badge priority in `consul/src/lib/commissioning.ts`; robotStore selectors
- [x] 4.2 `actuatorZeroStore` no longer persists/fabricates readiness; Reference from wire only
- [x] 4.3 Hardware table badges + limb/robot aggregation chrome
- [x] 4.4 Set Limits + Set Zero co-located on Hardware settings; ACTIVE guard retained; Enable UI shown disabled (Phase 5 / wire gate)
- [x] 4.5 Vitest: badge priority, Ready-from-wire, ACTIVE guard, warnings non-blocking

## TDD Cycle Evidence

| Task | Test File | Layer | Safety Net | RED | GREEN | TRIANGULATE | REFACTOR |
|------|-----------|-------|------------|-----|-------|-------------|----------|
| 4.1 | `lib/__tests__/commissioning.test.ts` | Unit | ✅ 21 related | ✅ module missing | ✅ 18 pass | ✅ priority + agg cases | ✅ fault path simplified |
| 4.2 | `state/__tests__/actuatorZeroStore.test.ts` | Unit | ✅ store+set-limits | ✅ localStorage writes | ✅ no-op store | ✅ stale LS ignored | ✅ Set Zero path cleaned |
| 4.3–4.4 | `hardware/__tests__/hardware-overview.test.tsx` | Integration | ✅ 7/7 overview | ✅ badges/Enable absent | ✅ 11/11 | ✅ Fault>Active + Ready wire | ✅ aggregation pure helpers |
| 4.5 | (above + set-limits ACTIVE) | Unit+Integration | ✅ | ✅ Written first | ✅ 89 touched pass | ✅ multi-case | ✅ Clean |

### Test Summary

- **Total tests written (Phase 4)**: ~25 new (18 commissioning + 3 zero-store + 4 hardware UI)
- **Total tests passing (focused)**: 89 across touched consul areas
- **Layers used**: Unit (commissioning, store), Integration (HardwareOverview jsdom)
- **Approval tests**: None
- **Pure functions created**: `resolveJointBadge`, `aggregateWorstBadge`, `limbReady`, `robotReady`, `wireFacetsLive`, `isReferenceReady`, `buildFacetSnapshots`, `limbBadgeForMembers`, `robotWireFacetsLive`

## Deviations from Design

- Anatomical `MASTER_LIMBS` mirrored in Consul from `config/robot.yaml` because gateway config snapshot does not yet expose `limbs` (Phase 5 scope API territory).
- Enable-all-Ready-in-scope is visible but always disabled in Phase 4 — no fake enable client; Phase 5 wires targeted enable.

## Issues Found

None.

## Remaining Tasks

Phase 5–6 unchecked in `tasks.md` (scope persistence + Testing cleanup).

## Workload / PR Boundary

- Mode: stacked PR slice (PR4 on cutover branch)
- Current work unit: Hardware facets/commands UI
- Boundary: consul-only facet derivation, Hardware badges/aggregation, zero-store readiness removal, disabled Enable chrome; no crates/bins/proto
- Estimated review budget impact: ~400–600 LOC class; keep as PR4 slice
