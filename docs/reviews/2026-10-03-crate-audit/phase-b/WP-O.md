# Phase B — WP-O: motor-repl / MCP tool rot

Branch `audit/wp-o`, based on `edbaebaf`.

Fix commits on this branch: `b38d14bd`, `881c5aa5`, `9d0ee10c`, `016963f0`, `3c875edf`, `600cbae0`, `8515a471`. This report also records the independently-owned gateway fix `125f0798` from WP-K; it is not part of this branch.

“Red → green” below describes the baseline behavior and the regression assertion added or already present. The WP-O worker did not run the gates; the integrator should run them once after all work packages land, then append the results here.

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-motor-repl-01 | **CONFIRMED** | Before: the fresh ordinary owner could not acquire reference permission, so `home`/`enable`/`jog` could only fail; MCP's `pi_motor_enable`/`pi_jog` therefore advertised unusable routes. After: `motion.test.ts` asserts those tools are absent, and motor-repl's `obsolete_motion_and_mode_commands_are_not_admitted` asserts the removed commands are rejected. Tests not run in this worker. | `b38d14bd` removed both MCP tools; `600cbae0` removed unreachable motion/mode commands. `docs/pi-commissioning.md`, `bins/AGENTS.md`, and motor-repl codemap now describe the supported CLI and same-process `marengo-pi` reference/enable path. |
| L-motor-repl-02 | **CONFIRMED** | Before: read-only invocations constructed Davout `Supervisor`, sending type-24 active-reporting On frames. After: `read_only_commands_bypass_supervisor_construction` covers status/gravity-preview; MCP `readonly.test.ts` now requires status to bypass Supervisor and send no startup type-24 reports, and requires gravity-preview not to open CAN. Tests not run in this worker. | `881c5aa5`/`600cbae0` split read-only commands from the owner path; removed the obsolete firmware-speed config flag; updated MCP descriptions, ownership comment, and commissioning docs. `status` still opens SocketCAN; `gravity-preview` is local-only. |
| L-marengo-pi-17 | **CONFIRMED** | Before: EOF/read failure left the CAN owner alive. After: `stdin_eof_enqueues_quit_for_normal_shutdown` covers EOF and `explicit_quit_does_not_enqueue_a_second_shutdown_command` covers explicit quit. Tests not run in this worker. | `016963f0` sends Quit through normal cleanup on EOF/read failure. |
| L-marengo-gateway-20 | **CONFIRMED** | Before: the MCP and `pi-remote.sh` session-list requests omitted the gateway log token and received 401. WP-K reports commit `125f0798` forwards the configured token as `x-marengo-log-token`; its MCP `npm test` passed (200 tests). | Fixed on WP-K; no WP-O source changes. |
| L-marengo-log-cli-02 | **CONFIRMED** | Before: absent `/opt/marengo/bin` on PATH caused a shell fallback to prune hot logs without registering/archiving. After: motion-session generation tests in `tools/marengo-pi-mcp/test/motion.test.ts` cover calling the installed Pi CLI and preserving the archive sequence. Tests not run in this worker. | `9d0ee10c` invokes `${piRoot}/bin/marengo-log-cli`, removes prune-on-missing-CLI fallback, and archives only after register/finalize succeed. |
| L-marengo-pi-11 | **CONFIRMED** | Before: invalid `torque-cmd`/`wave` numbers silently became `None`. After: `invalid_numeric_commands_report_a_parse_error_and_are_rejected` checks invalid parse rejection and operator diagnostic path. Test not run in this worker. | `parse_number` emits an operator error before the command is rejected. |
| L-marengo-pi-18 | **CONFIRMED** | Before: commands deferred during a reference could replay after reference failure. After: `stdin_discards_deferred_commands_after_reference_failure` and `failure_skips_remaining_joints_and_discards_deferred_commands` assert discard, including deferred Enable/HoldOn; tests not run in this worker. | `016963f0` clears deferred commands after acquisition failure and reports the discard. |
| L-marengo-support-01 | **ALREADY-FIXED** | Current `env_filter` defaults to INFO and `unset_filter_defaults_to_info_and_explicit_directives_override_it` asserts both unset and explicit-filter behavior. Test not run in this worker. | No WP-O code change required. |
| L-marengo-log-cli-03 | **CONFIRMED** | Before: empty candump session data could be passed as `--candump ""`. After: MCP motion-session tests cover omitted candump arguments when no capture exists. Tests not run in this worker. | `9d0ee10c` conditionally emits `--candump` only for a non-empty capture path. |
| L-marengo-pi-13 | **CONFIRMED** | Before: deferred stdin commands accumulated without bound during reference. After: `deferred_commands_are_bounded` fills the queue to `MAX_DEFERRED_COMMANDS` and asserts the next command is refused. Test not run in this worker. | `016963f0` caps the queue at 64 and prints a refusal when full. |
| L-motor-repl-06 | **CONFIRMED** | Before: `--config-dir` changed process environment after tracing initialization. Current startup applies the parsed directory before `init_tracing`; the parser test `global_options_are_parsed_before_the_subcommand_and_are_not_command_args` covers the config-dir option. | `600cbae0` parses global options before dispatch and sets `MARENGO_CONFIG_DIR` before tracing starts. |
| L-motor-repl-07 | **CONFIRMED** | Before: owner selection/global option handling depended on raw `args[1]`, and late global flags could silently be treated as command arguments. After: `global_options_after_the_subcommand_are_rejected` asserts late `--config-dir` is refused; the parser test also asserts the set-zero command remains intact with preceding global options. Tests not run in this worker. | `600cbae0` parses global bus args before dispatch and selects physical-reference ownership by parsed subcommand. |
| L-motor-repl-08 | **CONFIRMED** | Before: indexing `args[0]` and accepting late flags could panic or misparse. After: `argument_parser_rejects_missing_program_or_command` covers empty/program-only input, and `global_options_after_the_subcommand_are_rejected` covers invalid flag position. Tests not run in this worker. | `600cbae0` validates the program argument and global option position. |
| L-motor-repl-03 (WP-G lead; WP-O owns the CLI/MCP seam) | **CONFIRMED** | Before: partial pose vectors defaulted to all zeros and extra angles were truncated. After: `gravity_preview_requires_an_empty_or_complete_pose` asserts rejection for partial/excess vectors and acceptance for zero/full vectors; MCP harness tests cover full configured-joint order. Tests not run in this worker. | `3c875edf` and `600cbae0` require either no angles or the complete configured-joint vector; weighted harness preview passes the full vector. |

## NEEDS-DECISION

None. No physical tuning values or open safety behavior were changed.

## Cross-package edits

- `crates/davout/src/lib.rs` and Davout integration tests: removed unreachable legacy `JointCommand` and firmware `SpeedCommand` surfaces after retiring motor-repl `jog`/`speed`/`speed-stop`; `speed_control_at(..., 0.0)` and the reference-cancellation stop path remain. `fault_authority` still exercises supported MIT motion, Enable, and calibration routes.
- `crates/berthier/src/lib.rs`: removed the legacy `Controller` facade/tests as explicitly directed by the integrator; the active `ControlLoop` path is unchanged.
- `crates/marengo-config/src/lib.rs`, `config/control.yaml`: removed `allow_firmware_speed_mode`, now unreachable after speed-mode CLI retirement; no physical tuning values were changed. The WP-H serde changes in the shared config file were preserved.
- `tools/marengo-pi-mcp/src/tools/readonly.ts` and `test/readonly.test.ts`: corrected stale status/gravity-preview side-effect descriptions and regression assertions.
- Documentation/codemaps updated: `docs/pi-commissioning.md`, `bins/AGENTS.md`, `bins/motor-repl/codemap.md`, MCP README and CAN-owner comment.

## Gate commands

Not run by this worker to avoid concurrent workspace gates while other packages were still in flight. Main integrator should run once after integration:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings
cargo clippy -p marengo-pi --all-targets -- -D warnings
cargo test --workspace
cd tools/marengo-pi-mcp && npm test
```
