# bins/motor-repl/

## Responsibility
**Bench motor CLI** — interactive and one-shot commands for bring-up: status, enable, disable, jog, set-zero, gravity-preview, homing-status. All motion through Davout.

## Design
- Subcommand parser: `status`, `enable`, `disable`, `jog`, `set-zero`, `gravity-on`, `gravity-preview`, `homing-status`, `hold-on`, `hold-at`
- Shares `ControlLoop<RuntimeBus>` construction with marengo-pi
- `preflight_gravity_saturation` gate before enable
- `--config-dir` and `MARENGO_CONFIG_DIR` for bringup profile selection

## Flow
1. Parse bus args (`--can`, `--config-dir`)
2. Open SocketCAN → Supervisor → ControlLoop
3. Execute subcommand (single-shot or interactive REPL)
4. Return/exit after the subcommand; reliable all-exit stop cleanup remains CS07.

Every fresh Supervisor starts joints Unhomed; calibration history cannot transfer
readiness between CLI processes. Full constructor errors (including corrupt
history) occur before subcommand dispatch, so this fresh CLI is not a qualified
emergency stop. `set-zero <joint> [--sign-tested]` builds the loop with
`ControlLoop::from_repo_with_physical_reference` (journal from
`resolve_reference_journal_path`) and runs Davout's qualified physical workflow
via `calibrate_joint_zero` (ADR 0036). The resulting grant ends with the process;
home and enable in one `marengo-pi` (stdin `home <joint>... sign-tested`). Other
subcommands keep plain `from_repo` and cannot acquire reference. Installed-owner
client migration and general all-exit cleanup remain in the
[repair roadmap](../../docs/reviews/2026-09-29/implementation-roadmap.md).

## Integration
- **Primary bench tool** for MCP `pi_hold_on`, `pi_motor_recover`, `pi_set_zero`
- **Crates**: berthier, davout, robstride, marengo-config, armee-dynamics

**Detailed map**: [src/codemap.md](src/codemap.md)
