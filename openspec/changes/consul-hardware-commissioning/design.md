# Design: Consul Hardware Commissioning

## Architecture Overview

Hardware joins master `robot.yaml`/motor metadata with `RobotState`; it owns commissioning mutations. Telemetry renders the same rows with live values but no commands. Runtime truth remains `marengo-homing` + Davout and is published through Chappe—never browser storage.

`marengo-config` owns a shared, versioned scope loader. Gateway provides authenticated CRUD; marengo-pi reloads the file for every Enable and intersects it with its boot-time `MARENGO_JOINT_SUBSET`. Davout owns targeted drive activation and the active-joint command boundary.

**Spec reconciliation:** done. `hardware-commissioning-state` and `hardware-management-api` match the approved proposal: Robot Ready is full-master (unbuilt Offline do not block; scope does not redefine Ready); no scope file requires Robot Ready before Enable; persisted scope enables Verified in-scope without full Robot Ready.

## Architecture Decisions

| Decision | Choice and rationale | Rejected |
|---|---|---|
| Wire facets | Add proto `JointHomingState` (`UNSPECIFIED`, `UNHOMED`, `HOMING`, `VERIFIED`, `FAULTED`) and `JointState.homing_state=7`, `drive_active=8`, `out_of_limits=9`. Unspecified gates old runtimes; omission/last-seen feedback means Offline. | HTTP homing snapshot or localStorage: split/fabricated truth. |
| Anatomy/Ready | Add anatomical `limbs` membership to master `robot.yaml`. Joint Ready = Verified; Limb Ready covers online or motor-mapped built members (unbuilt members do not block); Robot Ready covers all master actuated joints. `SafetyState.mode` remains FSM state, not this aggregation. | Name-prefix/static Inventory grouping. |
| Scope format | `/opt/marengo/var/commissioning-scope.yaml`: `version: 1` plus sorted unique canonical `joints`. Store expanded joints, not limb labels, so later group edits cannot silently widen scope. Atomic rename; unknown joints rejected. | Session/localStorage scope or persisting limb aliases. |
| Scope API | Authenticated `GET/PUT/DELETE /hardware/commissioning-scope`; GET returns persisted, startup ceiling, and effective intersection. PUT requires `confirm_widen=true` only when effective scope grows. | New Chappe scope message: unnecessary second persistence path. |
| Enable filtering | Reuse `EnableRequest`. With scope, target effective joints that are Verified, Online, fault-free, and in bounds; skip others. Without a scope file, require full Robot Ready and all loaded master targets Online/Nominal. Empty target rejects. | Extending request with client-selected joints: TOCTOU/bypass risk. |
| Atomic activation | Davout tracks `active_joints`, enables only validated targets, and on any enable/run-mode failure calls `disable_all`. Berthier emits/checks feedback only for active joints; Davout rejects commands outside that set. | Global `mode==ACTIVE` as per-joint authority. |

## Command Matrix

| Surface | Commands |
|---|---|
| Hardware | Set Limits, Set Zero, scope apply/clear, Enable-all-Ready-in-scope, Disable |
| Telemetry | None; live values/facets/limits only |
| Testing | Motion, go-to-zero, E-stop; no Enable or Home/Mark READY |

## Data Flows

```mermaid
sequenceDiagram
  Operator->>Gateway: PUT scope + confirm_widen
  Gateway->>Gateway: validate master joints; atomic YAML
  Gateway-->>Hardware: persisted + ceiling + effective
```

```mermaid
sequenceDiagram
  Hardware->>Gateway: EnableRequest(true)
  Gateway->>Pi: robot/enable
  Pi->>Pi: reload scope; ceiling intersection; facet filter
  Pi->>Davout: enable_targets(joints)
  alt any failure
    Davout->>Davout: disable_all
  else success
    Davout-->>Pi: active_joints
  end
  Pi-->>Hardware: RobotState/SafetyState
```

```mermaid
sequenceDiagram
  Pi->>Pi: restart; load calibration + persisted scope
  Pi-->>Hardware: Disabled; drive_active=false; effective scope
  Operator->>Hardware: confirm Enable
  Hardware->>Pi: explicit EnableRequest
```

## File-Level Change Map

| Files | Action |
|---|---|
| `proto/marengo/v1/marengo.proto`, `consul/src/gen/marengo/v1/marengo_pb.ts` | Add/regenerate facet wire fields. |
| `crates/marengo-config/src/{lib.rs,commissioning_scope.rs}`, `config/robot.yaml` | Scope schema/intersection and limb metadata. |
| `crates/marengo-homing/src/{lib.rs,commissioning.rs,registry.rs}` | Proto mapping, Ready aggregation, classified verification state. |
| `crates/davout/src/lib.rs`, `crates/berthier/src/loop.rs` | Targeted activation, active command filtering, facet publication. |
| `bins/marengo-pi/src/main.rs` | Remove enable-time auto-home and HomingComplete consumer; resolve targets. |
| `bins/marengo-gateway/src/{http.rs,hardware.rs,hardware_tests.rs}` | Scope CRUD; retire `/command/home`. |
| `consul/src/lib/{gateway-api.ts,commissioning.ts}`, `consul/src/state/{robotStore.ts,actuatorZeroStore.ts}` | Scope client/facet derivation; remove readiness storage. |
| `consul/src/components/dashboard/hardware/**` | Facets, aggregation, scope, Enable, Set Zero. |
| `consul/src/pages/{telemetry.tsx,subsystems.tsx}`, `consul/src/components/dashboard/{telemetry/**,inventory/**}` | Create read-only Telemetry/redirect; retire Inventory command UI. |
| `consul/src/routes/config.tsx`, `consul/src/data/sidebar-nav.ts`, `consul/src/pages/actuators.tsx`, `consul/src/components/dashboard/actuators/**` | IA cutover and Actuators deletion. |
| `consul/src/components/dashboard/testing/{testing-overview.tsx,enable-disable-buttons.tsx}`, `consul/src/state/testingStore.ts`, affected hooks/data defaults | Keep motion/E-stop; remove Enable/Home and bringup defaults. |
| `CONTEXT.md` | Fold PR #117 glossary. |

## Deploy / Order Constraints

Use six approximately 400-line slices: (1) IA/chrome + glossary, (2) Telemetry, (3) proto/codegen/runtime wire, (4) Hardware facets/commands, (5) scope + targeted Enable, (6) Testing/default cleanup. Deploy slice 3 runtime before facet UI; never expose scoped Enable before Davout targeted-command enforcement.

## Testing Strategy

Rust unit tests cover scope parsing/intersection, aggregation, out-of-limits mapping, target filtering, inactive-command rejection, and injected partial enable failure/all-off. Gateway temp-dir tests cover auth, atomic CRUD, widening, and unknown joints. Proto round trips and Consul Vitest cover old-wire Unknown, badge priority, scope confirmation, routes, command absence, and post-restart Disabled. Run `cargo test --workspace`, `cd consul && npm test`, proto regeneration/build, then Pi smoke: restart, scope persistence, explicit Enable, CAN/position trace, and partial-failure all-off.

## Risks and Mitigations

- Mixed wire rollout can lie: gate facets/Enable on non-UNSPECIFIED wire and deploy wire first.
- Scope/command divergence can energize excluded joints: Pi recomputes eligibility and Davout enforces active membership.
- Disk corruption/stale scope: version validation, atomic rename, fail closed.

## Open Questions

None.
