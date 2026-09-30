# Proposal: Consul Hardware Commissioning

## Intent
Make Hardware the honest commissioning surface, replacing bringup pages and localStorage readiness with runtime truth and scoped Enable.

## Proposal Question Round
Waived. Grilling #116 and issue #118 lock behavior; feasibility items are engineering locks below.

## Scope
### In Scope
- Keep Hardware; replace Inventory/Subsystems with read-only `/telemetry` (`/subsystems` redirects); remove Actuators.
- Replace `bench_4dof`/`arm_4dof_right` chrome and Testing defaults with master inventory.
- Hardware owns Set Limits, Set Zero, facets/Ready, commissioning scope, and Enable all Ready-in-scope.
- Testing retains motion, go-to-zero, and E-stop.

### Out of Scope
Motion redesign, Active restore, duplicate Enable, and physical changes.

## Capabilities
### New Capabilities
- `hardware-commissioning-state`: Presence/reference (Ready≡Verified)/drive/health facets, aggregation, scope, and scoped Enable.
- `live-hardware-telemetry`: Read-only live Telemetry.

### Modified Capabilities
- `hardware-operator-workspace`: Hardware ownership; Inventory/Actuators retirement.
- `hardware-management-api`: Scope read/apply/clear and scoped Enable.

## Approach
- Use Approach C. Add `JointHomingState homing_state = 7`, `bool drive_active = 8`, and `bool out_of_limits = 9` to `JointState`, mirroring `marengo-homing` on Chappe without split truth.
- Persist versioned scope at `/opt/marengo/var/commissioning-scope.yaml`. `MARENGO_JOINT_SUBSET` is the startup ceiling; effective scope is their intersection. Confirm apply/widen.
- Reuse `EnableRequest`: Pi selects Verified in-scope targets, excludes Fault/OutOfLimits, and Davout enables that set. Partial failure disables all. No scope requires Robot Ready and targets loaded master.
- Remove enable-time auto-`set_homing_complete`; derive Joint→anatomical Limb (offline/unbuilt do not block)→honest full-master Robot Ready. Retire `HomingComplete`/Testing Home shortcuts.
- v1 exposes OutOfLimits from verification or Davout bounds; unclassified safety failures fall back to Fault.
- Deploy wire before UI. Use ~400-line slices: (1) IA/chrome + glossary PR #117, (2) Telemetry, (3) wire, (4) facets/commands, (5) scope/Enable, (6) Testing/defaults.

## Affected Areas
| Area | Impact |
|---|---|
| `proto/`, homing, Davout | Joint truth; filtering |
| Pi, gateway | Publish, persistence, commands |
| `consul/src/`, `CONTEXT.md` | IA, facets, scope, glossary |

## Risks
- Mixed rollout can lie about Ready; gate UI on wire availability.
- Scope/FSM errors can energize unintended joints; validate on Pi.

## Rollback Plan
Reverse slices, UI before wire. Additive fields tolerate old clients. Disable all before runtime rollback; preserve but ignore scope; never restore Active.

## Dependencies
Hardware SoT contracts; feasibility locks; glossary PR #117; Rust/TS proto regeneration; wire-first deployment.

## Success Criteria
- [ ] Priority is Fault > OutOfLimits > Offline > Active > Ready > Online; locked aggregation holds.
- [ ] Scope survives restart, confirms apply/widen, obeys the ceiling, and excludes Fault/OutOfLimits.
- [ ] Active never restores; scoped Enable disables all on partial failure.
- [ ] Telemetry is read-only; Testing has motion/E-stop, not Enable/Home.
- [ ] Full-master chrome ships; proto, Rust, Consul, motion, and E-stop checks pass.
