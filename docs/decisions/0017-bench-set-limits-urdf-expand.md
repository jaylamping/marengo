# ADR 0017: Bench Set Limits may expand URDF hard envelopes

**Status:** Accepted  
**Date:** 2026-07-22  
**Supersedes (partially):** ADR 0009 alternative “Raise hard limits only — rejected”

## Context

Davout hard limits are `URDF ∩ motors.yaml` bench ([ADR 0009](0009-dynamic-position-limit-envelope.md)). Consul Set Limits taught motors.yaml live via [ADR 0012](0012-config-db-overrides.md), but never widened URDF. Operators saw Apply succeed while Enable still tripped on the URDF clamp.

ADR 0009 rejected raising hard limits alone because CAD/URDF was treated as immutable mechanical truth. Bench bringup URDFs are discovery envelopes, not final CAD export.

## Decision

1. **Bench Set Limits Apply** (active live path and inactive `upsert_joint_limits`) **expand-only** widens the profile URDF `<limit>` hard envelope when taught hard exceeds current URDF hard. Soft operator bounds go to `control.yaml` with ADR 0009 inset (~27 mrad), not soft≡hard.
2. **Live path:** mutate in-memory `urdf_robot` in Davout `apply_limit_patch`, then async write-behind motors + control + URDF. Persist is URDF-first then YAML; Durable only when all succeed. Overlay enqueue rollback restores URDF + motors + control.
3. **Coalesce-safe disk URDF:** derive expands from the full motors snapshot vs on-disk URDF (not a per-joint payload that latest-wins can drop).
4. **Local git checkout:** optional Durable-gated sync via `marengo-limit-sync` + loopback `tools/limit-sync-local` — same Rust helpers, never on Pending alone.
5. **ADR 0009** remains the source of truth for velocity-scaled **margins** and the hard/soft gap. This ADR owns **bench URDF expand** as operator envelope, not CAD kinematics SoT for the full humanoid.

## Consequences

- Taught ranges past the old URDF hard envelope become effective after Apply without restart.
- Expand-only is a one-way ratchet on URDF hard; shrink uses motors∩URDF.
- Deploy/`pi_sync_bench_urdf` from a checkout that never received the expand can clobber Pi URDF — sync local after Durable or pull from Pi before deploy. `install-pi.sh` also merge-preserves previous Pi taught motors hard, control soft, and expand-only URDF union unless `MARENGO_REPLACE_LIMITS=1`.
- Full-humanoid CAD URDF workflow stays out of Set Limits scope.

## References

- [ADR 0009](0009-dynamic-position-limit-envelope.md)
- [ADR 0012](0012-config-db-overrides.md)
- [docs/safety.md](../safety.md)
