# Harness

Wire gauges, lengths, and routing between subsystems.

| Run | From | To | Gauge | Length | Notes |
|-----|------|-----|-------|--------|-------|
| imu_i2c | Pi 5 GPIO header | BNO085 GY-BNO085 | 28 AWG or ribbon | ≤30 cm | SDA pin 3, SCL pin 5, 3.3 V only; address **0x4b** (ADR high) |
