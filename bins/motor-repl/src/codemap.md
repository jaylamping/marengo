# bins/motor-repl/src/

## Responsibility
CLI argument parsing and command dispatch in `main.rs`.

## Flow
`main` dispatches `status` / `gravity-preview` directly without Supervisor or CAN
side effects (`status` opens CAN only). `disable` → `run_disable` (stop.rs, no
Supervisor) → exit code. Remaining supported commands construct a Supervisor;
`set-zero` first arms the independent exit stop, then runs its qualified
reference workflow, and uses the exit stop on error. `protocol-inspect [joint...]`
builds Davout's transmit-free inspection owner
(`Supervisor::from_repo_for_protocol_inspection`, ADR 0037) and prints one
`inspect <joint> <iface>:<id> firmware=… uid=… run_mode=… mech_pos=… mech_vel=…
can_timeout=… zero_sta=… add_offset=…` line per drive. It arms no exit stop: it
never enables, and Davout stops every drive before and after the queries.

`stop.rs` is the independent stop: `stop_addresses` (from `motors.yaml` via
`marengo_config::load_motor_stop_targets`), `disable_drives` (one Disable then
one type-24 Off per drive, one bus open per interface, per-drive outcome per
frame), and `install_signal_stop`
(SIGTERM/SIGINT/SIGHUP thread with its own sockets). The retired commands
`home`, `enable`, `jog`, `speed`, `speed-stop`, `gravity-on`, `gravity-off`, and
`torque-cmd` are rejected before owner construction.
