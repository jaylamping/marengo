# robstride

Driver for **Robstride** actuators (RS00, RS02, RS03, RS04) on CAN1. Vendor CAD lives under [`hardware/cad/vendor/robstride/`](../../hardware/cad/vendor/robstride/). Used by [Berthier](../berthier/) after [Davout](../davout/) approves commands.

Supported firmware frames:

- MIT / operation control (`run_mode=0`) — production path for Marengo impedance and gravity compensation.
- Lifecycle (`ENABLE`, `DISABLE`, `SET_ZERO_POSITION`) — called only through Davout.
- Parameter reads/writes for firmware `run_mode`, `spd_ref`, `loc_ref`, `iq_ref`, and bench limits.
- Firmware speed mode (`run_mode=2`) helpers for bench diagnostics; disabled by default in `config/control.yaml`.

Marengo `ControlMode::Position` is not the same thing as Robstride firmware Position mode (`run_mode=1`). The production control stack keeps using MIT frames unless a bench diagnostic explicitly switches firmware mode through Davout.
