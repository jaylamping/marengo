# Intent card — `marengo-pi`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-pi` |
| Path | `bins/marengo-pi` |
| Kind | bin (`src/main.rs`); features `socketcan` (→ `robstride/socketcan`), `linux-i2c` (→ optional `marengo-imu`) — `Cargo.toml:13-16` |
| Baseline | `a2b55b3` (audit worktree) |
| LOC | src 3396 (`main.rs` 1697, `overlay.rs` 690, `limit_persist.rs` 541, `reference_queue.rs` 232, `imu.rs` 156, `host_metrics.rs` 80); tests 5368 in 11 `*_tests.rs` files + 2 tests in `main.rs:1659-1697` |
| Sources | `bins/AGENTS.md`, `bins/codemap.md`, `bins/marengo-pi/codemap.md`, `bins/marengo-pi/src/codemap.md`, root `AGENTS.md`, `codemap.md`, `CONTEXT.md`, ADR 0008/0012/0017/0023/0024/0036, `docs/homing.md`, `docs/safety.md`, `docs/roadmap.md`, `scripts/systemd/marengo-pi.service`, `scripts/env.example`, `tools/marengo-pi-mcp/src/tools/motion.ts`, prior review `control.md`, `gateway.md`, `finding-index.md`, `implementation-ledger.json`, `git log -- bins/marengo-pi` (60 commits, first `61fe36d` 2026-05-19) |

## 2. Intent

`marengo-pi` is the single long-running **owner** of the robot's motors on the Pi: it composes Berthier `ControlLoop` → Davout `Supervisor` → robstride `RuntimeBus` over SocketCAN and ticks it at `control.loop_hz` (`main.rs:1087-1108,1360-1489`; `bins/AGENTS.md` "Active — main runtime"; `AGENTS.md:35,61`). It is the only process in which a **current reference** (Joint Ready) can be acquired *and* used for Enable, because grants are process-private (ADR 0036 §"Process lifetime"; `docs/homing.md:99-115`); hence it hosts the operator reference queue for stdin `home <joints> sign-tested` and Consul **Set Zero** (`reference_queue.rs:1-4`, commit `24eb1e7`). It is the Chappe producer for robot telemetry (`RobotState` via Berthier tick, `SafetyState`, `Heartbeat`, limits, audit, IMU, host metrics) and the consumer of gateway commands (enable, set_zero, leases, status poll, Testing MIT batches, actuator tuning / **Live limit patch**) (`main.rs:1122-1128,698-765`; `overlay.rs:1-15`). It owns the shutdown order "stop before persistence" (ADR 0024; `main.rs:1259-1341`) and the write-behind of live limit/tuning changes to the **Master YAML set** + master URDF (ADR 0012/0017; `limit_persist.rs:1,456-463`).

Conflicting statements: `bins/AGENTS.md` rule "thin `main`, logic in `crates/`" vs 3.4k LOC of state machines/persistence here (see §3). `docs/pi-commissioning.md:232` says `marengo-pi.service` stays disabled by default (manual runs), while MCP descriptions and `bins/codemap.md` "Flow" treat it as the systemd owner.

## 3. Owns / Must not

| Owns (cited) | Must not (cited) |
|---|---|
| Process composition: config load, SocketCAN open, physical-reference owner + journal (`main.rs:1039-1108`) | Direct CAN/robstride access outside Davout — `bins/AGENTS.md` anti-pattern; `docs/roadmap.md` "not building: Direct robstride calls from Berthier or bins". Only `RuntimeBus::socketcan_from_motors` construction is used (`main.rs:1087`) — OK |
| stdin operator grammar + reference queue (`main.rs:122-256`, `reference_queue.rs`) | Grant reference from history — ADR 0022/0036; startup is Unhomed (`main.rs:1177-1179`) — OK |
| Chappe command intake & telemetry publish (`main.rs:370-668,698-765`) | `println!` for runtime logs in a Chappe producer (`bins/AGENTS.md`). **Violation (deliberate):** stdout is the reference contract (`emit_reference_events`, `main.rs:266-289`) and `print_status`/`handle_command` use `println!/eprintln!` (`main.rs:767-1000`) |
| Owner shutdown order and outcome report (`main.rs:1259-1341`) | "Logic in `bins/`" (`bins/AGENTS.md`). **Violation:** reference queue state machine, write-behind persistence worker, overlay dispatch, gravity preflight (duplicated in `motor-repl`) live in the bin |
| Write-behind config persistence (`limit_persist.rs`) | Block the 200 Hz tick on SD I/O (`overlay.rs:9-11`, `limit_persist.rs:1`) — upheld for persistence; but `PositionTrace` CSV writes run on the tick (Berthier `loop.rs:1303`) |

## 4. Interface

No Rust dependents (bin). Consumers are processes/operators.

| Group | Surface | Consumers | Depth |
|---|---|---|---|
| CLI | `--config-dir PATH`, `--no-stdin-ctl`, `-h` (`main.rs:1011-1037`) | `scripts/systemd/marengo-pi.service:14` (no args), MCP `marengoPiLaunchShell` (`motion.ts`) | shallow |
| stdin grammar | `home`, `home <j..> [--]sign-tested`, `enable [op] [force]`, `disable`, `gravity-on/off`, `torque-cmd <j> <nm>`, `impedance-on/off`, `hold-on`, `hold-at [j] <rad>`, `hold-off`, `wave <j> <min> <max> <cycles> [half]`, `status`, `quit/exit`, `help` (`main.rs:122-238`). Stdout contract: `reference <j> current pos=…`, `… failed: …`, `… skipped: earlier joint failed`, `discarded N deferred command(s)…` (`reference_queue.rs:53-68`), `home failed: …` (`main.rs:878`) | MCP `pi_hold_on`, `pi_bench_harness`, `pi_gravity_calibrate`, `pi_marengo_pi_script`, `pi_motor_recover` (`motion.ts`, `harness/scripts.ts`, `gravity-calibrate.ts`: `home`, `enable bench`, `hold-at`, `hold-on`, `gravity-on`, `status`, `disable`, `quit`, `wave`); `scripts/profile-pi-loop.sh`. **No automated caller** for `impedance-on/off`, `hold-off`; `torque-cmd`/`gravity-off` only in `docs/commissioning/limb-playbook.md` | deep (queue hides Davout reference handles) |
| Chappe subscribe | `robot/enable` (EnableRequest), `robot/homing` (HomingComplete, retired), `robot/set_zero` (SetZeroRequest), `robot/active_reporting_lease`, `robot/motor_status_poll`, `robot/testing/mit_command_batch` (MitCommandBatch incl. `wave:` joint-name encoding), `robot/actuator/command` (OperatorCommand) (`main.rs:1122-1128`) | Producer: `marengo-gateway` (`http.rs`), admitted by `chappe::ipc::command_is_current` (`crates/chappe/src/ipc.rs:276-300`, ≤1000 ms age) | mixed; `wave:` string encoding in a joint name is a shallow wire hack (`main.rs:597-599,678-696`) |
| Chappe publish | `robot/safety`, `robot/heartbeat` (`main.rs:747,756`), `robot/actuator/limits`, `robot/audit/tuning`, `robot/audit/action` (`overlay.rs:41-44`, `limit_persist.rs:53`), `sensors/imu/torso` (`imu.rs:14`), `host/metrics/pi` (`host_metrics.rs:26`), `robot/state` + `logs/structured` (Berthier tick / `chappe::tracing_layer`, `main.rs:1111,1432`) | gateway `state.rs`/`http.rs`, Consul `lib/chappe-config.ts` | — |
| Seams | `ReferenceDriver` trait (`reference_queue.rs:16-24`): 1 prod adapter (`Supervisor`, `:26-42`) + scripted test driver → real seam. `MotorBus` generic on all handlers (`main.rs:300+`): prod `RuntimeBus`, tests use Davout test buses. `PersistTestHooks` (`limit_persist.rs:32-39`) test-only seam | — | — |

Coverage (`metrics/coverage-by-crate.md:16`, `metrics/coverage-by-file.md`): crate 57.1 % lines / 52.1 % regions / 63.6 % functions. Per file: `reference_queue.rs` 96.5 % (139/144), `limit_persist.rs` 84.7 %, `overlay.rs` 78.9 %, `main.rs` 37.4 % (425/1137), `host_metrics.rs` 0.0 % (0/61). `imu.rs` absent from the report (compiled only for `target_os = "linux"` + `linux-i2c`, `main.rs:4`) → effectively unmeasured. 58 tests (`metrics/test-counts.md:57`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) |
|---|---|---|
| Fresh process starts Disabled/Unhomed; Enable needs in-process physical reference | `ControlLoop::from_repo_with_physical_reference` (`main.rs:1096`); Davout gating | Davout tests; no marengo-pi startup test (untested here) |
| Reference admission requires sign-tested, ≥1 known joint, idle queue; refusal queues nothing | `ReferenceQueue::admit` (`reference_queue.rs:104-129`) | `reference_queue_tests::admit_refuses_missing_sign_tested_unknown_joint_and_busy`, `reference_dispatch_tests::home_refusals_queue_nothing` |
| One joint at a time; failure skips rest; E-stop cancels before polling | `pump` (`reference_queue.rs:146-200`) | `joints_acquire_in_order_one_at_a_time`, `failure_skips_remaining_joints`, `hardware_estop_cancels_queue_before_polling` |
| While busy, stdin commands other than home/disable/quit are deferred and replayed in order; Disable/Quit cancel and discard | `defers_while_referencing`, `dispatch_stdin_command` (`main.rs:293-311`), `take_ready_deferred` (`reference_queue.rs:136-142`) | `stdin_defers_while_busy_and_replays_after_failure`, `disable_and_quit_cancel_the_queue`, `only_home_joints_disable_and_quit_bypass_deferral` |
| Consul Set Zero requires `confirm` + `sign_test_passed`; same-tick `enable(true)` refused while queue busy (set_zero drained first) | `handle_chappe_set_zero` (`main.rs:551-574`), ordering comment `main.rs:380-381`, `handle_chappe_enable` (`:339-343`) | **untested** (no test references `handle_chappe_set_zero`/`SetZeroRequest`; grep of `*tests.rs`) |
| Actuator overlay refused while reference busy | `apply_operator_command` (`overlay.rs:232-239`) | `reference_busy_overlay_tests::reserved_reference_refuses_actual_persist_and_runtime_overlay_before_mutation` |
| Stop before persistence; mandatory reference cleanup even with `disable_on_exit=false`; stop/storage outcomes separate | `finish_owner_shutdown` (`main.rs:1260-1341`) | `shutdown_tests::*` (8), `reference_shutdown_tests::armed_reference_shutdown_stops_before_real_storage_under_both_exit_policies`, `reference_journal_shutdown_tests::both_writers_share_one_shutdown_budget_after_the_required_stop` |
| Observed shutdown/Quit exits before later commands/ticks | shutdown checks throughout `run_control_loop` (`main.rs:1372-1448`), `drain_*` | `stdin_quit_prevents_a_later_actuator_command_and_motion_tick`, `observed_owner_shutdown_preserves_later_stdin_and_chappe_commands` |
| Retained Davout faults stay published across healthy ticks and ordinary Disable | `publish_safety` reads `safety_snapshot()` (`main.rs:698-753`) | `safety_publication_tests::*` (5), `safety_receive_diagnostic_tests::*` (6) |
| Gravity preflight refuses stdin `enable` if max τ_g > `tau_ff_max` | `preflight_gravity_saturation` (`main.rs:808-855`) | **untested** (inside `main.rs`, 37.4 % line coverage) — gap |
| Live config mutated only after enqueue succeeds; limit patch rolled back if enqueue fails | `apply_tuning_change` (`overlay.rs:329-389`), `apply_limit_patch_command` (`:425-463`) | `config_overlay_*`, `limit_patch_after_owner_exit_rejects_closed_admission_without_live_mutation`, `limit_patch_persist_failure_emits_distinct_failed_action` |
| Runtime MIT tuning is gains-only, finite, ≥0, only in Impedance/Position | `apply_runtime_param` (`overlay.rs:512-551`) | `runtime_overlay_*` (6) |
| Persist drain is bounded, never joins an unfinished worker, reports typed outcome | `close_and_drain` (`limit_persist.rs:283-320`) | `limit_persist_qualification_tests::zero_budget_close_reports_live_work_and_rejects_later_valid_admission`, `limit_persist_tests::*` |

## 6. Inputs / outputs

- **Env:** `MARENGO_ROOT`, `MARENGO_CONFIG_DIR` (set from `--config-dir`, `main.rs:1042-1045`), `MARENGO_CHAPPE_SOCKET` (`chappe::ipc::socket_path_from_env`, `main.rs:1113`; provided only via `/etc/marengo/env`, `scripts/env.example:25`), `MARENGO_IMU_BUS/ADDRESS/REPORT_HZ/FRAME_ID` (`imu.rs:40-63`), `MARENGO_GATEWAY_HTTP` (`host_metrics.rs:71`), transitively `MARENGO_JOINT_SUBSET` (Davout `lib.rs:560`), `MARENGO_CALIBRATION_RECORD` (Davout `lib.rs:432`), `MARENGO_POSITION_TRACE[_HZ]` (Berthier `position_trace.rs:19,24`), `RUST_LOG`.
- **Config:** `control.yaml` (`loop_hz`, `chappe_state_hz`, `disable_on_exit`, `bench.*`, joints/impedance/friction), `motors.yaml` (`can_interface`, `device_id`, `bench.position_*_rad`), `robot.yaml` (URDF path), `homing.yaml`, command-joint allowlist (`load_command_joint_allowlist_from`, `overlay.rs:94`), reference journal path (`resolve_reference_journal_path`, `main.rs:1079`).
- **Files written:** control.yaml / motors.yaml / expand-only URDF via `write_control_config_from` / `write_motors_control_and_urdf` (`limit_persist.rs:456-463`); reference journal (Davout); optional position-trace CSV (Berthier).
- **CAN:** all via Davout/robstride (MIT, Enable/Disable, type-24 active reporting, SetZero/type-17 reference frames).
- **Signals:** SIGINT (`ctrlc`), SIGTERM (`signal_hook`) → shutdown flag (`main.rs:1132-1145`).
- **Network:** TCP connect probe to gateway `127.0.0.1:8080` each second (`host_metrics.rs:70-80`).

## 7. Prior review reconciliation

| ID | Prior | Current status | Evidence |
|---|---|---|---|
| CS09 | Pi waits for persistence before stop | **fixed** | `finish_owner_shutdown` stops first (`main.rs:1267-1290`) then drains (`:1292-1305`); `shutdown_tests`; ledger verified PR221 / commit `3939b3d` |
| CS07 (Pi half) | mandatory reference cleanup on exit | **fixed for Pi** | `cancel_reference_for_shutdown` (`main.rs:1268`) under both policies; `reference_shutdown_tests` |
| CS13 | transient fault publication | **partial** — retained Davout faults now published (`main.rs:703-729`, commits `01a5c40`, `2d0fd40`); `active_fault` still ephemeral per tick (`main.rs:1432-1444`) and only shown when no retained fault (`:730-739`) | ledger partial |
| control.md "E-stop scaffold" (`publish_safety` hardcoded false) | **superseded/partial** — now `snapshot.hardware_estop_asserted` (`main.rs:743`), but no production caller of `Supervisor::set_hardware_estop` (only tests: `crates/davout/tests/*`) | grep |
| CS17 | IMU republishes stale samples with fresh timestamps; poll errors don't restart | **open** | `imu.rs:118-147` unchanged pattern; driver `crates/marengo-imu/src/driver.rs:84-87` still returns cached `last_rotation`; ledger open |
| CS19 | Testing gains applied before mode switch; wave skips gains | **open** | `main.rs:599-627` (wave `continue` before gains), `:628-656` (gains before `enter_position_hold_at`); ledger open |
| G01 | unbounded IPC queue | **fixed (chappe)**; Pi reports bounded queue stats | `host_metrics.rs:47-66`; ledger verified |
| G04 | coalescing drops motors/URDF write + ACK | **open** | `limit_persist.rs:265` `slot.request.replace(request)` replaces a `motors: Some` request with a later `motors: None` one; pinned by `overlay_tests::persist_queue_coalesces_to_latest_draft`; ledger open |
| G09 | URDF writer race with gateway activation | **open** (Pi side unchanged) | worker writes URDF `limit_persist.rs:456-462` without generation CAS |
| T05 | MCP sessions compete with systemd owner | out of crate (tooling); MCP now advertises sole-owner stop (`SOLE_CAN_OWNER_NOTE`) — **unverifiable here** | `motion.ts` |

## 8. Drift

| Doc | Says | Code |
|---|---|---|
| `bins/marengo-pi/codemap.md` "PiCommand enum" | home, home sign-tested, enable, disable, status, hold-on, hold-at, gravity-on, quit | also `gravity-off`, `torque-cmd`, `impedance-on/off`, `hold-off`, `wave`, `help` (`main.rs:84-120,156-232`) |
| `bins/marengo-pi/codemap.md` Integration | crates list | `marengo-homing`, `marengo-support` declared (`Cargo.toml:30,32`) but unused (0 refs in `src/`) |
| `bins/codemap.md` Integration | "All bins use `marengo-support::init_tracing()`" | marengo-pi uses `chappe::tracing_layer::init_subscriber` (`main.rs:1111`), correctly per `bins/AGENTS.md` |
| `bins/AGENTS.md` / `bins/codemap.md` | "9 binaries" table | 10 workspace bins; `marengo-limit-sync` missing from both tables |
| `print_usage` "`home` (readiness check only)" (`main.rs:243`) | check only | calls `set_homing_complete`, which **transitions mode to Ready** (`davout/src/lib.rs:851-866`); `docs/homing.md:106` says "→ Ready" correctly |
| `overlay.rs:9-11` | persistence never on tick | true for YAML; Berthier `PositionTrace` writes CSV on the tick (`berthier/src/loop.rs:1303-1310`) |
| `bins/marengo-pi/codemap.md` "Actual Pi GPIO wiring … unqualified" | E-stop input exists | there is no GPIO/E-stop input code at all (no prod caller of `set_hardware_estop`) |

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Deleting touches |
|---|---|---|---|
| `robot/homing` subscription + `HomingComplete` drain (`main.rs:498-510,1123`), `homing_rx` plumbing (`:374,1189,1349`) | superseded (comment "retired … compat drain" `main.rs:508`); zero producers in gateway/consul/tools (grep) | high | `ControlLoopRuntime`, `drain_chappe_commands` signature, `safety_publication_tests.rs:195`, `shutdown_tests.rs:462,1359`, `chappe/src/ipc.rs:279` allowlist entry, proto `HomingComplete` (check other users) |
| Unused deps `marengo-homing`, `marengo-support` (`Cargo.toml:30,32`) | zero references (grep of `src/`; confirmed by `cargo machete`, `metrics/unused-deps.md:31-33`) | high | `Cargo.toml` only |
| Overlay motion payload branch Enable/Mode/Jog/Hold/Preset → "gated until motion unlock" (`overlay.rs:255-270`), `action_label` arms (`:646-650`) | gateway rejects non-tuning payloads with 400 (`marengo-gateway/src/actuator.rs` test `command_actuator_rejects_non_tuning_with_400`); only reachable by a raw Chappe publisher | low (defence-in-depth; keep a reject arm) | `overlay.rs` |
| Duplicate `TOPIC_AUDIT_ACTION` + `publish_action_event` in `limit_persist.rs:53-64` vs `overlay.rs:44,676-686` | duplicate implementation | high | two modules |
| `preflight_gravity_saturation` (`main.rs:808-855`) duplicated in `motor-repl/src/main.rs:23-66` | duplicate implementation (move to Berthier) | high (dedupe, not delete) | both bins |
| Test-only shims kept "for archived probes": `ActuatorOverlay::drain_commands` (`overlay.rs:103-119`), `wait_persist_idle` (`:284-288`), `ConfigPersistQueue::spawn_with_test_hooks` ignored `_owner_shutdown` arg (`limit_persist.rs:204-213`), `is_busy`/`wait_idle`/`wait_idle_for_test` (`:356-381`) | test pinning an implementation detail / scaffold with no production consumer | med | `overlay_tests.rs`, `shutdown_tests.rs`, `limit_persist_*tests.rs`, `reference_*_tests.rs` call sites (spawn_with_test_hooks used 15×) |
| Duplicate config loads in `main` (`main.rs:1047-1071`) that Davout reloads (`davout/src/lib.rs:557-559`); `robot` used only for URDF path check | duplicate implementation | med | `main`; but note `motors` is also used to open SocketCAN (`:1087`) — see lead L12 |
| `overlay_tests::persist_queue_coalesces_to_latest_draft` | test pinning implementation detail that G04 must change | med | test file |

## 10. Phase-B leads

| # | Location | Suspicion |
|---|---|---|
| L1 | `limit_persist.rs:255-279` | G04 still live: a pending `limit_patch` request (`motors: Some`) replaced by a later ConfigOverlay draft (`motors: None`) loses motors.yaml + URDF write; the limit patch's Pending ACK never gets Durable/Failed ("no invented completion event", `:107-110`). Live hard bounds diverge from disk until restart reverts them. |
| L2 | `main.rs:576-668` | Testing MIT batches bypass the reference queue: no `queue.is_busy()` check (unlike `handle_chappe_enable` `:341`). Set Zero admitted in the same drain runs `drain_testing_commands` before the queue requests the reference at `pump` (`:1445`), and `ensure_active_for_motion` (`berthier/src/loop.rs:712-721`) can **auto-enable**. Safety relies on Davout refusing `request_reference` while Active. |
| L3 | `main.rs:339-358`, `berthier/src/loop.rs:712-721` | Chappe `enable(true)` and Testing auto-enable skip `preflight_gravity_saturation`; only stdin `enable` (`:881-887`) runs it. Same safety check, inconsistent by entry point. |
| L4 | `main.rs:808-855`, `armee-dynamics/src/lib.rs:160-170` | Preflight sweeps one joint with all others at q=0, uses static `motor.bench.position_*` not live envelope, and `unwrap_or(0.0)` on model error (fail-open); joints lacking motor/policy are silently skipped (`filter_map`). |
| L5 | `main.rs:1432-1444` | Tick error: `let _ = disable_all()` swallows stop failure; `CommWatchdog` keeps `ControlMode::Position`, so a later enable/Testing auto-enable may resume the stale hold target. |
| L6 | `main.rs:1432,730-739` | `active_fault` resets every tick; non-retained `LoopError`s are visible only if a 25 Hz publish coincides with the failing 200 Hz tick (residual CS13). |
| L7 | `main.rs:481-510,581-584` | `let Ok(bytes) = rx.try_recv() else { break }` treats `Lagged` as empty: a lagged `robot/enable` burst can silently drop a **disable** (capacity 256, `chappe/src/lib.rs:32`); set_zero/lease loops warn instead. |
| L8 | `main.rs:1110-1111` | Tracing subscriber installed **after** `ControlLoop` construction: Davout startup events (e.g. `MARENGO_JOINT_SUBSET` info, `davout/src/lib.rs:562`) never reach journal/Chappe. |
| L9 | `main.rs:1113-1121` | Missing/failed `MARENGO_CHAPPE_SOCKET` only warns; runtime runs motors with no gateway command/telemetry path (service unit sets no socket env, `marengo-pi.service`; relies on `/etc/marengo/env`). Fail-open for operator visibility. |
| L10 | no prod caller of `set_hardware_estop` | Hardware E-stop input never wired; `SafetyState.hardware_estop_asserted` always false and the queue's E-stop cancel (`reference_queue.rs:154-156`) unreachable in production. Gap, not prune. |
| L11 | `imu.rs:116-148` | CS17: cached quaternion republished with fresh `timestamp_ms` each poll; poll errors only warn, never restart; `backoff` never resets. accel/gyro hard-coded 0 with `has_*=false`. |
| L12 | `main.rs:1073-1087` vs `davout/src/lib.rs:560` | SocketCAN opened from the unfiltered `motors` (ignores `MARENGO_JOINT_SUBSET`); log `interfaces` likewise. |
| L13 | `host_metrics.rs:52` | `gateway_rtt_ms: 0.0` hard-coded — fabricated metric published as fact. |
| L14 | `main.rs:159-162,191-226` | `torque-cmd`/`wave` with unparsable numbers return `None` via `?` with no operator message (only `hold-at`/`wave` arity errors print). |
| L15 | `main.rs:345-348,888` | `resolve_enable_targets(repo_root())` re-reads commissioning scope from disk on the control thread per enable [INFERENCE: reads file]. |
| L16 | `main.rs:1486-1488` | Sleep-based pacing (`period - elapsed`) with no catch-up; overruns only logged at debug (`:1564-1575`). |
| L17 | `main.rs:302-310`, `reference_queue.rs:131-133` | `deferred` VecDeque is unbounded; stdin flood while referencing grows without bound (bench-only risk). |
| L18 | `metrics/coverage-by-file.md` vs §5 | **`main.rs` 37.4%** lines vs **`reference_queue.rs` 96.5%** — Chappe set_zero / gravity preflight / tick-error paths marked **untested** in §5 are coverage **gaps** on the owner safety surface, not prune signals. |
| L19 | `host_metrics.rs` **0%** (`coverage-by-file.md`) | Uncovered observability path; publishes fabricated `gateway_rtt_ms` (L13) — observability debt, not dead code. |


[You have received this identical output 3 times. Re-reading '/Users/joseph/code/marengo-wt/audit/docs/reviews/2026-10-03-crate-audit/intent/marengo-pi.md:raw' will not change it — use a narrower selector (path:A-B), or proceed with the edit.]