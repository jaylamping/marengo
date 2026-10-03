# Handoff 2026-10-03: right-arm gravity check and bench hold (Pi session)

Move to the macOS host. The CAD work from [handoff-2026-10-03-right-arm-urdf.md](handoff-2026-10-03-right-arm-urdf.md) is done on branch `claude/right-arm-urdf-from-cad` (not merged to `main`):

```bash
git fetch origin && git switch claude/right-arm-urdf-from-cad
```

## Goal

Load the CAD-derived URDF on the Pi, confirm the gravity model, then re-run the bench hold that failed on 2026-10-03 (handoff steps 4–5).

## What changed

- `903c5a2` **URDF.** `assets/urdf/marengo.urdf` now has right-arm joint origins and link inertials from CAD, plus the Pi's taught hard limits (read from `/opt/marengo/assets/urdf/marengo.urdf` on 2026-10-03). Axes and soft limits are unchanged. Derivation, masses and assumptions are in [right-arm-urdf-from-cad-2026-10-03.md](right-arm-urdf-from-cad-2026-10-03.md).
- `2ae3fb5` **Tests.** Four tests read URDF limits and masses from the live file instead of literals. They had hard-coded the old elbow `-0.50 / 1.2` and a 0.3 kg link.
- **Checks.** `just check` (Docker) and `./scripts/validate-urdf.sh` pass on this branch.
- **No MCP source changes.** No `just mcp-build` needed for this branch.

## Steps

1. **Sync.** If Set Limits was applied on the Pi after 2026-10-03, pull the Pi URDF first and keep its limits (ADR 0017). Then run `pi_sync_bench_urdf` (default `marengo.urdf`, `install_to_opt: true`).
2. **Rest check.** Run `pi_gravity_preview` with `angles: [0, 0, 0, 0, 0]`. Order is pitch, roll, upper yaw, elbow, lower yaw (`config/robot.yaml`); always pass all five, because a partial vector becomes all zeros. Expect about `+0.034, -0.026, 0, -0.003, 0` Nm. Every joint must be under 0.20 Nm.
3. **Elevated check.** Run `pi_gravity_preview` with `angles: [0.48, 0, 0, 0, 0]`. Expect about `+1.34` pitch and `+0.16` elbow, with pitch rising as pitch goes up (`[1.57, 0, 0, 0, 0]` gives about +2.83).
4. **Hold.** Support the arm at mechanical zero, then run `pi_hold_on` with `set_zero: true`, `at_mechanical_reference: true`, profile `arm_attached`. The gate (`gravity_model_mismatch`) should pass. Afterwards run `pi_candump_summary` and `pi_logs_last_fault`.
5. **Elevated hold.** Do a supported hold near pitch 0.48 and compare drive torque with the +1.34 Nm model. The 2026-08-12 reference was about +0.57 Nm; possibly without the forearm stack, which the model puts at about 0.7. If the residual is over 0.20 Nm, weigh the printed parts and redo the link masses from the per-part table in the derivation doc.
6. **Merge.** Merge the branch to `main` once the hold is clean.

## State at handoff

- **Pi** (`pi_health`, 2026-10-03 ~07:50Z):
  - deploy `123a433`
  - `can0`/`can1` up, gateway healthy
  - no `marengo-pi` or `motor-repl` running
  - all five joints Unhomed (no reference grant held), calibration record present
  - This session only read the Pi URDF; nothing on the Pi was changed.
- **CAD.** Read only; nothing saved.
- **SolidWorks MCP** (`feat/part-modeling`, pushed):
  - `solidworks_body_mass_properties` added; the mass tools now warn about hidden bodies
  - `export_link_transforms` writes `output_path`
  - On the Windows host, Claude Code now loads `solidworks` and `marengo-pi` MCP at user scope.

## Open

- **Lower-arm yaw axis.** In CAD the RS00 axis runs along the forearm (Z at q = 0); the URDF says Y. It doesn't matter at rest; fix it after a sign check.
- **Roll and elbow stops.** The CAD stops sit near zero: roll 1.46° abducted, elbow 5.3° flexed. If zero is set against them, rest τ_g is off by up to ~0.07 Nm. That's still inside the gate.
- **Unmodelled mass.** Printed-part masses are estimates. Fasteners, inserts and cables (~0.1 kg) are not modelled.
