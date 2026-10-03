# Phase B — WP-G: gravity preflight and calibration fit

Branch `audit/wp-g`, verified against main snapshot `edbaebaf` plus the WP-G working changes. Fixes are committed locally after report completion; no push or merge.

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-berthier-23 | **CONFIRMED** | `berthier::loop::tests::active_gravity_refuses_when_a_coupled_joint_lacks_feedback`: a scoped Active joint with a silent modeled peer returns `MissingFeedback` before coupled gravity evaluation; the regression also excludes non-neutral MIT torque from the fault-cleanup trace. Red behavior was the old `read_positions` missing-feedback substitution (`unwrap_or(0.0)`). | Berthier requires fresh feedback for every modeled joint before evaluating coupled gravity; the bounded post-enable bootstrap remains neutral and does not evaluate gravity. Test fixtures now provide actual feedback for every modeled joint when exercising later gravity/position output. |
| L-armee-dynamics-06 | **ALREADY-FIXED** (WP-E) | WP-E report records red→green coverage: singular/non-finite/iteration-limit fits refuse as `NotConverged`; posterior uncertainty is recomputed at the final fit and non-finite/non-positive covariance does not report zero sigma. | No duplicate changes. |
| L-marengo-pi-03 | **CONFIRMED** | `stop_path_tests::both_enable_owners_refuse_gravity_preflight_without_feedback` and `motion_owner_chappe_tests::testing_position_auto_enable_requires_gravity_preflight_feedback` refuse before any Enable CAN frame. The forced stdin test confirms `force` does not bypass. | One fail-closed saturation preflight is called by stdin Enable, Chappe `enable(true)`, and Testing Position auto-enable. A Testing Position batch already Active does not re-run an enable-only preflight. |
| L-marengo-pi-04 | **CONFIRMED** | The preflight no longer uses `filter_map`/zero-on-error: it requires every modeled joint's motor configuration, live limit policy and measured position; samples coupled model torque over all live command envelopes; refuses missing data, invalid bounds, model errors, non-finite torque, unbounded work and motor torque-limit exceedance. Enable path tests cover missing feedback; WP-E model tests cover unevaluable/non-finite dynamics. | Removed silent joint omission and uses the live envelope, coupled five-point-per-joint sweep and fail-closed model evaluation. No physical tuning values changed. |
| L-motor-repl-03 | **ALREADY-FIXED** (WP-O ownership) | Integrator confirmed WP-O's `parse_gravity_pose` rejects both short and excess vectors and MCP callers supply all five joint angles. This lead and files are owned by WP-O; WP-G did not edit them. | No duplicate change. |
| L-armee-dynamics-08 | **CONFIRMED** | `gravity_calibration::invalid_calibration_windows_are_refused` rejects a non-finite bound and a reversed window instead of admitting samples. | Validate each configured window as finite and `lower_rad <= upper_rad` before accepting fit samples. |
| L-marengo-log-cli-05 | **CONFIRMED** | `gravity_fit::tests::trace_parser_groups_interleaved_ticks_and_rejects_time_regression` rejects decreasing trace timestamps. Settled and measurement-window elapsed-time calculations use `checked_sub`; the final workspace suite passed. | Validate trace timestamps in encounter order and reject regressions; retain checked subtraction at each elapsed-time boundary. |
| L-marengo-log-cli-08 | **CONFIRMED** | `gravity_fit_cli` tests assert generated proposal files are under `--out-dir`, not in the evidence session; the CLI regression also checks absent explicit paths refuse without creating a proposal. | Write proposed URDF and patch artifacts only under the required output directory; evidence directories are read-only. |
| L-armee-dynamics-09 | **NEEDS-DECISION** | Confirmed test-oracle gap: current gravity calibration truth and fitter share the gravity model's link-transform implementation and fixtures load the mutable repository URDF/config. Passing tests therefore cannot independently detect a shared kinematics error. | No oracle was fabricated in this package. See decision options below. |
| L-armee-dynamics-11 | **CONFIRMED** (gap closed) | `gravity_calibration::refuses_unknown_joints_non_finite_samples_and_implausible_mass` now exercises unknown fit joints, non-finite measurements and an implausible fitted mass; the package tests passed in the workspace run. | Added regression coverage at the public calibration API. |
| L-marengo-log-cli-07 | **CONFIRMED** | `gravity_fit_cli::gravity_fit_requires_explicit_output_and_local_urdf_paths` refuses omitted `--out-dir` / `--repo-urdf`; `run` reads the explicit repository URDF as a hard error rather than silently skipping comparison. MCP command test checks both explicit paths. | Make both paths required and propagate repository-URDF read errors. Updated MCP invocation and operator docs. |
| L-armee-dynamics-07 | **CONFIRMED** | `gravity_calibration::friction_cancellation_refuses_mismatched_vectors` expects a length error for mismatched approach vectors; caller propagation is exercised by gravity-fit tests. | `cancel_friction` returns a `Result` and refuses unequal vector lengths instead of silently truncating via `zip`. |
| L-marengo-log-cli-06 | **CONFIRMED** | `gravity_fit_cli::refuses_fused_sessions_with_different_calibration_windows` asserts differing per-session effective windows refuse fusion without creating output. | Fusion now independently derives each session's windows from its own `control.yaml` and `motors.yaml`, and requires equality with the first session in addition to identical Pi URDF bytes and joint lists. |
| L-marengo-log-cli-10 | **CONFIRMED** | `gravity_fit::tests::trace_parser_groups_interleaved_ticks_and_rejects_time_regression` groups interleaved joint rows under one tick while also covering timestamp regression. | Preserve first-seen tick order while indexing ticks by ID; do not split a tick merely because another joint row intervened. |
| L-marengo-log-cli-09 | **NEEDS-DECISION** | Confirmed limitation: the line-oriented patcher does not support alternate valid XML layouts such as one-line `<inertial>` blocks or multi-line `<link>` openers. It returns a patch error if required mass/origin lines were not rewritten; accepted proposals are round-trip verified before output. | No unsafe best-effort XML rewrite was added. See decision options below. |

## NEEDS-DECISION

### L-armee-dynamics-09 — Independent calibration oracle

- **Option A:** Add immutable, hand-derived or independently generated known-torque fixtures for a small gravity chain, using an oracle that does not call `UrdfGravityModel::link_transform`. This provides the strongest regression signal but requires selecting, reviewing and maintaining the external derivation/tool and reference values.
- **Option B:** Keep current same-model calibration tests and narrow their claim to parameter-fit/calibration behavior; do not claim they verify the gravity model's forward-kinematics physics independently.
- **Recommendation:** A, before relying on calibration tests as a physical correctness oracle. Until that independent reference is reviewed, retain the tests but document their limited claim; do not block this package's fit safety fixes on invented expected values.

### L-marengo-log-cli-09 — URDF layout support

- **Option A:** Add a well-supported XML parser and patch the DOM while preserving unrelated URDF content; this supports normal XML layout variation but adds a dependency and needs preservation/round-trip tests.
- **Option B:** Keep the narrow line-oriented format and fail closed on unsupported layouts; maintain explicit patch/round-trip refusal rather than risk mutating the wrong element.
- **Recommendation:** B for this bench calibration workflow until there is a demonstrated requirement for arbitrary formatting. The current refusal is safe and no partial output is accepted; expand format support as a deliberate parser/dependency decision.

## Cross-package edits

- `crates/berthier/src/loop.rs` and Berthier test fixtures: fail before coupled gravity on missing modeled-joint feedback; bootstrap remains neutral. Test fixtures were updated to provide all-joint feedback where required.
- `bins/marengo-pi`: shared gravity preflight wiring and fail-closed checks plus enable-path regressions; `bins/marengo-pi/Cargo.toml` includes its existing dynamics dependency needed by the preflight.
- `crates/armee-dynamics`: calibration-window validation, `cancel_friction` length error and tests; gravity-fit callers propagate the result.
- `bins/marengo-log-cli`: safe timestamps, session compatibility checks, explicit paths, evidence-safe outputs, corresponding CLI regressions and codemap updates.
- MCP `gravity-calibrate.ts` / tests / README: explicit fit output and repository URDF paths; preserve optional requested fit parameters in the command. WP-O alone owns `motor-repl gravity-preview` and its other MCP callers.
- Documentation updated: `docs/safety.md`, commissioning limb playbook, MCP README, and CLI codemaps.

## Gates

- `cargo fmt --all -- --check` — **PASS**.
- `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings` — **PASS**.
- `cargo clippy -p marengo-pi --all-targets -- -D warnings` — **FAIL**, due to the recorded macOS-only `marengo-host-metrics/src/sample_state.rs` dead-code warnings (`counters`, `rates`, `rx_bytes`/`tx_bytes`, and `SampleState` fields). The workspace Clippy command excluding `marengo-pi` and `marengo-host-metrics` passed.
- `cargo test --workspace` — **PASS**, 1,158 passed, 0 failed, 1 ignored (122 suites).
- `cd tools/marengo-pi-mcp && npm test` — **PASS**, 200 tests, 43 suites. Initial invocation lacked installed `tsx`; after `npm ci`, the test exposed the omitted optional `--fit` argument, which was restored and the suite passed.
