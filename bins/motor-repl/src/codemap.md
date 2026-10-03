# bins/motor-repl/src/

## Responsibility
CLI argument parsing and command dispatch in `main.rs`.

## Flow
`main` → `disable` → `run_disable` (stop.rs, no Supervisor) → exit code.
`main` → arm exit stop for drive-touching commands → `run_command` (config,
bus, Supervisor, subcommand match) → exit code → exit stop when required.

`stop.rs` is the independent stop: `stop_addresses` (from `motors.yaml` via
`marengo_config::load_motor_stop_targets`), `disable_drives` (one Disable per
drive, one bus open per interface, per-drive outcome), `install_signal_stop`
(SIGTERM/SIGINT/SIGHUP thread with its own sockets).
