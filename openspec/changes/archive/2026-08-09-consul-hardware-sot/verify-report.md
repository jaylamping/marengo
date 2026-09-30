# Verify Report: consul-hardware-sot

**Date:** 2026-08-09  
**Branch:** `jl/hardware-openspec-5293`  
**Verdict:** **PASS WITH WARNINGS**

## Summary

All four apply phases are complete. Host-native CI gate `./scripts/check.sh` reports `check: ok`. Pi Enable/homing/gravity smoke remains operator-deferred per `smoke-phase4.md` (not a verify fail).

## Gates executed

| Gate | Result |
|------|--------|
| `./scripts/check.sh` | **ok** (aarch64 cross-build warn non-fatal: missing aarch64 sysroot headers in this VM) |
| `cargo test -p marengo-config` | pass (included in check) |
| `cargo test -p marengo-gateway` | 33 pass |
| `cargo test -p marengo-pi -- overlay` | 14 pass |
| `cargo test -p davout` / `berthier` / `armee-dynamics` / `marengo-homing` | pass after master joint-name/path retarget |
| `cd consul && npm test` | 240 pass |
| `cd consul && npm run build` | green (via check) |
| `cd tools/marengo-pi-mcp && npm test` | 70 pass |
| `./scripts/validate-urdf.sh` | pass |

## Spec compliance (runtime + static)

- **hardware-description-sot:** Master `config/` + `marengo.urdf`; bringup deleted; completeness warn-only; ADR 0012/0017 paths covered by overlay + config tests.
- **hardware-management-api:** Gateway `/hardware/*` routes + auth + activate archive tests green; multipart upload and archive list/restore dedicated tests still light.
- **hardware-operator-workspace:** `/hardware` table + Set Limits + inventory read-only tests green; 3D toggle / full import Accept-Cancel Vitest coverage still thin.

## Warnings (non-blocking)

1. **Ephemeral subset** is `MARENGO_JOINT_SUBSET` env (design), not an HTTP subset endpoint (spec wording looser than design).
2. **Import wizard / archive browser** UI coverage is thinner than gateway lifecycle tests.
3. **`consul/src/lib/bringup-presets.ts`** may still mention retired profile API in comments — cosmetic.
4. **Pi smoke** (deploy, Enable, homing, gravity) not run in cloud — follow `smoke-phase4.md`.
5. Transient `urdf_merge` temp-path race fixed (unique temp files); clippy `expect_used` removed in merge preview.

## Recommendation

**Archive** after human review of PR #112 and operator bench smoke. No apply blockers remain for software gates.
