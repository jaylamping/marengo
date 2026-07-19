# config/bringup/arm_3dof_right/

Active right-arm bench profile: shoulder roll (RS03 id 1), pitch (RS03 id 2),
upper-arm yaw (RS02 id 3) on `can0`.

| File | Role |
|------|------|
| `robot.yaml` | Joint list + URDF path (`arm_3dof_right.urdf`) |
| `motors.yaml` | CAN map, direction, bench position/torque caps |
| `control.yaml` | Impedance / trajectory / velocity groups |
| `homing.yaml` | Manual-reference zero verify for all three joints |
