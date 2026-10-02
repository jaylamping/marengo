# bins/marengo-pi/

## Responsibility
**Primary Pi runtime** — CAN I/O, Berthier control loop, Chappe telemetry publish, stdin operator REPL, and optional IMU/host metrics.

## Design
- **Event loop**: `run_control_loop` owns command dispatch and ticks at configured Hz; stdin and the Chappe IPC bridge supply its queues.
- `PiCommand` enum: enable, disable, status, set-zero, hold-on, hold-at, gravity-on, quit.
- Chappe subscribers for `EnableRequest`, homing commands, testing panel commands from Consul.
- Preflight `preflight_gravity_saturation` before enable (refuses if τ_g exceeds motor limits).
- Periodic `SafetyState` reads Davout's retained fault authority, publishing every fault's stable ID/class/message/joint and the observed hardware E-stop input. Healthy ticks and ordinary Disable cannot publish a retained fault as clear. Actual Pi GPIO wiring and physical recovery remain unqualified.
- `src/safety_publication_tests.rs` exercises the installed control loop and Chappe wire across one-shot transport/device faults, two peers, observed E-stop input and healthy startup, including subsequent ordinary Disable.

## Flow
1. `main` → parse args → load config → `RuntimeBus::open(can_interface)`
2. Build `ControlLoop<RuntimeBus>` with dynamics model
3. Spawn stdin reader + Chappe IPC bridge
4. `run_control_loop`: tick → publish RobotState/SafetyState/Heartbeat on Chappe
5. `handle_command` for operator stdin; `drain_chappe_commands` for remote enable
6. Observed Quit/shutdown exits dispatch before later commands/ticks; `finish_owner_shutdown` clears intent, performs mandatory live-reference cleanup even with `disable_on_exit=false`, applies the ordinary exit policy (reusing any reference stop), and retains distinct results before closing/draining persistence.

The independent filesystem worker completes retained writes and matching local
audit publication after owner exit. Typed drain outcomes preserve failed writes,
worker failure and unfinished work; actual thread joining establishes termination.
Local publication and transport stop acceptance do not establish client delivery
or physical stop. Existing no-disable exit policy reports a skipped stop.

Fresh startup leaves all configured joints Unhomed regardless of saved history.
Normal Enable requires current reference; the complete qualified transaction and
client cutover remain open. Davout's private permission gates scoped Enable as
well as normal Enable; physical reference and Set Zero currently refuse before
arming. Disable in the already running owner does not reload
history or require reference readiness. See [homing](../../docs/homing.md).

## Integration
- **Crates**: berthier, davout, robstride, chappe, marengo-config, armee-dynamics, marengo-host-metrics
- **Consumed by**: systemd `marengo-pi.service` on bench Pi
- **Peers**: marengo-gateway (Chappe IPC), Consul (via gateway)

**Detailed map**: [src/codemap.md](src/codemap.md)
