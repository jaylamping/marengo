# Hardware

Physical robot design for Marengo: CAD, electrical, prints, and BOM. Mechanics and wiring are defined here first; software reads the exports.

## Layout

| Path | Purpose |
|------|---------|
| [`../cad/`](../cad/) | SolidWorks parts, assemblies, vendor STEP, MCP manifests |
| [`electrical/`](electrical/) | PDB (KiCad stub), harness and CAN documentation |
| [`prints/`](prints/) | STL sources and slicer notes |
| [`bom/`](bom/) | Master BOM and sourcing |
| [`docs/`](docs/) | Kinematics (joint SSOT), torso design, assembly, homing sensors, CAD standards, hardware ADRs |

## Active design

| Target | Status |
|--------|--------|
| Humanoid rev-a (G1/R1-class biped, 1524 mm, 23 DOF) | Torso frame 125×150×380 mm committed; pelvis/shell draft — [`docs/kinematics.md`](docs/kinematics.md) |
| Right arm (5 DOF, live bench) | First subsystem wired and commissioning: [`config/robot.yaml`](../config/robot.yaml) + [`assets/urdf/marengo.urdf`](../assets/urdf/marengo.urdf) (geometry and inertials from CAD, 2026-10-03). It is the first slice of the humanoid arm, not a separate robot |
| Electrical | Pi 5 + 2-CH MCP2515 CAN HAT + BNO085 IMU on the bench. PDB/PDU KiCad project is an empty stub; 48 V pack and full harness are draft |

Full-body runtime templates (not loaded until remaining joints are commissioned): [`config/robot_humanoid.yaml`](../config/robot_humanoid.yaml), [`config/motors_humanoid.yaml`](../config/motors_humanoid.yaml).

## Revision

When you cut a build, record the mechanical/electrical revision here (e.g. `rev-a`, date, known deltas from prior rev).

| Rev | Date | Scope |
|-----|------|-------|
| rev-a (in progress) | 2026 | 2020 upper-torso cage committed; right 5-DOF arm built and on the bench; everything else draft |
