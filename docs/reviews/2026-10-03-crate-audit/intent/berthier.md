# Intent card: berthier

## 1. Header

| Field | Value |
|---|---|
| Crate | `berthier` ("Realtime control stack for Marengo", `Cargo.toml:4`) |
| Path | `crates/berthier` |
| Kind | lib (no bin target; `Cargo.toml:1-31`) |
| Baseline | `a2b55b3` (audit worktree, branch `audit/2026-10-03`) |
| LOC | ≈5.2k production src (incl. ~250 lines of `#[cfg(test)]` helpers inside `impl PositionHold`, `position_hold.rs:400-637`) / ≈8.0k test (in-file test modules + `src/position_hold_tests/*` + `src/{mode_isolation,reference_grant_tests,reference_journal_tests}.rs` + `tests/*`). Largest: `loop.rs` 3,396 (prod 1–1606), `position_hold.rs` 1,852 (prod 1–1376), `position_setpoint.rs` 716 |
| Metrics (`metrics/`) | `loc.md:12`: 10,628 src lines (21 files, includes in-file test modules) / 2,608 `tests/` lines (14 files). `test-counts.md:12`: 174 tests. `coverage-by-crate.md:10`: 91.8 % lines / 91.5 % regions / 92.9 % functions. Per file (`coverage-by-file.md`): `position_trace.rs` 52.4 % (`:23`), `position_profile.rs` 88.6 % (`:65`), `loop.rs` 88.7 % (`:66`), `lib.rs` 88.7 % (`:67`), `position_setpoint.rs` 93.1 % (`:88`), `position_hold.rs` 94.6 % (`:94`), `position_trajectory.rs` 95.4 % (`:97`), `gain_runtime.rs` 96.5 % (`:104`), `friction.rs` 97.6 % (`:112`), `mit_feedforward.rs` 97.9 % (`:113`), `position_feedforward.rs`, `position_wave.rs`, `torque_cmd.rs` 100 % (`:120-122`). Coverage of `#[allow(dead_code)]` helpers is inflated by their own unit tests. |
| Feature | `reference-journal-test-support` → `davout/reference-journal-test-support` (`Cargo.toml:13-14`); enabled only by `bins/marengo-pi` dev-dependency (`bins/marengo-pi/Cargo.toml:41`) |
| Normal deps | armee-dynamics, armee-kinematics, armee-proto, chappe, davout, marengo-config, thiserror, tracing (`Cargo.toml:16-24`; `cargo tree -p berthier --depth 1`). `robstride` is dev-only (`Cargo.toml:30`), used only by test files (`src/reference_*_tests.rs`, `tests/*`) |
| Reverse deps | marengo-pi, motor-repl, wave-demo (`cargo tree -i berthier --depth 1`) |
| Sources consulted | `src/lib.rs` //!, `codemap.md`, `src/codemap.md`, `README.md`; root `AGENTS.md`, `crates/AGENTS.md`, `bins/AGENTS.md`, `codemap.md`, `CONTEXT.md`; `docs/rust-patterns.md`, `docs/safety.md`, `docs/tuning.md:42`, `docs/position-hold-control-review.md`; ADR 0004, 0007, 0009, 0010, 0025, 0036 (by reference in code); prior review `docs/reviews/2026-09-29/{finding-index.md, control.md, implementation-ledger.json, implementation-roadmap.md, control-implementation-plan.md, test-quality-plan.md}`; `git log -- crates/berthier` (128 commits, 2026-05-19 → 2026-10-03); consumer grep over `bins/`, `tools/`, `crates/`, `consul/src` (excl. gen) |

## 2. Intent

Berthier is the joint-space **outer control loop**: each tick it reads Davout's **joint feedback**, computes `τ_g(q)` through `armee-dynamics`, composes friction/damping/impedance terms and assembles an MIT batch that it hands to Davout, which alone filters and sends it (`lib.rs:1-5`, `lib.rs:9`; `AGENTS.md:46,50,59`; `crates/AGENTS.md:13,42`). It owns *how* to command while Davout's `OperationalMode` is Active — `ControlMode` GravityComp / Impedance / Position / TorqueOnly (ADR 0004 table; `lib.rs:42-49`) — and the **Position hold** motion primitive (trapezoid planner + setpoint mapping + MIT composition), which is meant to be the single joint-space executor for operator hold-at/hold-on and future Talleyrand streams (`CONTEXT.md:14`; `docs/rust-patterns.md:205`). It also owns runtime gain policy (**GainRuntime**: Testing overrides + 20-tick mode ramp, `CONTEXT.md:16`), the TorqueOnly operator `τ_cmd` latch (`docs/rust-patterns.md:207`), two software position-hold fuses (`docs/safety.md:130-150`; ADR 0025; commit `123a433`), and `RobotState` telemetry on Chappe (`lib.rs:19`). History confirms the arc: gravity comp first (`5d44f5c` 2026-05-19), position hold-on (`c84d8d1`), wave (`bede6d6`), GainOverride (`fccfeb1`), module extractions (`0d75eed`, `138fc23`, `a6de275`), TorqueOnly un-aliasing (`fb30060`), then the September safety remediation (feedback admission `d4c869b`, measured-progress fuse `eb1359c`, reference ownership `18f281c`/`e1a1771`, hold-tracking `123a433`).

Conflicting statements of intent:
- `README.md:9` says Berthier "consumes planner setpoints from Talleyrand"; `crates/AGENTS.md:45` and `docs/rust-patterns.md:205` say Talleyrand is only a *future* source and is not a dependency (`Cargo.toml:16-24`).
- `lib.rs:31` "Does not apply torque/position limits" vs ADR 0009 / `docs/rust-patterns.md:224-225`: Berthier *does* clamp targets/`q_traj`/`q_des` to the limit envelope and planner `v_max` to the velocity cap (advisory shaping; Davout is authoritative). Same tension in `AGENTS.md:50` ("Must not: limits").
- GravityComp gains: ADR 0004 table and `CONTEXT.md:15` say kp/kd hard-zero; code takes YAML `gravity_comp` gains plus ramp (`gain_runtime.rs:359-361,383-386`; `mit_feedforward.rs:49`). Zero only because `config/control.yaml:50-53` (etc.) sets 0.
- `docs/position-hold-control-review.md:63` "`kd_mit = 0` until firmware velocity quality is validated" vs code sending `kd` while `|dq_f| ≥ deadband` and `|settle_error| < 0.1` (`position_hold.rs:1308-1312`, `position_setpoint.rs:170-181`; test `loop.rs:2245`); `docs/safety.md` "Danger zones" bullet still assumes `kd_mit = 0`.

## 3. Owns / Must not

Owns (cited):
- `ControlLoop::tick` recv → q → τ_g → compose → `send_mit_batch` → post-send drain → Chappe (`loop.rs:1029-1442`; `AGENTS.md:225`; `crates/AGENTS.md:31`).
- Mode transitions and intent lifecycle: `set_control_mode_inner` clears Position/Wave on leave, TorqueOnly latch on leave, arms gain ramp and seeds Davout τ_ff limiter (`loop.rs:797-826`).
- Stop-generation/fault-aware intent discard (`loop.rs:1459-1471`, `1048-1057`; codemap `codemap.md:20-21`).
- Enable-session neutral bootstrap (≤2 grace ticks, zero gains/torque) and `MissingFeedback` (`loop.rs:1104-1137`, `1388-1423`).
- Position hold law, planner, setpoint mapping, friction/damping FF, fuses (`position_hold.rs`, `position_trajectory.rs`, `position_setpoint.rs`, `position_feedforward.rs`, `friction.rs`).
- GainRuntime (`gain_runtime.rs`), TorqueCmdLatch (`torque_cmd.rs`), MitFeedforward (`mit_feedforward.rs`), in-loop Wave (`position_wave.rs`), CSV trace (`position_trace.rs`).
- Controller-fault classification for its own errors: `MissingFeedback`, `AscentStall`, `HoldTracking` → `Supervisor::latch_control_fault` (`loop.rs:1033-1047`).

Must not (cited) and verification:
| Rule | Source | Status at a2b55b3 |
|---|---|---|
| Never open CAN / call robstride | `AGENTS.md:46,153-156`; `crates/AGENTS.md:42`; `lib.rs:30` | **Holds structurally**: `robstride` only in `[dev-dependencies]` (`Cargo.toml:30`); production src has no `robstride`/socket use (grep). Only `lib.rs:161-164` (test) touches `bus_mut()`. |
| No E-stop / comm watchdog / danger zones / hard enforcement | `lib.rs:31`; `codemap.md:75` | Holds: Berthier only calls Davout APIs; Davout watchdog deadline enforced independently (`codemap.md:57`). |
| No joint↔motor transform / motor sign | `crates/AGENTS.md:43,57`; `docs/rust-patterns.md:106-114` | Holds: reads joint-space `joint_feedback` only (`loop.rs:1477-1501`). |
| No IK / multi-joint timing | `crates/AGENTS.md:45` | Holds. |
| MIT/keepalive/MissingFeedback cover only `active_joints()` | `docs/rust-patterns.md:187` | Holds for output (`loop.rs:724-730,1383,1393-1398`) and feedback check (`loop.rs:1113-1128`); **but** τ_g still consumes default `q=0.0` for non-reporting peers (lead L5). |
| Never mark homing Verified | `loop.rs:707-711` | Holds in Berthier; bins call `supervisor_mut().set_homing_complete` directly (6 sites, marengo-pi), outside Berthier. |
| Boundary bypass | prior arch gap 4 (`control.md` "Architectural gaps" #4) | **Open**: `supervisor_mut()` exposes the whole Supervisor; marengo-pi uses it 49×, motor-repl 18× (incl. `disable_all`, `enable_targets`, `request_enable`, `bus_mut`, `send_joint_command`, `send_speed_command`). Not a Berthier CAN violation, but the facade is shallow. |

## 4. Interface

| Group | Public surface | Consumers | Depth notes |
|---|---|---|---|
| Construction | `ControlLoop::from_repo` (`loop.rs:376`), `from_repo_with_physical_reference` (`:396`, ADR 0036); simulation family on `ControlLoop<SimulationBus>`: `from_simulation` (`:328`), `from_simulation_with_calibration_record_path` (`:348`), `from_simulation_with_reference_journal` (`:237`), `from_simulation_with_current_reference_journal` (`:265`), test-feature `from_simulation_with_paused_reference_journal` (`:207`), `from_simulation_with_paused_current_reference_journal` (`:292`). All funnel through private `from_repo_inner` (`:415-461`) with a `build_supervisor` closure. | `from_repo_with_physical_reference`: `bins/marengo-pi/src/main.rs:1096`, `bins/motor-repl/src/main.rs:194`. `from_repo`: `bins/motor-repl/src/main.rs:207` + many tests. `from_simulation*`: berthier tests, marengo-pi tests. `from_simulation_with_reference_journal` and `from_simulation_with_current_reference_journal`: **zero callers** (grep). | One construction path, good seam (closure). Simulation constructors are concrete-type gated (`impl ControlLoop<SimulationBus>`), matching ADR 0031 intent. |
| Tick & telemetry | `tick(Option<&Bus>)` (`:1029`), `take_tick_phase_averages` (`:464`), `TickPhaseAverages`, `tick_count`, `loop_period`, `configured_loop_hz`, `proto_control_mode` (`:1597`) | marengo-pi `main.rs:1432,1576`; motor-repl | Deep: one call hides drain/admission/bootstrap/compose/send. |
| Mode control | `set_control_mode`, `control_mode`, `enter_torque_only_zero`, `set_torque_cmd`, `torque_cmd`, `clear_torque_cmd`, `clear_torque_cmds`, `inhibit_motion_for_shutdown` | marengo-pi (set_control_mode 18× with Disabled/GravityComp/Impedance only), motor-repl; `clear_torque_cmds` zero callers; `clear_torque_cmd` tests only | `set_control_mode` silently no-ops while reference busy (`:771-777`). |
| Position hold | `enter_position_hold`, `enter_position_hold_at`, `set_joint_position_setpoint`, `latch_position_setpoints`, `clear_position_hold`, `position_setpoints`, `position_hold_commands`, `ensure_active_for_motion`, `start_position_wave`, `position_wave_active` | marengo-pi (`enter_position_hold_at` 4×, `start_position_wave` 4×, `enter_position_hold` 2×); `latch_position_setpoints` only internal (`:701`) | Position only reachable via these (bins never `set_control_mode(Position)`). Arm+seed logic duplicated at `:475-481`, `:739-745`, `:660-667`. |
| Gains | `apply_gain_override(s)`, `clear_gain_override`, `clear_all_overrides`, `gain_override`, `GainOverride`, `mode_allows_gain_override` | marengo-pi `main.rs:630-644`, `overlay.rs:27`; `clear_all_overrides` tests only | Silent no-op outside Impedance/Position returns `Ok` (`gain_runtime.rs:93-99`). |
| Dynamics passthrough | `preview_gravity_torques` (`:1540`), `dynamics_model() -> &dyn DynamicsModel` (`:1545`) | motor-repl `main.rs:46,449`; marengo-pi `main.rs:831` | `DynamicsModel` trait has **one adapter** (`armee-dynamics/src/urdf_gravity.rs:236`) → hypothetical seam; `ControlLoop.dynamics` is concrete `UrdfGravityModel` (`loop.rs:102`), so no plant/model injection for tests. |
| Supervisor access | `supervisor()`, `supervisor_mut()` | marengo-pi 43+49, motor-repl 18 | Shallow escape hatch (see §3). `MotorBus` seam belongs to Davout (≥4 production adapters in robstride/davout). |
| Errors | `LoopError` (`:37-83`), `HoldFuseTrip` (re-export `lib.rs:77`), `ControlError` (`lib.rs:84-88`) | `LoopError`: marengo-pi `overlay.rs:65`, tests; `ControlError`: none outside lib.rs | |
| Legacy facade | `Controller<B>` (`lib.rs:90-131`): `new`, `from_repo`, `supervisor_mut`, `mode`, `command_position` | **None** outside its own unit test (`lib.rs:139-169`) | Wraps Davout legacy `send_joint_command` which emits kp=0/kd=0 (test asserts zero gain bytes `lib.rs:168`; prior CS20). |
| Internal modules (crate-private) | `PositionHold` (`position_hold.rs:321`), `GainRuntime` (`gain_runtime.rs:70`), `MitFeedforward` (`mit_feedforward.rs:36`), `JointPositionPlanner` (`position_trajectory.rs:16`), `PositionWave`, `PositionTrace`, `TorqueCmdLatch` | `ControlLoop` only | `PositionHold::tick(HoldWorld)` is a deep, directly-testable law seam (used by `position_hold_tests/*`). |

No CLI/stdin/HTTP surface of its own (library); operator commands arrive via marengo-pi/motor-repl.

## 5. Invariants owned

| # | Invariant | Enforcing code (file line coverage, `metrics/coverage-by-file.md`) | Tests that fail if broken |
|---|---|---|---|
| I1 | Never touch CAN; all output via `Supervisor::send_mit_batch` | dependency graph (`Cargo.toml:16-24`), `loop.rs:1384,1420` | none (structural only) |
| I2 | MIT only for Davout `active_joints()` | `filter_mit_to_active` `loop.rs:724-730`; bootstrap filter `:1398` | `loop.rs:1732` `tick_partial_enable_sends_mit_only_for_active_joints`; `tests/cs24_inactive_peer.rs:32` |
| I3 | No τ_g/PD from missing or prior-session pose; ≤2 neutral solicit ticks per enable session, then `MissingFeedback` → controller fault | `loop.rs:1104-1137,1388-1423,1034-1037` | `tests/feedback_bootstrap.rs:90,105,142`; `tests/feedback_failure_propagation.rs:309`; `loop.rs:1781,1815` |
| I4 | Davout stop generation change (incl. disable/re-enable between ticks) discards planner/Wave/torque/gain intent | `synchronize_stop_generation` `loop.rs:1465-1471`, `discard_motion_intent` `:1459-1463` | `tests/feedback_failure_propagation.rs:335`; `bins/marengo-pi/src/shutdown_tests.rs:1054` |
| I5 | Post-send receive/safety errors propagate before success | `loop.rs:1384-1386,1420-1422` | `tests/feedback_failure_propagation.rs:81,106,156` |
| I6 | No new intent while Davout fault authority latched or refresh fails | `refresh_joint_positions` `loop.rs:1445-1457`; `set_torque_cmd` `:855` | `tests/feedback_failure_propagation.rs:192,213,234,256,282`; `tests/cs24_controller_stiction.rs:55` |
| I7 | No controller intent/receive while a Davout reference reservation is busy; tick returns Ok after owner advance | `loop.rs:772-785,843,929,945,1066-1095` | `tests/reference_intent_admission.rs:12`; `tests/reference_transaction_owner.rs:311,328`; `src/reference_grant_tests.rs` |
| I8 | AscentStall: commanded non-home target above `q` by > return band, no new credited high for 2000 ms nominal → error → controller fault | `position_hold.rs:1066-1082,1269-1281`, `ProgressBudget` `:51-98`; `loop.rs:1038-1041` | `position_hold_tests/{encoder_stop.rs:59, progress_matrix.rs:226, period_contract.rs:126, numeric_contract.rs:169,206}`; `tests/cs24_controller_stiction.rs:55`, `cs24_one_code_crawl.rs:266`, `cs24_inactive_peer.rs:32`; `loop.rs:2628,2693` |
| I9 | HoldTracking: `|q−target|>0.03`, net `tau_p+tau_ff` opposing, no new closest for 2000 ms → error → controller fault | `position_hold.rs:167-171,1284-1300`; `loop.rs:1042-1045` | law level: `position_hold_tests/hold_tracking.rs:108,147,241,272,281`. **Controller-level latch/discard (`loop.rs:1042-1057`) untested** (no `HoldTracking` match in `tests/` or `loop.rs` tests) |
| I10 | Home latch: target within 2 feedback counts of 0 commanded as exactly 0.0 | `home_classified` `position_hold.rs:410-421`; `home_target_tolerance` `position_setpoint.rs:25-28` | `hold_tracking.rs:197`; `loop.rs:2593` |
| I11 | Position `q_des` inside limit envelope | `position_hold.rs:1102-1113,1228-1230`; `clamp_hold_target` `loop.rs:598-603` | `loop.rs:3080,2870,3093` |
| I12 | Planner `v_max` ≤ Davout velocity cap (ADR 0010) | `position_hold.rs:790-792,863-884`; `loop.rs:605-609` | **untested in Berthier** (every fixture uses `velocity_cap: Some(2.0)`; Davout enforces same cap) |
| I13 | TorqueOnly: `τ_ff=τ_cmd`, kp=kd=0; finite/known-joint only; cleared on leave; `gravity-off` ⇒ τ_cmd≡0 | `mit_feedforward.rs:50`; `loop.rs:811-813,832-861` | `mit_feedforward.rs:118,133`; `loop.rs:1914,1927,1940,1954,1963,1975` |
| I14 | Gain overrides: finite, ≥0, known joint, preflighted batch, clamped to motor-type limits; ignored/cleared outside Impedance/Position | `loop.rs:924-979`; `gain_runtime.rs:86-126,141-171,297-337` | `tests/gain_validity.rs:28,56`; `loop.rs:2004,2028,2052,3217-3377`; `gain_runtime.rs:461-569` |
| I15 | Mode change slews: ramp 20 ticks (non-Disabled↔non-Disabled) and seeds Davout τ_ff limiter | `loop.rs:819-825`; `gain_runtime.rs:157-170` | `gain_runtime.rs:499,522,547,569`; limiter seeding itself tested in Davout (`davout/src/lib.rs:3627`) |
| I16 | Position non-gravity FF (τ_f, τ_d) independent of τ_g | `position_feedforward.rs:21-66` (τ_g only summed at `:64`) | `mode_isolation.rs:28` proptest (varies τ_g only, all other inputs fixed) |
| I17 | Impedance friction override reaches wire; GravityComp ignores it | `mit_feedforward.rs:49-60` | `tests/friction_mode_output.rs:50` |
| I18 | Zero-rounded loop period rejected before config/reference | `loop.rs:423-429`; `position_hold.rs:661-665` | `tests/cs24_constructor_period.rs:16`, `tests/cs24_period_precedence.rs:7`, `period_contract.rs`, `numeric_contract.rs:206` |
| I19 | Shutdown inhibit discards retained intent, no drive writes | `loop.rs:793-795` | `bins/marengo-pi/src/shutdown_tests.rs:1054` |
| I20 | Simulation construction admits only declared virtual joints; ordinary construction unreferenced | `loop.rs:205-373` | `tests/simulation_admission.rs:13,29`; `tests/simulation_resources.rs:124` |

File coverage behind the invariants: `loop.rs` 88.7 % (I2–I7, I12, I18–I20; also the fault-latch match arms in `tick`), `position_hold.rs` 94.6 % (I8–I12), `gain_runtime.rs` 96.5 % (I14–I15), `mit_feedforward.rs` 97.9 % (I13, I17), `position_feedforward.rs` 100 % (I16). Line coverage at file level does not show that the `HoldTracking` arm at `loop.rs:1042-1045` runs: no test names it (G1). The low coverage of `position_trace.rs` (52.4 %) is in diagnostics, not a safety invariant.

## 6. Inputs / outputs

| Kind | Item | Where |
|---|---|---|
| Config dir | `resolve_config_dir(root)` → `MARENGO_CONFIG_DIR` env, else `/opt/marengo/config` if present, else `<root>/config` (`marengo-config/src/lib.rs:219-228`); simulation constructors force `<root>/config` | `loop.rs:385,407,219-342` |
| robot.yaml | `robot.joints` (ordered joint list), `robot.urdf` | `loop.rs:430-433` |
| URDF | `UrdfGravityModel::from_urdf(urdf, joints)` once at construction (no hot reload) | `loop.rs:433` |
| control.yaml `joints.<j>` (via `supervisor.control`) | `gravity_comp{kp,kd}`, `impedance{kp,kd,ki}`, `friction{fc,fv,fo,k}`, `position_slew_rad_s`, `position_slew_max_lead_rad`, `position_trajectory_velocity_deadband_rad`, `position_trajectory_velocity_rad_s`, `position_trajectory_threshold_rad`, `position_trajectory_accel_rad_s2`, `position_hold_trim_rad`, `motor_type` | `loop.rs:508-515,586-596,611-626,879-896,1007-1026,1172-1208,1363-1375` |
| control.yaml `motor_type_defaults.<t>` | `kp_max`, `kd_max`, `tau_ff_max_nm` | `loop.rs:992-1005` |
| Davout-derived | `joint_feedback` (q, dq, τ, temp, fault), `joint_limit_policy`, `joint_velocity_cap`, `joint_position_progress_threshold`, `joint_drive_active`, `active_joints`, `enable_session_started_at`, `enable_writes_pending`, `stop_generation`, `reference_busy/work_pending`, `joint_commissioning_wire` | `loop.rs` passim |
| Env vars | `MARENGO_POSITION_TRACE` (CSV path), `MARENGO_POSITION_TRACE_HZ`, `MARENGO_POSITION_ONSET_LOG_MS` (default 250) | `position_trace.rs:19,24`; `loop.rs:1579` |
| Files written | position trace CSV, append, 512 KiB BufWriter, no rotation | `position_trace.rs:30-32` |
| Journal path | physical reference journal (absolute, distinct from calibration history; caller resolves via `marengo_config::resolve_reference_journal_path`) | `loop.rs:393-412` |
| Chappe publish | topic `robot/state`, source `berthier`, type `marengo.v1.RobotState` (per-joint position/velocity/effort/temp/fault/homing_state/drive_active/out_of_limits), rate `chappe_hz` | `loop.rs:1503-1537` |
| CAN | none directly; MIT batches via `Supervisor::send_mit_batch` | `loop.rs:1384,1420` |
| HTTP | none | — |

## 7. Prior review reconciliation

| Prior id | Prior status | Current status @a2b55b3 | Evidence |
|---|---|---|---|
| CS01 (Berthier part: no PD from missing pose) | verified PR214 | **fixed** | `loop.rs:1104-1137,1388-1423`; tests `feedback_bootstrap.rs:90,105,142` |
| CS03 (Berthier part: gain/torque admission) | verified PR214 | **fixed** | `loop.rs:850-855,959-979`; `tests/gain_validity.rs:28,56`; `loop.rs:1963` |
| CS10 (PD torque unbounded by bench cap) | open | **open** (Davout-owned policy; Berthier still sends `kp` up to `kp_max` with `tau_p` unbounded in Berthier, `position_hold.rs:1261,1365`) | ledger CS10 `status: open` |
| CS11 (limiter seeding on mode change) | verified PR213 | **fixed** (Berthier side) | `loop.rs:819-825` |
| CS12 (post-send drain errors discarded) | verified PR215 | **fixed** | `loop.rs:1384-1386,1420-1422`; `tests/feedback_failure_propagation.rs:81,106,156` |
| CS13 (transient faults / auto re-arm) | partial | **partial** (Berthier side done: stop-generation discard + controller fault latch `loop.rs:1033-1057,1465-1471`; `ensure_active_for_motion` still auto-enables when not latched `loop.rs:712-721`) | ledger CS13 evidence |
| CS19 (first Testing hold ignores gains) | open | **open**: `GainRuntime::apply` still silently ignores outside Impedance/Position and returns Ok (`gain_runtime.rs:93-99`; `loop.rs:924-935`); ordering lives in marengo-pi | ledger CS19 |
| CS20 (CLI no-op effects; legacy kp=0 position) | open | **open**; Berthier's unused `Controller::command_position` (`lib.rs:115-130`) is the same kp=0 legacy path | ledger CS20; `lib.rs:168` |
| CS24 (velocity-tail resets ascent fuse) | verified PR223 / `eb1359c` | **fixed** (ADR 0025 measured high-water `ProgressBudget`) and extended by `123a433` HoldTracking | `position_hold.rs:51-133`; tests `cs24_*`, `position_hold_tests/*` |
| CS23 / CS21 (gravity transform / stale tests) | CS23 verified, CS21 partial | consumer only; no Berthier change needed | `loop.rs:433,1141` |
| Arch gap 4 (safety depends on callers; `bus_mut` etc.) | open | **open** for Berthier's `supervisor_mut()` facade (§3) | `loop.rs:758-760`; bin usage counts |
| Arch gap 5 (system-level plant tests for PositionHold) | open | **open**: no independent plant; tests are law/fixture level (`codemap.md:43` says so) | `implementation-roadmap.md:115-119`; `control-implementation-plan.md:512-516` |
| Arch gap 7 (per-tick allocation / realtime bounds) | open | **open** (lead L12) | `loop.rs:1113,1149,1162-1210,1358-1377`; `position_hold.rs:1346` |
| test-quality-plan row `mode_isolation::impedance_tau_f_independent_of_tau_g` (REPLACE then REMOVE) | — | **done**: property removed; replaced by `tests/friction_mode_output.rs:50` | `mode_isolation.rs:8-9`; `test-quality-plan.md:52,176` |
| test-quality-plan row `position_non_gravity_ff_independent_of_tau_g` (KEEP and strengthen) | — | **kept, not strengthened**: still one fixed operating point, only τ_g varied; no mode-transition output coverage for Position | `mode_isolation.rs:28-50`; `test-quality-plan.md:53` |
| CS13/CS15 control-plan C3 "one MotionSession; Berthier as executor" | planned | **not started** in Berthier | `control-implementation-plan.md:76` |

## 8. Drift

| Doc claim | Code reality |
|---|---|
| `codemap.md:32` module `position_friction` | no such module; friction lives in `src/friction.rs` (`lib.rs:51`) |
| `codemap.md:34`, `loop.rs:128`, `loop.rs:633` "triangle wave"; `src/codemap.md:16` "Sine-wave" | raised-cosine wave (`position_wave.rs:1-6,53-58`) |
| `src/codemap.md:11` position_feedforward = "PD torque" | composes `τ_g+τ_f+τ_d`, no P term (`position_feedforward.rs:19-65`) |
| `src/codemap.md:13` position_profile "Trapezoidal/s-curve profiles" | only cruise `v_max` selection + `PlannerEvent` (`position_profile.rs:33-60`); trapezoid is in `position_trajectory.rs:122-177`; no s-curve anywhere |
| `src/codemap.md:15` position_trajectory "Time-parameterized position paths" | per-tick trapezoid state machine (`position_trajectory.rs:93-98`) |
| `README.md:9` consumes Talleyrand setpoints | no talleyrand dependency (`Cargo.toml:16-24`) |
| `codemap.md:6,17`, `lib.rs:20` Legacy `Controller` for "REPL / bring-up" | no REPL/bin uses it (grep); motor-repl uses `ControlLoop` + `Supervisor::send_joint_command` directly |
| `codemap.md:74` "Called by: marengo-pi" | also motor-repl (`bins/motor-repl/src/main.rs:8,194,207`) and wave-demo (dependency only, unused: `bins/wave-demo/src/main.rs` is a 4-line scaffold) |
| `CONTEXT.md:15`, ADR 0004 table: GravityComp kp/kd hard-zero | YAML-driven + ramp (`gain_runtime.rs:359-361,383-386`; `mit_feedforward.rs:49`) |
| `docs/position-hold-control-review.md:63` `kd_mit = 0`; `docs/safety.md` danger-zone bullet "when Berthier sends `kd_mit = 0`" | `kd_mit = kd` while moving near target (`position_hold.rs:1308-1312`) |
| `lib.rs:31`, `AGENTS.md:50` "does not apply limits" | clamps targets/`q_traj`/`q_des` to envelope and `v_max` to cap (ADR 0009/0010; `position_hold.rs:1102-1113,1228-1230`) |
| ADR 0025 "Actual loop construction rejects a zero-rounded period" | `loop_hz = 0` is silently coerced to 1 Hz (`loop.rs:423`) rather than rejected; only absurdly high rates hit `InvalidLoopPeriod` |
| ADR 0004 "Impedance kp/kd from control.yaml" (implies stiffness) | Impedance `q_des = measured q` (`mit_feedforward.rs:51-60`), so kp gives ~no restoring torque; `docs/tuning.md:42` documents this ("setpoint tracks measured q") — ADR silent |
| `AGENTS.md:197` names `PositionTrace::flush()` as a never-call | function is `#[allow(dead_code)]` with zero callers (`position_trace.rs:60-63`) |
| `codemap.md:12` "no unchecked supervisor-injection factory" | true for `ControlLoop`; `Controller::new(Supervisor)` (`lib.rs:96`) is one, unused |

## 9. Prune candidates

| # | Candidate | Evidence class | Conf. | Deleting touches |
|---|---|---|---|---|
| P1 | Legacy `Controller<B>` + `ControlError` (`lib.rs:84-131`) and its test (`lib.rs:133-170`) | zero references incl. bins/tools/consul (grep: only `lib.rs`); superseded by `ControlLoop` (`codemap.md:6`) | high | `lib.rs`, `codemap.md:6,17`, `src/codemap.md:18`, `lib.rs:20` doc |
| P2 | `ControlLoop::from_simulation_with_reference_journal` (`loop.rs:235-261`) and `from_simulation_with_current_reference_journal` (`loop.rs:263-289`) — ungated `pub` | zero callers anywhere (grep); only the `paused_*` test-feature variants are used (`reference_journal_tests.rs:47`, `reference_grant_tests.rs:78`, marengo-pi `reference_journal_shutdown_tests.rs:223`) | high (verify with AuditDavout that Davout's matching factories are not otherwise needed) | `loop.rs`; `codemap.md:67-72`, `lib.rs:25-26` wording |
| P3 | `clear_torque_cmds` (`loop.rs:869-871`) | zero references incl. tests (grep; `metrics/pub-usage.md:13` also lists it under zero use) | high | `loop.rs` |
| P4 | Disabled stub `approach_stuck_mit_pull` (always `false`) + `approach_stuck_mit_pull_lead_rad`, `outbound_low_angle_stuck`, `outbound_low_angle_stuck_pull_rad` (`position_setpoint.rs:183-241`, all `#[allow(dead_code)]`) and test `loop.rs:2498` | superseded (doc says "disabled"; production uses bounded planner recovery, `position_setpoint.rs:212-216`); test pins a stub | high | `position_setpoint.rs`, `loop.rs:1614` import, `loop.rs:2498-2535` |
| P5 | `trajectory_friction_torque` (`friction.rs:202-…`, `#[allow(dead_code)]`) + tests `friction.rs:613-626` | zero production refs; tests pin a dead helper | high | `friction.rs` |
| P6 | `trajectory_damping_torque` (`position_trajectory.rs:179-183`) + tests `position_trajectory.rs:396-410`, `loop.rs:2978` | zero production refs (production uses `position_hold_damping_torque`) | high | `position_trajectory.rs`, `loop.rs` tests |
| P7 | `PositionTrace::flush` (`position_trace.rs:60-63`) | zero references | med (could instead be wired to shutdown to avoid trace loss, lead L11; `AGENTS.md:197` mentions it) | `position_trace.rs`, `AGENTS.md:197` |
| P8 | `PositionHold::targets_raw` (`position_hold.rs:387-391`, "ControlLoop may wire later") | scaffold with no production consumer (tests only: `hold_tracking.rs:205`, `numeric_contract.rs:80`, `loop.rs:2610`) | med | tests switch to diag `target_raw` |
| P9 | `clear_all_overrides` (`loop.rs:987-989`), `clear_torque_cmd` (`loop.rs:864-866`), `apply_gain_overrides` (`loop.rs:941-957`) | test-only callers (`loop.rs:3347`, `loop.rs:1922`, `tests/gain_validity.rs:73`; `metrics/pub-usage.md:38` flags `apply_gain_overrides` as own-crate-tests-only, and grep finds no bin caller: marengo-pi uses per-joint `apply_gain_override` at `main.rs:630`) | low-med (operator API may be wanted by Consul later; batch preflight is part of I14, so a deletion must keep per-joint validation) | `loop.rs`, `tests/gain_validity.rs:56-80` |
| P10 | `ADVANCE_MAX_LEAD_DEFAULT` 0.10 vs 0.15 split and other `unwrap_or` fallbacks for missing `control.yaml` joint entries (`position_hold.rs:38-40`; `loop.rs:515,617-623,1153-1157,1184-1203`) | both fields read the same config key `position_slew_max_lead_rad` (`loop.rs:1184,1189`); fallbacks only live when a robot.yaml joint lacks control.yaml config [INFERENCE: config validation may already require entries — confirm with AuditConfigHoming] | low | `loop.rs`, `position_hold.rs` |
| P11 | Unused params `_q`, `_target` of `position_hold_mit_kd` (`position_setpoint.rs:170-176`) | signature cruft | low | call site `position_hold.rs:1309` |
| P12 | Duplicate arm+seed sequences (`loop.rs:475-481`, `:739-745`, `:660-667`); `GainRuntime::wire_gains_now` duplicating wire half of `resolve_all` (`gain_runtime.rs:193-222` vs `:238-290`) | duplicate implementation | low (consolidate, not delete) | `loop.rs`, `gain_runtime.rs` |
| P13 | `wave-demo` bin's `berthier` dependency (`bins/wave-demo/Cargo.toml:18`) | scaffold with no consumer (main logs one line); `metrics/unused-deps.md:23` (cargo machete) flags it | high (for the bins audit) | `bins/wave-demo` |

Suppression evidence (`metrics/suppressions.md:40-59`): each `#[allow(dead_code)]` site in P4–P8 is listed there (`friction.rs:203`, `position_trajectory.rs:180`, `position_hold.rs:388`, `position_setpoint.rs:185,205,218,233`, `position_trace.rs:60`). Two items in `pub-usage.md` were missed by its heuristic (it scans only `crates/*/src` and counts names without paths) and are confirmed here by grep: P1 `Controller` and P2 `from_simulation_with_{,current_}reference_journal`.

Not prunable (keep; gaps listed in §10): fuses, MissingFeedback/bootstrap, stop-generation discard, reference-busy refusal, `inhibit_motion_for_shutdown`.

## 10. Phase-B leads

| # | Lead | Where | Why suspicious |
|---|---|---|---|
| L1 | Fuse torque uses **law** kp, wire uses **resolved** kp | `position_hold.rs:1261,1286` vs `loop.rs:1234` | During a 20-tick ramp (e.g. GravityComp→Position ramps wire kp from 0, `gain_runtime.rs:157-170`) HoldTracking's `tau_p` and diag/trace `kp` describe torque that was not sent. Same for `HoldFuseTrip.tau_p` evidence. |
| L2 | "Ascent" = positive joint direction; home = 0 lower side | `position_hold.rs:1066-1068` (ADR 0025 "positive joint direction"); `position_setpoint.rs:267-271,284-291`; `position_hold.rs:1219-1227` (stuck pulls `min(q−pull)`, `max(target+pull)`) | Joints whose gravity-opposing direction is negative q (roll/yaw/mirrored left arm) get no AscentStall and direction-wrong breakaway pulls; only HoldTracking (opposing-torque gated) remains. Sign assumption tuned on right shoulder pitch. |
| L3 | Wave joint is fuse-free and unvalidated | `position_hold.rs:911-921` (resets ascent budget every tick), `:1285` (tracking exempt); `loop.rs:645-653` (`NaN`/`inf` pass `>=`/`<=` checks) | `q_traj` is set directly to the wave target (`resume_cruise_toward`), bypassing planner accel and velocity-cap shaping (only dq FF clamped `:911`); amplitude not checked against envelope. Only lead clamp + Davout protect. |
| L4 | Non-finite hold target admitted | `loop.rs:492-584` (no `is_finite`); `armee-kinematics/src/limits.rs:136` (`clamp` propagates NaN); `position_hold.rs:492` (`(o−NaN).abs()>1e-6` is false → `changed=false`, no retarget but NaN latched) | Relies on Davout CS03 rejection → fault/disable instead of a clean refusal; trims NaN into `setpoints`. |
| L5 | τ_g with `q = 0.0` for joints lacking feedback | `read_positions` `loop.rs:1477-1487` feeding `gravity_torques` `:1141`; MissingFeedback checks only active joints `:1113-1128` | Under scoped enable a non-reporting distal peer is modeled at 0 rad, biasing τ_g of enabled proximal joints. Davout free-drive feedback TTL may hide peers (`loop.rs:1504-1505`). |
| L6 | Telemetry failure stops motion | `loop.rs:1087,1433` (`?` on Chappe publish) → marengo-pi `main.rs:1432-1440` `disable_all()` on any tick Err | `chappe::Bus::publish_bytes` can return Err on a receiver race (`chappe/src/lib.rs:115-119`) and takes an `RwLock` read in the tick (`:109`). In the reference-busy branch the Err follows a completed `advance_reference_work` and skips `tick_count += 1`. |
| L7 | Contradictory fault policy between Berthier and marengo-pi | `loop.rs:1048-1057` discards intent on **every** `LoopError::Safety` | marengo-pi `main.rs:1437-1441` tries to preserve Position hold on `CommWatchdog` — dead branch; clarify intended behavior. |
| L8 | Any >1e-6 retarget resets both fuse budgets | `mark_retarget` `position_hold.rs:743-759`, `set_clamped_target` `:484-497` | A stream of small retargets (Testing jog, future Talleyrand stream, `hold-at` repeat with trim) every <2 s keeps a sagging/stalled joint unfused indefinitely. ADR 0025 accepts "true retarget ends the episode" but does not bound retarget rate. |
| L9 | Fuse and planner time are nominal, not wall | `HoldWorld.dt = loop_period` `loop.rs:1222`; `ProgressBudget` `position_hold.rs:86` | Tick overruns stretch the 2000 ms fuse in wall time and slow the reference (ADR 0025 explicitly scopes this out; still a timing assumption). |
| L10 | Panics in tick path | `assert_eq!` `gain_runtime.rs:199-203,244-248` | `crates/` rule forbids unwrap-style panics; invariant held only by caller construction (`loop.rs:1158,1354`). |
| L11 | Position trace blocks/loses data | `position_trace.rs:32,56` (512 KiB BufWriter → periodic `write(2)` inside tick, the SD stall `AGENTS.md:197` warns about); errors swallowed `loop.rs:1337` (also listed in `metrics/suppressions.md:78`); append-only unbounded file; `from_env` silently drops open errors (`position_trace.rs:20`); no flush on exit (P7); file has the lowest coverage in the crate, 52.4 % (`metrics/coverage-by-file.md:23`) | Env-gated debug path, but it runs in the 200 Hz loop. |
| L12 | Per-tick allocation | `loop.rs:1113,1149,1162-1210` (Vec, `FrictionGains` clone, String clones), `:1358-1377`; `position_hold.rs:1326-1370` (String names, `format!("{traj_phase:?}")` per joint per tick even when not logged, `:1346`) | Engineering rule "NEVER avoidable allocation" + prior arch gap 7; jitter on Pi unmeasured. |
| L13 | Inconsistent missing-config fallbacks | slew 0.15 (`loop.rs:515`) vs 0.25 (`:617,1194`); Position default impedance kp 20/kd 1 (`:1153-1157`) vs ramp endpoints 0/0 (`:880-884`) vs Impedance zero (`:1349-1353`); max_lead 0.15 vs 0.10 (`:1184-1190`) | Fail-open defaults (nonzero stiffness) for an unconfigured joint; harmless only if config guarantees entries. |
| L14 | Silent no-ops return success | `set_control_mode` while reference busy (`loop.rs:771-777`); `apply_gain_override(s)` outside Impedance/Position (`gain_runtime.rs:93-99,114-117`); batch entries without limits skipped (`:119-122`) | Callers cannot tell intent was dropped (root of CS19). |
| L15 | GravityComp not hard-zero; ramp into GravityComp | `gain_runtime.rs:157-170,359-361,383-386`; `mit_feedforward.rs:49` | Leaving Impedance/Position ramps nonzero kp/kd into GravityComp for 20 ticks; ADR 0004/CONTEXT.md assume 0/0. Low risk because `q_des = q`, but kd acts on velocity. |
| L16 | Ramp length tick-based | `gain_runtime.rs:167-168` | 20 ticks = 100 ms only at 200 Hz. |
| L17 | Integral state persists outside its window | `position_hold.rs:1253-1259` | Integral accrues only when `|e|<0.1` and age >1 s but is reset only on retarget/arm; stale integral re-applies on re-entry. Clamp `MAX_INTEGRAL_NM / ki` assumes ki>0 (guarded). |
| L18 | `loop_hz = 0` coerced to 1 Hz | `loop.rs:423` | Silent reconfiguration vs ADR 0025 rejection language. |
| L19 | Error misclassification | `HoldError::LenMismatch → MissingSetpoint{"len_mismatch"}` `loop.rs:91-93` | Hides an internal invariant break as an operator error; not in fault-latch list (`loop.rs:1033-1047`). |
| L20 | Trapezoid with non-positive `a_max`/`v_max` | `position_trajectory.rs:140-157` | `a_max=0` → `stop_dist=inf`, planner frozen; negative `v_max` reverses direction. Depends on CS15 config validation (partial). |
| L21 | Impedance stiffness is illusory | `mit_feedforward.rs:51-60` (`q_des = q`, `v=0`) | Testing kp overrides in Impedance have ~no effect; only kd/τ_g/τ_f act. Documented in `docs/tuning.md:42` but not in ADR 0004 — confirm intent. |
| G1 (gap) | HoldTracking controller-level path untested | `loop.rs:1042-1045,1048-1057` (`loop.rs` is 88.7 % covered overall, `metrics/coverage-by-file.md:66`) | No test drives `ControlLoop::tick` into `HoldTracking` → controller fault + intent discard. |
| G2 (gap) | Berthier velocity-cap clamp untested | `position_hold.rs:790-792,863-884`; `loop.rs:605-609` | All fixtures use `velocity_cap: Some(2.0)`. |
| G3 (gap) | No independent plant for PositionHold | `codemap.md:43`; prior arch gap 5 | Fuses/planner validated against scripted q only; wrong-τ_g scenarios (`hold_tracking.rs`) are law-level replays. |
| G4 (gap) | Mode-isolation proptest is one operating point | `mode_isolation.rs:28-50` | Only τ_g varies; property is structurally guaranteed by `position_feedforward.rs:64` signature. |
| G5 (gap) | Uncommanded/wave exemption for HoldTracking untested | `position_hold.rs:1284-1286` | `cs24_inactive_peer.rs` predates `123a433` and asserts AscentStall scope only. |

Excluded per assignment: the hold-on/enable latch issue (fix in flight on main, not in this baseline).
