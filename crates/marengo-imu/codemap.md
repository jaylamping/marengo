# crates/marengo-imu/

## Responsibility
**IMU driver** for BNO085 over I2C (SHTP protocol). Publishes orientation data for future balance/estimation.

## Design
- `shtp.rs`: SHTP packet framing and BNO085 register access
- Linux I2C backend behind `linux-i2c` feature
- Used by `bins/imu-probe` (read-only hardware check) and `marengo-pi` (optional telemetry)

## Integration
- **Consumed by**: `bins/imu-probe`, `bins/marengo-pi` (feature-gated `imu` module)

**Detailed map**: [src/codemap.md](src/codemap.md)
