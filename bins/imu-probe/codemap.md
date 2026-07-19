# bins/imu-probe/

## Responsibility
Read-only **BNO085 IMU hardware check** — samples rotation quaternions over I2C without enabling motors.

## Design
- Thin wrapper around `marengo-imu` SHTP driver
- CLI args: `--bus`, `--address`, `--samples`
- Used by MCP `pi_imu_probe` for bench validation

## Integration
- **Depends on**: marengo-imu (linux-i2c)

**Detailed map**: [src/codemap.md](src/codemap.md)
