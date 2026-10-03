# Contracts inventory — config, proto, gateway routes and Chappe topics

Baseline: audit worktree `/Users/joseph/code/marengo-wt/audit`, commit `a2b55b3`, 2026-10-03. Phase A of the crate audit, read-only. Grep scopes are stated in each section. "Unused" never counts generated code (`consul/src/gen`, prost output) as a consumer. Safety, fault, stop and watchdog items are listed as gaps, never as prune candidates.

## Summary

| Inventory | Totals | Flags | Highest-value flags |
|---|---|---|---|
| 1. Config | 79 YAML key paths (+1 network.yaml), 109 struct fields | C01–C27 | C01 no unknown-key rejection (typo → serde default, e.g. watchdog enabled); C02 persist erases YAML comments; C05 five velocity "limits" in master that contradict each other; C21 `homing.defaults.method` never read; C22 Hall/search homing schema with no behaviour; C24 `network.yaml` placeholder; C18 `_rad` suffix on a rad/s field |
| 2. Proto | 59 messages, 308 fields, 11 enums / 47 values, 0 services / 0 rpcs, 0 `reserved` | P01–P30 | P01/P02 11 zero-reference messages + 1 enum (log-store mirror that has drifted from the JSON wire); P03 `HomingComplete` retired; P04 five motion `ActuatorCommand` variants rejected end to end; P08 e-stop fields never read by the UI (safety gap); P17 `batch.mode == 4` magic and in-band `wave:` command; P26 `buf breaking` FILE rules block field/enum deletion |
| 3. Gateway + Chappe | 44 HTTP routes + 2 static + 1 WebTransport path; 19 topics | R01–R12, T01–T09 | 0 mutating/motion routes without auth; G07 fixed (PR #245, `df46849`) and not regressed; R01 Consul posts to a missing `/command/mit`; R06 MCP log listing always gets 401 and falls back silently; R07 public `/version/status?refresh` has side effects; T01 `robot/homing` subscribed but never published; T03 `robot/testing/telemetry` copied at state rate, never subscribed |

**Cross-references (same fact found from two sides):** R01 = P18; R03 = P30; R12 = P05; T01 = P03; T04 = P06; T05 ≈ P16 (MIT batch echo); T02 ≈ P13 (no Jetson producer).

**Prior findings:**

| Finding | Status | Evidence |
|---|---|---|
| G07 | fixed, not regressed | §3 G07 block; ledger "verified"; finding-index.md:56 still says Open (R11) |
| CS15 | partial | §1 prior findings |
| CS10 | open | §1 prior findings |
| T03 | open | §1 prior findings |
| T04 | partial (ledger) | not re-verified here |

**Phase-B guardrail:** to delete any proto item you need a `buf breaking` exception or a deprecate-then-reserve step (P26).

## 1. Config contracts: `config/{robot,motors,control,homing}.yaml` ↔ `marengo-config`

**Counts.** Master YAML has **79 distinct key paths**: robot 6, motors 13 per entry × 5 entries, control 48, homing 12. `network.yaml` adds 1 more (`chappe_bind`). The `marengo-config` structs have **109 leaf fields**: Robot+Bench 7, MotorEntry+BenchLimits 14, Control family 58, Homing family 29, Network 1. The audit flags **27 config items (C01–C27)**:
- 4 keys never read outside validation or tests
- 11 fields whose only readers are validation or reference-identity checks, with no behaviour behind them
- 13 struct fields absent from master that run on serde defaults
- 6 duplicated meanings
- 5 unit or name ambiguities

Several flags fall in more than one class.

**Method / grep scope ("S").** `git grep -E '\.<field>([^A-Za-z0-9_]|$)'` over `crates bins tools scripts consul/src` (excluding `consul/src/gen`). Readers listed are non-test unless marked. "Validation" means `crates/marengo-config/src/safety_validation.rs` or `lib.rs` validators. "Ref-identity" means the field is only compared in Davout's reference-binding equality (`crates/davout/src/reference.rs:338-404`, `reference_transaction.rs:1467-1474`), so a change revokes references but drives no behaviour.

**Global contract facts**
- **Load path.** `resolve_config_dir` checks `MARENGO_CONFIG_DIR` first, then `/opt/marengo/config` if it exists, then `<root>/config` (lib.rs:219-235). Loaders are at lib.rs:277-310.
- **Env vars.** `MARENGO_ROOT` (lib.rs:210, commissioning_scope.rs:46), `MARENGO_CONFIG_DIR` (lib.rs:220), `MARENGO_REFERENCE_JOURNAL` (lib.rs:243), `MARENGO_CALIBRATION_RECORD` (lib.rs:246), `MARENGO_JOINT_SUBSET` (bench_joints.rs:84).
- **No `deny_unknown_fields` anywhere** (grep over S: 0 hits). A misspelled key is silently ignored and its field takes the serde default (C01).
- **Writers serialize the structs.** `write_control_config_from` is at lib.rs:1070-1089. `profile_txn::serialize_yaml` (profile_txn.rs:299-310) is called from Set Limits persist (bins/marengo-pi/src/limit_persist.rs:463) and `write_motors_and_control` (profile_txn.rs:29). So every persist **drops all YAML comments and unknown keys and writes defaulted fields out explicitly** (C02).
- **Coverage.** lib.rs 78.6%, safety_validation.rs 92.9% (metrics/coverage-by-file.md:38,85).

### 1a. robot.yaml (`RobotConfigFile`, lib.rs:112-132)

| Key (yaml line) | Field | Default if absent | Readers (non-test) | Flag |
|---|---|---|---|---|
| robot.name (5) | `RobotSection.name` | required | validation only (safety_validation.rs:58) | C03 |
| robot.urdf (6) | `.urdf` | required | resolve URDF lib.rs:1289; profile_txn.rs:188,359; MCP gravity-calibrate.ts:441 (awk) | — |
| robot.bench.max_joint_velocity_rad_s (8) | `BenchSection.max_joint_velocity_rad_s` | required | validation only (safety_validation.rs:61). ADR 0010:35 and docs/safety.md:14 forbid using it for caps | C04, C05 |
| robot.bench.max_joint_torque_nm (9) | `.max_joint_torque_nm` | required | Davout effort cap (davout lib.rs:3054); Hall search cap (safety_validation.rs:419) | — |
| robot.joints (10-15) | `.joints` | required | allowlist and joint vector order: davout lib.rs:563; berthier loop.rs:431; gateway config.rs:166; log-cli gravity_fit.rs:213; bench_joints.rs | — |
| robot.limbs (18-30) | `.limbs` | `{}` (lib.rs:124) | log-cli gravity_fit.rs:272 only. Consul hardcodes its own copy (consul/src/lib/commissioning.ts:9-25, "gateway does not yet expose limbs") | C06 |

### 1b. motors.yaml (`MotorEntry`, lib.rs:149-185), per entry × 5

| Key | Field | Default | Readers (non-test) | Flag |
|---|---|---|---|---|
| joint | `joint` | required | everywhere (`motor_for_joint`) | — |
| driver | `driver` | required | ref-identity (reference.rs:340); candump filter `!= "robstride"` (crates/marengo-candump/src/robstride.rs:38). Never validated to equal `robstride` [INFERENCE: no validator in safety_validation.rs:78-129] | C07 |
| motor_type | `motor_type` (enum rs00/02/03/04) | required | MIT ranges davout lib.rs:508; gateway config.rs:125; Consul build-hardware-rows.ts:112 | C08 (duplicated in control.joints) |
| can_interface | `can_interface` | required | marengo-pi main.rs:785,1076; davout active_reporting.rs; gateway config.rs:122 | — |
| device_id | `device_id` | required | marengo-pi main.rs:786; gateway config.rs:123; uniqueness check safety_validation.rs:89 | — |
| direction | `direction` | required | transform davout lib.rs:2972; validation ±1 (safety_validation.rs:98) | — |
| gear_ratio | `gear_ratio` | **1.0** (lib.rs:156,163) | davout lib.rs:2966 | — |
| recv_can_id | `recv_can_id` | required | **ref-identity only** (reference.rs:346). Not used for CAN filtering (no non-test hit in crates/robstride/src). Not cross-checked against device_id | C07 |
| firmware_version | `firmware_version` | required | **ref-identity only** (reference.rs:347). Master value `"0.0.0-pending"` for lower_arm_yaw (motors.yaml:76) | C07 |
| bench.position_lower_rad / _upper_rad | `MotorBenchLimits.*` | required | Davout hard bounds = URDF ∩ bench (davout lib.rs:3002-3003); gateway config.rs:127; limit-sync main.rs:62; MCP gravity-calibrate.ts:318 | — |
| bench.velocity_limit_rad_s | `.velocity_limit_rad_s` | required | validation (safety_validation.rs:120), ref-identity (reference.rs:350), copied by urdf_expand.rs:313. **Not a command cap** (ADR 0010:35; pinned by test lib.rs:1571 `resolve_joint_velocity_cap_ignores_bench_yaml`) | C05 |
| bench.torque_limit_nm | `.torque_limit_nm` | required | Davout effort and tau_ff caps (davout lib.rs:3053,3056); gateway config.rs:129; overlay.rs:418 | C09 |

### 1c. control.yaml (`ControlSection` family, lib.rs:323-473, 826-851)

| Key (yaml line) | Field | Default | Readers (non-test) | Flag |
|---|---|---|---|---|
| loop_hz (3) | `loop_hz` | required | davout lib.rs:1542; berthier loop.rs:670,911; marengo-pi main.rs:1100; motor-repl main.rs:198 | C10 |
| chappe_state_hz (4) | `chappe_state_hz` | required | marengo-pi main.rs:1101,1365; motor-repl main.rs:199 | — |
| comm_watchdog_ms (5) | `comm_watchdog_ms` | required | davout lib.rs:997,1187,1665,1929 | — |
| *(absent)* | `feedback_poll_budget_us` | **3000** (lib.rs:167) | davout lib.rs:2543 | C11 |
| *(absent)* | `feedback_drain_quiet_us` | **300** (lib.rs:171) | davout lib.rs:2547 | C11 |
| tau_ff_rate_limit_nm_per_s (6) | same | required | davout lib.rs:2671 | — |
| disable_on_exit (7) | same | required | marengo-pi main.rs:1202,1269 | — |
| bench.allow_firmware_speed_mode (9) | `ControlBenchSection.*` | false; section `#[serde(default)]` | davout lib.rs:2251; motor-repl main.rs:345 | — |
| bench.active_reporting_diagnostics (11) | same | **false** (lib.rs:175) | davout lib.rs:2440,2475 | — |
| motor_type_defaults.{rs00,rs02,rs03}.kp_max / kd_max (14-28) | `MotorTypeDefaults` | required map | berthier gain_runtime.rs:299-315 (clamp); validation lib.rs:1221-1248; marengo-pi overlay.rs:584 (snapshot); gateway actuator.rs:165 | C12 |
| motor_type_defaults.*.tau_ff_max_nm | same | required | davout lib.rs:1271,2666,3057; berthier loop.rs:1003; overlay.rs:581 | — |
| motor_type_defaults.*.velocity_max_rad_s | same | required | cap fallback (lib.rs:619-636). **Shadowed for all 5 master joints**, because every robot joint is in an `actuator_groups` entry (control.yaml:30-45) | C05 |
| actuator_groups.*.{joints, velocity_max_rad_s} (30-45) | `ActuatorGroupEntry` | `{}` | cap resolution `actuator_group_for_joint` (lib.rs:580-618). Consumers: davout lib.rs:3050, reference.rs:82, gateway config.rs:148 via `resolve_joint_velocity_cap` | C05 |
| joints.*.motor_type | `JointControlEntry.motor_type` | required | berthier loop.rs:994 (type defaults); validated == motors.yaml (lib.rs:776-781) | C08 |
| joints.*.gravity_comp.{kp,kd,ki} | `ModeGains` | required | berthier gain_runtime.rs:256, loop.rs:891,1021. **Must be 0.0** (lib.rs:702-716, ADR 0004) | C13 |
| joints.*.impedance.{kp,kd,ki} | `ModeGains` | required | berthier gain_runtime.rs:278; marengo-pi overlay.rs:487-489 | C12 |
| joints.*.friction.{fc,fv,fo,k} | `FrictionGains` | required | berthier friction.rs:112-236 via loop.rs:1021; overlay.rs:490; log-cli gravity_fit.rs | C14 |
| *(absent)* | `velocity_max_rad_s` (per joint) | None (lib.rs:432) | cap resolution, top priority (lib.rs:597-609); LimitPatch writes it (limit_patch.rs:121) | C11, C15 |
| joints.*.position_slew_rad_s | same | 0.25 (lib.rs:527) | berthier loop.rs:514,551,617,1194 | — |
| joints.*.position_slew_max_lead_rad | same | 0.15 | berthier loop.rs:1184,1189 | — |
| joints.*.position_trajectory_threshold_rad | same | 0.15 | berthier loop.rs:546,622,1199 | — |
| joints.*.position_trajectory_velocity_rad_s | same | **0.30** | berthier loop.rs:552,619,1196; validated ≤ cap (lib.rs:797) | C16 |
| joints.*.position_trajectory_accel_rad_s2 | same | **0.20** | berthier loop.rs:1202; **also Davout envelope decel** `decel_rad_s2` (davout lib.rs:3082) | C16, C17 |
| joints.*.position_trajectory_velocity_deadband_rad | same | 0.02 | berthier loop.rs; Davout `velocity_deadband_rad_s` (lib.rs:3080) | C18 |
| joints.*.position_hold_trim_rad | same | 0.0 | berthier loop.rs:593; ref-identity reference.rs:367 | — |
| joints.*.position_limit_margin_{min_rad,k_v_s,k_stop} | same | 0.01 / 0.02 / 0.5 | davout lib.rs:3077-3079 (ADR 0009) | — |
| joints.*.position_limit_measured_fault_slack_rad | same | 0.03 | davout lib.rs:3081 | — |
| joints.*.position_soft_{lower,upper}_rad | `Option<f64>` | None (falls back to URDF `safety_controller`) | davout lib.rs:3017-3020; gateway config.rs:155; Consul enrich-inventory.ts, build-hardware-rows.ts; MCP gravity-calibrate.ts:344; `commanded_position_window` (lib.rs:813) used by log-cli gravity_fit.rs:217 | — |
| danger_zones[].{name,joint,position_above_rad,velocity_below_rad_s,action,max_velocity_rad_s} (203-211) | `DangerZoneRule` | list required | davout `apply_danger_zone_clamps` (lib.rs:2765-2799) | C19 |
| *(absent)* | `danger_zones[].max_torque_nm` | None | davout lib.rs:2789 | C19 |
| wrong_sign_watchdog.enabled (217) | `WrongSignWatchdogConfig.enabled` | **true** (lib.rs:358) | davout lib.rs:2692. Master sets **false** | C20 |
| wrong_sign_watchdog.expected_sign_at_positive_q | same | -1 | davout lib.rs:2710; ref-identity reference.rs:378 (even while disabled) | C20 |
| wrong_sign_watchdog.min_velocity_rad_s / min_opposition_ticks / grace_period_ticks | same | 0.05 / 10 / 20 | davout lib.rs:2705,2722,2702 | C10 |

### 1d. homing.yaml (`HomingSection` family, lib.rs:859-1057)

| Key (yaml line) | Field | Default | Readers (non-test) | Flag |
|---|---|---|---|---|
| zero_verify_tolerance_rad (2) | same | required | davout lib.rs:577, feedback_consumer.rs:246, reference_transaction.rs | — |
| calibration_record_path (3) | same | required | davout lib.rs:573 (`root.join`, absolute path wins); env override `MARENGO_CALIBRATION_RECORD` (lib.rs:246); scripts/homing-preflight.sh:31 (grep) | — |
| defaults.method (5) | `HomingJointDefaults.method` | ManualReference | **never read**: `effective_joint_ref` takes `entry.method` (lib.rs:1041), and `HomingJointEntry.method` has its own default (lib.rs:955). Grep `defaults\.method\|d\.method` over S: 0 hits | C21 |
| defaults.home_offset_rad (6) | `.home_offset_rad` (+ per-joint override) | 0.0 | marengo-homing verify.rs:71,117. **Non-zero → reference `Unsupported`** (reference_transaction.rs:798) | C22 |
| defaults.search_direction / search_velocity_rad_s / search_torque_nm / backoff_rad / allow_sensor_overlap (7-13) | same (+ overrides) | Positive / 0.15 / 0.5 / 0.05 / false | **validation + ref-identity only** (safety_validation.rs:301-349, 394-427; reference.rs:397-403). No search or back-off motion exists | C22 |
| defaults.search_timeout_s (10) | same | 30.0 | reference acquisition timeout (reference_transaction.rs:801; davout lib.rs:917) | C23 |
| defaults.sign_test_required (12) | same | true | davout lib.rs:1113; reference_transaction.rs:622; marengo-homing verify.rs:107 | — |
| joints.*.method (15-24) | `HomingJointEntry.method` | ManualReference | effective method (lib.rs:1041); Hall gating (safety_validation.rs:351,394; marengo-homing lib.rs:58) | C22 |
| *(absent)* | `HomingJointOverrides.*` (8 overrides) + `sensors{home,min_limit,max_limit}{gpio,active_high}` | None / active_high true | merge lib.rs:1042-1050. `sensors.is_some()` → `Unsupported` (reference_transaction.rs:798). Sensor API (`ThreeHallInputs`, `MemorySensorProvider`, `read_three_hall`, `classify_sensor_pattern`) is used only inside marengo-homing (grep over crates/bins/tools) | C11, C22 |

### 1e. Outside the four master files

| File / item | Readers | Flag |
|---|---|---|
| `config/network.yaml` `network.chappe_bind`, `NetworkConfigFile`, `load_network_config` (lib.rs:187-195, 1261-1265) | test only (lib.rs:1404-1407). ADR 0014:35 calls it a "placeholder… no bridge implementation". `load_network_config` ignores `MARENGO_CONFIG_DIR` (lib.rs:1263) | C24 |
| `config/robot_humanoid.yaml`, `motors_humanoid.yaml` | tests only (lib.rs:1440-1443; armee-kinematics lib.rs:274) | C25 |
| Consul hardcoded caps `GAIN_LIMITS` (telemetry-gauge-grid.tsx:19-41: tau_ff 17–120 Nm, velocity 15–50 rad/s) and `DISPLAY_STATIC_JOINT_LIMITS` (consul/src/data/actuator-joints.ts:24-50) | Display fallback after live snapshot / config (telemetry-gauge-grid.tsx:101-103). Labelled "Display-only… not a command trust boundary" (actuator-joints.ts:22-23) | C26 |

### Config flags

| id | Item | Class | Evidence (scope S) | Conf |
|---|---|---|---|---|
| C01 | No unknown-key rejection | Silent defaults / fail-open on typos | 0 `deny_unknown_fields` in S. A typo such as `wrong_sign_watchdog.enable` yields `enabled=true` (lib.rs:358). A typo in `position_trajectory_accel_rad_s2` yields 0.20, which also feeds the Davout envelope decel (davout lib.rs:3082). Phase-B lead, not prune | high |
| C02 | Persist rewrites YAML from structs | Comments and unknown keys lost; defaults materialized | `serde_yaml::to_string(cfg)` at lib.rs:1076 and profile_txn.rs:305. Called from bins/marengo-pi/src/limit_persist.rs:463. The operator rationale comments in control.yaml:58-60,71-73,123,204-215 and motors.yaml:41,56-57,72,78 are erased on the first Set Limits persist [INFERENCE: comments cannot survive struct round-trip]. Doc/process gap | high |
| C03 | `robot.name` | Read only by validation | safety_validation.rs:58; no other reader in S | med (prune candidate: field + validation + MCP harmless) |
| C04 | `robot.bench.max_joint_velocity_rad_s` | Never read for behaviour; ADR says it must not be | safety_validation.rs:61 + test lib.rs:1317 only; ADR 0010:35; docs/safety.md:14. Deleting touches lib.rs:130, safety_validation.rs:60-63, tests/safety_validation.rs:86, gateway hardware_tests.rs:505, robot.yaml:8 | high (prune candidate) |
| C05 | Velocity "limits" in 5 places | Duplicated meaning | Effective cap = joint override > `actuator_groups` > `motor_type_defaults` (lib.rs:592-637, ADR 0010). Also present: `robot.bench.max_joint_velocity_rad_s` (2.0), `motors[].bench.velocity_limit_rad_s` (2.0 / 1.0) and URDF `limit.velocity`, none of which cap. **Master disagrees with itself**: pitch and roll bench say 2.0 but the group cap is 2.5 (control.yaml:33,36); elbow and lower-arm-yaw bench say 1.0 but the group cap is 1.5 (motors.yaml:65,81 vs control.yaml:42,45). `motors.bench.velocity_limit_rad_s` is still in ref-identity (reference.rs:350), so it cannot be dropped silently | high (fact); prune of motors bench velocity is med |
| C06 | `robot.limbs` duplicated in Consul | Duplicate implementation | consul/src/lib/commissioning.ts:9-25 hardcodes `MASTER_LIMBS` because the gateway does not expose limbs | med |
| C07 | `recv_can_id`, `firmware_version`, `driver` | Identity-only metadata, unvalidated | Only readers: reference.rs:340,346-347 and candump robstride.rs:38. Placeholder `firmware_version: "0.0.0-pending"` (motors.yaml:76) is accepted. No check that recv_can_id is consistent with device_id | med (keep as identity; gap: validate or document) |
| C08 | `motor_type` in both control.joints and motors.yaml | Duplicated meaning | Validated equal (lib.rs:776-781). Berthier reads the control copy (loop.rs:994); Davout reads the motors copy (lib.rs:508) | med (prune candidate: derive control copy) |
| C09 | `motor.bench.torque_limit_nm` applied twice | Redundant computation | `effort = urdf.min(torque_limit).min(robot max)`, then `tau_ff_max = effort.min(torque_limit).min(type max)` (davout lib.rs:3051-3057). The second `.min(torque_limit)` is a no-op | high (trivial) |
| C10 | Tick-denominated watchdog fields | Unit ambiguity | `min_opposition_ticks`, `grace_period_ticks` are in control ticks (davout lib.rs:2700-2722), so changing `loop_hz` silently rescales trip time (10 ticks = 50 ms at 200 Hz). Validation does not relate them to loop_hz (safety_validation.rs:281-297) | med |
| C11 | Fields absent from master | Silent defaults | `feedback_poll_budget_us`=3000, `feedback_drain_quiet_us`=300 (lib.rs:167-173); `joints.*.velocity_max_rad_s`=None; `danger_zones[].max_torque_nm`=None; homing overrides and sensors. 13 fields total | high (fact) |
| C12 | `ki` capped by `kp_max` | Unit ambiguity | lib.rs:1239-1248 ("schema has no ki_max"); berthier gain_runtime.rs:318-325. Different units (Nm/rad vs Nm/(rad·s)) | med |
| C13 | `gravity_comp.{kp,kd,ki}` | Config key whose only legal value is 0 | lib.rs:702-716 rejects non-zero (ADR 0004). Read by berthier gain_runtime.rs:256 and loop.rs:891 | low (prune candidate needs an ADR 0004 change; keep) |
| C14 | `friction.k`, `friction.fo` | Unit ambiguity | No unit suffix; `k` is the tanh sharpness [INFERENCE from friction.rs:150-236]. `fo` may be negative (lib.rs:1214; test tests/safety_validation.rs:422) | low |
| C15 | Per-joint `velocity_max_rad_s` write paths disagree | Inconsistent policy | The overlay rejects it as "gated until Davout limits rebuild is wired" (lib.rs:1157-1161). `/config/patch` → LimitPatch writes it (limit_patch.rs:121-122; gateway limit_patch.rs:281) and Pi persists it. Phase-B lead: does Davout rebuild caps after that patch? | med |
| C16 | Serde defaults far from master values | Risky default | Defaults are traj velocity 0.30 and accel 0.20 (lib.rs:539-545); master uses 1.10–1.43 and 2.5–4.5. A new joint without these keys gets about 15× lower accel and, through C17, much larger Davout stopping margins | med |
| C17 | `position_trajectory_accel_rad_s2` has two meanings | Duplicated meaning | Berthier planner accel (loop.rs:1202) **and** Davout envelope `decel_rad_s2` (davout lib.rs:3082), so tuning planner feel changes the safety envelope | med |
| C18 | `position_trajectory_velocity_deadband_rad` | Unit mismatch in name | Doc says rad/s ("when \|dq_des\| exceeds this (rad/s)", lib.rs:449). Davout maps it to `velocity_deadband_rad_s` (lib.rs:3080). The `_rad` suffix is wrong | high |
| C19 | Danger-zone `clamp_torque` fallback | Dead branch with unit mix | `max_torque_nm.unwrap_or(rule.max_velocity_rad_s)` (davout lib.rs:2788-2791) uses rad/s as Nm. Validation now rejects clamp_torque without a cap (safety_validation.rs:256-261), so the fallback is dead unless validation is bypassed. Also `action` is a free string; an unknown action is a silent no-op at runtime (lib.rs:2795) but rejected by validation (safety_validation.rs:250) | med (lead) |
| C20 | Wrong-sign watchdog disabled in master; default enabled | Safety gap, not prune | control.yaml:216-217 (`enabled: false`, ADR 0015 policy comment); default true (lib.rs:358). `expected_sign_at_positive_q` is profile-global, while the master has mixed motor directions (motors.yaml:13,27,42,58,73) | high (fact) |
| C21 | `homing.defaults.method` | Parsed, never read | See 1d. Editing it has no effect. Prune: lib.rs:882-883, 905, homing.yaml:5 | high |
| C22 | Hall / search homing schema (`search_direction`, `search_velocity_rad_s`, `search_torque_nm`, `backoff_rad`, `allow_sensor_overlap`, `sensors`, `SensorInput`, `HomingMethod::HallThreeSensor`, non-zero `home_offset_rad`) | Scaffold with no behaviour consumer | Only validation and ref-identity read them. The reference path refuses sensors or a non-zero offset as `Unsupported` (reference_transaction.rs:798-799). The marengo-homing sensor API has zero consumers outside marengo-homing; `method_requires_sensors` has zero refs (metrics/pub-usage.md). ADR 0006:8,14,23 plans three-Hall homing (keep/prune is an owner decision) | med (prune candidate or keep-as-roadmap) |
| C23 | `search_timeout_s` | Name drift | Used as the manual-reference acquisition timeout, not a search timeout (reference_transaction.rs:801; davout lib.rs:917 with `.unwrap_or(Duration::ZERO)`) | med |
| C24 | `network.yaml` / `chappe_bind` / `load_network_config` | Zero non-test refs; placeholder | Test only (lib.rs:1405); ADR 0014:35. Also in README.md:46, config/AGENTS.md:13, crates/marengo-config/README.md:3, lib.rs:14 | high (prune candidate unless the ADR 0014 bridge is scheduled) |
| C25 | `robot_humanoid.yaml` / `motors_humanoid.yaml` | Future inventory, tests only | lib.rs:1440; armee-kinematics lib.rs:274 | low (keep) |
| C26 | Consul static cap tables | Duplicate of control.yaml, values wrong | telemetry-gauge-grid.tsx:28-41 shows tau_ff 17/60/120 Nm and velocity 44–50 rad/s, versus master 3.0/5.0 Nm and 2.0 rad/s (control.yaml:14-28). Display-only fallback | med |
| C27 | `homing.calibration_record_path` parsed by grep in a script | Duplicate parser | scripts/homing-preflight.sh:31 greps YAML instead of using `marengo-config` | low |

**Prior findings touching config.**
- **CS15** (numeric/identity validation) is **partial**, matching ledger `status: partial`. Numeric admission has landed: safety_validation.rs (commit d4c869b) and tests/safety_validation.rs. Still open: unknown-key rejection (C01) and config generations.
- **CS10** (bench caps bound feedforward only) is **open**. `effort` checks only the MIT `torque_nm` (davout lib.rs:2832); PD stiffness and damping torque are not bounded by config.
- **T03** (MCP config sync bypasses persistence) is **open** per the ledger. `pi_sync_bench_config` still rsyncs master YAML (tools/marengo-pi-mcp/src/tools/admin.ts:101).

## 2. Proto contracts — `proto/marengo/v1/marengo.proto` (baseline a2b55b3)

**Counts:** 1 file · **59 messages** · **308 fields** (including 9 oneof members) · **2 oneofs** (`HostMetrics.platform` :238-241, `ActuatorCommand.payload` :518-526) · **11 enums / 47 values** · **0 services / 0 rpcs** · **0 `reserved` statements** · 7 `[deprecated]` fields (all in dead log-store messages).
**Flagged:** 30 flags (P01–P30). Dead or zero-reference: 11 messages + 1 enum (P01–P02). Retired or placeholder: 6 messages (P03–P04). Write-only: about 75 fields, including the whole of `TuningChangeEvent` and `IpcQueueHealth`. Never-set (always default): 15 fields (P11).

**Method / scope.** Rust types come from `prost_build` with no serde (crates/armee-proto/build.rs:17-23), re-exported via `include!` (crates/armee-proto/src/lib.rs:5). Prost uses CamelCase types, snake_case fields and `Enum::Variant`. Consul uses protobuf-es v2 (`target=ts`, consul/buf.gen.yaml; consul/package.json:11,24). In that output fields are camelCase, `XSchema` is used with `create/fromBinary/toBinary`, and enum values drop their prefix (e.g. `ControlMode.POSITION`, consul/src/state/testingStore.ts:171). `consul/src/gen` is gitignored and empty in the worktree (scripts/proto-checksum.sh:2). The MCP server (TypeScript) does not import generated code: it hand-decodes `RobotState` by field number (tools/marengo-pi-mcp/src/robot-state.ts:73-98).
**Grep scope ("S")** for every consumer/"unused" claim: `crates/ bins/ tools/ (incl. tools/marengo-pi-mcp) scripts/ consul/src` excluding `consul/src/gen` and `crates/armee-proto` (generated, plus its own roundtrip tests). Field checks used over-approximating regexes: Rust `.field`, `field:`, struct shorthand, `.field =`/`.push(`; TS `.camelField`, `camelField:`. Generic names (`name`, `mode`, `joint` …) were verified by hand. Test-only = `*_tests.rs`, `*_test.rs`, `tests/`, `__tests__/`, `*.test.ts(x)`, `#[cfg(test)]` modules. Rust hits on same-named non-proto structs (e.g. `davout::SafetySnapshot.hardware_estop_asserted`) do not count as reads.

### Messages — producers/consumers (P = sets/constructs, C = reads)

Transport: **T:** = Chappe topic, **H:** = gateway HTTP, **WT** = WebTransport fan-out of a topic. Field notes: **W** = write-only (set, never read outside tests); **U** = never set (always default). "All read" = every field has a non-test reader.

| Message (line) | #f | Transport | Rust P | Rust C | Consul TS | MCP | Field-level notes |
|---|---|---|---|---|---|---|---|
| Envelope (6) | 4 | wraps every topic payload | chappe `Bus::publish` (crates/chappe/src/lib.rs), gateway http.rs:382-387 | all subscribers; chappe/src/ipc.rs:299 reads `timestamp_ms` | chappe-transport.ts:160 (C), gateway-api.ts:52-62 (P) | — | `source_node` **W** (P22) |
| Heartbeat (13) | 2 | T:robot/heartbeat, WT, H:/snapshot/robot/heartbeat | marengo-pi main.rs:755-765 | gateway state.rs:240,289 (snapshot only) | chappe-transport.ts:177-178 → no-op handler use-chappe-telemetry.ts:141 | — | `timestamp_ms`, `node_id` **W** (P07) |
| JointState (28) | 9 | inside RobotState | berthier loop.rs:1514 | marengo-homing commissioning.rs:12-22 (enum map) | commissioning.ts:114-216, telemetry-gauge-grid.tsx, use-chappe-telemetry.ts, actuator-detail-body.tsx | robot-state.ts:73-90 (#1,2,4,7,8) | all read |
| RobotState (40) | 2 | T:robot/state, WT, H:/snapshot/robot/state | berthier loop.rs:1531-1535; gateway demo webtransport.rs:359 | gateway state.rs:223 (fan-out to testing telemetry), :279 | chappe-transport.ts:171-172, use-chappe-telemetry.ts:33, teach-sample-bus.ts:23 | robot-state.ts:93-98 | all read |
| ImuSample (55) | 15 | T:sensors/imu/torso, WT, H:/snapshot/sensors/imu/torso | marengo-pi imu.rs:121-139 | gateway state.rs:294 (snapshot) | chappe-transport.ts:180-181 → robotStore.ts:82 → use-live-inventory.ts:24 | — | only `quaternion_real` read; the other 14 **W** (P09) |
| Fault (90) | 4 | inside SafetyState | marengo-pi main.rs:718-737 | — | enable-feedback.ts:15-17 | — | `severity` **W** (P08) |
| SafetyState (97) | 5 | T:robot/safety, WT, H:/snapshot/robot/safety | marengo-pi main.rs:740-752; gateway demo webtransport.rs:375 | gateway restart.rs:53,86 (`mode`) | chappe-transport.ts:174-175, use-chappe-telemetry.ts:130, enable-feedback.ts:123 | — | `hardware_estop_asserted`, `software_estop_latched`, `timestamp_ms` **W** (P08, P24) |
| EnableRequest (105) | 3 | H:/command/enable (proto body) → T:robot/enable | gateway http.rs:378-387 (decode + re-encode) | marengo-pi main.rs:481-497, 334-367 | gateway-api.ts:92-100 (P) | — | `timestamp_ms` **W** (P24) |
| HomingComplete (112) | 2 | T:robot/homing (no producer) | **none** | marengo-pi main.rs:498-510 (decode, warn, discard) | — | — | both **W**/never produced (P03) |
| SetZeroRequest (118) | 5 | H:/command/set_zero (JSON body) → T:robot/set_zero | gateway http.rs:472-486 | marengo-pi main.rs:386-403, 554-568 | gateway-api.ts:197-205 (JSON, not proto) | — | `timestamp_ms` **W**; `operator_id`/`confirm`/`sign_test_passed` hard-set http.rs:477-480 (P24) |
| ActiveReportingLeaseRequest (136) | 6 | H:/command/active_reporting_lease (JSON) → T:robot/active_reporting_lease | gateway http.rs:532-576 | marengo-pi main.rs:419-438, 513-543 | gateway-api.ts:247-255 (JSON) | — | `timestamp_ms`, `operator_id` **W** (P24) |
| MotorStatusPollRequest (150) | 2 | H:/command/motor_status_poll (JSON) → T:robot/motor_status_poll | gateway http.rs:615-626 | marengo-pi main.rs:468-478, 1605-1607 | gateway-api.ts:219-224 (JSON) | — | `timestamp_ms` **W** (P24) |
| MitJointCommand (167) | 8 | inside MitCommandBatch | gateway test only | marengo-pi main.rs:593-665 | testingStore.ts, use-compound-playback.ts:95-100, actuator-home.ts:71-79 (P) | — | `velocity`, `torque_ff` **W**, always 0 (P16); `name` overloaded (P17) |
| MitCommandBatch (178) | 3 | H:/command/testing_mit (proto) → T:robot/testing/mit_command_batch | gateway http.rs:396-404 (re-encode) | marengo-pi main.rs:585-592 | testingStore.ts:96,130,168, use-compound-playback.ts:81-84, actuator-home.ts:66-69 (P) | — | `timestamp_ms` **W** (P16); `mode` compared to magic `4` (P17) |
| GatewaySubscribe (187) | 2 | WT first client frame | — | gateway webtransport.rs:124-141 | chappe-transport.ts:274 (P) | — | all read |
| GatewaySubscriptionAdmission (194) | 2 | WT first server frame | gateway webtransport.rs:150 | — | chappe-transport.ts:282-286 | — | all read |
| LogEvent (200) | 6 | T:logs/structured, WT | chappe tracing_layer.rs:188-201 | gateway logs.rs:233-250 (→ SQLite) | chappe-transport.ts:183-187, use-chappe-telemetry.ts:146-149 | — | all read |
| HostMetrics (224) | 20 | T:host/metrics/{pi,jetson}, WT, H:/snapshot/host/metrics/* | marengo-host-metrics lib.rs:136-197 via marengo-pi host_metrics.rs:24-28; gateway `--demo` webtransport.rs:344-538 | — | chappe-transport.ts:189-191, hostMetricsStore.ts, pi-host-card.tsx, jetson-host-card.tsx, host-debug-info.ts, can-traffic-spectrum.ts | — | `services` **W** (P10) |
| BuildInfo (252) | 3 | in HostMetrics | lib.rs:183 | — | host-debug-info.ts:36-50, teach-transit.ts | — | all read |
| ChappeHealth (258) | 4 | in HostMetrics | sample_state.rs:94-96 | — | hostMetricsStore.ts:65 | — | `gateway_reachable`, `ipc_queue` **W** (P14) |
| IpcQueueHealth (267) | 12 | in ChappeHealth | sample_state.rs:96 (`into_proto`) | tests only (sample_state.rs:132-164) | — | — | all 12 **W** (P14) |
| RuntimeConnectionState (285) | 2 | T:gateway/runtime_connection, WT | gateway state.rs:95-105 | — | chappe-transport.ts:168-169; handler ignores payload (use-chappe-telemetry.ts:112) | — | both **W**, signal-only (P23) |
| RuntimeObservationGap (291) | 1 | WT (gateway-local) | gateway framing.rs:102 | — | chappe-transport.ts:165-166; handler ignores payload (use-chappe-telemetry.ts:113) | — | `lagged_envelopes` **W** (P23) |
| ClockMetrics (296) | 3 | in HostMetrics | lib.rs:432-449 | — | hostMetricsStore.ts:68-69 | — | `sync_source` **W**; `offset_ms` **U** (P10, P11) |
| CpuMetrics (302) | 7 | in HostMetrics | cpu.rs:46-60, lib.rs:248 | — | pi/jetson-host-card.tsx (`usagePercent`, `sampleValid`) | — | `core_count`, `per_core_usage_percent`, `freq_mhz`, `iowait_percent`, `cores` **W** (P15) |
| CpuCoreMetrics (315) | 2 | in CpuMetrics | cpu.rs:60 | tests only | — | — | both **W** (P15) |
| MemoryMetrics (321) | 7 | in HostMetrics | lib.rs:268-280 | — | pi-host-card.tsx:41-42, jetson-host-card.tsx:37-38 | — | `available/buffers/cached/swap_total/swap_used_bytes` **W** (P10) |
| LoadMetrics (331) | 3 | in HostMetrics | lib.rs:290-294 | — | host cards (`load1m`) | — | `load_5m`, `load_15m` **W** (P10) |
| ThermalMetrics (337) | 4 | in HostMetrics | lib.rs:315-319 | — | pi-host-card.tsx:57, jetson-host-card.tsx:43 (`cpuCelsius`) | — | `gpu_celsius` **W** (demo-only P); `zones` **W**; `soc_celsius` **U** (P10, P11) |
| ThermalZone (344) | 2 | in ThermalMetrics | lib.rs:312 | — | — | — | both **W** (P10) |
| DiskMetrics (349) | 11 | in HostMetrics | diagnostics.rs:133-160 | — | hostMetricsStore.ts:56, pi-host-card.tsx:51-52 | — | `filesystem`, `source_device` **W**; `read/write_bytes_per_sec` **U** (P10, P11) |
| NetworkInterfaceMetrics (364) | 9 | in HostMetrics | lib.rs:361-376 | — | can-traffic-spectrum.ts, hostMetricsStore.ts | — | `up`, `rx_errors_total`, `tx_errors_total` **W** (P10) |
| ServiceStatus (376) | 3 | in HostMetrics | lib.rs:398-430 | — | — | — | all **W** (P10) |
| PiPlatformMetrics (382) | 8 | oneof `pi` | lib.rs:451-459 | — | pi-host-card.tsx (`throttledNow`, `throttleEvents`) | — | `pmic_temp_celsius` **W** + mislabelled (P12); 5 freq/voltage fields **U** (P11) |
| JetsonPlatformMetrics (393) | 10 | oneof `jetson` | lib.rs:163-171 (no bin uses the Jetson role); demo webtransport.rs:482-490 | — | jetson-host-card.tsx:50-52,175 | — | `jetson_model`, `gpu_usage_percent` (demo) **W**; 5 gpu/emc/power fields **U** (P11, P13) |
| LogSessionMeta, LogSessionList, StructuredLogEntry, StructuredLogList, CandumpFrame, CandumpPage, CandumpIdCount, CandumpInterfaceSummary, CandumpSummary (408-494) | 9+1+7+2+10+6+2+3+12 | H:/logs/* is **serde JSON**, not proto | **none** | **none** | **none** (Consul uses DTOs, log-api.ts:4-70) | none (MCP curls JSON, tools/marengo-pi-mcp/src/tools/logs.ts:63) | whole messages unreferenced (P01) |
| SessionStartRequest / SessionStartResponse (498, 503) | 2+2 | — | none | none | none | none | unreferenced (P02) |
| OperatorCommand (508) | 5 | H:/command/actuator (Envelope) → T:robot/actuator/command | gateway limit_patch.rs:130-157, actuator.rs:34-139 (decode, re-publish) | marengo-pi overlay.rs:240-270, 601-642 | gateway-api.ts:50-62 (P; no non-test caller, P05) | — | `seq` **W**, always 1 (P20) |
| ActuatorCommand (516) | 8 | in OperatorCommand | limit_patch.rs:139 (`LimitPatch`) | overlay.rs:248-270; gateway actuator.rs:59-64 accepts `Tuning` only | — | — | 5 motion variants rejected everywhere (P04) |
| LimitPatchCommand (530) | 7 | oneof `limit_patch` | gateway limit_patch.rs:139 | overlay.rs:252, 403-410 | — (Consul uses JSON route) | — | all read |
| TuningChange (557) | 4 | oneof `tuning` | no non-test producer in S (P05) | overlay.rs:249, 320-374; gateway actuator.rs:59-122 | — | — | all read |
| ModeChange, JogCommand, HoldCommand, PresetCommand, EnableChange (564-583) | 1+1+2+1+1 | oneof variants | tests only (gateway actuator.rs:420; pi shutdown_tests.rs) | variant tag matched, contents never read (overlay.rs:255-259, 646-650) | — | — | all fields dead (P04) |
| TuningChangeEvent (585) | 9 | T:robot/audit/tuning | overlay.rs:601-619, 664-673 | tests only (shutdown_tests.rs:571) | — | — | whole message **W** (P06) |
| ActionEvent (597) | 10 | T:robot/audit/action | overlay.rs:621-642, limit_persist.rs:59-61, gateway action_ack.rs:57-60 (re-wrap) | gateway action_ack.rs:24,102; state.rs:203-215; limit_patch.rs:181-250 | — | — | `revision`, `operator_id`, `joint` **W** (P19) |
| JointActuatorLimit (611) | 10 | in ActuatorLimitSnapshot | overlay.rs:582-590 | tests only | actuatorStore.ts (all except `wired`), telemetry-gauge-grid.tsx | — | `wired` **W** (P21) |
| ActuatorLimitSnapshot (626) | 2 | T:robot/actuator/limits; H:/snapshot/actuator/limits (proto) | overlay.rs:557-595, 301-303 | gateway state.rs:309-311 (snapshot) | gateway-api.ts:38-47, actuatorStore.ts:23-25,114 | — | `timestamp_ms` **W** (P24) |

### Enums

| Enum (line) | Values | Zero value | Produced (non-test) | Consumed (non-test) | Note |
|---|---|---|---|---|---|
| JointHomingState (20) | 5 | `_UNSPECIFIED=0` ✓ | marengo-homing commissioning.rs:12 | commissioning.rs:22; Consul commissioning.ts, telemetry-facets.ts; MCP robot-state.ts:11 | OK |
| ImuAccuracy (47) | 5 | ✓ | imu.rs (`ProtoImuAccuracy::*`) | none | write-only (P09) |
| OperationalMode (76) | 4 | ✓ | marengo-pi main.rs:742 | gateway restart.rs:53; use-chappe-telemetry.ts:130 | OK |
| FaultSeverity (83) | 4 | ✓ | main.rs:721-735 (`Fault`, `Estop` only) | none | `WARNING` never produced; whole enum unread (P08, P27) |
| ActiveReportingLeaseAction (129) | 4 | ✓ | gateway http.rs:532-534 | main.rs:517-539 | OK |
| ControlMode (157) | 6 | ✓ | Consul `POSITION`, `IMPEDANCE` (testingStore.ts:171) | main.rs:592 (`== 4` literal); berthier loop.rs:1597 (debug log only, main.rs:1643) | P17 |
| HostNodeRole (211) | 3 | ✓ | host_metrics.rs:24 (Pi only) | use-chappe-telemetry.ts (`nodeRole`) | OK |
| ServiceState (217) | 4 | ✓ | lib.rs:418-422 | none | write-only (P10) |
| CandumpTimestampMode (453) | 3 | ✓ | none | none | dead (P01) |
| PersistStatus (542) | 5 | ✓ | overlay.rs, limit_persist.rs | gateway state.rs:205-215, limit_patch.rs:206 | OK |
| TuningTier (550) | 4 | ✓ | no non-test producer (P05) | overlay.rs:320 | OK as a contract |

### Flags

| id | item | class | evidence (scope S unless stated) | conf |
|---|---|---|---|---|
| P01 | 9 log-store messages (:408-494) + `CandumpTimestampMode` (:453) | zero refs; **stale mirror that drifts from the real wire** | No type or `…Schema` reference in S (the only hits are marengo-store's re-export of `marengo_candump::{Frame as CandumpFrame, Summary as CandumpSummary}`, crates/marengo-store/src/lib.rs:23). Gateway serves serde JSON (bins/marengo-gateway/src/logs.rs:37-127) and Consul has its own DTOs (consul/src/lib/log-api.ts:4-70). Drift: JSON `interfaces`/`top_ids` are object arrays (logs.rs:123-124), while the proto has `repeated string` :483-484 plus `interface_summaries`/`top_id_counts`. JSON lacks `parsed_frame_hz` (:490) and `timestamp_mode` (:463). All 7 deprecated fields live here. | high |
| P02 | `SessionStartRequest`/`SessionStartResponse` (:498-506) | zero refs | No match for the name, `…Schema`, `session/start` or `sessionStart` in S | high |
| P03 | `HomingComplete` (:112) | retired: no producer, drain-only consumer | No constructor in S. Pi decodes and drops it with a warning (bins/marengo-pi/src/main.rs:498-510). Gateway `/command/home` returns 410 (bins/marengo-gateway/src/http.rs:158,410-415). Deleting it also means removing the `homing_rx` drain. | high |
| P04 | `ActuatorCommand` variants `mode`/`jog`/`hold`/`preset`/`enable` and messages `ModeChange`, `JogCommand`, `HoldCommand`, `PresetCommand`, `EnableChange` (:519-524, :564-583) | placeholder, rejected end-to-end | Gateway accepts only `Tuning` ("until motion PR-5", bins/marengo-gateway/src/actuator.rs:59-64). Pi rejects as "motion commands gated" (bins/marengo-pi/src/overlay.rs:255-270). Only constructors are tests (actuator.rs:420, after `#[cfg(test)]` at :189; pi shutdown_tests.rs). No TS producer: Consul `mode`/`hold`/`preset` hits are unrelated UI state. | high (unused); keep/delete is a roadmap decision |
| P05 | `OperatorCommand` tuning path via `POST /command/actuator` and `TuningTier` | no non-test client in S | `postActuatorCommand` (consul/src/lib/gateway-api.ts:50) is called only from runtime-credentials.test.tsx:41. MCP and scripts never call `/command/actuator`. `TuningTier` is not imported in consul/src. The live `OperatorCommand` producer is the gateway's own limit patch (bins/marengo-gateway/src/limit_patch.rs:130-157). | med (external/manual clients possible) |
| P06 | `TuningChangeEvent` (:585) | write-only message | Published on `robot/audit/tuning` (bins/marengo-pi/src/overlay.rs:601-619, 664-673). Decoded only in bins/marengo-pi/src/shutdown_tests.rs:571. Gateway only lists the topic (bins/marengo-gateway/src/state.rs:46). No Consul subscriber in S. | high |
| P07 | `Heartbeat` (:13) contents | write-only | Consul handler is `onHeartbeat: () => {}` (consul/src/hooks/use-chappe-telemetry.ts:141). Gateway only caches it for `/snapshot/robot/heartbeat` (http.rs:96, state.rs:289), which has no consumer in S. | med (liveness use outside S possible) |
| P08 | `SafetyState.hardware_estop_asserted`, `.software_estop_latched`, `Fault.severity`, `FaultSeverity` | write-only **safety surface** | Set at main.rs:721-725, 743-744. Readers are only safety_publication_tests.rs:295-328 and safety_receive_diagnostic_tests.rs:163. Consul reads only `mode`/`activeFaults` → `joint`/`code`/`message` (enable-feedback.ts:15-17,123). `software_estop_latched = !faults.is_empty()` (main.rs:744), so the name means something else. **Do not prune:** wire into the UI or rename. | high (unread) |
| P09 | `ImuSample` 14 of 15 fields + `ImuAccuracy` | write-only | Producer bins/marengo-pi/src/imu.rs:121-139. Only reader is `quaternionReal` (consul/src/hooks/use-live-inventory.ts:24). No other `quaternion*`, `accel*`, `gyro*`, `has*`, `accuracy` or `frameId` reads in S. | med (diagnostic telemetry) |
| P10 | HostMetrics-family write-only fields: `services`, `ServiceStatus.*`, `ServiceState`; `MemoryMetrics.available/buffers/cached/swap_*`; `LoadMetrics.load_5m/15m`; `ThermalMetrics.zones`, `ThermalZone.*`, `gpu_celsius`; `ClockMetrics.sync_source`; `DiskMetrics.filesystem/source_device`; `NetworkInterfaceMetrics.up/rx_errors_total/tx_errors_total` | write-only | Producers crates/marengo-host-metrics/src/lib.rs:175-196, 268-280, 290-294, 312-318, 361-376, 398-449. Exhaustive camelCase read search over consul/src (non-test) shows only the reads listed in the inventory table. No Rust readers. | high |
| P11 | Never set (always default): `ClockMetrics.offset_ms`, `ThermalMetrics.soc_celsius`, `DiskMetrics.read/write_bytes_per_sec`, `PiPlatformMetrics.arm/core/sdram/gpu_freq_mhz` + `core_voltage_v`, `JetsonPlatformMetrics.gpu_mem_used/total_bytes`, `gpu_freq_mhz`, `emc_freq_mhz`, `power_draw_w` | read-never-set / dead | Constructors use `..Default::default()` (lib.rs:169, 318, 444, 457). `sample_disks(_elapsed)` ignores the rate inputs (lib.rs:322-324). No reader in S. | high |
| P12 | `PiPlatformMetrics.pmic_temp_celsius` (:390) | mislabelled + write-only | Set from CPU thermal: `pmic_temp_celsius: thermal.cpu_celsius` (lib.rs:456). No reader in S. | high |
| P13 | `JetsonPlatformMetrics` / Jetson `HostMetrics` producer | no real producer | Only the Pi role is sampled (bins/marengo-pi/src/host_metrics.rs:24). bins/marengo-jetson has no armee_proto or host-metrics use. The Jetson branch (lib.rs:163-171) runs only via the gateway `--demo` publisher (bins/marengo-gateway/src/main.rs:216-218, webtransport.rs:344-538). Consul still renders it (jetson-host-card.tsx:50-52). | med (planned Jetson) |
| P14 | `IpcQueueHealth` (12 fields), `ChappeHealth.gateway_reachable`, `.ipc_queue` | write-only | Set at crates/marengo-host-metrics/src/sample_state.rs:94-96 and bins/marengo-pi/src/host_metrics.rs:47-51. Read only by the test at sample_state.rs:132-164 (after `#[cfg(test)]` at :101). Consul reads only `ipcConnected`/`lastPublishAgeMs` (hostMetricsStore.ts:65). Added 2026-10-01 (b5b16fa), so ask the owner. | med |
| P15 | `CpuMetrics.core_count/per_core_usage_percent/freq_mhz/iowait_percent/cores`, `CpuCoreMetrics` | write-only | Set at crates/marengo-host-metrics/src/cpu.rs:46-60 and lib.rs:248. Reads only in cpu.rs tests (:106-170). Consul reads only `usagePercent`/`sampleValid` (pi-host-card.tsx, jetson-host-card.tsx). `cores` added 2026-10-01 (c349b3f). | med |
| P16 | `MitJointCommand.velocity`, `.torque_ff`; `MitCommandBatch.timestamp_ms` | write-only (always 0 for MIT fields) | Consul sets `velocity: 0, torqueFf: 0` (use-compound-playback.ts:99-100,280-281,308-309; actuator-home.ts:78-79). The Pi reads only `name`, `kp`/`kd`/`ki`/`fc`, `position` and batch `mode`/`joints` (main.rs:585-665). Gateway re-encodes without reading (http.rs:396-404). | high |
| P17 | `MitCommandBatch.mode` / `MitJointCommand.name` | contract smell | `let want_position = batch.mode == 4;` (main.rs:591-592) bypasses `ControlMode::Position`. `name` carries an in-band `wave:<joint>:<min>:<max>:<cycles>:<half_period_sec>` command (main.rs:597-599). | high |
| P18 | Consul `postMitCommandBatch` → `/command/mit` | dead TS producer, route missing | consul/src/lib/gateway-api.ts:108-121. The gateway has only `/command/testing_mit` (bins/marengo-gateway/src/http.rs:155-168). The only reference is the re-export at consul/src/lib/chappe-client.ts:12. (Overlaps the route inventory.) | high |
| P19 | `ActionEvent.revision`, `.operator_id`, `.joint` | write-only | Set at overlay.rs:213,636 and limit_persist.rs. Gateway reads `session_id`/`action`/`accepted`/`reject_reason`/`persist_status`/`config_revision`/`timestamp_ms` only (action_ack.rs:24,57,102; limit_patch.rs:181-250; state.rs:205). No Consul consumer. | med (audit-trail value) |
| P20 | `OperatorCommand.seq` (:512) | write-only constant | Hard-coded `seq: 1` (bins/marengo-gateway/src/limit_patch.rs:136; actuator.rs:267 test). No reader in S. | high |
| P21 | `JointActuatorLimit.wired` (:619) | write-only | Set at overlay.rs:590. Read only in overlay_tests.rs. Consul has no `.wired` read outside tests (it appears only in enable-feedback.test.ts and enrich-inventory.test.ts fixtures). | med |
| P22 | `Envelope.source_node` (:8) | write-only metadata | No `.source_node`/`.sourceNode` read in S. All consumers dispatch on `message_type`. | low (keep as envelope metadata) |
| P23 | `RuntimeConnectionState.*`, `RuntimeObservationGap.lagged_envelopes` | signal-only payload | Handlers drop the payload (use-chappe-telemetry.ts:112-113). The design comment says receipt itself carries the meaning (proto :283-294). | low (keep) |
| P24 | `timestamp_ms` write-only on EnableRequest, SetZeroRequest, ActiveReportingLeaseRequest, MotorStatusPollRequest, SafetyState, ImuSample, ActuatorLimitSnapshot; `ActiveReportingLeaseRequest.operator_id`; SetZeroRequest `operator_id`/`confirm`/`sign_test_passed` constant | write-only / constant | No `request.timestamp_ms` read in bins/marengo-pi/src/main.rs. TS `.timestampMs` reads appear only for RobotState, LogEvent and HostMetrics (use-chappe-telemetry.ts:33,146; teach-sample-bus.ts:23; host-debug-info.ts:51). Constants are at http.rs:477-480. | low |
| P25 | Unit/name ambiguity | naming | Unsuffixed physical quantities: `JointState.position/velocity/effort` (:30-32; MCP documents rad/Nm at robot-state.ts:15-18); `MitJointCommand.position/velocity/torque_ff/kp/kd/ki/fc` (:169-175); `TuningChange.value` and `TuningChangeEvent.before/after` (unit depends on `param`); `JointActuatorLimit.kp_max/kd_max`. `pos_*_rad` (:616-622) vs `position_*_rad` (:531-535) names the same concept two ways. `timestamp_ms` epoch basis is undocumented. | low |
| P26 | Reserved hygiene / Phase B guardrail | history clean, but deletion is CI-blocking | `git log -p --follow` (24 commits) shows no deleted field, enum value or message. Commit 15bf285 only added `[deprecated]` to 7 fields in place. There are 0 `reserved` statements, so nothing is missing. **Guardrail:** proto/buf.yaml:5-7 sets `breaking: use: [FILE]` and scripts/check.sh:66 runs `buf breaking` against main. In FILE, `FIELD_NO_DELETE`, `ENUM_NO_DELETE` and `ENUM_VALUE_NO_DELETE` reject deletion even when reserved (buf.build/docs/breaking/rules). Pruning any P01–P04 item needs either a breaking-check exception or deprecate-then-reserve, plus `reserved` numbers and names. `MESSAGE_NO_DELETE` ∈ FILE [INFERENCE]. | high |
| P27 | Enum zero values | conformance OK | All 11 enums have a prefixed `*_UNSPECIFIED = 0` (:21,48,77,84,130,158,212,218,454,543,551). `FAULT_SEVERITY_WARNING` is never produced outside tests (main.rs:721-735). | high |
| P28 | Services/rpcs | none defined | No `service`/`rpc` in the proto. Transport is `Envelope` over Chappe topics or bare proto/JSON HTTP bodies, so there are no rpc impl gaps. | high |
| P29 | `HostMetrics` field-number gaps 8-9, 19, 22-29; `ActuatorCommand` 2-9 | intentional gaps | Never used in history (see P26). No action. | high |
| P30 | Snapshot routes `/snapshot/robot/{safety,heartbeat}`, `/snapshot/sensors/imu/torso`, `/snapshot/host/metrics/*` | no consumer in S | http.rs:95-105. Only `/snapshot/robot/state` (MCP robot-state.ts:104) and `/snapshot/actuator/limits` (gateway-api.ts:38) are fetched. Defer to the route inventory. | med |

## 3. Gateway HTTP/WebTransport routes and Chappe topics

Baseline: audit worktree `a2b55b3`. Only `marengo-gateway` serves HTTP or WebTransport. A grep for `TcpListener::bind|axum::serve|hyper::server|warp|actix` over `bins/`, `crates/` and `tools/marengo-pi-mcp/src` finds non-test listeners only at `bins/marengo-gateway/src/main.rs:230-233`. `marengo-pi`, `marengo-jetson`, `marengo-log-cli` and the other bins serve nothing. `bins/marengo-jetson/src/main.rs:1-6` is a scaffold that only logs.

**Consumer grep scope** (applies to every "no consumer" or "unused" claim below): `consul/src` excluding `consul/src/gen`, `tools/marengo-pi-mcp/{src,test}`, `scripts/`, `crates/`, `bins/` (including their in-file tests and `tests/` dirs). `consul/e2e`, `consul/tests` and a top-level `tests/` do not exist. Docs and ADRs are not counted as consumers.

**Counts**
- HTTP routes: **44** (method+path) in `bins/marengo-gateway/src/http.rs:90-168`. There are also **2** static-file services, available only on the HTTPS listener (`/assets/*` at http.rs:188 and the SPA fallback at http.rs:196), and **1** WebTransport CONNECT path, `/chappe` (webtransport.rs:83).
- Auth on the 44 routes: **29 always protected**, **1 protected depending on topic** (`/stream/chappe`), **14 public**. Mutating or motion routes: **15**. One of them (the retired `/command/home`) is public, and one public GET (`/version/status`) has side effects.
- Chappe topics: **19** production topic strings. 10 go runtime→gateway over IPC, 2 are made up by the gateway, and 7 are gateway→runtime commands. **1** extra string (`robot/limits/patch`) appears only as a stale test literal.
- Flags: **12 route flags (R01-R12)** and **10 Chappe flags (T01-T10)**. **0** motion or mutating routes lack auth (`/command/home` is a public stub that does nothing). G07 is **fixed (software verified), not regressed**.

### Auth model as implemented (ADR 0033 / G07)

| Aspect | Implementation | Evidence |
|---|---|---|
| Gate | One axum middleware, `authorize_api`, layered on every API route for HTTP and HTTPS. It runs before body extraction and before the handler. | http.rs:169-173, 202-259 |
| Origin check | If an `Origin` header is present it must be unique and allowed. Allowed means: in the default set `http://localhost:5173` and `http://127.0.0.1:5173`, or in `MARENGO_GATEWAY_ALLOWED_ORIGINS`, or an `https` origin on the robot's HTTPS port whose host matches the request `Host`. A missing Origin is allowed (non-browser clients). Every route is checked, public ones included. | access.rs:32-43, 105-117, 139-176; http.rs:207-209 |
| Credentials | SHA-256 digests compared in constant time. `Authorization: Bearer` or the legacy `x-marengo-log-token`; supplying both or duplicating either is refused. | access.rs:121-133, 187-210, 213-223 |
| Roles | Env vars at startup, read once: OPERATOR and LOG carry all 5 capabilities; CONTROL, CALIBRATION, CONFIG, MANAGEMENT and READ carry one each. | access.rs:70-104; state.rs:118-129 |
| Fail-closed | No grants means `authenticate` returns 401. An invalid config falls back to `AccessPolicy::default()`, which has no grants. | access.rs:205-207; state.rs:121-129 |
| Capability map | `/stream/chappe` needs SensitiveRead only if any topic is sensitive. `/logs/*`, `/snapshot/logs/recent` and `/settings` need SensitiveRead. `/hardware/urdf*` (any method) needs Configuration. For POST/PUT/DELETE: `/command/set_zero` needs Calibration; other `/command/*` except `/command/home` need Control; `/config/*` and `/hardware/*` need Configuration; `/control/*` need Management; `/command/home` needs nothing; any other mutation defaults to Management. All other GETs are public. | http.rs:213-248 |
| Extra check after admission | `/command/actuator` tuning also needs Configuration (RuntimeMit+persist or ConfigOverlay) or Calibration (Firmware). | actuator.rs:83-96 |
| Sensitive topics | `logs/structured`, `robot/audit/action`, `robot/audit/tuning`, `robot/testing/mit_command_batch` | http.rs:261-269 |
| WebTransport | Path must be exactly `/chappe` with no query or userinfo, and Origin is validated. The credential arrives inside the protobuf `GatewaySubscribe.runtime_credential`; header credentials are refused. SensitiveRead is required for sensitive topics. Limits: 16 KiB subscription, 5 s handshake deadline, 64 sessions. | webtransport.rs:19, 59-95, 116-149 |
| Bind | The binary defaults to loopback (`127.0.0.1:8080`, WebTransport on `127.0.0.1:8443`). The deployed unit uses `[::]:8080` plain HTTP, `[::]:8444` HTTPS and `[::]:8443` WebTransport. | main.rs:49-55; scripts/systemd/marengo-gateway.service:13; scripts/install-pi.sh:338 |
| Build-token leak (second half of G07) | Consul uses runtime tab-memory credentials. The dist gate forbids `VITE_MARENGO_LOG_TOKEN`, `VITE_AUTO_LEARN_TOKEN` and `VITE_CHAPPE_`. | consul/src/lib/runtime-credentials.ts:38; scripts/check-consul-dist.sh:24-28 |

**G07 status: FIXED (software verified), not regressed.**
- The ledger marks it `"status": "verified"`, delivered in main `df468496` (PR #245): docs/reviews/2026-09-29/implementation-ledger.json:2659-2676.
- `git log` shows `df46849 fix(gateway): enforce access and enter credentials at runtime (#245)` in the audit history, and `git merge-base --is-ancestor df468496 HEAD` succeeds.
- `git diff --stat df468496 HEAD -- bins/marengo-gateway/src/{access,http,webtransport}.rs` is empty. The only gateway commit since then is `2ae3fb5`, which changes tests only.
- Every original G07 route (`/command/enable`, `/command/testing_mit`, `/command/set_zero`, all in docs/reviews/2026-09-29/gateway.md:93-95) is now Control or Calibration gated, and conformance-tested at gateway_access_conformance_test.rs:84-171.
- Hardware acceptance is not established (ledger:2674).
- Doc drift: docs/reviews/2026-09-29/finding-index.md:56 still says **Open** (flag R11).

### HTTP route inventory

Effect column: R = read-only, W = mutation (files, config, process), M = publishes a motion or drive command to the runtime. Auth column: `—` means public. All handlers are in `bins/marengo-gateway/src/`.

| # | Method path | Handler | Request → response | Effect | Auth | Consumers |
|---|---|---|---|---|---|---|
| 1 | GET /health | http.rs:271 | — → JSON `{ok,node,dropped_log_inserts}` | R | — | MCP readonly.ts:46,151; deploy-wait.ts:22; scripts/pi-remote.sh:57; cloud-pi-lib.sh:255. Consul `fetchGatewayHealth` (gateway-api.ts:76) has no caller; its only reference is the re-export at chappe-client.ts:10 |
| 2 | GET /tls/fingerprint | http.rs:284 | — → JSON sha-256 | R | — | consul chappe-transport.ts:65 |
| 3 | GET /stream/chappe?topics= | http.rs:300 | query → chunked length-prefixed Envelope stream | R | SensitiveRead only for sensitive topics | consul chappe-transport.ts:315 (HTTP fallback); tests http.rs:685, gateway_access_public_test.rs:143,168 |
| 4 | GET /snapshot/robot/state | http.rs:337 | → protobuf RobotState | R | — | MCP robot-state.ts:104 (test robot-state.test.ts:63) |
| 5 | GET /snapshot/robot/safety | http.rs:341 | → SafetyState | R | — | **none** (R03) |
| 6 | GET /snapshot/robot/heartbeat | http.rs:345 | → Heartbeat | R | — | **none** (R03) |
| 7 | GET /snapshot/sensors/imu/torso | http.rs:349 | → ImuSample | R | — | **none** (R03) |
| 8 | GET /snapshot/host/metrics/pi | http.rs:353 | → HostMetrics | R | — | **none** (R03) |
| 9 | GET /snapshot/host/metrics/jetson | http.rs:357 | → HostMetrics | R | — | **none** (R03) |
| 10 | GET /snapshot/actuator/limits | actuator.rs:21 | → ActuatorLimitSnapshot | R | — | consul gateway-api.ts:38, called from set-limits-panel.tsx:255 and use-actuator-harness.ts:22; test actuator.rs:574 |
| 11 | GET /snapshot/logs/recent | logs.rs:318 | ?limit → JSON log entries | R | SensitiveRead | consul log-api.ts:147 |
| 12 | GET /logs/sessions | logs.rs:357 | ?limit → JSON sessions | R | SensitiveRead | consul log-api.ts:156; scripts/pi-remote.sh:96 and cloud-pi-lib.sh:263 send a token (cloud-pi-lib.sh:219-221). MCP logs.ts:63,102 and pi-remote.sh:83 send **no token** (R06) |
| 13 | GET /logs/sessions/latest/candump | logs.rs:439 | page → JSON frames | R | SensitiveRead | consul log-api.ts:186 |
| 14 | GET /logs/sessions/latest/candump/summary | logs.rs:463 | → JSON summary | R | SensitiveRead | consul log-api.ts:196 |
| 15 | GET /logs/sessions/{id}/bench | logs.rs:400 | page → lines | R | SensitiveRead | consul log-api.ts:207 |
| 16 | GET /logs/sessions/{id}/trace | logs.rs:413 | page → lines | R | SensitiveRead | consul log-api.ts:217 |
| 17 | GET /logs/sessions/{id}/candump | logs.rs:426 | page → frames | R | SensitiveRead | consul log-api.ts:187 |
| 18 | GET /logs/sessions/{id}/candump/summary | logs.rs:451 | → summary | R | SensitiveRead | consul log-api.ts:197 |
| 19 | GET /logs/sessions/{id}/download | logs.rs:543 | ?type → file | R | SensitiveRead | **tests only**: conformance_test.rs:93 (R04) |
| 20 | GET /logs/structured | logs.rs:488 | query → JSON | R | SensitiveRead | consul log-api.ts:175; scripts/pi-remote.sh:101; test logs.rs:602 |
| 21 | GET /settings | logs.rs:526 | → JSON | R | SensitiveRead | **tests only**: conformance_test.rs:95 (R04) |
| 22 | GET /config/snapshot | config.rs:174 | → JSON config inventory | R | — | consul config-api.ts:74, which sends a Configuration token anyway (R08) |
| 23 | POST /config/patch | config.rs:185 | JSON limit patch → JSON result. Publishes `robot/actuator/command` (limit_patch.rs:154) and writes the audit store | W+M (live limits) | Configuration | consul config-api.ts:90 |
| 24 | GET /hardware/completeness | hardware.rs:174 | → JSON | R | — | consul hardware-api.ts:92, called from hardware-overview.tsx:73 (R08) |
| 25 | GET /hardware/urdf | hardware.rs:183 | → URDF bytes | R | Configuration | Consul `fetchLiveUrdf` (hardware-api.ts:114) has **no production caller**; tests hardware_tests.rs:98,117 and runtime-credentials.test.tsx:46 (R05) |
| 26 | POST /hardware/urdf/upload | hardware.rs:198 | XML → JSON upload_id | W (archive file) | Configuration | consul hardware-api.ts:135, called from import-wizard.tsx:81 |
| 27 | POST /hardware/urdf/resolve-preview | hardware.rs:220 | JSON → preview | R [INFERENCE] | Configuration | consul hardware-api.ts:159, called from import-wizard.tsx:114 |
| 28 | POST /hardware/urdf/activate | hardware.rs:248 | JSON → result (writes live URDF) | W | Configuration | consul hardware-api.ts:188, called from import-wizard.tsx:142 |
| 29 | GET /hardware/urdf/archive | hardware.rs:375 | → list | R | Configuration | consul hardware-api.ts:270, called from import-wizard.tsx:349 |
| 30 | GET /hardware/urdf/archive/{id} | hardware.rs:414 | → JSON | R | Configuration | Consul `fetchUrdfArchive` (hardware-api.ts:280-288) has **no production caller**; tests hardware_tests.rs:136,167 (R05) |
| 31 | POST /hardware/urdf/archive/{id}/restore | hardware.rs:448 | → result | W | Configuration | consul hardware-api.ts:310, called from import-wizard.tsx:167 |
| 32 | GET /hardware/commissioning-scope | hardware.rs:588 | → JSON | R | — | consul gateway-api.ts:149, called from commissioning-scope-editor.tsx:43 (R08) |
| 33 | PUT /hardware/commissioning-scope | hardware.rs:597 | JSON → JSON (writes scope file) | W | Configuration | consul gateway-api.ts:163, called from commissioning-scope-editor.tsx:59 |
| 34 | DELETE /hardware/commissioning-scope | hardware.rs:627 | → JSON | W | Configuration | consul gateway-api.ts:177, called from commissioning-scope-editor.tsx:72 |
| 35 | POST /control/restart-marengo-pi | restart.rs:72 | `{confirm}` → result. Restarts the runtime; refused while Active or while a persist is pending | W (process) | Management | consul config-api.ts:144, called from restart-confirm-dialog.tsx:95 |
| 36 | GET /version/status?refresh | deploy.rs:50 | → VersionStatus. Side effects: GitHub curl (marengo-deploy upstream.rs:148), cache write (upstream.rs:195), job-file reconcile write (status.rs:119) | R plus side effects | — | consul version-api.ts:135, called from use-sidebar-self-update.ts:184,263 (R07) |
| 37 | POST /control/deploy | deploy.rs:60 | `{confirm}` → job. Runs the self-update script via sudo | W (install) | Management | consul version-api.ts:153, called from use-sidebar-self-update.ts:297 |
| 38 | POST /command/enable | http.rs:374 | protobuf EnableRequest → `{ok}`. Publishes `robot/enable` | M | Control | consul gateway-api.ts:97 (imported by hardware-overview.tsx:30, testingStore.ts:11); tests public_test.rs:62,187 |
| 39 | POST /command/testing_mit | http.rs:392 | MitCommandBatch → `{ok}`. Publishes `robot/testing/mit_command_batch` | M | Control | consul gateway-api.ts:129, called from testingStore.ts:96,130,168, actuator-home.ts:66, use-compound-playback.ts:81 |
| 40 | POST /command/home | http.rs:158,410 | → 410 Gone (retired) | none | — (http.rs:240-241) | **none** (R02) |
| 41 | POST /command/set_zero | http.rs:431 | JSON `{joint,confirm,sign_test_passed,client_id}` → `{ok}`. Publishes `robot/set_zero` | M (encoder zero) | Calibration | consul gateway-api.ts:197, called from set-limits-panel.tsx:16 |
| 42 | POST /command/active_reporting_lease | http.rs:509 | JSON → `{ok}`. Publishes `robot/active_reporting_lease` | M (drive reporting) | Control | consul gateway-api.ts:247, called from use-active-reporting-lease.ts:50,66,89 |
| 43 | POST /command/motor_status_poll | http.rs:596 | JSON `{client_id}` → `{ok}`. Publishes `robot/motor_status_poll` | M (CAN Disable re-transmit) | Control | consul gateway-api.ts:219, called from use-motor-status-poll.ts:40 |
| 44 | POST /command/actuator | actuator.rs:34 | protobuf Envelope(OperatorCommand) → `{ok}`. Publishes `robot/actuator/command` | M (gain tuning) | Control, plus Configuration or Calibration by tier | Consul `postActuatorCommand` (gateway-api.ts:50-59) has **no production caller**; tests runtime-credentials.test.tsx:41, actuator.rs:325-558 (R12) |
| — | GET /assets/*, SPA fallback | http.rs:188,196 | static files (HTTPS only) | R | outside `authorize_api` (layered after it) | browser |
| — | WT CONNECT /chappe | webtransport.rs:45-189 | GatewaySubscribe → admission, then Envelope stream | R | SensitiveRead for sensitive topics | consul chappe-config.ts:33 and chappe-transport.ts (WebTransport path) |

Consul calls one route that does not exist: `POST /command/mit` (gateway-api.ts:113). See R01.

### Chappe topic inventory

**Transport.** In-process `Bus` broadcast (crates/chappe/src/lib.rs:100-121). `Bus::publish_bytes` sends every topic to the IPC fanout (lib.rs:108-113). The outbox only passes 7 "latest" topics (crates/chappe/src/ipc_outbox.rs:11-19) and 3 event topics (ipc_outbox.rs:143-148); anything else is dropped (ipc_outbox.rs:149-153).

**Gateway ingest.** Runtime→gateway frames arrive through `IpcListener` and `on_frame` (marengo-gateway main.rs:159-184) into `ingest_runtime_frame` (state.rs:197-230). The gateway's own bus is also drained for `ALLOWED_TOPICS` (state.rs:34-48, 344-359). Streams are filtered to `ALLOWED_TOPICS`, with `gateway/runtime_connection` always added (state.rs:326-340).

**Gateway→runtime commands.** Sent by `publish_command_envelope`, which writes to IPC and the local bus (state.rs:253-277). The Pi admits only the 7-topic allowlist, and only if the envelope is at most 1000 ms old (crates/chappe/src/ipc.rs:265-301).

The stream also carries a synthetic `marengo.v1.RuntimeObservationGap` envelope when a receiver lags (framing.rs:84-105). It is not a topic.

| Topic | Dir | Payload | Publisher(s) | Subscriber(s) / consumer(s) | Rate | Notes |
|---|---|---|---|---|---|---|
| robot/state | Pi→GW | RobotState | crates/berthier/src/loop.rs:1535 (`publish_robot_state` at loop.rs:1503) | GW snapshot state.rs:238 and stream; Consul chappe-config.ts:125,138 (dispatch chappe-transport.ts:171); MCP via HTTP #4 | `chappe_state_hz` (required key, no serde default; master 25 Hz at config/control.yaml:4; loop.rs:447) | outbox latest slot ipc_outbox.rs:14 |
| robot/safety | Pi→GW | SafetyState | bins/marengo-pi/src/main.rs:747-751 (called at main.rs:1462) | GW snapshot state.rs:239, used by restart.rs:86 and deploy.rs:90; Consul chappe-config.ts:126 | `chappe_state_hz` (main.rs:1365,1460) | |
| robot/heartbeat | Pi→GW | Heartbeat | marengo-pi main.rs:755-760 | GW snapshot state.rs:240, used by restart.rs:87 and deploy.rs:91; Consul chappe-config.ts:127 | 1 Hz (main.rs:1478) | |
| sensors/imu/torso | Pi→GW | ImuSample | marengo-pi imu.rs:139 (const imu.rs:14) | GW snapshot state.rs:241; Consul chappe-config.ts:128 (chappe-transport.ts:180) | `MARENGO_IMU_REPORT_HZ`, default 50 (imu.rs:49-52); only if `MARENGO_IMU_BUS` is set (imu.rs:41) | |
| host/metrics/pi | Pi→GW | HostMetrics | marengo-pi host_metrics.rs:25-28 (topic from marengo-host-metrics lib.rs:13,16-21) | GW snapshot state.rs:242; Consul chappe-config.ts:130 | 1 Hz (host_metrics.rs:34-35) | |
| host/metrics/jetson | (Jetson)→GW | HostMetrics | **No production publisher.** Only the gateway `--demo` publisher (webtransport.rs:520-523) | GW snapshot state.rs:243; Consul chappe-config.ts:131,144; outbox ipc_outbox.rs:17 | — | T02 |
| robot/actuator/limits | Pi→GW | ActuatorLimitSnapshot | marengo-pi overlay.rs:300-304 (on change, `limits_dirty` at overlay.rs:296; checked at main.rs:1469 and overlay.rs:168) | GW snapshot state.rs:244, used by actuator.rs:115 clamp. Consul reads it through HTTP #10, not the stream | on change, polled at `chappe_state_hz` | |
| logs/structured | Pi,GW→GW | LogEvent | chappe tracing_layer.rs:201 in marengo-pi (main.rs:1111) and marengo-gateway (main.rs:122) | GW persists it (state.rs:198-201); Consul chappe-config.ts:129, sent only with a credential (chappe-transport.ts:222) | at most 40/s per process (tracing_layer.rs:17) | sensitive; event queue ipc_outbox.rs:144 |
| robot/audit/action | Pi→GW | ActionEvent | marengo-pi limit_persist.rs:58-61, overlay.rs:680-683 | GW persist flags state.rs:203-219; `action_ack` action_ack.rs:22 (`/config/patch` ack). No client subscribes to the stream | event | sensitive |
| robot/audit/tuning | Pi→GW | TuningChangeEvent | marengo-pi overlay.rs:668-671 | **None in production.** Allowed and sensitive on the stream (state.rs:46, http.rs:266) but no client subscribes; test shutdown_tests.rs:468 | event | T04 |
| gateway/runtime_connection | GW→client | RuntimeConnectionState | GW state.rs:90-111 (on IPC connect or disconnect) | Consul chappe-config.ts:124 (chappe-transport.ts:168,285); always added (state.rs:332-338) | on transition | |
| robot/testing/telemetry | GW→client | RobotState (copy) | GW state.rs:225-229 (every robot/state frame is copied) | **none** | same as robot/state | T03 |
| robot/enable | GW→Pi | EnableRequest | GW http.rs:382-387 (#38) | Pi main.rs:1122 (handled at main.rs:481-496) | per request | Consul constant `CHAPPE_TOPICS.enable` is unused (T07) |
| robot/set_zero | GW→Pi | SetZeroRequest | GW http.rs:483-488 (#41) | Pi main.rs:1124 | Motion rate limit (http.rs:464) | |
| robot/active_reporting_lease | GW→Pi | ActiveReportingLeaseRequest | GW http.rs:573-578 (#42) | Pi main.rs:1125 (handler main.rs:513) | Diagnostics bucket (http.rs:553) | |
| robot/motor_status_poll | GW→Pi | MotorStatusPollRequest | GW http.rs:623-628 (#43) | Pi main.rs:1126 | global about 0.5/s, burst 2 (http.rs:594-595,607) | |
| robot/testing/mit_command_batch | GW→Pi | MitCommandBatch | GW http.rs:400-405 (#39) | Pi main.rs:1127. The GW also feeds its own copy back into the stream (state.rs:43, 274-276, 344-352) and no client subscribes to it | per request | sensitive; T05 |
| robot/actuator/command | GW→Pi | OperatorCommand | GW actuator.rs:136-139 (#44); limit_patch.rs:154-157 (#23) | Pi main.rs:1128 (overlay.rs:41) | Tuning bucket (actuator.rs:127) | |
| robot/homing | GW→Pi | HomingComplete | **none** (route #40 is retired) | Pi main.rs:1123, drained and ignored at main.rs:498-510; IPC allowlist ipc.rs:279 | — | T01 |
| *(robot/limits/patch)* | — | — | none | test literal only: gateway_access_conformance_test.rs:79 | — | T06, not a real topic |

ADR 0014 plans `heartbeat/jetson`, `perception/frame` and `navigator/intent` (docs/decisions/0014-jetson-perception-semantic-motion.md:132-135). None of them has code; they are not counted.

### Flags

Scope column: **S** is the full consumer scope stated at the top. **S-consul** is `consul/src` minus `consul/src/gen`.

| id | Item | Class | Evidence | Scope | Conf. |
|---|---|---|---|---|---|
| R01 | `POST /command/mit` | Consumer calls a route that does not exist (404 or 405), plus dead TS export | Consul fetches it at gateway-api.ts:113. No such route in http.rs:90-168. `postMitCommandBatch` has no caller; its only reference is the re-export at chappe-client.ts:12 | S-consul | high |
| R02 | `POST /command/home` and Consul `postHomeCommand` | Dead retired stub (public, returns 410) | Route at http.rs:158,410-415, auth exempt at http.rs:240-241. Consul throws locally without fetching (gateway-api.ts:141-145). Only references are the re-export at chappe-client.ts:14 and a mock at testing-overview.test.tsx:17. Not in the conformance list (conformance_test.rs:84-114) | S | high |
| R03 | GET `/snapshot/robot/{safety,heartbeat}`, `/snapshot/sensors/imu/torso`, `/snapshot/host/metrics/{pi,jetson}` | Dead routes: no consumer and no test | Defined at http.rs:95-102. Grep `snapshot/(robot/(safety\|heartbeat)\|sensors/imu\|host/metrics)` matches only http.rs. They are documented as a contract in docs/decisions/0008-chappe-webtransport-transport.md:30-31, and someone may curl them by hand | S | med |
| R04 | GET `/logs/sessions/{id}/download`, GET `/settings` | Tests-only routes | http.rs:121,123. Only hits are conformance_test.rs:93,95; grep `/download\|/settings\|session_download\|get_settings` finds nothing else | S | med |
| R05 | GET `/hardware/urdf`, GET `/hardware/urdf/archive/{id}` | No production consumer (tests only) | Consul `fetchLiveUrdf` (hardware-api.ts:105-114) and `fetchUrdfArchive` (hardware-api.ts:280-288) are never called outside tests (runtime-credentials.test.tsx:46; mock hardware-overview.test.tsx:58). Gateway tests hardware_tests.rs:98,117,136,167. MCP `pi_sync_bench_urdf` uses rsync, not HTTP | S | med |
| R06 | MCP `pi_logs_list`, `pi_logs_archive_list`; `pi-remote.sh logs-list` | Consumer and auth mismatch: always 401, then a silent fallback | They curl `/logs/sessions` with no credential (tools/marengo-pi-mcp/src/tools/logs.ts:59-63,98-102; scripts/pi-remote.sh:82-83). The route requires SensitiveRead (http.rs:224-225), so `curl -sf` fails and the script falls back to hot files or log-cli | S | high |
| R07 | GET `/version/status?refresh=1` | Public route with side effects, no auth | Public (http.rs:153,246-247). `refresh` forces a GitHub curl (crates/marengo-deploy/src/upstream.rs:74,148), writes the cache file (upstream.rs:189-195), and the reconcile path may write the job file (status.rs:119). ADR 0033:18-19 requires auth before "management work" | S | med |
| R08 | GET `/config/snapshot`, `/hardware/completeness`, `/hardware/commissioning-scope` | Public config and inventory reads (policy question) | They fall into the public GET branch (http.rs:246-247). ADR 0033:16 makes only health and "ordinary telemetry" public. Consul sends a Configuration token to these anyway (config-api.ts:74, hardware-api.ts:92-93, gateway-api.ts:149-150) | S | low |
| R09 | `POST /config/patch` | Capability mismatch on a route that changes live motion limits | Admitted with Configuration only (http.rs:236-237), then publishes `robot/actuator/command` (limit_patch.rs:154). Tuning on the same topic needs Control plus Configuration (actuator.rs:83-96; ADR 0033:47-48). The ADR text covers only tuning | S | low |
| R10 | Deployed bind | Bearer tokens sent over plaintext HTTP on all interfaces | systemd unit at scripts/systemd/marengo-gateway.service:13 and install-pi.sh:338 bind `[::]:8080`; the binary default is loopback (main.rs:49-55). scripts/env.example:33 says "LAN bench only". ADR 0033 does not require TLS | S | low |
| R11 | G07 doc status | Doc inconsistency | finding-index.md:56 says "Open". The ledger says "verified" (implementation-ledger.json:2666) and ADR 0033:3 says delivered at df468496 | docs | high |
| R12 | `POST /command/actuator` | No production consumer (tests only) | Consul `postActuatorCommand` (gateway-api.ts:50-59) is called only from runtime-credentials.test.tsx:41. No other `OperatorCommandSchema` producer in S-consul. Gateway tests actuator.rs:325-558. [INFERENCE] That leaves `robot/audit/tuning` (overlay.rs:669) with no production trigger. **Do not prune without asking**: it is the gain-tuning path | S | med |
| T01 | `robot/homing` | Subscribed, never published | Pi subscribes at main.rs:1123 and drains it at main.rs:498-510 with a "retired" warning. IPC allowlist ipc.rs:279. Test subscribers safety_publication_tests.rs:195, shutdown_tests.rs:462,1359. No publisher anywhere (#40 returns 410) | S | high |
| T02 | `host/metrics/jetson` | Subscribed, no production publisher | marengo-jetson is a scaffold (bins/marengo-jetson/src/main.rs:1-6). Only `HostNodeRole::Jetson` use is the demo at webtransport.rs:444,520-523. Subscribed at chappe-config.ts:131,144, outbox ipc_outbox.rs:17, route #9. Planned in ADR 0014:133,185 | S | high (fact); keep or prune is a decision for Phase B |
| T03 | `robot/testing/telemetry` | Published (made up by the gateway), never subscribed | state.rs:26,44,225-229 copies every robot/state frame (about 50 Hz) into the broadcast channel (capacity 4096, state.rs:50). No subscriber in S; Consul's subscribe list is chappe-config.ts:135-145 | S | high |
| T04 | `robot/audit/tuning` | Published, no production subscriber | Publisher overlay.rs:668-671; bridged at ipc_outbox.rs:146; stream allowlist state.rs:46. No client subscribes (S-consul, MCP); only test subscriber shutdown_tests.rs:468 | S | med |
| T05 | `robot/testing/mit_command_batch` on the stream | Stream echo with no consumer | In `ALLOWED_TOPICS` (state.rs:43). The gateway publishes its own command to its local bus (state.rs:274-276), the fanout re-ingests it (state.rs:344-352), and it reaches stream subscribers as a sensitive topic (http.rs:267). No subscriber in S; only the conformance probe at conformance_test.rs:176 | S | med |
| T06 | `robot/limits/patch` | Stale test literal (topic does not exist) | Only occurrence is gateway_access_conformance_test.rs:79. The real limit path publishes `robot/actuator/command` (limit_patch.rs:154), so the "no publication" assertion is vacuous for this entry | S | high |
| T07 | Consul `CHAPPE_TOPICS.enable` | Dead constant | chappe-config.ts:132. Not in the subscribe list (chappe-config.ts:135-145) and has no other reference | S-consul | high |
| T08 | Topic strings duplicated with no shared constant | Drift risk; all values currently match | `robot/audit/action`: limit_persist.rs:53, overlay.rs:44, GW state.rs:29, http.rs:265, ipc_outbox.rs:145. `robot/actuator/command`: overlay.rs:41, state.rs:32, ipc.rs:284. `robot/state`: loop.rs:1535, state.rs:17, ipc_outbox.rs:14, chappe-config.ts:125. `logs/structured`: tracing_layer.rs:15, state.rs:22, http.rs:264, ipc_outbox.rs:144. Host metrics: marengo-host-metrics lib.rs:13-14, state.rs:23-24, ipc_outbox.rs:16-17. Command topics as literals: GW http.rs:383,401,484,574,624, Pi main.rs:1122-1127, ipc.rs:278-284. GW `TOPIC_TESTING_MIT_COMMAND_BATCH` (state.rs:25) is not used by its publisher (http.rs:401 uses a literal) | S | high |
| T09 | Commands arriving on the Pi | Each command is counted as an outbox drop | ipc.rs:267 republishes each IPC command on the Pi bus. `Bus::publish_bytes` sends every topic to IPC (lib.rs:108-113), and the outbox drops non-telemetry topics (ipc_outbox.rs:149-153; test ipc_backpressure.rs:41 expects `robot/enable` to be Dropped). [INFERENCE] This inflates the dropped counter reported in host-metrics IPC health | S | low |
| T10 | `chappe::transport` module (`Transport` trait, `SharedBus`, `with_ipc_fanout`, `TransportError`), `Bus::ipc_configured`, `IpcListener::spawn_server` | Dead pub API in the Chappe crate | The module's only external reference is the re-export at crates/chappe/src/lib.rs:21,30. No `SharedBus` or `Transport` bound/use outside crates/chappe/src/transport.rs:11-91. `ipc_configured` is defined at lib.rs:166 and never called. `spawn_server` (ipc.rs:341) is unused; the gateway calls `spawn_server_with_lifecycle` (marengo-gateway main.rs:180). Corroborated by the zero-use list in docs/reviews/2026-10-03-crate-audit/metrics/pub-usage.md:14-17, which scans only `crates/*/src`; this grep also covered `bins/` | crates/, bins/ (Rust-only items) | high |

No route or topic was found that mutates state or touches motion without an auth gate. The only public POST is `/command/home`, which returns 410 without doing any work (http.rs:410-415).
