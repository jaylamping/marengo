# bins/motor-repl/

## Responsibility
**One-shot bench motor CLI** — status, disable, set-zero, and gravity-preview. All drive commands use Davout; independent `disable` uses the minimal stop path.

## Design
- Subcommands: `status`, `disable`, `set-zero`, `gravity-preview`
- `status` opens SocketCAN without constructing a Supervisor or sending type-24 startup reports; `gravity-preview` loads the robot model without CAN
- `set-zero` uses `ControlLoop<RuntimeBus>` with the physical-reference owner
- `--config-dir` and `MARENGO_CONFIG_DIR` select the bring-up profile

## Flow
1. Parse global bus args before the subcommand.
2. `status` opens SocketCAN without a Supervisor; `gravity-preview` loads the robot model without CAN.
3. `disable` reads only motors.yaml stop addresses; remaining commands construct the required owner.
4. Return/exit after the command. `set-zero` arms an independent SIGTERM/SIGINT/SIGHUP stop and uses it on error exit.

Every fresh Supervisor starts joints Unhomed; calibration history cannot transfer
readiness between CLI processes. `home`, `enable`, `jog`, `speed`, `speed-stop`,
`gravity-on`, `gravity-off`, and `torque-cmd` are not available in this one-shot
CLI. Home and enable in one long-running `marengo-pi` process (stdin
`home <joint>... sign-tested`). `set-zero <joint> [--sign-tested]` builds the
loop with `ControlLoop::from_repo_with_physical_reference` (journal from
`resolve_reference_journal_path`) and runs Davout's qualified physical workflow
via `calibrate_joint_zero` (ADR 0036). The resulting grant ends with the process.
`disable` is not a qualified emergency stop; use the physical E-stop.

## Integration
- **MCP consumers**: `pi_set_zero`, `pi_motor_disable`, `pi_motor_recover`

**Detailed map**: [src/codemap.md](src/codemap.md)
