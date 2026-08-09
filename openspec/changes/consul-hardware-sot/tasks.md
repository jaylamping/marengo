# Tasks: Consul Hardware Source of Truth

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~1,400–1,800 (four vertical PRs) |
| 400-line budget risk | High (per phase: SoT Medium, API High, UI High, deploy Medium) |
| Chained PRs recommended | Yes |
| Suggested split | PR1 SoT → PR2 API → PR3 UI → PR4 inventory/deploy |
| Delivery strategy | vertical-pr |
| Chain strategy | feature-branch-chain |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: feature-branch-chain
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Master SoT cutover | PR1 | Base `jl/hardware-openspec-5293`; `just check-native` gate |
| 2 | Gateway hardware API | PR2 | Base PR1 branch; gateway + marengo-config tests |
| 3 | Consul `/hardware` workspace | PR3 | Base PR2; `cd consul && npm test` |
| 4 | Inventory/deploy/MCP cutover | PR4 | Base PR3; sync smoke + MCP tests |

## Phase 1: SoT

- [ ] 1.1 Copy `config/bringup/arm_4dof_right/{robot,motors,control,homing}.yaml` → root `config/`; set `robot.urdf: assets/urdf/marengo.urdf`
- [ ] 1.2 Promote `assets/urdf/arm_4dof_right.urdf` → `assets/urdf/marengo.urdf`; archive placeholder + slice URDFs under `assets/urdf/archive/<upload-id>/` with `manifest.json`
- [ ] 1.3 Add `assets/urdf/staging/.gitkeep`; seed archive manifests per #108 (checksum, source, timestamp)
- [ ] 1.4 Delete `config/bringup/**` runtime SoT; update `config/AGENTS.md` + `codemap.md` for master-only tree
- [ ] 1.5 Default `MARENGO_CONFIG_DIR` to `/opt/marengo/config` in `marengo-config`, `bins/marengo-pi`, `scripts/env.example`
- [ ] 1.6 Remove bringup profile reads in `crates/marengo-config/src/bringup_presets.rs` and `bench_joints.rs`; delete or retire `bringup_presets.rs`
- [ ] 1.7 Retarget `profile_txn.rs` + `urdf_expand.rs` to master paths only (no inactive profile CAS)
- [ ] 1.8 Update `marengo-config` tests/fixtures off `bringup/` paths; RED tests for master boot resolution
- [ ] 1.9 GREEN: `cargo test -p marengo-config`; URDF validation script passes on promoted `marengo.urdf`
- [ ] 1.10 Update `docs/decisions/0012-config-db-overrides.md` disk SoT paths; gate: feasibility **Go** recorded before merge

**Phase 1 budget:** ~350–450 lines | Medium

## Phase 2: API

- [ ] 2.1 Create `crates/marengo-config/src/completeness.rs` — warn-only v1 rules (mass/COM, kinematics, hard limits, config coverage)
- [ ] 2.2 RED: `completeness` unit tests — missing mass, unmapped joint, limit gap; no `blocking` flag
- [ ] 2.3 Create `crates/marengo-config/src/urdf_merge.rs` — joint-name overlap, field diff, kinematics-critical detection (#108)
- [ ] 2.4 RED: `urdf_merge` fixture tests — axis/origin/parent conflicts require explicit resolution
- [ ] 2.5 GREEN: export modules from `marengo-config/src/lib.rs`; `cargo test -p marengo-config`
- [ ] 2.6 Create `bins/marengo-gateway/src/hardware.rs` — `GET /hardware/completeness`, `GET /hardware/urdf`
- [ ] 2.7 Add URDF lifecycle routes: upload → `staging/`, resolve-preview, activate, archive list/fetch/restore
- [ ] 2.8 Wire auth (`x-marengo-log-token`) on mutations; atomic activate writes `marengo.urdf` + archives contributor
- [ ] 2.9 Register routes in `http.rs`; delete `profiles.rs` and `/config/profiles*` from route table
- [ ] 2.10 Modify `config.rs` — master snapshot paths only; remove inactive profile apply paths
- [ ] 2.11 RED: gateway axum/temp-dir tests — unauthorized read rejected; activate archives; completeness advisory (upload not blocked)
- [ ] 2.12 GREEN: `cargo test -p marengo-gateway`; retire `profiles_tests.rs` or replace with hardware tests
- [ ] 2.13 Verify `POST /config/patch` unchanged for Set Limits (`persist_status` pending/durable/failed)

**Phase 2 budget:** ~500–650 lines | High

## Phase 3: UI

- [ ] 3.1 Create `consul/src/lib/hardware-api.ts` — completeness, URDF read/upload/resolve/activate/archive client
- [ ] 3.2 Add `/hardware` route in `consul/src/routes/**`; table-first layout with nav entry
- [ ] 3.3 Create `consul/src/components/dashboard/hardware/` — actuator/joint table, warn badges from completeness
- [ ] 3.4 Add optional Table/3D toggle (vanilla Three.js); 3D view read-only, no durable writes
- [ ] 3.5 Unified settings sheet — live actuator snapshot vs disk `GET /config/snapshot` labels (ADR 0012)
- [ ] 3.6 Import wizard — upload, field-level resolve picks, Accept → activate API, Cancel preserves master
- [ ] 3.7 Set Limits on Hardware sheet — `persistJointLimits` via `/config/patch`; ACTIVE guard; `persist_status` UI
- [ ] 3.8 Modify `persist-joint-limits.ts` — remove default `arm_4dof_right` profile; Hardware-only durable path
- [ ] 3.9 Strip durable Set Limits from `set-limits-panel.tsx` — read-only + deep-link to `/hardware`
- [ ] 3.10 Remove `fetchProfiles` / `applyActuatorConfig` consumers from `config-api.ts`
- [ ] 3.11 RED: Vitest — warnings do not disable Accept/Apply; Set Limits blocked when ACTIVE; Inventory no persist
- [ ] 3.12 GREEN: `cd consul && npm test && npm run build`

**Phase 3 budget:** ~450–600 lines | High

## Phase 4: inventory/deploy

- [ ] 4.1 Inventory panels read-only limits from live snapshot; deep-link to `/hardware` for calibration
- [ ] 4.2 Update `scripts/deploy-pi.sh`, `install-pi.sh`, `env.example` — `MARENGO_CONFIG_DIR=/opt/marengo/config`, master URDF paths
- [ ] 4.3 Update `scripts/pi-remote.sh`, `cloud-pi-lib.sh`, `homing-preflight.sh`, `profile-pi-loop.sh` off bringup defaults
- [ ] 4.4 Retarget `tools/marengo-pi-mcp/src/tools/sync-config.ts` — sync root `config/` + `assets/urdf/marengo.urdf`
- [ ] 4.5 Update MCP `pi_sync_bench_config` / `pi_sync_bench_urdf` descriptions and defaults; Durable-gated URDF sync (ADR 0017)
- [ ] 4.6 Add optional `MARENGO_JOINT_SUBSET` env — runtime joint allowlist in `bench_joints.rs` / marengo-pi overlay (ephemeral 3-DOF smoke)
- [ ] 4.7 Update `tools/marengo-pi-mcp/test/*.ts` — master paths, subset harness; `just mcp-build`
- [ ] 4.8 Integration: overlay write-behind order URDF-first on master paths (`overlay_tests` or marengo-pi tests)
- [ ] 4.9 Smoke checklist: deploy master tree, `pi_sync_main`, bench Enable/homing/gravity on 4-DOF (operator or Tailscale)

**Phase 4 budget:** ~350–450 lines | Medium
