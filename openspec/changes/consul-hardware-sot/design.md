# Design: Consul Hardware Source of Truth

## Technical Approach

Deliver the **full Hardware destination** (#110) as **one OpenSpec change** with **vertical PRs**: **SoT → API → UI → inventory/deploy**. Master durable description is root `config/{robot,motors,control,homing}.yaml` plus live `assets/urdf/marengo.urdf`; Pi mirror `/opt/marengo/config/` and `/opt/marengo/assets/urdf/`. Bringup profiles and `/config/profiles*` retire in the same campaign — no dual-read bridge.

Completeness v1 is **warn-only**, computed in **gateway + `marengo-config`**; Consul renders badges. Import is **full field-level resolve** with kinematics-critical explicit picks; **Accept = Active** master. Set Limits durable path moves to Hardware only; live semantics stay **ADR 0012** (memory SoT + YAML/URDF write-behind) and **ADR 0017** (expand-only URDF hard). Limb subsets are **ephemeral runtime filters** on master — never alternate URDF/config SoTs.

Proto-first only if HTTP JSON contracts need Chappe wire types; otherwise typed HTTP DTOs in gateway + `consul/src/lib/hardware-api.ts`.

## Architecture Decisions

| Decision | Choice | Rejected | Rationale |
|----------|--------|----------|-----------|
| Execution vehicle | One change; vertical PRs SoT→API→UI→deploy (#110) | Per-subsystem propose/apply cycles | Avoids split-brain cutover |
| Master YAML | Single cutover: collapse `arm_4dof_right` → root `config/`; delete bringup SoT | Symlink bridge; dual-read | #110 — no mixed writers |
| Day-1 URDF seed | Promote `arm_4dof_right.urdf` → `marengo.urdf`; archive placeholder + slices | Empty-master wizard gate | Bench enableable immediately |
| URDF on-disk layout | Live `marengo.urdf`; `staging/`; `archive/<upload-id>/` + `manifest.json` (#108) | Upload-only, no archive API | Seed from existing slice URDFs day one |
| Merge key | Actuated **joint names** vs master overlap + motors CAN map | Filename-based merge | #108 research |
| Kinematics conflicts | Explicit operator pick (axis, origin, parent, type) | Auto-pick incoming | Preserve operator intent |
| Completeness home | `marengo-config` rules + gateway `GET` endpoints | Pi boot fail; browser URDF parse | #110; shared Rust kinematics |
| Completeness UX | Warn-only everywhere | Hard HTTP/UI gates | #110 |
| Set Limits UI | Hardware sheet only; Inventory durable path removed | Dual persist UI | #110 |
| Live limits path | Reuse `POST /config/patch` → Chappe `LimitPatchCommand` | New motor/CAN shortcut | ADR 0012; Berthier→Davout→robstride |
| Inactive profile txn | Retire with `/config/profiles*` | Keep CAS inactive writers | Master-only durable edits |
| Limb subsets | Runtime joint allowlist overlay on master | Baked 3-DOF/4-DOF URDF variants | #110 |
| Profile registry API | Delete `profiles.rs` routes | Read-only profile inventory | #110 — git history for slices |

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Consul /hardware                                                         │
│  table · 3D toggle · settings sheet · import wizard · Set Limits        │
│  completeness badges (render only)                                       │
└───────────────┬───────────────────────────────┬─────────────────────────┘
                │ HTTP                           │ Chappe stream
                ▼                                ▼
┌───────────────────────────┐         ┌──────────────────────────────────┐
│ marengo-gateway           │         │ marengo-pi                        │
│ hardware/urdf/*           │         │ ActuatorOverlay                   │
│ hardware/completeness       │         │  LimitPatch → Davout (memory)     │
│ config/snapshot (master)  │◄─Chappe─│  ConfigPersistQueue (write-behind)│
│ config/patch (limits)     │         │   URDF → motors → control         │
│ [profiles* RETIRED]       │         └──────────────────────────────────┘
└─────────────┬─────────────┘
              │ reads/writes
              ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Durable tree (repo + Pi)                                                 │
│ config/{robot,motors,control,homing}.yaml  → robot.urdf = marengo.urdf   │
│ assets/urdf/marengo.urdf (live)                                          │
│ assets/urdf/staging/ · assets/urdf/archive/<upload-id>/                  │
└─────────────────────────────────────────────────────────────────────────┘
              ▲
              │ validate · completeness · merge
┌─────────────┴─────────────┐
│ marengo-config            │
│ completeness.rs · urdf_merge.rs (new)                     │
│ profile_txn retired for inactive profiles                 │
└───────────────────────────┘
```

## Vertical Phases

| Phase | Scope | Gate |
|-------|-------|------|
| **1 SoT** | Seed root YAML from `arm_4dof_right`; `robot.urdf` → `marengo.urdf`; archive slice URDFs; default `MARENGO_CONFIG_DIR=/opt/marengo/config`; remove bringup runtime reads; feasibility **Go** before merge | `marengo-pi` boots 4-DOF bench from master paths; URDF validation passes |
| **2 API** | `hardware/completeness`; URDF read/upload/staging/resolve-preview/activate/archive list/fetch/restore; retire `/config/profiles*` + inactive `apply` paths; unit tests in gateway + marengo-config | Gateway + config crate tests green |
| **3 UI** | `/hardware` route; table + 3D toggle; settings sheet; import wizard; Set Limits + `persistJointLimits`; Inventory strips durable Set Limits | `cd consul && npm test` |
| **4 inventory/deploy** | Inventory read-only/deep-link; `deploy-pi.sh`, `install-pi.sh`, `env.example`, `pi-remote.sh`; MCP `pi_sync_*` master paths; optional `MARENGO_JOINT_SUBSET` for harness | Bench sync smoke; MCP tests updated |

## Sequence: Import → Resolve → Activate

```mermaid
sequenceDiagram
    participant Op as Operator
    participant C as Consul Hardware
    participant G as marengo-gateway
    participant M as marengo-config
    participant FS as assets/urdf/

    Op->>C: Upload limb URDF or pick archive
    C->>G: POST /hardware/urdf/upload (multipart)
    G->>FS: Write staging/<upload-id>/contributor.urdf
    G->>M: Parse + joint-name overlap vs master + motors
    G-->>C: upload_id + field diff badges

    Op->>C: Resolve kinematics-critical picks
    C->>G: POST /hardware/urdf/resolve-preview (choices)
    G->>M: Simulate merge (no durable write)
    G-->>C: Resolved preview + remaining warnings

    Op->>C: Accept
    C->>G: POST /hardware/urdf/activate {upload_id, resolutions}
    G->>M: merge_by_joint_names → new master model
    G->>FS: Atomic: write marengo.urdf; move contributor to archive/<id>/
    G->>FS: Write manifest.json (source, checksums, resolutions)
    G-->>C: ok + completeness snapshot
    Note over Op,C: Accept = Active on disk; structural URDF change → NeedsRestart (ADR 0012)
    Op->>C: Restart marengo-pi if prompted
```

## Sequence: Set Limits Write-Behind (ADR 0012 + 0017)

```mermaid
sequenceDiagram
    participant Op as Operator
    participant C as Consul Hardware sheet
    participant G as marengo-gateway
    participant Ch as Chappe
    participant Pi as marengo-pi overlay
    participant D as Davout
    participant Q as ConfigPersistQueue
    participant FS as config/ + marengo.urdf

    Op->>C: Sweep joint; Apply limits
    C->>C: hard ± margin; soft inset (ADR 0009)
    C->>G: POST /config/patch {joint, pos_*, soft_*}
    G->>Ch: LimitPatchCommand (live)
    Ch->>Pi: OperatorCommand
    Pi->>D: apply_limit_patch
    Note over D: expand-only URDF hard in memory (ADR 0017)
    D-->>Pi: ok + rebuilt JointLimitPolicy
    Pi-->>Ch: ACK persist_status=pending
    Ch-->>G: ActionAck
    G-->>C: ok, persist_status=pending|durable

  par Async write-behind
    Pi->>Q: enqueue URDF + motors + control
    Q->>FS: URDF expand (coalesce-safe vs full motors snapshot)
    Q->>FS: motors.yaml + control.yaml
    Q-->>Pi: durable | failed
    Pi-->>Ch: limit_patch_persist ACK
  end

    C->>G: GET /snapshot/actuator/limits (invalidate live Range)
    Note over C: Durable only → optional marengo-limit-sync local checkout
```

## Data Flow (Completeness)

```
master YAML + marengo.urdf
        │
        ▼
marengo-config::completeness_report()
  · mass/COM present per link
  · joint kinematics vs motors map
  · hard limits: URDF ∩ motors bench
  · config coverage: robot.joints ↔ motors ↔ control
        │
        ▼
GET /hardware/completeness → warn[] (never blocks)
        │
        ▼
Consul row/sheet badges (no gate on upload/activate/Set Limits)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `config/robot.yaml`, `motors.yaml`, `control.yaml`, `homing.yaml` | Modify | Collapse `bringup/arm_4dof_right` content; `urdf: assets/urdf/marengo.urdf` |
| `config/bringup/**` | Delete | Remove bringup SoT (#110); history in git |
| `assets/urdf/marengo.urdf` | Modify | Promote from `arm_4dof_right.urdf` |
| `assets/urdf/archive/**` | Create | Seed manifests for placeholder + slice URDFs |
| `assets/urdf/staging/` | Create | Empty dir placeholder (gitkeep) |
| `crates/marengo-config/src/completeness.rs` | Create | Warn-only rules v1 |
| `crates/marengo-config/src/urdf_merge.rs` | Create | Joint-keyed merge + field diff (#108) |
| `crates/marengo-config/src/bringup_presets.rs` | Delete/retire | Remove `BRINGUP_PROFILE_SLUGS` SoT |
| `crates/marengo-config/src/profile_txn.rs` | Modify | Master-only paths; drop inactive profile CAS |
| `crates/marengo-config/src/bench_joints.rs` | Modify | Master allowlist + optional runtime subset filter |
| `bins/marengo-gateway/src/hardware.rs` | Create | Completeness + URDF lifecycle routes |
| `bins/marengo-gateway/src/profiles.rs` | Delete | Retire profile registry + inactive apply |
| `bins/marengo-gateway/src/http.rs` | Modify | Route table: `hardware/*`; remove `profiles*` |
| `bins/marengo-gateway/src/config.rs` | Modify | Master snapshot; patch targets active config dir only |
| `proto/marengo/v1/marengo.proto` | Maybe | Only if completeness/URDF events need Chappe |
| `consul/src/routes/**` | Modify | Add `/hardware` |
| `consul/src/lib/hardware-api.ts` | Create | URDF + completeness HTTP client |
| `consul/src/components/dashboard/hardware/**` | Create | Table, 3D toggle, sheet, wizard, Set Limits |
| `consul/src/lib/persist-joint-limits.ts` | Modify | Call from Hardware; drop `profile` default `arm_4dof_right` |
| `consul/src/components/dashboard/inventory/set-limits-panel.tsx` | Modify | Remove or read-only + deep-link |
| `consul/src/lib/config-api.ts` | Modify | Remove `fetchProfiles` / `applyActuatorConfig` consumers |
| `scripts/deploy-pi.sh`, `install-pi.sh`, `env.example` | Modify | `MARENGO_CONFIG_DIR=/opt/marengo/config` |
| `tools/marengo-pi-mcp/src/tools/sync-config.ts` | Modify | Sync root `config/` + master URDF |
| `docs/decisions/0012-config-db-overrides.md` | Modify | Update disk SoT paths (master tree) |
| `config/AGENTS.md`, `codemap.md` | Modify | Document master SoT |

## Interfaces / Contracts

Gateway URDF v1 (auth: `x-marengo-log-token` on mutations):

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/hardware/urdf` | Live `marengo.urdf` bytes + revision/checksum |
| POST | `/hardware/urdf/upload` | → `staging/<upload-id>/`; returns diff summary |
| POST | `/hardware/urdf/resolve-preview` | Field-level merge simulation |
| POST | `/hardware/urdf/activate` | Durable master + archive contributor |
| GET | `/hardware/urdf/archive` | List upload-ids + manifest metadata |
| GET | `/hardware/urdf/archive/{id}` | Fetch archived contributor + manifest |
| POST | `/hardware/urdf/archive/{id}/restore` | Staging restore for re-resolve |
| GET | `/hardware/completeness` | `{ warnings: CompletenessWarning[] }` — never `blocking: true` |

`CompletenessWarning`: `{ code, severity: "warn", joint?, link?, message }`.

Runtime subset (phase 4): `MARENGO_JOINT_SUBSET` comma-list filters `CommandJointAllowlist` without editing master YAML.

Set Limits: keep existing `ConfigPatchDto` / `ConfigPatchResultDto` (`persist_status`: `pending` | `durable` | `failed`).

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | `urdf_merge` field diff + kinematics-critical detection | marengo-config tests with fixture URDFs |
| Unit | `completeness_report` warn-only | Missing mass, unmapped joint, limit gap |
| Unit | Gateway route auth + activate atomic write | axum tests with temp dirs |
| Integration | Limit patch write-behind order URDF-first | Existing `overlay_tests` + master paths |
| Component | Import wizard resolve + Accept | Vitest + mocked `hardware-api` |
| Component | Set Limits on Hardware; Inventory no Apply | Vitest regressions |
| Smoke | Deploy + `pi_sync_*` master | MCP test harness updates |

## Migration / Rollout

1. Record feasibility **Go** in `feasibility-brief.md` before phase 1 merge.
2. Pre-cutover: Pi snapshot (in-memory limits + durable YAML/URDF checksum manifest via archive API or dated copy) — proposal rollback plan.
3. Ship phases as sequential PRs; **never** run legacy profile writers against new master paths in parallel.
4. Post-cutover: `pi_restart_marengo_pi` after URDF activate; verify Enable, homing, gravity on 4-DOF bench.
5. Durable Set Limits: sync local checkout after `durable` ACK (ADR 0017) before deploy to avoid URDF clobber.

## Risks

| Risk | Mitigation |
|------|------------|
| Split-brain during cutover | Single release train; atomic FS writes; no dual SoT |
| Deploy clobbers URDF expand (ADR 0017) | Durable-gated sync; pull-before-deploy |
| Bad merge alters kinematics | Staging-first; critical field picks; archive every contributor |
| Completeness noise | v1 warn-only; tune rules in gateway |
| Import Accept without restart | UI prompt NeedsRestart on structural URDF delta (ADR 0012) |
| MCP harness regression | Phase 4 explicit; `MARENGO_JOINT_SUBSET` for 3-DOF smoke |

## Open Questions

- [ ] Exact HTTP path prefix: `/hardware/*` vs `/config/hardware/*` (prefer `/hardware/*` for clarity; align in spec).
- [ ] Whether phase 1 deletes `config/bringup/` entirely or leaves a README pointer — #110 says delete SoT, not necessarily delete folder; recommend delete tree + doc link to git history.
