# crates/marengo-imu/src/

## Responsibility
SHTP protocol implementation and I2C read loop.

## Design
- `shtp.rs`: primary module for BNO085 communication; `bus.rs` mock transport exists only under `cfg(test)`
