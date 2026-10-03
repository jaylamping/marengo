# Intent card: marengo-homing

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-homing` |
| Path | `crates/marengo-homing` |
| Kind | lib |
| Baseline | `a2b55b3` |
| LOC | src 1325, of which about 750 is non-test (`calibration.rs` 36, `commissioning.rs` 147, `lib.rs` 31, `registry.rs` 254, `sensor.rs` 151, `verify.rs` 130); integration tests 1242 (metrics/loc.md:21); 23 tests (metrics/test-counts.md:53) |
| Line coverage | crate 88.5% (metrics/coverage-by-crate.md:16). `sensor.rs` 67.5%, `registry.rs` 79.1%, `commissioning.rs` 93.7%, `lib.rs` 94.7%, `verify.rs` 96.1% (metrics/coverage-by-file.md:26,41,91,95,100) |
| Deps | `armee-proto`, `marengo-config`, `serde`, `serde_yaml`, `thiserror`, `chrono` (`Cargo.toml:13-19`) |
| History | `fc07165` 2026-05-25 "Add verified homing and zero reference for bench startup" (creation, ADR 0006). `9a7894e` 2026-08-09 added the commissioning facets / `select_enable_targets`. `8c3f62e` 2026-09-30 separated history from startup readiness (ADR 0022). `7951f10` 2026-09-30 put motor admission behind private reference authority and added scalar input validation (ADR 0023). `d541bd5` 2026-09-30 made a failed history write leave the record unchanged (batch08). No change since ADR 0036 (`e1a1771`, 2026-10-02). |
| Sources | `src/*.rs`, `codemap.md`, `src/codemap.md`, `crates/AGENTS.md:19,38`, `crates/codemap.md:19`, `CONTEXT.md:18-25`, ADR 0006/0022/0023/0036, `docs/homing.md`, `docs/safety.md:18-56`, `docs/rust-patterns.md:185`, prior review control.md CS05/CS06, ledger, batch05/batch08, Davout call sites, `scripts/homing-preflight.sh`, metrics/* |

## 2. Intent

**Original intent (ADR 0006, `fc07165`).** The crate owned the per-joint homing lifecycle: Unhomed → Homing → Verified/Faulted. It also owned the 3-Hall sensor truth table, the host calibration record (`zero_registry.yaml`) and the scalar zero verification. All of these together gated Davout `Ready`/`Enable` (ADR 0006 "Consequences"; `docs/decisions/0006-homing-zero-reference.md:58-63`).

ADRs 0022, 0023 and 0036 removed that authority step by step:
- **ADR 0022:** history never grants anything, and every registry starts `Unhomed`.
- **ADR 0023:** "Davout owns one private current-reference authority … History and scalar policy checks in marengo-homing do not confer permission" (`0023:17-18`).
- **ADR 0036:** grants are process-local and come from a qualified physical transaction. That transaction is journaled in SQLite (`reference-journal.sqlite3`), not in the YAML history. "Neither history nor the journal grants anything" (`0036:103-104`, `:208-213`).

Davout's per-joint state now comes from `reference_authority`, not from the registry (`crates/davout/src/lib.rs:646-657`).

**Live intent at baseline.** Derived from the production call sites listed in §4, the crate has four jobs:
- (a) Define the `JointHomingState` vocabulary. Davout re-exports it (`davout/src/lib.rs:291`), and it maps to the Chappe proto enum.
- (b) Implement the Joint/Robot Ready aggregation and Enable-target selection for the commissioning scope (`select_enable_targets`).
- (c) Hold the per-joint **OutOfLimits** health flag that Davout's feedback consumer latches.
- (d) Parse the calibration-history resource at Supervisor construction. This fails closed on corrupt or unreadable history (ADR 0022).

The rest of the crate is vestigial or dormant. That covers the lifecycle state machine, the scalar verifier and history writer, the sensor module, and the limb aggregation (§9).

**Conflicting statements of intent.**
- The crate is still described as the reference authority in several places: `Cargo.toml:4` ("Homing state machine … calibration registry"); `crates/AGENTS.md:19,38` ("Homing state machine, zero registry"; "Homing state → marengo-homing + davout HomingRegistry"); `crates/codemap.md:19` ("encoder zero verification, calibration record persistence"); `CONTEXT.md:25` ("Set Zero: … → calibration record → Joint Ready"); `docs/safety.md:22,24` ("Sensor health first"; "Calibration audit. Host registry … records who/when/how zero was established").
- These contradict ADR 0023/0036 and the code, where nothing in production writes the calibration record (§4).
- The crate's own docs (`lib.rs:1-9`, `codemap.md:7-11`) are up to date with ADR 0022/0023 but predate ADR 0036.

## 3. Owns / Must not

| Owns (live) | Evidence |
|---|---|
| `JointHomingState` enum and its proto mapping | `lib.rs:37-43`; `commissioning.rs:12-19` |
| Ready aggregation and Enable-target resolution policy (scope → in-scope Verified; no scope → full-master Robot Ready) | `commissioning.rs:50-146`; `docs/rust-patterns.md:185` |
| OutOfLimits flag storage | `registry.rs:86-98` |
| History-resource load with typed errors; missing file = empty | `registry.rs:57-77,237-254`; ADR 0022 |

| Must not | Evidence | Status |
|---|---|---|
| Grant current reference, Ready or output permission | ADR 0023:17-24; `lib.rs:7-9`; `registry.rs:24-27` | Respected. Davout ignores `registry.joint_state` (`davout/src/lib.rs:646-657`), and `crates/davout/tests/current_reference.rs:146-160` pins that. |
| Read environment variables or select resources | ADR 0022 "explicit resource binding"; `registry.rs:39-40` | Respected. The env var is read in Davout (`davout/src/lib.rs:432`) and marengo-config. |
| Hardware I/O (GPIO) | `lib.rs:3` "GPIO/hardware I/O lives in bins via SensorProvider" | Respected. No GPIO adapter exists anywhere (`grep "impl SensorProvider"`: only `sensor.rs:66,87`). |
| Rewrite history on load | ADR 0022 | Respected (`registry.rs:57-77`). Tested in `tests/reference_history.rs:85,96,120`. |

No layer violation was found. Note that `JointFacetInput.drive_active` (`commissioning.rs:47`) is filled in by Davout (`davout/src/lib.rs:1426`) but no code in this crate reads it.

## 4. Interface

| Group | Public surface | Production consumers | Test-only / none |
|---|---|---|---|
| State vocabulary | `JointHomingState` (`lib.rs:37`); `to_proto_homing_state` (`commissioning.rs:12`) | `davout/src/lib.rs:291,646-680,3222,3237` (wire facets, `joint_commissioning_wire`) | — |
| Wire decode | `from_proto_homing_state`, `wire_homing_is_unspecified` (`commissioning.rs:22,87`) | none (Rust). Consul decodes in TS. | own tests only |
| Ready / Enable policy | `JointFacetInput`, `select_enable_targets` (`commissioning.rs:40,107`) | `davout/src/lib.rs:160,1396-1452` (`commissioning_facets`, `resolve_enable_targets`) | — |
| Ready helpers | `robot_ready`, `is_enable_eligible` (used internally by `select_enable_targets`), `limb_ready` (`commissioning.rs:68-97`) | `limb_ready`: none in Rust. Limb aggregation is reimplemented in Consul (`consul/src/lib/commissioning.ts`, `consul/src/components/dashboard/hardware/commissioning-aggregation.tsx`). | own tests |
| Registry: construction | `HomingRegistry::with_record_path` (`registry.rs:57`), `RegistryError` | `davout/src/lib.rs:571-581`; `davout/src/reference_journal_*_tests.rs` | `HomingRegistry::new` (`registry.rs:41`): own tests only (`tests/reference_history.rs:44,90,99,213,273`) |
| Registry: OutOfLimits | `is_out_of_limits`, `mark_out_of_limits` (`registry.rs:86,90`) | `davout/src/lib.rs:670-672`; `davout/src/feedback_consumer.rs:854` | `clear_out_of_limits` (`registry.rs:96`): called only from `record_verification` (`registry.rs:190`), which itself has no production caller |
| Registry: history read | `calibration()` (`registry.rs:123`), `CalibrationRecord`, `JointCalibration`, `find_joint` | none in production. Accessor `davout::Supervisor::homing_registry` (`davout/src/lib.rs:642`) is used only by Davout tests (`tests/reference_history.rs:62,98,297`, `tests/reference_boundary_public.rs:89`, `tests/current_reference.rs:151`). metrics/pub-usage.md:45 lists `davout::homing_registry` as test-only. | — |
| Registry: legacy local lifecycle | `joint_state`, `all_verified`, `any_faulted`, `mark_fault`, `require_ready`, `zero_tolerance_rad`, `configured_joints`, `sensor_health`, `set_sensor_health`, `check_sensor_health`, `record_verification`, `persist` (`registry.rs:79-235`) | **none** (grep across crates/bins/tools; metrics/pub-usage.md:22-23,53-55) | own tests; `joint_state` is used by one Davout test to prove it is *not* authority |
| Scalar verifier | `verify_manual_reference`, `VerifyError`, `VerifyOutcome`, `verify_error_is_out_of_limits` (`verify.rs:7-129`; `commissioning.rs:34`) | **none** | own tests and `tests/manual_reference_validation.rs`, `tests/history_*_atomicity.rs` |
| Sensors | `SensorProvider` trait, `ThreeHallInputs`, `MemorySensorProvider`, `SensorSnapshot`, `SensorPattern`, `SensorHealth`, `classify_sensor_pattern` (`sensor.rs`) | **none** | own tests |
| Config helpers | `effective_homing_for_robot`, `method_requires_sensors` (`lib.rs:46-59`) | **none** | `effective_homing_for_robot` own test |

Depth assessment:
- `select_enable_targets` is a reasonably deep function: one call hides the scope/Robot-Ready/eligibility policy.
- `HomingRegistry` is wide and shallow: 17 public methods, of which 3 are used in production. Its live job could be a `HashMap<String,bool>` for OutOfLimits plus a one-shot `load_calibration` check. Davout re-wraps registry state anyway (`davout/src/lib.rs:646-680`).
- `SensorProvider` is a hypothetical seam. It has 2 adapters, neither real: `ThreeHallInputs` always returns `Err` (`sensor.rs:66-74`), and `MemorySensorProvider` is test/sim only (`sensor.rs:76-92`). No GPIO adapter exists.
- Feature flags: none.
- Dependencies: `marengo-pi` declares `marengo-homing` (`bins/marengo-pi/Cargo.toml:30`) but never uses it (grep; metrics/unused-deps.md:31-32).

## 5. Invariants owned

| Invariant | Enforcing code | Tests that fail if broken | File line coverage |
|---|---|---|---|
| A fresh registry starts every configured joint `Unhomed`; history never grants (ADR 0022) | `registry.rs:57-77` | `tests/reference_history.rs:64` (exact row), `:69` (mismatches), `:143`, `:170` (reconstruction refusal) | registry.rs 79.1% |
| Missing history = empty; any other IO or parse error → typed `RegistryError`; bytes never rewritten on load | `registry.rs:237-254` | `tests/reference_history.rs:85,96,120` | 79.1% |
| A failed history write leaves the in-memory record, flags and state unchanged; a retry can succeed | `registry.rs:185-191` (stage → `persist_record` → publish) | `tests/history_commit_atomicity.rs:92`, `tests/history_replacement_atomicity.rs:162` | 79.1%. The path has no production caller (§9). |
| Scalar verifier rejects nonfinite input, reversed bounds, negative tolerance, joint mismatch and non-manual methods before any mutation | `verify.rs:55-88` | `tests/manual_reference_validation.rs:59-155` (22-case matrix per `codemap.md:23`); `verify.rs:250,280` | 96.1%. No production caller. |
| Scoped Enable targets only in-scope joints that are Verified, Online, not faulted and not OutOfLimits. Unscoped Enable requires full-master Robot Ready (built joints only). An empty target set is an error. | `commissioning.rs:95-146` | `commissioning.rs:380,392,404,414` | 93.7% |
| Unbuilt (not online, not mapped) joints never block Robot/Limb Ready; scope never fabricates Robot Ready | `commissioning.rs:52-84` | `commissioning.rs:278,310,334,367` | 93.7% |
| `to_proto_homing_state` never emits `Unspecified` | `commissioning.rs:12-19` | `commissioning.rs:176` | 93.7% |
| OutOfLimits is set only for configured joints | `registry.rs:90-94` | indirect: `davout/src/lib.rs:3382` (`measured_position_fault_marks_out_of_limits`) | 79.1% |
| Sensor truth table: overlap is an error unless allowed | `sensor.rs:115-139` | `lib.rs:101,111` | sensor.rs 67.5%. Dormant: no production caller. |

## 6. Inputs / outputs

| Kind | Item |
|---|---|
| Files read | Calibration history YAML at the path supplied by Davout: `homing.yaml` `calibration_record_path` (`config/homing.yaml:3` = `/opt/marengo/var/calibration/zero_registry.yaml`), or the `MARENGO_CALIBRATION_RECORD` override (resolved in `davout/src/lib.rs:432,465,573`) |
| Files written | The same file, via `persist_record` (`registry.rs:198-213`, in-place `fs::write`). There is no production caller, so the file is **not written at baseline**. |
| Config read (via `marengo-config` types) | `MotorEntry` (verifier), `EffectiveHomingJoint`, `HomingMethod`, `HomingSensors` (`sensor.rs:56-63`), `HomingConfigFile` (`lib.rs:46`) |
| Proto | `armee_proto::JointHomingState` ↔ `JointState.homing_state` (`commissioning.rs:3,12-31`). Published by Davout/marengo-pi on Chappe `RobotState`. |
| Env vars, CAN, HTTP | none (by design, `lib.rs:3-6`) |
| Time | `chrono::Utc::now()` timestamps history rows (`registry.rs:181`). This is the crate's only use of `chrono`. |

## 7. Prior review reconciliation

| Prior ID | Prior claim | Current status | Evidence |
|---|---|---|---|
| CS05 (P1, ledger "partial") | A same-name persisted row marks a joint Verified (`registry.rs:54-57` @4bc77ba) | **Fixed in this crate. Remaining work superseded in Davout** (ADR 0023/0036) | `8c3f62e` + `tests/reference_history.rs:64,69`. Davout state comes from `reference_authority` (`davout/src/lib.rs:646-657`). The ledger keeps CS05 partial for owner/receipt work outside this crate. |
| CS06 (P1, partial) | Set Zero verifies against cached feedback | **Superseded** by ADR 0036's physical transaction (type-2 ack + type-17 readback) in Davout. This crate's verifier was hardened (`7951f10`, `verify.rs:55-88`) but no longer has a production caller. | grep: `verify_manual_reference` has no non-test callers |
| batch08 (CS05 sub-problem) | History published before the write succeeded | **Fixed** | `d541bd5`; `registry.rs:185-191`; `tests/history_commit_atomicity.rs:92`, `tests/history_replacement_atomicity.rs:162` |
| M03 (open) | Hall method / polarity honesty | **Open.** Sensor module present with no GPIO adapter. Davout refuses non-manual methods at reference request (`davout/src/lib.rs:1108`). `ThreeHallInputs::from_config` drops per-input polarity (§10). | `sensor.rs:56-74` |
| ADR 0022 consequence (corrupt history blocks a fresh motor-repl before `disable`) | — | **Open, and now pure cost**: history has no reader (§9) | `davout/src/lib.rs:574-580` maps the load error to `DavoutError::Homing` at construction |

## 8. Drift

| Source | Claim | Reality |
|---|---|---|
| `Cargo.toml:4`, `crates/AGENTS.md:19` | "Homing state machine … calibration registry" | No production code drives the state machine. Davout derives state from `reference_authority`. |
| `crates/AGENTS.md:38` | "Homing state → marengo-homing + davout HomingRegistry" | Homing state is `davout::Supervisor::joint_homing_state` (`davout/src/lib.rs:646`). The registry holds only the OutOfLimits flag and history. |
| `crates/codemap.md:19` | "encoder zero verification, calibration record persistence" | Neither path is called in production. ADR 0036 journals to SQLite in Davout. |
| `codemap.md:11,17` | "Qualified reference/recovery remains follow-up work"; "installed adapter has no qualified reference capability and refuses before arming" | Out of date since ADR 0036 (`e1a1771`). Physical owners now acquire. |
| `codemap.md:27` | Consumed by `bins/motor-repl`, `bins/marengo-pi` | motor-repl does not depend on it. marengo-pi declares it but never uses it (metrics/unused-deps.md:31-32). Only Davout consumes it. |
| `CONTEXT.md:25` | Set Zero → "calibration record → Joint Ready" | ADR 0036: journal row (SQLite) → process-local grant. The YAML calibration record is not written. |
| `CONTEXT.md:19` | Joint Ready = "firmware Set Zero accepted + homing Verified" | Matches ADR 0036 in spirit, but "Verified" is Davout's grant, not this crate's state. |
| `docs/safety.md:22,24` | Startup sensor-health check; host registry records who/when/how zero was established | No sensor-health call exists in production. The registry is never written. The ADR 0036 journal holds that audit (`docs/homing.md:120-125`). |
| `docs/homing.md:40` | "`Ready` requires all configured joints Verified and no latched homing/sensor faults" | Sensor faults are never set (`set_sensor_health` has no callers). Ready uses Davout authority. |
| `docs/homing.md:186-197` "Calibration record" | Describes the current writer and the scalar validator as live behavior | The writer and validator are only reachable from tests. |
| `scripts/homing-preflight.sh:41-45` (run by `pi_health`/`install-pi.sh`) | Reports `calibration record: MISSING` | That file is never created on a fresh install, so the report misleads operators. |
| `crates/AGENTS.md:3` | Each crate's `//!` declares its allowed deps | `lib.rs:1-9` has no allowed-deps list. |

## 9. Prune candidates

| # | Candidate | Evidence class | Confidence | Deleting touches |
|---|---|---|---|---|
| P1 | `marengo-homing` dependency in `bins/marengo-pi/Cargo.toml:30` | Zero references | high | 1 Cargo.toml line (metrics/unused-deps.md:31-32) |
| P2 | Scalar verifier + history writer: `verify.rs` (all), `registry.rs:163-213` (`record_verification`, `persist`, `persist_record`), `CalibrationRecord::upsert`, `VerifyError`, `VerifyOutcome`, `verify_error_is_out_of_limits`, the `chrono` dependency | **Superseded by ADR 0036**: the physical transaction plus the SQLite journal replace it, and grants are process-local. Zero production references. | med-high. Needs an owner decision to retire the ADR 0006 "Now" row and ADR 0022's "legacy scalar check" language. | `verify.rs`; `registry.rs`; `commissioning.rs:34-36,261-275`; tests `manual_reference_validation.rs`, `history_commit_atomicity.rs`, `history_replacement_atomicity.rs`, `reference_history.rs:170-231` (≈880 test lines); `docs/homing.md:186-197`; `codemap.md:10-11,22-23` |
| P3 | Legacy local lifecycle in `HomingRegistry`: the `joint_states` map, `joint_state`, `set_state`, `all_verified`, `any_faulted`, `mark_fault` (which also discards its `message`, `registry.rs:118-121`), `require_ready`, `zero_tolerance_rad`, `configured_joints` | Zero production references. Davout's state comes from `reference_authority` (ADR 0023). | high once P2 lands. `joint_state` is used by `davout/tests/current_reference.rs:151` only to assert non-authority, so drop that assertion. | `registry.rs:79-121,215-235`; `JointHomingState::Homing` becomes unused in Rust but stays as a proto variant |
| P4 | Sensor module (`sensor.rs`), `check_sensor_health`/`sensor_health`/`set_sensor_health` (`registry.rs:135-157`), `method_requires_sensors` (`lib.rs:57`) | Scaffold with no consumer. `ThreeHallInputs` is a fake adapter that always errors. ADR 0006 "Next" row and M03 keep Hall homing as *planned* intent. | low-med. Prune only if the owner defers Hall homing. Otherwise keep it, rewritten when a real GPIO adapter exists. Not safety code today because it never runs. | `sensor.rs`; `lib.rs:24-27,91-125`; config `HomingSensors` stays (validated in marengo-config) |
| P5 | `limb_ready` (`commissioning.rs:68-74`) | Duplicate implementation of the Consul TS limb aggregation, which is the actual consumer (`consul/src/lib/commissioning.ts`) | med | `commissioning.rs:68-74,334-364` |
| P6 | `from_proto_homing_state`, `wire_homing_is_unspecified` (`commissioning.rs:22-31,87-92`) | Zero references (decode lives in Consul TS) | med | `commissioning.rs` + tests at `:199-258` |
| P7 | `effective_homing_for_robot` (`lib.rs:46-54`), `HomingRegistry::new` (`registry.rs:41-52`) | Zero production references | med (`new` is in the ADR 0022 text, so update the ADR) | lib/test lines |
| P8 | `JointFacetInput.drive_active` (`commissioning.rs:47`) | Write-only field: set at `davout/src/lib.rs:1426`, read by no policy | low-med (may be intended as a diagnostic) | struct + Davout constructor + test helper |
| P9 | Calibration-history *read* at construction (`registry.rs:57-77,237-254`) | **Needs a decision.** ADR 0022 keeps history "for inspection", yet no production code reads `calibration()`. Its only runtime effect is to fail Supervisor construction on corrupt or unreadable history, which ADR 0022:137-145 flags as a stop-path hazard. | low (ADR-backed). If pruned, keep `homing.yaml` `calibration_record_path` or replace it: it is still load-bearing for the journal location (`marengo_config::resolve_reference_journal_path`, `marengo-config/src/lib.rs:239-257`) and the reference policy binding (`davout/src/reference.rs:318`). | `registry.rs`; Davout constructor `davout/src/lib.rs:571-581`; Davout tests `reference_history.rs`, `reference_boundary_public.rs`; ADR 0022 |

What would remain after P2–P8: `JointHomingState` + proto map, `JointFacetInput` + `select_enable_targets`/`robot_ready`/`is_enable_eligible`, and an OutOfLimits flag set. That is small enough to fold into Davout (≈150 lines). Doing so would remove the crate and its `marengo-config` dependency edge. [INFERENCE: a design option for Phase B, not a finding.]

## 10. Phase-B leads

| # | Lead | Where | Why suspicious |
|---|---|---|---|
| L1 | **OutOfLimits latch is never cleared in production** | `registry.rs:96` (only caller `registry.rs:190`, itself unreachable); set at `davout/src/feedback_consumer.rs:854` | Once measured q exceeds hard + slack, `is_enable_eligible` and `robot_ready` stay false (`commissioning.rs:62-64,95-97`) until the process restarts. This holds even after a fault reset and a return inside limits. `CONTEXT.md:18` treats OutOfLimits as a health facet, which implies it can recover. It is fail-closed, but the recovery story is undefined. Untested either way. |
| L2 | Unused history now only adds a startup failure mode | `davout/src/lib.rs:574-580`; ADR 0022:137-145 | A corrupt or directory-typed `zero_registry.yaml` blocks a fresh `motor-repl` (including `disable`) and `marengo-pi` startup, and nothing reads the data. |
| L3 | `ThreeHallInputs::from_config` takes `active_high` from `home` only | `sensor.rs:56-63` | min/max polarity is ignored. If Hall homing is ever wired, a mixed-polarity board misreads limits. This is dormant safety code: a gap, not a prune target. |
| L4 | With overlap allowed, any multi-active pattern (including all three active) classifies as `MidTravel`, so boot health reports `Healthy` | `sensor.rs:132,141-150` | Contradicts the doc comment "stuck-all-active faults" (`sensor.rs:141`) when `allow_sensor_overlap` is true. Dormant. |
| L5 | `MemorySensorProvider` reports unknown pins as inactive (`unwrap_or(false)`) | `sensor.rs:89` | A typo in a GPIO number passes boot health. Test/sim only today. |
| L6 | Joint subset silently narrows Robot Ready | `commissioning.rs:52-54,78-84` with `davout/src/lib.rs:1404-1432` | With `MARENGO_JOINT_SUBSET` set, wired joints outside the subset count as "unbuilt" (`motor_mapped=false`, no feedback), so unscoped Enable passes Robot Ready even though those joints are Unhomed. `CONTEXT.md:21` calls Robot Ready the "honest full-robot status". Low safety impact, because those joints are not loaded. |
| L7 | `persist_record` writes in place without temp + rename or fsync | `registry.rs:198-213` | A crash can truncate history, and the next startup then fails (L2). Moot if P2/P9 land. |
| L8 | `mark_fault` discards `message` | `registry.rs:118-121` | Error swallowing. Moot under P3. |
| L9 | Ready aggregation ignores `drive_active` | `commissioning.rs:62-64` | Probably intended (Ready is about reference, not drive). Confirm with the owner before pruning P8. |
