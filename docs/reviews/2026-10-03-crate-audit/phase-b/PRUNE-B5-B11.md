# Phase B prune report: B5 + B11 (`audit/prune-b5b11`)

Worktree `prune-b5b11`. Resumed from a killed session with 15 uncommitted files;
reviewed that partial work, kept what was correct, finished the rest.

## B5 — Berthier dead helpers and simulation factories

| Row | Verdict | Evidence / Fix |
|---|---|---|
| P-davout-11 (reference/commit API → pub(crate)+feature) | KEPT | Demotion is infeasible: `begin/advance/cancel_reference`, `begin/advance/cancel_reference_commit`, `reference_commit_snapshot`, `reference_generation` are called by integration tests (`davout/tests/*.rs`, `berthier/tests/reference_transaction_owner.rs`, `marengo-pi` shutdown tests) that link the lib **without** `cfg(test)`, so feature-gating breaks default `cargo test`; `pub(crate)` is impossible cross-crate. They are the documented cross-crate reference-test seam. |
| P-davout-12 (`inspect_reference_journal`, `ReferenceHistoryRecord`, `drain_raw` → feature) | KEPT | Same reason: `inspect_reference_journal` is used by `davout/tests/physical_reference.rs`, `drain_raw` by `receive_bounds.rs`/`reference_transaction.rs`, `reference_generation` by limit-restore tests. All integration tests; gating breaks the default test build. |
| P-davout-14 (unpaused HistoryOnly factory) | DELETED | `Supervisor::from_simulation_with_reference_journal` gated to `#[cfg(any(test, feature = "reference-journal-test-support"))]` in `davout/src/simulation.rs`. Verified all callers are `cfg(test)` unit modules; no integration-test/bin callers. Paused variants untouched. |
| P-berthier-02 (ungated berthier journal factories) | DELETED | Both `ControlLoop::from_simulation_with_reference_journal` / `…_current_reference_journal` removed from `berthier/src/loop.rs` (prior session). Zero callers confirmed; paused feature-gated variants remain and are used. |
| P-berthier-03 (`clear_torque_cmds`) | DELETED | Removed with its only test caller rewritten to mode-leave clearing (`set_torque_cmd_enters_torque_only_and_latches` now asserts GravityComp entry clears the latch — the production path). |
| P-berthier-04 (disabled ascent-pull stubs) | DELETED | `outbound_low_angle_stuck*`, `approach_stuck_mit_pull*` + pinning test removed from `position_setpoint.rs`/`loop.rs` (prior session). No production refs. |
| P-berthier-05 (`trajectory_friction_torque`) | DELETED | Removed + its two tests (prior session). No production refs. |
| P-berthier-06 (`trajectory_damping_torque`) | DELETED | Removed + its tests; `damping_spike_cap` test now uses the inline `kd*(dq_des-dq)` reference (prior session). |
| P-berthier-08 (`PositionHold::targets_raw`) | DELETED from production surface | Removed the `pub` accessor; tests now use a `#[cfg(test)] pub(crate) raw_targets_for_test` accessor (`loop.rs`) or read `setpoints_raw` directly in-crate (`hold_tracking.rs`, `numeric_contract.rs`). Codemap updated to point at the per-tick diag `target_raw`. |
| P-berthier-09 (`clear_all_overrides`, `clear_torque_cmd`, `apply_gain_overrides`) | DELETED | All three `ControlLoop` methods removed with all callers. Follow-on orphans removed too: `GainRuntime::apply_batch` + its test, `GainRuntime::clear_all`, `TorqueCmdLatch::clear` (+ trimmed its test). `gain_validity.rs` batch test rewritten to per-joint refusal with identical safety property (invalid gain leaves installed state intact). Per-joint `apply_gain_override`/`clear_gain_override` kept (live `marengo-pi` callers). |
| P-berthier-10 (`ADVANCE_MAX_LEAD_DEFAULT` split + `unwrap_or` fallbacks) | NEEDS-DECISION | Deletion is gated on confirming config validation guarantees `control.yaml` entries for every robot joint (open L-berthier-09, owned by WP-D). See below. |
| P-berthier-11 (unused `_q`, `_target` of `position_hold_mit_kd`) | DELETED | Params removed; production caller (`position_hold.rs`) and unit test updated. |
| P-berthier-12 (arm+seed dup / `wire_gains_now` vs `resolve_all`) | NEEDS-DECISION | Consolidation, not deletion, and it touches the fuse paths WP-D owns. See below. |

## B11 — Test hygiene and fixtures

| Row | Verdict | Evidence / Fix |
|---|---|---|
| P-davout-13 (diagnostics-rewrite fixture ×~11) | DELETED | New shared helper `fixture_tree_without_diagnostics` (+ `disable_copied_diagnostics` for the conditional site) in `crates/berthier/tests/support/mod.rs` — the file all three crates already share via `#[path]`. Migrated 19 sites across `davout/src` (3), `davout/tests` (7), `berthier/src` (2), `berthier/tests` (6), `marengo-pi/src` (3). Excepted: `davout/src/reference_model.rs` (uses `TestDirectory` with extra copied files, not `FixtureTree`) and `davout/tests/physical_reference.rs` (sets the struct field directly, not a file rewrite). |
| P-armee-kinematics-01 (`arm_4dof_mjcf` fixtures) | DELETED | Both MJCF helpers removed (prior session); zero refs confirmed. |
| P-armee-kinematics-02 (`joint_entry_count`) | DELETED | Removed + its assertion (prior session); `actuated_joint_count` covers it. |
| P-armee-kinematics-03 (unused `_q` param) | ALREADY-FIXED | `340f502f` (in main) renamed `_q`→`q` and uses it in the envelope-collapse branch. Nothing to do. |
| P-armee-kinematics-04 (`Default for LimitMarginConfig`) | ALREADY-FIXED | Removed by `340f502f`; no `Default` impl exists and davout builds margins from config. Nothing to do. |
| P-armee-kinematics-05 (ignored humanoid test + dev-deps) | DELETED | Test + `serde_yaml` dev-dep removed (prior session; `Cargo.lock` updated). |
| P-armee-kinematics-06 (`fixtures` in production API) | DELETED | Module gated to `#[cfg(any(test, feature = "test-support"))]` (prior session); `PathBuf` import gated likewise (this session — it warned otherwise). Consumers: `armee-dynamics` dev-dep and `sim-harness` dep carry the feature. |
| P-armee-dynamics-02 (`as_slice`/`Index`) | PARTIAL | Card evidence was half-wrong: `[T]` has no `as_slice`, so `as_slice()` is **not** provided by `Deref` — and it has a live production caller (`berthier/src/loop.rs:1405`). Restored `as_slice`; deleted the genuinely redundant `Index` impl instead (indexing works identically via `Deref<Target=[f64]>`). |
| P-armee-dynamics-04 (archived-model unit tests) | DELETED | Four weak-assertion tests removed (prior session); analytic oracles + saturation tests remain. |
| P-marengo-store-09 (duplicated harness helpers) | PARTIAL | The 4 identical `bounded` copies → `crates/marengo-store/tests/common/mod.rs::bounded`, migrated in all 4 files (imports pruned). The 13 `observe` helpers are **kept per-file**: each observes a different `View`/`State` type for its own test target; unifying them would couple unrelated tests. |
| P-armee-proto-01 (prost round-trip tests) | DELETED | Six round-trip tests removed (prior session); enum-value test kept. Unused imports pruned + `cargo fmt` (this session). |
| P-sim-harness-01 (pub API → private/cfg(test)) | DELETED | Remaining 4 fns + their imports gated `#[cfg(test)]` (this session — top-level private fns warn `dead_code` otherwise). |
| P-sim-harness-02 (arm_4dof MJCF pair + tests) | DELETED | MJCF↔URDF tests removed (prior session); `assets/mjcf/arm_4dof.xml` + `arm_4dof_right.xml` deleted via `git rm` (this session; zero refs outside the deleted tests). URDF seeds stay (live `armee-kinematics`/`armee-dynamics` users). |

## NEEDS-DECISION

1. **P-berthier-10 (missing-config fallbacks).** Options: (a) delete the `unwrap_or` fallbacks + unify `ADVANCE_MAX_LEAD_DEFAULT` once WP-D confirms validation guarantees entries; (b) keep as fail-safe defaults. Recommendation: (a), done together with L-berthier-09 in WP-D — deleting fail-open stiffness (slew 0.15 vs 0.25, max_lead 0.15 vs 0.10, `f64::MAX` clamp limits) without the coverage guarantee risks unconfigured-joint behavior.
2. **P-berthier-12 (arm+seed / wire-gains duplication).** Options: (a) consolidate in WP-D's fuse work, which already touches `gain_runtime.rs`/`loop.rs` mode-enter paths; (b) consolidate here. Recommendation: (a) — single editor per region, and the assignment forbids restructuring the tick here.

## Cross-package edits

- `bins/marengo-pi/src/reference_{shutdown,busy_overlay,journal_shutdown}_tests.rs`: adopted the shared P-davout-13 helper (test-only, mechanical). Noted: `reference_journal_shutdown_tests.rs` keeps its own local tempfile-based `FixtureTree` copy — a further dedupe, out of scope for this batch.
- `crates/davout/tests/*.rs` (7 files): adopted the shared helper (test-only, mechanical).
- `crates/davout/src/simulation.rs`: P-davout-14 gating (owned: davout simulation factory rows).

## Sequencing with concurrent work

- WP-R (CI hygiene) adds an MJCF parser + production-parity tests in `sim-harness/src/lib.rs` (different region, no conflict) and a new bench robot.yaml-vs-marengo.urdf check replacing the deleted `#[ignore]` humanoid test: lands after B11, rebased on top.
- WP-NP owns `marengo-store`: P-marengo-store-09 `bounded` dedupe done here for review; `observe` helpers kept per-file per owner input.

## Regression evidence

No behavior fixes in this batch, so no red→green behavior tests apply; the compiler was
the regression net and it fired live during this session: restoring-then-fixing `as_slice`
(E0599), `clear_torque_cmd`/`apply_gain_overrides` callers (E0599), orphaned
`GainRuntime`/`TorqueCmdLatch` methods (`-D warnings` dead_code), and five unused imports.
Final green suites: berthier (203 unit + all integration incl. rewritten gain/torque tests),
davout (127 unit + all integration incl. helper-migrated journal/grant/commit tests),
marengo-pi (98), marengo-store (all 16 files incl. `tests/common` migration),
armee-kinematics/dynamics/proto, sim-harness.

## Gate (2026-10-03, green)

```
cargo fmt --all -- --check                    # clean (exit 0)
cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings  # clean
cargo clippy -p marengo-pi --all-targets --target aarch64-unknown-linux-gnu -- -D warnings                 # clean
cargo test --workspace                         # 121 suites, 1158 passed, 0 failed
```
