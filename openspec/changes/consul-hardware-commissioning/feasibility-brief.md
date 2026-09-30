# Feasibility Brief: consul-hardware-commissioning

**Topic key:** `feasibility/consul-hardware-commissioning/brief`  
**Domains:** software, pi, control (robotics). No CAD / mech / EE layout change.

## Assumptions

- Grilling #116 / issue #118 lock Consul IA and facet/Ready/scope product rules.
- Hardware SoT (`consul-hardware-sot`) already cut over master config/URDF; this change is chrome + commissioning honesty.
- Motor path remains Berthier → Davout → robstride.
- Active does not persist across restart today (Davout boots Disabled).

## Risks

- Enable-all today enables every motor in Davout’s loaded set with no per-joint Verified/Fault filter.
- `handle_chappe_enable` can call `set_homing_complete()` before enable — conflicts with explicit Ready / Enable-all-Ready-in-scope.
- Facet UI before Chappe homing publish repeats localStorage lies.
- Scope vs `MARENGO_JOINT_SUBSET` precedence must be explicit.
- OutOfLimits may not be on wire yet for badge priority.

## Unknowns (must lock in proposal)

1. Proto shape for per-joint homing (on `JointState` vs snapshot message).
2. Scope storage path/schema and env-subset precedence.
3. Robot-wide Ready vs per-joint Verified; fate of `HomingComplete` / Testing Home.
4. OutOfLimits wire exposure vs map to Faulted.
5. Enable API: reuse `EnableRequest` with scope validation vs new command.
6. Testing residual command matrix after Enable moves to Hardware.
7. Fold glossary PR #117 terms to match wire enums.

## ExpertVerdicts

| Domain | Verdict | Notes |
|--------|---------|-------|
| robotics | **Revise** | Direction sound; pin enable/scope semantics and remove auto-`set_homing_complete` on enable before apply slices that energize. |
| mech / cad / ee / kinematics | N/A | No layout/URDF kinematics redesign in this change. |

## Decision

**Revise → proceed to propose** with the unknowns above as hard proposal locks (not open product questions). Not No-Go: stack already has homing registry, calibration persistence, subset filtering, and Disabled boot.
