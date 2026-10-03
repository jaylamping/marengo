# crates/marengo-imu/

## Responsibility
**IMU driver** for BNO085 over I2C (SHTP / SH-2). Initializes the sensor, enables the rotation vector and parses reports into a normalized quaternion with an accuracy class. It does not publish: `marengo-pi` publishes `sensors/imu/torso`.

## Design
- `driver.rs`: `Bno085` session (reset, product ID, Set Feature, packet drain). `poll` returns only samples that arrived since the previous poll; a silent sensor yields `None`, never a re-stamped cached sample. `sample_seq` counts fresh samples. `initialize_while` lets the owner cancel the ~22 s worst-case init.
- `shtp.rs`: pure SHTP helpers (header parse, framing, batch split by report length, rotation parse). Zero-norm quaternions are dropped. The rotation-vector stride (12 vs 14 bytes, CS18) is unresolved pending a raw capture.
- `bus.rs`: 3-call `I2cBus` seam and errno classification (EREMOTEIO/EAGAIN = no packet; ENODEV and the rest = error).
- `i2c_linux.rs`: Linux backend behind the `linux-i2c` feature.
- `parse_i2c_address`: 7-bit hex address (`4b` or `0x4b`), shared by both bins.

## Integration
- **Consumed by**: `bins/imu-probe`, `bins/marengo-pi` (feature-gated `imu` module)

**Detailed map**: [src/codemap.md](src/codemap.md)
