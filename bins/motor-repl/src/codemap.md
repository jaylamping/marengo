# bins/motor-repl/src/

## Responsibility
CLI argument parsing and command dispatch in `main.rs`.

## Flow
`main` dispatches `status` / `gravity-preview` directly without Supervisor or CAN
side effects (`status` opens CAN only). `disable` → `run_disable` (stop.rs, no
Supervisor) → exit code. Remaining supported commands construct a Supervisor;
`set-zero` first arms the independent exit stop, then runs its qualified
reference workflow, and uses the exit stop on error.

`stop.rs` is the independent stop: `stop_addresses` (from `motors.yaml` via
`marengo_config::load_motor_stop_targets`), `disable_drives` (one Disable per
drive, one bus open per interface, per-drive outcome), and `install_signal_stop`
(SIGTERM/SIGINT/SIGHUP thread with its own sockets). The retired commands
`home`, `enable`, `jog`, `speed`, `speed-stop`, `gravity-on`, `gravity-off`, and
`torque-cmd` are rejected before owner construction.
