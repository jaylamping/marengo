# Exploration: consul-hardware-commissioning

Locked by grilling #116 / issue #118. Do not re-litigate IA or facet rules.

## Current State

### Routes (today)

| Route | Role |
|-------|------|
| `/hardware` | Master table, import, completeness; **Set Limits + Set Zero** in settings sheet |
| `/subsystems` | **Inventory table** (static humanoid + live enrichment) — not read-only telemetry |
| `/testing` | **Enable / Disable / Home**, dry-run default `true`, full-inventory actuator picker |
| `/actuators` | Feature-flagged shell — retire |
| `/` | Overview; subtitle still `arm_4dof_right · bench` |

No `/telemetry` route. Sidebar lists Subsystems, not Telemetry.

### Runtime / API

| Concern | Wire today | Consul workaround |
|---------|------------|-------------------|
| Drive | Robot-wide `SafetyState.mode` | Same on all joint badges |
| Health | `JointState.fault`, `SafetyState.active_faults` | Testing badge helper |
| Presence | `ActuatorLimitSnapshot.wired` + feedback | Hardware `onCan` |
| Reference (Ready) | **Not published** | `actuatorZeroStore` (localStorage) + READY ⇒ all zeroed |
| Set Zero | `POST /command/set_zero` | Hardware `SetLimitsPanel` |
| Set Limits | `POST /config/patch` | Hardware only (SoT shipped) |
| Enable | `POST /command/enable` | Testing bar only |
| Mark READY | `POST /command/home` → `HomingComplete` | Testing **Home** (all Verified) |
| Go-to-zero | MIT hold | Inventory modal `HomeActuatorButton` — off Hardware per lock |
| Scope | **None** (env `MARENGO_JOINT_SUBSET` at boot only) | — |

Gateway: `GET /config/snapshot` returns profile `"master"` for repo `config/`. No homing/scope endpoints.

### Bringup chrome leftovers

`routes/config.tsx`, `site-header.ts`, `bringup-presets.ts`, `enrich-inventory.ts`, `robot-inventory.ts` (`bench_4dof`), `actuator-joints.ts`, Testing hook defaults `arm_4dof_right`, needs-restart copy, proto comment on bringup CAS.

## Affected Areas

- Routes/nav: `consul/src/routes/config.tsx`, `data/sidebar-nav.ts`
- Subsystems → Telemetry: `pages/subsystems.tsx`, `components/dashboard/inventory/**`
- Hardware commissioning: `components/dashboard/hardware/**`, shared `set-limits-panel.tsx`
- Testing trim: `enable-disable-buttons.tsx`, `testingStore.ts`, hooks with profile defaults
- Wire: `proto/marengo/v1/marengo.proto`, `bins/marengo-pi`, `bins/marengo-gateway`, `marengo-homing` / Davout
- Glossary: `CONTEXT.md` (PR #117)

## Approaches

| Approach | Pros | Cons | Effort |
|----------|------|------|--------|
| **A. UI-only facets + localStorage scope** | Fast, no deploy | Breaks scope persistence + Verified honesty | Med — **rejected by #116** |
| **B. HTTP homing/scope snapshots only** | Per-joint Verified without full proto | Split HTTP vs Chappe; incomplete drive/OutOfLimits | Med–High |
| **C. Proto + Chappe publish (recommended)** | Honest facets, single pipeline, ADR-aligned | Cross-stack deploy order | High |

## Feasibility

- **domains_touched:** software, pi, control
- **feasibility_required:** Yes (light)
- **feasibility_topic_key:** `feasibility/consul-hardware-commissioning/brief`
- **feasibility_status:** pending → see `feasibility-brief.md` in this change folder

## Recommendation

**C + persisted scope on Pi** (`/opt/marengo/var/…`), gateway CRUD with confirm-on-widen.

**PR slices (review budget ~400 lines each):**

1. IA + chrome rip (Telemetry route, redirect `/subsystems`, kill Actuators, strip bringup strings, fold #117 glossary)
2. Telemetry read-only (live table; strip commissioning modals)
3. Wire homing snapshot (proto + pi + gateway)
4. Hardware facets, scope UI, Enable-all-Ready-in-scope; go-to-zero not on Hardware
5. Scope persistence + post-restart Enable-all-Ready; Active never auto-restores
6. Testing defaults + remove `arm_4dof_right` hook fallbacks

## Risks

- Facet UI before wire homing repeats localStorage lies
- Scope vs `MARENGO_JOINT_SUBSET` must converge
- Enable move from Testing — operator habit + safety copy
- OutOfLimits may need Davout expose for badge priority

## Ready for Proposal

**Yes**, after light feasibility brief with **Go**. Proposal must lock proto fields, scope storage, Telemetry vs Hardware command matrix, and Testing residual (motion-only + E-stop).
