# bins/marengo-pi/

## Responsibility
**Primary Pi runtime** — CAN I/O, Berthier control loop, Chappe telemetry publish, stdin operator REPL, and optional IMU/host metrics.

## Design
- **Event loop**: `run_control_loop` owns command dispatch and ticks at configured Hz; stdin and the Chappe IPC bridge supply its queues.
- `PiCommand` enum: home, home `<joints>` sign-tested, enable, disable, status, hold-on, hold-at, gravity-on, quit.
- **Motion owner** (`motion_owner.rs`): exactly one source (stdin or Chappe) owns motion per process (`--motion-owner stdin|chappe`, default chappe; MCP exports `MARENGO_MOTION_OWNER=stdin`). Disable/stop is always accepted; enable, Set Zero, hold/wave/torque/mode commands, Testing batches and runtime tuning from the non-owner are refused with a `motion_refused` `ActionEvent` on `robot/audit/action`. Testing batches also refuse while the reference queue is busy. A lagged `robot/enable` channel fails closed (stop, `stop_on_lag` event); an operator disable blocks implicit re-enable until an explicit enable.
- **Enable gate** (`enable_gate.rs`): `enabled (operator=…) targets=…` is printed only once Berthier's `enable_completion` holds (every target's Enable written, fresh session pose for each), after `waiting for enable to complete (operator=…)`. Stdin `hold-on`/`hold-at`/`wave` before then wait (`<cmd> waiting for enable to complete`) and run after a later tick. Both waits are bounded by 2 s: `enable failed: not complete within 2000 ms: …` (all drives stopped) or `<cmd> failed: …`. Consul testing-panel Position commands re-arm first and are refused (warn) until enable completes; the next batch retries.
- **Reference queue** (`reference_queue.rs`): stdin `home <j1> [<j2>...] sign-tested` (also `--sign-tested`) and Consul `robot/set_zero` (operator from payload, default `consul`; `confirm` and `sign_test_passed` required) share one queue. Joints are acquired one at a time through Davout's physical reference workflow (`request_reference` after each tick, polled with `reference_outcome`). Stdout lines: `reference <joint> current pos=<f:.4>`, `reference <joint> failed: <message>`, `reference <joint> skipped: earlier joint failed`; admission refusals print `home failed: <message>` (missing sign-tested, no/unknown joint, queue busy). While busy, other stdin commands are deferred and replayed in order after it drains; Disable, Quit, hardware E-stop and signal shutdown cancel it (`reference <joint> failed: cancelled`, plus `discarded <n> deferred command(s): reference queue cancelled`). Chappe enable(true) is refused while busy. Plain `home` keeps the readiness check.
- Chappe subscribers for `EnableRequest`, homing commands, testing panel commands from Consul.
- Preflight `preflight_gravity_saturation` before enable (refuses if τ_g exceeds motor limits).
- Periodic `SafetyState` reads Davout's retained fault authority, publishing every fault's stable ID/class/message/joint and the observed hardware E-stop input. Healthy ticks and ordinary Disable cannot publish a retained fault as clear. Actual Pi GPIO wiring and physical recovery remain unqualified.
- `src/safety_publication_tests.rs` exercises the installed control loop and Chappe wire across one-shot transport/device faults, two peers, observed E-stop input and healthy startup, including subsequent ordinary Disable.

## Flow
1. `main` → parse args → load config → `RuntimeBus::open(can_interface)`
2. Resolve the reference journal (`marengo_config::resolve_reference_journal_path`; fatal on error) and build `ControlLoop::from_repo_with_physical_reference` (qualified physical Robstride owner + durable journal)
3. Spawn stdin reader + Chappe IPC bridge
4. `run_control_loop`: deferred/new stdin → `dispatch_stdin_command`; tick → pump reference queue → publish RobotState/SafetyState/Heartbeat on Chappe
5. `handle_command` for operator stdin; `drain_chappe_commands` for remote enable/disable and Set Zero admission
6. Observed Quit/shutdown exits dispatch before later commands/ticks; `finish_owner_shutdown` clears intent, performs mandatory live-reference cleanup even with `disable_on_exit=false`, applies the ordinary exit policy (reusing any reference stop), and retains distinct results before closing/draining persistence.

The independent filesystem worker completes retained writes and matching local
audit publication after owner exit. Typed drain outcomes preserve failed writes,
worker failure and unfinished work; actual thread joining establishes termination.
Local publication and transport stop acceptance do not establish client delivery
or physical stop. Existing no-disable exit policy reports a skipped stop.

Fresh startup leaves all configured joints Unhomed regardless of saved history.
Normal Enable requires current reference, granted only by a successful physical
reference transaction (stdin `home <joint> sign-tested` or Consul Set Zero).
Davout's private permission gates scoped Enable as well as normal Enable.
Disable in the already running owner does not reload history or require
reference readiness. See [homing](../../docs/homing.md).

## Integration
- **Crates**: berthier, davout, robstride, chappe, marengo-config, armee-dynamics, marengo-host-metrics
- **Consumed by**: systemd `marengo-pi.service` on bench Pi
- **Peers**: marengo-gateway (Chappe IPC), Consul (via gateway)

**Detailed map**: [src/codemap.md](src/codemap.md)
