# Feasibility Brief: consul-hardware-sot

**Change:** `consul-hardware-sot`  
**Date:** 2026-08-09  
**Scope:** Software + Pi bench config/URDF SoT cutover (no new SolidWorks CAD)  
**Locked decisions:** grilling [#110](https://github.com/jaylamping/marengo/issues/110)  
**Research:** [#108](https://github.com/jaylamping/marengo/issues/108) → `docs/research/wayfinder/limb-master-urdf-merge.md` (on branch `research/limb-master-urdf-merge`; not yet in this tree)

## Assumptions

1. **Single master SoT** — Root `config/{robot,motors,control,homing}.yaml` and `assets/urdf/marengo.urdf` become the only durable hardware description; `config/bringup/*` is retired as runtime SoT and gateway `/config/profiles*` is removed in the same campaign (no dual-read bridge).
2. **Day-1 seed** — Promote `assets/urdf/arm_4dof_right.urdf` → live `marengo.urdf`; archive placeholder `marengo.urdf` and slice URDFs via the archive API so the 4-DOF right bench enables immediately without an empty-master gate.
3. **YAML lift** — Collapse `config/bringup/arm_4dof_right/` into root `config/` (right-arm joint names, CAN map, taught limits), replacing today's stale root defaults (`shoulder_pitch` / left IDs in `config/motors.yaml`, `arm_4dof.urdf` pointer in `config/robot.yaml`).
4. **Pi layout** — `MARENGO_CONFIG_DIR` defaults to `/opt/marengo/config/` (master tree); URDF lives under `/opt/marengo/assets/urdf/` with `staging/`, `archive/<upload-id>/`, and `manifest.json` per research layout.
5. **ADR 0012 unchanged** — Live limit SoT remains Davout in-memory; YAML/URDF on disk are boot seed + write-behind after Durable ACK; completeness and import never hard-block HTTP or Enable.
6. **ADR 0017 on master** — Set Limits expand-only widens hard limits on `marengo.urdf`; persist is URDF-first then motors + control; deploy/sync must be Durable-gated to avoid clobbering Pi expands.
7. **Completeness v1** — Computed in `marengo-gateway` / `marengo-config` (mass/COM, kinematics alignment, hard limits, config coverage); Consul renders warn-only badges.
8. **Ephemeral subsets** — Testing, MCP harness, and smoke choose joint subsets at runtime against master YAML + master URDF; no alternate slice URDFs or profile SoTs; `sim/fixtures/` remain non-SoT CI fixtures.
9. **Motor path frozen** — Berthier → Davout → robstride; no CAN shortcuts; import/merge hot-reloads `Supervisor.urdf_robot` when motors are not Enabled (per #98 gate).
10. **Vertical PR order** — SoT → API → UI → inventory/deploy within one OpenSpec change; each phase gated by tests before merge.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| **Split-brain during cutover** — mixed old/new writers (bringup paths, profile txn, Inventory Set Limits, `pi_sync_bench_*`) | High | Enable faults, wrong limits, deploy clobber | Single coherent release per phase; atomic disk writes; checksum manifest; rollback restores gateway + Consul + marengo-pi + sync tools together |
| **Deploy clobbers Pi URDF expand** (ADR 0017) | Medium | Enable trips after taught limits | Durable-gated `pi_sync_bench_urdf` → master; pull-before-deploy; archive restore API |
| **Root vs bringup divergence today** — root config does not match active bench | High (today) | Bad seed if collapse is partial | Explicit Phase 1 task: copy `arm_4dof_right` YAML wholesale; validate with existing `marengo-config` tests |
| **URDF merge / resolve wizard scope** — no merge code today; only `urdf_expand.rs` | Medium | Schedule slip in API/UI phases | Research algorithm is specified; implement `marengo-config` diff/patch module with fixture tests before Consul wizard |
| **Kinematics-critical bad merge** — axis/origin/parent picks wrong | Medium | FK/gravity wrong, silent τ_g error | Import gate + explicit picks for critical fields; completeness warnings; archive every accepted contributor |
| **Retiring bringup regression profiles** — `arm_3dof_right`, weighted slices | Medium | Lost CI/regression paths | Ephemeral 3-joint subset on master; keep history in git only; update gateway tests off `profiles_tests.rs` |
| **Completeness false positives** | Low | Operator noise | v1 warn-only; tune rules in gateway without UI gates |
| **Large single-change surface** — full Hardware destination | Medium | Integration defects | Vertical PRs with phase gates; do not merge UI before API contract is stable |

## Unknowns

1. **Exact completeness rule set for v1** — [#99](https://github.com/jaylamping/marengo/issues/99) field priority is researched but not codified in `marengo-config` yet; first gateway implementation will need a minimal rule table and iteration.
2. **Proto vs HTTP-only** for URDF/completeness wire types — proposal marks proto as "maybe"; decision can wait until API design lands without blocking SoT cutover.
3. **Left/right generic bench names** during future humanoid merge — import gate alias table vs one-time rename (deferred in research).
4. **Mesh bundle merge** — v1 assumes primitives-only Consul uploads; `package://` mesh archives deferred.
5. **Whether `profile_content_revision` CAS extends to master URDF hash** — YAML-only revision today; gateway CAS must be extended or split for Config/Setup edits.
6. **Pi bench verification in cloud** — this VM cannot run marengo-pi MCP motion; Phase 1/4 bench gates rely on operator or Tailscale smoke after deploy.

## ExpertVerdicts

### Kinematics (expert-kinematics lens)

**Verdict: Go with Phase 1 seed discipline**

- Promoting `arm_4dof_right.urdf` → `marengo.urdf` is kinematically consistent with the active bench (`right_*` joints, axes documented in slice URDF header). Today's `marengo.urdf` placeholder (2-DOF skeleton) is not bench-truth and must not ship as live master without promotion.
- Research merge algorithm (joint-name actuator identity, kinematic units, critical-field picks) aligns with `armee-kinematics::actuated_joint_names` and `hardware/docs/kinematics.md` direction. No merge implementation exists yet — expected new work, not a spec contradiction.
- Gap: slice URDFs use inconsistent naming/axes (`arm_4dof.urdf` vs `arm_4dof_right.urdf`); retiring bringup removes competing SoTs but does not auto-normalize historical slices — archive + master promotion handles day-1; future imports use wizard.
- `resolve_urdf_path` + `robot.joints` subset model is sufficient; no second resolver needed.

### Robotics (expert-robotics lens)

**Verdict: Go — motor path and safety boundaries hold**

- Change does not alter Berthier → Davout → robstride; Set Limits remains gateway → Chappe `LimitPatchCommand` → Davout `apply_limit_patch` with expand-only URDF mutation.
- ADR 0012 contract preserved: Enable/hard faults = in-memory `URDF ∩ motors.yaml`; Consul Range reads actuator snapshot, not disk alone.
- ADR 0017 retarget to master URDF is a path-resolution change in persist coordinator, not a policy change; coalesce-safe expand from full motors snapshot already exists in `marengo-config`.
- Highest operational risk is deploy/sync clobbering taught URDF — mitigated by Durable-gated sync and rollback plan in proposal; not a No-Go on architecture.
- Ephemeral joint subsets must not bypass Davout enable filtering — existing motor map + `robot.joints` eligibility already enforce this.

### Mech / CAD / EE

**Not invoked** — no new SolidWorks CAD, harness, or CAN topology in this change. Electrical CAN map is copied from existing `arm_4dof_right/motors.yaml`.

## **Go**

Proceed to OpenSpec design/apply with vertical PRs **SoT → API → UI → inventory/deploy**.

**Rationale:** Business choices are locked (#110); URDF merge and Pi layout are researched (#108); ADRs 0012 and 0017 already define runtime limit semantics and expand-only bench envelopes on the active URDF file — this change retargets that file to `marengo.urdf` and collapses bringup without introducing motor-path shortcuts. Residual risks (split-brain cutover, root YAML divergence, new merge module) are implementation and release-discipline problems with explicit mitigations, not feasibility blockers. Phase 1 must promote bench URDF + lift `arm_4dof_right` YAML before API completeness or merge endpoints can be meaningfully tested.
