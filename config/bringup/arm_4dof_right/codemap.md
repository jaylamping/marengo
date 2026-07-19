# config/bringup/arm_4dof_right/

## Responsibility
Right 4-DOF bench profile: shoulder roll/pitch, upper-arm yaw, elbow pitch (RS02 id 4).

## Files

| File | Role |
|------|------|
| `robot.yaml` | Joint list + URDF path (`arm_4dof_right.urdf`) |
| `motors.yaml` | CAN map ids 1–4; elbow discovery envelope |
| `control.yaml` | Gains, danger zones, watchdog policy (disabled for mixed-sign bring-up) |
| `homing.yaml` | Manual-reference zeros for all four joints |
