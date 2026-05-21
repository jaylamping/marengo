# Hardware

Physical robot design for **Marengo** — CAD, electrical, prints, and BOM live here as the source of truth for mechanics and wiring.

## Layout

| Path | Purpose |
|------|---------|
| [`cad/`](cad/) | SolidWorks parts, assemblies, drawings, vendor STEP |
| [`electrical/`](electrical/) | PDB (KiCad), harness and CAN documentation |
| [`prints/`](prints/) | STL sources and slicer notes |
| [`bom/`](bom/) | Master BOM and sourcing |
| [`docs/`](docs/) | Kinematics, assembly, hardware ADRs |

## Active design

| Target | Status |
|--------|--------|
| **Humanoid rev-a** — G1-class biped, 1524 mm, 23 DOF | CAD + kinematics defined; see [`docs/kinematics.md`](docs/kinematics.md) |
| **Arm (4 DOF, bring-up)** | First subsystem wired; [`config/robot.yaml`](../config/robot.yaml) + [`assets/urdf/arm_4dof.urdf`](../assets/urdf/arm_4dof.urdf) — subset of humanoid arm kinematics |

Full-body runtime templates (not loaded until remaining joints are commissioned): [`config/robot_humanoid.yaml`](../config/robot_humanoid.yaml), [`config/motors_humanoid.yaml`](../config/motors_humanoid.yaml).

## Revision

Document the active mechanical/electrical revision here when you cut a build (e.g. `rev-a`, date, known deltas from prior rev).
