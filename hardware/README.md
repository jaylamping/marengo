# Hardware

Physical robot design for Marengo: CAD, electrical, prints, and BOM. Mechanics and wiring are defined here first; software reads the exports.

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
| Humanoid rev-a (G1/R1-class biped, 1524 mm, 23 DOF) | Torso frame 125×150×380 mm committed; pelvis/shell draft — [`docs/kinematics.md`](docs/kinematics.md) |
| Arm (4 DOF, bring-up) | First subsystem wired; [`config/robot.yaml`](../config/robot.yaml) + [`assets/urdf/arm_4dof.urdf`](../assets/urdf/arm_4dof.urdf), subset of humanoid arm kinematics |

Full-body runtime templates (not loaded until remaining joints are commissioned): [`config/robot_humanoid.yaml`](../config/robot_humanoid.yaml), [`config/motors_humanoid.yaml`](../config/motors_humanoid.yaml).

## Revision

When you cut a build, record the mechanical/electrical revision here (e.g. `rev-a`, date, known deltas from prior rev).
