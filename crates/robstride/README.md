# robstride

Driver for Robstride actuators (RS00, RS02, RS03, RS04) over SocketCAN at 1 Mbit/s (`can0` on the bench today). Only [Davout](../davout/) calls it; Berthier and the bins never do. Protocol notes: [hardware ADR 0002](../../hardware/docs/decisions/0002-robstride-protocol.md). Vendor STEP files are in `cad/vendor/robstride/` on the CAD machine (not in git).

Every receive poll is bounded (64 frames, 256 read attempts) and keeps raw delivery order, kernel receive timestamps and fault payloads, so Davout can judge feedback freshness. Production SocketCAN buses enable own-message echo.

Supported firmware frames:

- MIT / operation control (`run_mode=0`). Production path for Marengo impedance and gravity compensation.
- Lifecycle (`ENABLE`, `DISABLE`, `SET_ZERO_POSITION`). Called only through Davout.
- Parameter reads/writes for firmware `run_mode`, `spd_ref`, `loc_ref`, `iq_ref`, and bench limits.
- Firmware speed mode (`run_mode=2`) helpers for bench diagnostics. Disabled by default in `config/control.yaml`.

Marengo `ControlMode::Position` is not Robstride firmware Position mode (`run_mode=1`). Production keeps MIT frames unless a bench diagnostic explicitly switches firmware mode through Davout.
