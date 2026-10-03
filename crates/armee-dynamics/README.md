# armee-dynamics

Gravity torques and dynamics helpers for Marengo control ([ADR 0005](../../docs/decisions/0005-dynamics-library.md)).

## API

- [`DynamicsModel::gravity_torques`](src/lib.rs) — feedforward for MIT `torque_ff`
- [`UrdfGravityModel`](src/lib.rs) — built from a URDF path + actuated joint list
- [`calibration`](src/calibration.rs) — fit link mass scale / COM offset to bench holding torques
  (`marengo-log-cli gravity-fit`; workflow in [limb playbook](../../docs/commissioning/limb-playbook.md#gravity-calibration-after-hardware-changes))
