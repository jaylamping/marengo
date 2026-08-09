# Tasks: Consul Hardware Commissioning

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~2,000–2,800 across six slices |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR1 IA → PR2 Telemetry → PR3 wire → PR4 Hardware UI → PR5 scope/Enable → PR6 Testing |
| Delivery strategy | ask-on-risk |
| Chain strategy | stacked-to-main |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | IA/chrome + glossary (#117) | PR1 | Base `main`; no wire dependency |
| 2 | Telemetry read-only | PR2 | Base PR1; strip Inventory commands |
| 3 | Proto/codegen/runtime wire facets | PR3 | Deploy runtime before PR4; base PR2 |
| 4 | Hardware facets/commands UI | PR4 | Gate on non-UNSPECIFIED wire; base PR3 |
| 5 | Scope persistence + targeted Enable | PR5 | Davout `active_joints`; base PR4 |
| 6 | Testing/defaults cleanup | PR6 | Base PR5; final integration gate |

## Phase 1: IA / Chrome + Glossary (PR1)

- [x] 1.1 Add `/telemetry` route stub and sidebar entry in `consul/src/routes/config.tsx`, `consul/src/data/sidebar-nav.ts`
- [x] 1.2 Redirect `/subsystems` → `/telemetry` in `consul/src/pages/subsystems.tsx`
- [x] 1.3 Remove `/actuators` route, page, and sidebar entry (`consul/src/pages/actuators.tsx`, `consul/src/components/dashboard/actuators/**`)
- [x] 1.4 Replace `bench_4dof`/`arm_4dof_right` strings in `consul/src/routes/config.tsx`, `consul/src/data/site-header.ts`, `consul/src/lib/bringup-presets.ts`, overview subtitle
- [x] 1.5 Fold PR #117 glossary into root `CONTEXT.md` (facet labels, anatomy, scope terms)
- [x] 1.6 Vitest: nav targets Telemetry, Actuators absent, overview master chrome (`consul/src/**/__tests__/`)

## Phase 2: Telemetry Read-Only (PR2)

- [x] 2.1 Create `consul/src/pages/telemetry.tsx` and `consul/src/components/dashboard/telemetry/**` from Inventory table (live enrichment, no commands)
- [x] 2.2 Strip Set Limits/Enable/Home/go-to-zero from Inventory modals; read-only limit display with Hardware redirect copy
- [x] 2.3 Wire-gate facets: unknown placeholders when `homing_state` absent (no localStorage Ready)
- [x] 2.4 Vitest: Telemetry render, `/subsystems` redirect, no commissioning actions, read-only limit display

## Phase 3: Proto / Codegen / Runtime Wire Facets (PR3)

- [x] 3.1 Add `JointHomingState`, `homing_state`, `drive_active`, `out_of_limits` to `proto/marengo/v1/marengo.proto`; `buf lint`; regen Rust + `consul/src/gen/`
- [x] 3.2 Map homing registry → proto in `crates/marengo-homing/src/{lib.rs,commissioning.rs,registry.rs}`; OutOfLimits from verification/Davout
- [x] 3.3 Publish per-joint `drive_active` and facets in `crates/davout/src/lib.rs`, `crates/berthier/src/loop.rs`, `bins/marengo-pi/src/main.rs`
- [x] 3.4 Add anatomical `limbs` to `config/robot.yaml`; Ready aggregation Joint→Limb→Robot (unbuilt Offline non-blocking)
- [x] 3.5 Rust tests: proto round-trip, aggregation, OutOfLimits mapping, old-wire UNSPECIFIED gating

## Phase 4: Hardware Facets / Commands UI (PR4)

- [x] 4.1 Add facet derivation + badge priority (Fault>OutOfLimits>Offline>Active>Ready>Online) in `consul/src/lib/commissioning.ts`, `consul/src/state/robotStore.ts`
- [x] 4.2 Remove `actuatorZeroStore` localStorage readiness; derive Reference from wire only
- [x] 4.3 Render facet badges and limb/robot aggregation in `consul/src/components/dashboard/hardware/**`
- [x] 4.4 Co-locate Set Limits + Set Zero on Hardware settings; ACTIVE guard; wire-gate Enable UI until facets live
- [x] 4.5 Vitest: badge priority, Ready-from-wire, Set Limits ACTIVE guard, warnings non-blocking

## Phase 5: Scope Persistence + Targeted Enable (PR5)

- [x] 5.1 Implement `crates/marengo-config/src/commissioning_scope.rs`: versioned YAML, ceiling intersection, atomic rename
- [x] 5.2 Gateway CRUD `GET/PUT/DELETE /hardware/commissioning-scope` in `bins/marengo-gateway/src/{http.rs,hardware.rs}`; `confirm_widen` on widen
- [x] 5.3 Scope client in `consul/src/lib/gateway-api.ts`; Hardware scope editor with widen confirm
- [x] 5.4 Pi Enable: reload scope, filter Verified in-scope excluding Fault/OutOfLimits; no-scope requires full-master Robot Ready
- [x] 5.5 Davout `active_joints`: targeted enable, reject commands outside set, `disable_all` on partial failure
- [x] 5.6 Remove enable-time `set_homing_complete` from `bins/marengo-pi/src/main.rs`; retire `POST /command/home` operator path
- [x] 5.7 Rust/gateway tests: scope parse/intersect, unknown joint reject, widen confirm, partial-failure all-off, inactive-command rejection

## Phase 6: Testing / Defaults Cleanup (PR6)

- [ ] 6.1 Remove Enable/Home from `consul/src/components/dashboard/testing/enable-disable-buttons.tsx`; keep motion, go-to-zero, E-stop
- [ ] 6.2 Default Testing hooks to master inventory in `consul/src/state/testingStore.ts`, `use-manual-movement-controller.ts`, related hooks
- [ ] 6.3 Purge remaining `arm_4dof_right`/`bench_4dof` defaults in `consul/src/data/robot-inventory.ts`, teach/compound hooks
- [ ] 6.4 Vitest: Testing absent Enable/Home; motion/E-stop present; master hook defaults
- [ ] 6.5 Integration gate: `cargo test --workspace`, `cd consul && npm test`, proto regen build; Pi smoke (restart, scope persist, explicit Enable, partial-failure all-off)
