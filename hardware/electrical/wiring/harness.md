# Harness

Wire gauges, lengths, and routing between subsystems.

| Run | From | To | Gauge | Length | Notes |
|-----|------|-----|-------|--------|-------|
| imu_i2c | Pi 5 GPIO header | BNO085 GY-BNO085 | 28 AWG or ribbon | ≤30 cm | SDA pin 3, SCL pin 5, 3.3 V only; address **0x4b** (ADR high) |

## Bench harnesses

- [Left shoulder bench harness](left-shoulder-bench-harness.md) — left pitch RS03, roll RS03, upper-arm yaw RS02 on `can1` (plan, 2026-11-05)
- [Left arm wiring trunk plan](left-arm-trunk-plan.md) — robot harness architecture for all five left-arm motors: junction boards, joint jumpers, fusing (planning draft, 2026-11-06)
- [Left arm trunk, option B build spec](left-arm-trunk/README.md) — chosen 2026-11-06: spliced trunk, inline drop fuses, joint breaks; wiring diagram and cut list
