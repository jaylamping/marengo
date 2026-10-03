# Intent card: marengo-config

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-config` |
| Path | `crates/marengo-config` |
| Kind | lib |
| Baseline | `a2b55b3` |
| LOC | src 5124 (metrics/loc.md:18), of which ≈3680 is non-test: `lib.rs` 1301, `urdf_merge.rs` 658, `safety_validation.rs` 431, `profile_txn.rs` 324, `urdf_expand.rs` 260, `bench_joints.rs` 186, `completeness.rs` 178, `commissioning_scope.rs` 171, `limit_patch.rs` 148, `config_revision.rs` 24. Integration tests: 579 (`tests/safety_validation.rs`). 63 tests (metrics/test-counts.md:50). |
| Line coverage | crate 81.6% (metrics/coverage-by-crate.md:14). `urdf_expand.rs` 68.6%, `urdf_merge.rs` 72.4%, `lib.rs` 78.6%, `completeness.rs` 81.6%, `commissioning_scope.rs` 83.8%, `profile_txn.rs` 88.3%, `safety_validation.rs` 92.9%, `bench_joints.rs` 93.8%, `limit_patch.rs` 95.7% (metrics/coverage-by-file.md:27,30,38,49,57,63,85,92,98) |
| Deps | `armee-kinematics`, `rustc-hash`, `serde`, `serde_yaml 0.9`, `thiserror`, `urdf-rs` (`Cargo.toml:13-19`). No `tracing`. |
| History (53 commits) | `5106357`/`cc4276a` 2026-05-19 loaders. `bc8ba44` + `6a74cdd` 2026-06-13 velocity-cap resolver, then control.yaml as sole source (ADR 0010). `2da0f40` 2026-07-22 profile txn / CAS revision (ADR 0012). `f743825` 2026-07-22 URDF expand (ADR 0017). `8d6535a`/`9a7894e` 2026-08-09 URDF merge, commissioning scope, completeness (Hardware SoT). `d4c869b` 2026-09-29 `validate_safety_config` (CS15). `a5c4cdb` 2026-10-02 local mirror bound to its checkout (ADR 0032). `e1a1771` 2026-10-02 `resolve_reference_journal_path` (ADR 0036). `15a168d` 2026-10-02 per-tick allocation cuts. |
| Sources | `src/*.rs`, `tests/safety_validation.rs`, `README.md`, `codemap.md`, `src/codemap.md`, `AGENTS.md:25,82,182`, `crates/AGENTS.md:18,36`, `CONTEXT.md:22,34-48`, ADR 0004/0009/0010/0012/0014/0017/0022/0032/0036, `docs/homing.md:118-125`, `docs/rust-patterns.md:225`, prior review control.md CS15, gateway.md G04/G09, tooling.md T03, ledger, control-implementation-plan.md:73, metrics/* |

## 2. Intent

The crate owns the **Master YAML set** (`robot`/`motors`/`control`/`homing.yaml`, plus `network.yaml`) as typed data (`lib.rs:1-27`; `CONTEXT.md:37`). It provides five things:

1. **Declarative safety-policy admission.** This means numeric, identity, transform, envelope, gain, margin and homing checks across files. Davout calls it at startup, at every profile mutation and on every reference-binding check of every tick (`safety_validation.rs:1-4,374-431`; `davout/src/lib.rs:568,965,1095`; `davout/src/reference_transaction.rs:1452`).
2. **The single velocity-cap resolver**: joint → actuator group → motor type, from `control.yaml` only (ADR 0010; `lib.rs:592-646`).
3. **Durable write-behind of Set Limits.** It applies a **Live limit patch** to motors/control and expands the master URDF expand-only, with CAS revisions on the master dir (ADR 0012 §3, ADR 0017; `limit_patch.rs`, `urdf_expand.rs`, `profile_txn.rs`, `config_revision.rs`).
4. **Hardware-page support**: **Commissioning scope** persistence, warn-only completeness, and URDF merge preview/simulate (`commissioning_scope.rs`, `completeness.rs`, `urdf_merge.rs`; `CONTEXT.md:22-23,36-46`).
5. **Path resolution** for the repo root, config dir and the ADR 0036 reference-journal path (`lib.rs:209-274`).

**Conflicting statements of intent.**
- `lib.rs:3-4,21-24` says "no realtime logic" and "Does not enforce limits at runtime". Yet `safety_validation.rs:3-4` and commit `15a168d` tune these validators because they "run on every Davout control tick". That makes the validator part of the realtime path, even though Davout is the enforcer.
- `lib.rs:16-19` lists only parse/validate/URDF-path. It omits the persistence writers that are now a third of the crate.
- `README.md:3` frames the crate as a robot.yaml/network.yaml loader. In fact `network.yaml` has no consumer (§9).

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| Config schema types + loaders (`*_from(config_dir)` and env-resolving variants) | `lib.rs:110-321,854-1063` |
| Per-file + cross-file safety validation | `safety_validation.rs:56-431`; `lib.rs:648-810` |
| Velocity cap resolution (ADR 0010) | `lib.rs:592-646` |
| Limit-patch semantics: soft inset 27 mrad, soft clamp into hard | `limit_patch.rs:10-125`; ADR 0017 §1 |
| Atomic(ish) master YAML + URDF writers, CAS revision | `profile_txn.rs:29-88,259-297`; `urdf_expand.rs:19-132`; `config_revision.rs:10-23` |
| Commissioning-scope file format and effective scope (∩ `MARENGO_JOINT_SUBSET`) | `commissioning_scope.rs` |
| `MARENGO_JOINT_SUBSET` narrowing of robot/motors/control | `bench_joints.rs:83-151` |
| Repo/config/journal path resolution | `lib.rs:209-274` |

| Must not | Evidence | Status |
|---|---|---|
| Enforce limits at runtime / encode CAN / run loops | `lib.rs:21-24` | Respected for CAN and loops. Runtime enforcement is Davout's: it calls this crate's validator and revokes on failure (`davout/src/lib.rs:965-975`). |
| Hold live SoT | ADR 0012 §1: live SoT is in Davout memory | Respected. Writers are write-behind. |
| Redirect the local mirror via runtime env | ADR 0032 | Respected: `apply_local_limit_patch` uses `<root>/config` (`urdf_expand.rs:136-147`) |
| Shrink URDF hard limits on Set Limits | ADR 0017 §1 "expand-only" | Respected by `armee_kinematics::expand_urdf_joint_hard`. Rounding caveat in §10 L9. |

Dependency direction: `marengo-config → armee-kinematics` (URDF load/expand), while `armee-kinematics` dev-depends back on it (Cargo dev-cycle, `crates/armee-kinematics/Cargo.toml:18`). No violation found. Per `crates/AGENTS.md:3` the `//!` doc should declare allowed deps, and it does not (§8).

## 4. Interface

| Group | Key surface | Production consumers | Unused / test-only |
|---|---|---|---|
| Schema types | `RobotConfigFile`, `MotorsConfigFile`/`MotorEntry`/`MotorType`, `ControlConfigFile`/`ControlSection`/`JointControlEntry`/`DangerZoneRule`/`WrongSignWatchdogConfig`, `HomingConfigFile`/`HomingSection`/`EffectiveHomingJoint`, `NetworkConfigFile`, `ConfigError`, re-exported `FxHashMap` (`lib.rs:76`) | davout, berthier, robstride, armee-dynamics/kinematics, candump, gateway, pi, log-cli, motor-repl, limit-sync (cargo tree -i) | `NetworkConfigFile`: none |
| Loaders | `load_{robot,motors,control,homing}_config_from`, `load_*_config(repo_root)`, `load_network_config` | `berthier/src/loop.rs`, `davout/src/lib.rs`, `marengo-candump/src/lib.rs`, `marengo-gateway/src/{config,hardware}.rs`, `marengo-log-cli/src/gravity_fit.rs`, `marengo-pi/src/main.rs`, `motor-repl/src/main.rs` | `load_network_config` (`lib.rs:1262`): own test only |
| Paths | `resolve_repo_root`, `resolve_config_dir`, `resolve_urdf_path`, `resolve_reference_journal_path`, `DEFAULT_PI_CONFIG_DIR`, `REFERENCE_JOURNAL_FILE` | `resolve_reference_journal_path`: `marengo-pi/src/main.rs:1079`, `motor-repl/src/main.rs:193`, `berthier/src/loop.rs:395` (doc). Others are widely used. | consts: internal only |
| Safety validation | `validate_safety_config`, `validate_control_against_limits`, `validate_motors_against_robot`, `validate_robot_control_joint_coverage`, `validate_{robot,motors,homing}_config`, `validate_control_config`, `validate_joint_gains_against_motor_type`, `JointControlEntry::limit_margin_fields_valid` | `davout/src/lib.rs:156-157,568-584,742,760,965,1095`; `davout/src/limit_envelope.rs:58,111`; `davout/src/reference_transaction.rs:1452` | `validate_{robot,motors,homing}_config`, `validate_control_config`: called only internally by loaders. `validate_joint_gains_against_motor_type` (`lib.rs:1170`): own tests only. |
| Velocity cap | `resolve_joint_velocity_cap`, `resolve_desired_joint_velocity_cap`, `actuator_group_for_joint`, `motor_type_key` | `resolve_joint_velocity_cap`: `davout/src/lib.rs`, `davout/src/reference.rs`, `marengo-gateway/src/config.rs`. `motor_type_key`: berthier, davout, pi overlay. | `resolve_desired_…` and `actuator_group_for_joint`: internal/test only |
| Command allowlist / subset | `CommandJointAllowlist`, `load_command_joint_allowlist[_from]`, `resolve_command_joint`, `joint_subset_from_env`, `apply_joint_subset`, `validate_joint_subset` | gateway `main.rs`/`actuator.rs`/`http.rs`/`state.rs`/`hardware.rs`; `marengo-pi/src/overlay.rs`; `davout/src/lib.rs:560-561` | `validate_joint_subset`: internal |
| Commissioning scope | `load/save/clear_commissioning_scope`, `default_commissioning_scope_path`, `effective_commissioning_scope`, `scope_widens`, `validate_commissioning_scope_joints`, `CommissioningScopeFile` | `marengo-gateway/src/hardware.rs` (write path); `davout/src/lib.rs:1444-1450` (read path) | — |
| Limit patch | `LimitPatch`, `validate_limit_patch`, `apply_limit_patch_to_{motor,control}`, `ensure_soft_inset`, `soft_limits_with_inset`, `DEFAULT_SOFT_INSET_RAD` | `davout/src/limit_envelope.rs`, `marengo-pi/src/overlay.rs`, `marengo-gateway/src/limit_patch.rs`, `marengo-limit-sync/src/main.rs` | — |
| Writers / txn | `write_motors_control_and_urdf`, `apply_local_limit_patch`, `write_control_config_from`, `apply_joint_config_param`, `profile_content_revision`, `limit_patch_from_motor`; `write_motors_and_control`, `expand_urdf_file_to_cover_motors`, `upsert_joint_limits`, `add_joint_from_source`, `joint_in_motors`, `joint_in_profile_urdf`, `membership_slugs_for_joint`, `UpsertLimitResult`, `AddJointResult`, `control_config_path` | `write_motors_control_and_urdf`, `write_control_config_from`: `marengo-pi/src/limit_persist.rs`. `apply_joint_config_param`: `marengo-pi/src/overlay.rs`. `apply_local_limit_patch`: `marengo-limit-sync`. `profile_content_revision`: gateway `config.rs`/`limit_patch.rs` and pi `limit_persist.rs`/`overlay.rs`. `limit_patch_from_motor`: gateway `limit_patch.rs`. | `add_joint_from_source`, `joint_in_motors`, `joint_in_profile_urdf`, `membership_slugs_for_joint`, `AddJointResult`: **no consumer**. `upsert_joint_limits`: one Berthier test (`crates/berthier/tests/feedback_bootstrap.rs:149`). `write_motors_and_control`, `expand_urdf_file_to_cover_motors`: internal and tests. |
| URDF merge | `merge_preview_from_paths`, `simulate_merge_xml`, `unresolved_critical_fields`, `MergePreview`, `FieldResolution`; `apply_merge_xml`, `validate_merged_urdf_xml`, `merge_preview_from_robots`, `FieldDiff`, `ResolutionChoice` | `marengo-gateway/src/hardware.rs:17-18,210-300,465-490` | `apply_merge_xml`, `validate_merged_urdf_xml`, `merge_preview_from_robots`: called only by `simulate_merge_xml`. Private `is_actuated` is `#[allow(dead_code)]` (`urdf_merge.rs:72-78`; metrics/suppressions.md:496). |
| Completeness | `completeness_report`, `CompletenessReport`, `CompletenessWarning` | `marengo-gateway/src/hardware.rs:177,361` | — |
| Misc | `commanded_position_window` (`lib.rs:813`) | `marengo-log-cli/src/gravity_fit.rs` | — |

Depth assessment:
- **Deep:**
  - `validate_safety_config` is one call hiding ≈400 lines of cross-file policy.
  - `write_motors_control_and_urdf` hides the URDF-first ordering plus restore-on-YAML-failure.
  - `resolve_joint_velocity_cap` hides the 3-tier precedence.
- **Shallow or leaky:**
  - The validator surface exposes 9 overlapping entry points whose scopes nest. `validate_safety_config` already calls `validate_control_against_limits` (`safety_validation.rs:384`), which calls `validate_motors_against_robot`, `validate_control_config` and coverage (`lib.rs:742-744`). Davout still calls all of them again in sequence (`davout/src/lib.rs:568-570,584`).
  - `resolve_joint_velocity_cap` is a pure pass-through alias (`lib.rs:640-646`).
  - `profile_txn` exposes 7 unused helpers.
- **Seams:** there are no traits, and so no adapters. Filesystem and env access are hard-wired (`std::fs`, `std::env::var`). Tests use temp-dir copies (`tests/safety_validation.rs:17-38`).
- **Features:** none.

Metrics cross-check: `pub-usage.md` uses a crates-only heuristic, so it under-reports this crate. For example, it misses `add_joint_from_source`, which has no consumer at all. The grep in this card covers crates, bins and tools.

## 5. Invariants owned

| Invariant | Enforcing code | Tests that fail if broken | File line cov. |
|---|---|---|---|
| Robot joints non-empty and unique; bench caps finite ≥0 | `safety_validation.rs:56-75` | `tests/safety_validation.rs:80` | 92.9% |
| Motors: unique joint, unique (iface, id), direction ±1, gear_ratio >0, finite ordered hard bounds, caps ≥0 | `safety_validation.rs:78-129` | `tests/safety_validation.rs:101`; `lib.rs:1372` | 92.9% |
| Control timing: 1 ≤ loop_hz ≤ 1e6; chappe_hz ≤ loop_hz; watchdog >0; poll budget fits one tick; drain_quiet ≤ budget | `safety_validation.rs:168-201` | `tests/safety_validation.rs:140` | 92.9% |
| GravityComp wire gains must be exactly 0 (ADR 0004) | `lib.rs:702-716` | `lib.rs:1596,1610` | 78.6% |
| Impedance/friction gains finite, ≥0, ≤ motor-type maxima; ki ≤ kp_max | `lib.rs:1186-1258` (called per joint from `safety_validation.rs:230`) | `lib.rs:1678,1693`; `tests/safety_validation.rs:140` | 78.6% |
| Danger zones: unique name, known joint, action ∈ {clamp_velocity, clamp_torque}, clamp_torque requires a cap | `safety_validation.rs:232-280` | `tests/safety_validation.rs:140` | 92.9% |
| Wrong-sign watchdog sign ±1, min ticks >0 | `safety_validation.rs:281-297` | `tests/safety_validation.rs:140` | 92.9% |
| Actuator groups: cap >0, non-empty, members in control.joints and robot.joints, each joint in at most one group (ADR 0010) | `lib.rs:648-688,762-771` | `lib.rs:1581` | 78.6% |
| Velocity cap precedence joint > group > type; never NaN/≤0 | `lib.rs:592-637` | `lib.rs:1548,1562,1571` | 78.6% |
| Trajectory cruise ≤ resolved cap; soft bounds within motor hard; control motor_type == motors motor_type | `lib.rs:772-808` | `lib.rs:1636`; `tests/safety_validation.rs:335` | 78.6% |
| Every active joint has a motor, control and homing entry | `lib.rs:722-735,772-781`; `safety_validation.rs:386-392` | `lib.rs:1624`; `tests/safety_validation.rs:454` | — |
| Homing numbers finite/positive; timeout fits a `Duration`; Hall requires 3 distinct GPIOs; Hall search speed/torque ≤ caps | `safety_validation.rs:301-372,393-428` | `tests/safety_validation.rs:293,480` | 92.9% |
| Writers validate the full candidate before touching any artifact | `profile_txn.rs:266`; `urdf_expand.rs:111`; `lib.rs:1074` | `tests/safety_validation.rs:363,402` | 88.3% / 68.6% |
| URDF written first; YAML failure restores the URDF bytes (ADR 0017 §2) | `urdf_expand.rs:101-132` | `urdf_expand.rs:327` (happy path only); restore branch **untested** | 68.6% |
| Set Limits soft = hard inset by 27 mrad when absent; soft clamped into new hard | `limit_patch.rs:13-34,99-125` | `limit_patch.rs:228,236,276` | 95.7% |
| CAS: a stale `expected_revision` rejects without writing | `profile_txn.rs:237-249` | `profile_txn.rs:460` | 88.3% |
| Subset fails closed on unknown or empty | `bench_joints.rs:99-151` | `bench_joints.rs:278,286,248` | 93.8% |
| Commissioning scope: unknown version rejected; unknown joints rejected before save (validation is caller's job); widening detected | `commissioning_scope.rs:53-119` | `commissioning_scope.rs:233,243,285` | 83.8% |
| Journal path is absolute and normalized; default sits beside the calibration history (ADR 0036) | `lib.rs:239-274` | **untested** (no test references `resolve_reference_journal_path` or `MARENGO_REFERENCE_JOURNAL`) | 78.6% |
| Davout revokes reference when the installed policy fails validation (per tick) | enforced in Davout `davout/src/lib.rs:965-975` using `validate_safety_config` | `davout/tests/current_reference.rs:229,525`; `davout/src/reference_grant_tests.rs:645` | — |

## 6. Inputs / outputs

| Kind | Items |
|---|---|
| Config files read | `robot.yaml`, `motors.yaml`, `control.yaml`, `homing.yaml` under the config dir (`lib.rs:277-311`). `network.yaml` under `<repo_root>/config`, ignoring `MARENGO_CONFIG_DIR` (`lib.rs:1262-1265`). URDF at `<repo_root>/<robot.urdf>` (`lib.rs:1285-1295`). |
| Files written | `motors.yaml`, `control.yaml`, `robot.yaml`, `homing.yaml` via `*.yaml.tmp` + rename (`profile_txn.rs:259-297`); `control.yaml` alone (`lib.rs:1070-1091`); URDF via `*.urdf.tmp` + rename and an in-place restore (`urdf_expand.rs:82-93,121`); `var/commissioning-scope.yaml` (`commissioning_scope.rs:122-170`); temp `$TMPDIR/marengo-merge-*.urdf` (`urdf_merge.rs:370-391`) |
| Env vars | `MARENGO_ROOT` (`lib.rs:210`; `commissioning_scope.rs:46`), `MARENGO_CONFIG_DIR` (`lib.rs:220`), `MARENGO_REFERENCE_JOURNAL`, `MARENGO_CALIBRATION_RECORD` (`lib.rs:243-246`), `MARENGO_JOINT_SUBSET` (`bench_joints.rs:84`) |
| Fixed paths | `/opt/marengo/config` if it exists (`lib.rs:216-226`); `/opt/marengo/var/commissioning-scope.yaml` (`commissioning_scope.rs:45-50`); compile-time `CARGO_MANIFEST_DIR/../..` (`lib.rs:212`) |
| Key config fields with no runtime consumer | `robot.bench.max_joint_velocity_rad_s`: validated only (`safety_validation.rs:60-63`), with zero consumers outside the crate. `motors.*.bench.velocity_limit_rad_s`: only in the reference-binding equality check (`davout/src/reference.rs:350`; ADR 0010:35). `homing.search_velocity/torque/direction/backoff/allow_sensor_overlap`: binding check only (`davout/src/reference.rs:398`). `network.chappe_bind`: none (ADR 0014:35 "placeholder"). |
| Proto, CAN, HTTP | none directly. Consumers expose its data over Chappe `LimitPatchCommand` and the gateway Hardware routes. |

## 7. Prior review reconciliation

| Prior ID | Prior claim | Current status | Evidence |
|---|---|---|---|
| CS15 (P2, ledger partial) | Startup validators accept negative kd, NaN velocity, negative torque slew, duplicate joint mapping; gains check not in startup; no unknown-key rejection; no single validated config object | **Partial, unchanged since the ledger.** Fixed: negative/over-max gains (`lib.rs:1186-1258` via `safety_validation.rs:230`), NaN velocity (`lib.rs:694-701`), negative slew (`safety_validation.rs:202-205`), duplicate joint/address, direction, gearing (`safety_validation.rs:78-104`). Tests: `tests/safety_validation.rs:80-568`, commit `d4c869b`. **Open:** unknown/duplicate YAML keys (no `deny_unknown_fields` anywhere in `src/`); the immutable validated object, planned as `ValidatedRobotConfig` in `control-implementation-plan.md:73`, does not exist (grep), so Davout revalidates mutable raw structs per tick. | — |
| G09 (P1/P2, open) | URDF expand uses a fixed `marengo.urdf.tmp` shared with gateway activation; no generation CAS | **Open** | `urdf_expand.rs:82` still `with_extension("urdf.tmp")`. The restore at `urdf_expand.rs:121` writes back the pre-persist bytes over any later generation. |
| G04 (P1, open) | Pi persist coalescing drops motors/URDF writes | **Open (owner: marengo-pi).** This crate's writer takes full snapshots, so the defect is in the caller's queue. | `bins/marengo-pi/src/limit_persist.rs` (not re-read here) |
| T03 (P1, open) | MCP sync overwrites durable taught state | **Open (owner: tools).** `profile_content_revision` exists, but sync tools don't use it. | `tooling.md:68-76` |
| T04 (P1, partial) | Loopback limit writer | **Partial (owner: tools).** The crate side is fixed per ADR 0032: the writer is bound to its checkout (`urdf_expand.rs:136-147`, `a5c4cdb`). | — |
| T25 (P2, open) | Auto Learn ignores per-joint velocity caps | **Open (owner: tools).** The resolver is available (`lib.rs:640`). | ledger |
| CS05/CS06 | — | Not this crate (see the marengo-homing card) | — |

## 8. Drift

| Source | Claim | Reality |
|---|---|---|
| `lib.rs:3-4` | "no realtime logic" | The validator runs on every Davout tick, inside `reference_binding_valid` (`davout/src/lib.rs:958-975`), and is perf-tuned for that (`safety_validation.rs:3-4`, `15a168d`). |
| `lib.rs:8-14`, `README.md:3` | `network.yaml` → `NetworkConfigFile` consumed by "Chappe / bins" | Zero consumers. ADR 0014:35 calls it a placeholder. |
| `lib.rs:16-24` | Responsibilities = parse/validate/URDF path | Also persistence (4 writer modules), path resolution, commissioning scope and URDF merge. |
| `codemap.md:3`, `src/codemap.md:4` | "Typed loaders"; "resolve_repo_root for path resolution" | Same omissions. `src/codemap.md` does not mention `resolve_reference_journal_path`, `commissioning_scope.rs` or `limit_patch.rs`. |
| `codemap.md:12` | "Profile txn / URDF expand target master paths only (no bringup CAS)" | Accurate. But `profile_txn::upsert_joint_limits`/`add_joint_from_source` (the CAS txn API) have no production consumer. Live CAS is done by callers with `profile_content_revision`. |
| `crates/AGENTS.md:3` | Each `//!` declares allowed deps | `lib.rs:1-27` does not. |
| `crates/codemap.md:34` | `marengo-config ← berthier, davout, armee-dynamics` | Also robstride, candump, homing, gateway, pi, log-cli, motor-repl, limit-sync (cargo tree -i). armee-dynamics is a dev-dependency only. |
| ADR 0010:33 | "`resolve_joint_velocity_cap` (alias for `resolve_desired_joint_velocity_cap`)" | True, but the "desired vs effective" split behind the alias was removed in `6a74cdd` (the `min()` with bench and URDF caps was deleted). The alias is vestigial. |
| `bench_joints.rs:3-4` | "not a hardcoded left/right bench table" | `joint_lookup_candidates` hardcodes aliases (`bench_joints.rs:166-185`: `shoulder_*` → `left_*`, `elbow` → `right_elbow_pitch` then `left_elbow`). |
| `commissioning_scope.rs:4` | "Writes use temp + rename" | True, but without fsync, so not crash-durable. Same for `profile_txn.rs:259-297` "Atomic" (`:28`) and `lib.rs:1069`. ADR 0032 already disclaims multi-file power-loss durability. |
| `profile_txn.rs:28` | "Atomic motors.yaml + control.yaml" | It also rewrites `robot.yaml` and `homing.yaml` (`profile_txn.rs:267-272`), dropping their comments and order through serde re-serialization. A rename failure partway through leaves a mixed generation (§10). |
| `lib.rs:1158-1160` | `velocity_max_rad_s` overlay "gated until Davout limits rebuild is wired" | ADR 0012 §2 routes live `velocity_max_rad_s` through `LimitPatchCommand` instead. The gate message is stale. |

## 9. Prune candidates

| # | Candidate | Evidence class | Confidence | Deleting touches |
|---|---|---|---|---|
| P1 | `resolve_desired_joint_velocity_cap` → fold its body into `resolve_joint_velocity_cap` | Superseded by commit `6a74cdd` (the desired/effective split is gone); duplicate entry point | high | `lib.rs:592-646`; tests `lib.rs:1548-1570`; ADR 0010:33 wording. Keep the `resolve_joint_velocity_cap` name, which is cited by `AGENTS.md:182`, `crates/AGENTS.md:36` and `docs/rust-patterns.md:225`. |
| P2 | `add_joint_from_source`, `joint_in_motors`, `joint_in_profile_urdf`, `membership_slugs_for_joint`, `AddJointResult` | Zero references outside own tests. Membership is "master-only after SoT cutover" (`profile_txn.rs:224`). Bringup profiles are retired (`CONTEXT.md:38`). | high | `profile_txn.rs:90-194,225-235`; tests `profile_txn.rs:476-546` |
| P3 | `upsert_joint_limits` + `UpsertLimitResult` + `check_revision` | No production consumer. One Berthier test uses it as a fixture writer (`crates/berthier/tests/feedback_bootstrap.rs:149`). ADR 0017 §1 still names it as the "inactive" path. | med (needs ADR 0017 touch) | `profile_txn.rs:18-88,237-249`; Berthier test switches to `write_motors_control_and_urdf`; tests `profile_txn.rs:433-475` |
| P4 | `load_network_config`, `NetworkConfigFile`, `NetworkSection`, `config/network.yaml` | Scaffold with no consumer (ADR 0014:35 "placeholder … no bridge implementation") | med (planned Jetson bridge) | `lib.rs:187-195,1261-1265,1404-1407`; `README.md:3`; `config/AGENTS.md:13` |
| P5 | `is_actuated` (`urdf_merge.rs:72-78`) | Zero references; `#[allow(dead_code)]` (metrics/suppressions.md:496) | high | 7 lines |
| P6 | `load_urdf_from_str` temp-file round trip (`urdf_merge.rs:370-391`) → `urdf_rs::read_from_string` | Duplicate implementation: `armee_kinematics::load_urdf` → `urdf_rs::read_file` → `read_from_string` for `.urdf` (`urdf-rs-0.8.0/src/funcs.rs:16,71`) | med | 20 lines plus the `AtomicU64`/`SystemTime` imports. It also removes a temp-file leak (§10 L10). |
| P7 | Narrow visibility (pub → private): `apply_merge_xml`, `validate_merged_urdf_xml`, `merge_preview_from_robots`, `validate_joint_gains_against_motor_type`, `validate_{robot,motors,homing}_config`, `validate_control_config`, `actuator_group_for_joint`, `control_config_path`, `write_motors_and_control`, `expand_urdf_file_to_cover_motors`, `DEFAULT_PI_CONFIG_DIR`, `REFERENCE_JOURNAL_FILE`, `validate_joint_subset`, `limit_margin_fields_valid` | Zero external consumers. Shrinks the interface without deleting logic. | med | `lib.rs:39-69` re-exports; `tests/safety_validation.rs:9,12` imports `expand_urdf_file_to_cover_motors` and `write_motors_and_control` (keep pub or move those tests in-crate) |
| P8 | `robot.bench.max_joint_velocity_rad_s` | Superseded by ADR 0010 / `6a74cdd`. Validated but never consumed. | low-med (ADR 0010:35 allows it "for commissioning documentation") | `lib.rs:130`; `safety_validation.rs:60-63`; YAML files; the humanoid templates' test |
| P9 | Redundant validator calls in Davout | Duplicate validation: `validate_safety_config` already covers `validate_motors_against_robot` and `validate_robot_control_joint_coverage` (`lib.rs:742-744`) | med (owner: Davout) | `davout/src/lib.rs:569-570,584` |
| — | **Not prunable:** any validator, `commanded_position_window`, the danger-zone / wrong-sign schema, Hall `sensors` validation. These are safety policy, even where coverage is lower (`lib.rs` 78.6%). Gaps are listed in §10. | — | — | — |

## 10. Phase-B leads

| # | Lead | Where | Why suspicious |
|---|---|---|---|
| L1 | **Per-tick, per-joint full-policy validation** | `davout/src/lib.rs:646-657` → `reference_binding_valid` (`:958-975`) → `validate_safety_config` | `joint_homing_state` runs the whole cross-file validator on every call. It is called per joint from `commissioning_facets`, `joint_drive_active` and the wire facets, so cost is O(joints² + groups²) per tick on the 200 Hz path (`validate_actuator_groups` is quadratic, `lib.rs:664-676`). Planned fix C0 `ValidatedRobotConfig` (control-implementation-plan.md:73) is not implemented. Measure the loop jitter (M06). |
| L2 | **Multi-file write is not atomic** | `profile_txn.rs:287-295` | Renames run sequentially. A failure on file k leaves files <k renamed (new) and >k old, and cleanup only removes temp files. The mixed generation can pass per-file loaders but fail cross-file validation at next boot, or pass with wrong limits. There is no fsync (`:277`, `urdf_expand.rs:83`, `commissioning_scope.rs:148`, `lib.rs:1081`). |
| L3 | **Shared fixed temp names** | `urdf_expand.rs:82` (`urdf.tmp`), `profile_txn.rs:276`, `lib.rs:1080`, `commissioning_scope.rs:147` (`yaml.tmp`) | Concurrent writers clobber each other's temp files: gateway URDF activation (G09), Pi persist worker, local mirror, `write_control_config_from`. CAS is check-then-act with no lock (`profile_txn.rs:49` vs `:83`), so TOCTOU. |
| L4 | **URDF restore overwrites later generations, non-atomically** | `urdf_expand.rs:120-129` | On YAML failure the old bytes are written in place with `fs::write` (no temp + rename). Another writer's newer URDF can be reverted, or the file left torn on a crash. G09 is open. |
| L5 | **Writers rewrite `robot.yaml` and `homing.yaml` through serde** | `profile_txn.rs:35-38,267-272`; `urdf_expand.rs:120` | A Set Limits persist re-serializes untouched files, dropping comments and reordering `FxHashMap` keys (`control.joints`, `homing.joints`). The revision hash changes even though no semantic edit happened (`config_revision.rs:13-20`), which triggers spurious CAS mismatches. |
| L6 | **CAS hash uses `DefaultHasher`** | `config_revision.rs:3,12` | std docs say SipHash output is not stable across Rust releases. Gateway and Pi binaries built by different toolchains, or a revision persisted across upgrades, can disagree. It is a 64-bit, non-cryptographic hash. |
| L7 | **Lexical `..` collapse in journal path** | `lib.rs:259-273` | `std::path::absolute` keeps `..`, and the pop is lexical. Through a symlinked parent this can name a different file than the OS resolves. Davout later canonicalizes (`davout/src/reference_journal.rs:528`), but the "distinct from history" check depends on both sides resolving the same way. **Untested.** In dev, an absolute `calibration_record_path` (`config/homing.yaml:3`) places the journal at `/opt/marengo/var/calibration/` unless env overrides it. |
| L8 | **Inconsistent root resolution** | `lib.rs:209-213` (MARENGO_ROOT, else compile-time path) vs `commissioning_scope.rs:45-50` (MARENGO_ROOT, else `/opt/marengo`) vs `lib.rs:219-226` (`/opt/marengo/config` wins over the repo when present) | A dev/test process on a host with `/opt/marengo` mixes installed config or scope with checkout URDF and history. Davout reads scope from `default_commissioning_scope_path()` regardless of the repo root it was built with (`davout/src/lib.rs:1443-1444`). |
| L9 | **Expand-only can round inward** | `urdf_expand.rs:256-259` (`{:.6}` then trim) | A taught hard bound of e.g. −1.2345678 is written as −1.234568, which is narrower than motors.yaml. That breaks the ADR 0017 "URDF covers motors" invariant by <1e-6 rad. Davout uses URDF ∩ motors, so the effect is tiny, [INFERENCE] A rewrite is then likely re-triggered on every persist. |
| L10 | **Temp-file leak on parse error** | `urdf_merge.rs:388-389` | The `?` on `load_urdf` returns before `remove_file`, so every malformed upload leaves `$TMPDIR/marengo-merge-*.urdf` behind. Growth is unbounded. |
| L11 | **Text-search URDF rewrite can hit the wrong element** | `urdf_expand.rs:187-198`; `urdf_merge` `rewrite_*` | It finds the first `name="<joint>"` anywhere, then `rfind("<joint")`. A link, transmission or mimic using the same name earlier in the file redirects the edit to the previous joint's `<limit>`. |
| L12 | **Soft bounds silently clamped, may equal hard** | `limit_patch.rs:104-111` | Existing soft outside the new hard is clamped instead of rejected, giving soft ≡ hard, which ADR 0017 §1 forbids. This is pinned by test `limit_patch.rs:276`. `soft_limits_with_inset` caps the inset at span/4 (`:18`). |
| L13 | **No upper bound on `zero_verify_tolerance_rad`** | `safety_validation.rs:325-328` (only ≥0) | ADR 0036 uses it as the reference-evidence tolerance and the at-rest discontinuity bound. A typo like `5.0` makes physical evidence and continuity checks vacuous. `search_timeout_s` is likewise unbounded above, and it sets the blocking `calibrate_joint_zero` deadline. |
| L14 | **Misleading error kind** | `lib.rs:773-776` | A robot joint without a motor raises `UnknownMotorJoint`, whose message reads "unknown joint … in motors.yaml (not listed in robot.yaml)" (`lib.rs:89-90`), i.e. the opposite direction. |
| L15 | **Hardcoded cross-side aliases** | `bench_joints.rs:166-185` | Operator input `elbow` resolves to `right_elbow_pitch` while `shoulder_roll` resolves to `left_shoulder_roll`. Once both arms are wired, an ambiguous name commands a side the operator may not intend. Pinned by test `bench_joints.rs:269`. |
| L16 | **`load_commissioning_scope` treats `exists()==false` as absent** | `commissioning_scope.rs:57` | This is the same metadata-precheck pattern ADR 0022 removed for history. It is fail-closed here (no scope means full Robot Ready required), but a permission error masquerades as "no scope". Persisted joints are not revalidated against master on load. |
| L17 | **Unknown YAML keys accepted** | all `#[derive(Deserialize)]` in `lib.rs` | A typo such as `velocity_max_rad_S` silently drops a safety cap and falls through to the group or type default. CS15 open item. |
| L18 | **Zero is allowed for safety caps** | `safety_validation.rs:60-67,119-126,270-278` (nonnegative) | `max_joint_torque_nm: 0` or a danger-zone `max_velocity_rad_s: 0` passes. That is fail-safe (it clamps to zero) but may confuse operators. `tests/safety_validation.rs:426` pins "zero nonnegative caps" as accepted. Confirm intent. |
| L19 | **`DEFAULT_PI_CONFIG_DIR` presence check** | `lib.rs:222-224` | `is_dir()` on `/opt/marengo/config` silently overrides the checkout in dev. The bench_joints test `master_boot_resolution_uses_repo_config_when_pi_path_missing` (`bench_joints.rs:230`) only covers the absent case. |
