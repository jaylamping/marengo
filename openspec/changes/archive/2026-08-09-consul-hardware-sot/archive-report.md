# Archive Report: consul-hardware-sot

**Archived at**: 2026-08-09T15:51:00Z  
**Change**: consul-hardware-sot  
**Branch**: jl/hardware-openspec-5293  
**Verdict at archive**: PASS WITH WARNINGS

## Task Completion Gate

- Tasks artifact: 44/44 checked (`tasks.md`)
- apply_progress: 44/44 complete (4 vertical phases: SoT, API, UI, inventory/deploy)
- Stale-checkbox reconciliation: not required

## Spec Sync

| Domain | Action | Details |
|--------|--------|---------|
| hardware-description-sot | Created | Full spec copied to main (no prior main spec existed) |
| hardware-management-api | Created | Full spec copied to main (no prior main spec existed) |
| hardware-operator-workspace | Created | Full spec copied to main (no prior main spec existed) |

**Main spec paths**:

- `openspec/specs/hardware-description-sot/spec.md`
- `openspec/specs/hardware-management-api/spec.md`
- `openspec/specs/hardware-operator-workspace/spec.md`

Delta specs were complete specifications (not ADDED/MODIFIED sections). No destructive merge; no requirements removed.

## Archive Location

`openspec/changes/archive/2026-08-09-consul-hardware-sot/`

## Archive Contents

- proposal.md ✅
- feasibility-brief.md ✅
- specs/hardware-description-sot/spec.md ✅
- specs/hardware-management-api/spec.md ✅
- specs/hardware-operator-workspace/spec.md ✅
- design.md ✅
- tasks.md ✅ (44/44 complete)
- smoke-phase4.md ✅
- verify-report.md ✅
- state.yaml ✅
- archive-report.md ✅

## Verification Summary

- Final verdict: PASS WITH WARNINGS
- CRITICAL issues: none
- `./scripts/check.sh`: ok
- `cargo test -p marengo-gateway`: 33 pass
- `cd consul && npm test`: 240 pass
- `cd tools/marengo-pi-mcp && npm test`: 70 pass
- `./scripts/validate-urdf.sh`: pass

## Intentional Warnings (non-blocking)

Archive proceeded per orchestrator instruction; Pi smoke operator-deferred.

1. **Ephemeral subset** implemented as `MARENGO_JOINT_SUBSET` env, not HTTP subset endpoint (spec wording looser than design).
2. **Import wizard / archive browser** UI test coverage thinner than gateway lifecycle tests.
3. **`consul/src/lib/bringup-presets.ts`** may retain retired profile API comments — cosmetic.
4. **Pi smoke** (deploy, Enable, homing, gravity) not run in cloud — follow `smoke-phase4.md`.
5. Transient `urdf_merge` temp-path race fixed during apply; clippy `expect_used` removed in merge preview.

## Related Tickets

#96, #100, #108, #110, #111

## SDD Cycle

Fully planned, implemented, verified, and archived. Ready for next change.
