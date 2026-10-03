# crates/marengo-imu/src/

## Responsibility
BNO085 SHTP session (`driver.rs`) over an `I2cBus` seam, with pure protocol helpers in `shtp.rs`.

## Design
- `driver.rs`: init/reset/feature enable and the packet drain loop; `poll` reports fresh samples only; inbound SHTP sequence gaps are logged at debug.
- `shtp.rs`: header/framing, report lengths for skipping, rotation-vector parse.
- `bus.rs`: `I2cBus` trait, errno classification; the mock transport exists only under `cfg(test)`.
- `types.rs`: `Quaternion` (`normalize_checked` refuses zero norm), `ImuAccuracy`, `RotationVectorSample`.
