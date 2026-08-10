# Verify Report: consul-hardware-commissioning

**Date**: 2026-08-09  
**Branch**: `jl/hardware-commissioning-cutover-6ab6`  
**Verdict**: **PASS WITH WARNINGS**

## Completeness

| Metric | Value |
|--------|-------|
| Tasks (1.1–6.5) | 34 / 34 complete |
| OpenSpec artifacts | exploration, feasibility, proposal, specs×4, design, tasks, apply-progress |

## Runtime gates (re-executed)

| Gate | Result |
|------|--------|
| `cd consul && npm test` | **PASS** — 54 files / 288 tests |
| `cargo test --workspace` | **PASS** (re-run this verify session) |
| `just check` / `check-native` | Not re-run in cloud; deferred to CI on PR #119 |
| Pi smoke (restart, scope, Enable, partial all-off) | **Deferred** → [#115](https://github.com/jaylamping/marengo/issues/115) |

## Success criteria

| Criterion | Result | Evidence |
|-----------|--------|----------|
| Badge priority Fault>OutOfLimits>Offline>Active>Ready>Online; Joint→Limb→Robot aggregation | PASS | `consul/src/lib/__tests__/commissioning.test.ts`; `marengo-homing` aggregation |
| Scope survives restart; confirm widen; ceiling ∩; exclude Fault/OutOfLimits | PASS unit / WARN Pi | `commissioning_scope` + gateway CRUD tests; Pi #115 |
| Active never auto-restores; scoped Enable; partial failure → disable all | PASS unit / WARN Pi | Davout `enable_targets` / partial-failure tests; Pi #115 |
| Telemetry read-only; Testing motion/E-stop only (no Enable/Home) | PASS | Telemetry + TestingOverview Vitest |
| Master chrome; proto + Rust + Consul | PASS tests / WARN full check | Proto fields + regen present; CI `just check` pending |

## Spec / design coherence

- Wire facets on `JointState` (`homing_state`, `drive_active`, `out_of_limits`) — present
- Scope at `/opt/marengo/var/commissioning-scope.yaml` + gateway CRUD + `confirm_widen` — present
- No-scope Enable requires Robot Ready; persisted scope enables Verified in-scope — present in Pi filter
- Enable-time `set_homing_complete` removed; `POST /command/home` → 410 — present
- Command matrix Hardware / Telemetry / Testing — followed
- Scope editor UI landed in Phase 6 (documented deviation from Phase 5 stub)

## Warnings (non-blocking)

1. Pi bench smoke deferred to #115 (re-zero + gravity path also blocked on this cutover).
2. Full `just check` / deny/audit not re-executed here — rely on CI.
3. Residual `arm_4dof_right` strings may remain in test fixtures; production defaults use `master`.
4. CLI `home` path on marengo-pi (if any) may still exist; operator gateway/Chappe Home retired.

## Recommendation

Ship PR #119 for human/CI review. After merge + deploy, run #115 operator smoke. Archive OpenSpec change after merge + CI green.
