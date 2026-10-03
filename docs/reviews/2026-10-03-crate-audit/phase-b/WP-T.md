# Phase B — WP-T: Homing remnants & readiness facets (+ prune B4/B14)

Branch `audit/wp-t`, against the assigned worktree baseline. Prior commit
`aac9d904` (OutOfLimits recovery + subset Ready documentation) reviewed and
verified. User decisions (decisions.md D-3/D-4/D-9): retire the scalar
verifier, YAML history writer, startup history read, legacy registry and Hall
`sensor.rs`; keep `calibration_record_path`; fold the rest into Davout (D-8 in
prune-candidates is the same D-3 row). Two prune commits: B4 (Davout surface
trim) then B14 (homing retirement, crate deleted).

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-marengo-homing-01 | **CONFIRMED — fail-closed by design, documented** | The OutOfLimits latch has no in-process clear. Every production set goes through Davout's feedback consumer, which returns a `Limit` error latched as a permanent Feedback-class fault in the same call (faults never clear in-process, ADR 0020); reference acquisition requires fault-clear, so no fresh grant can exist while the flag is set. Recovery is a process restart. | No behavior change. `aac9d904` documented the recovery story on the registry; B14 makes it literal (`clear_out_of_limits` deleted with `record_verification`, its only caller) and moves the story to `davout::homing_facets::OutOfLimitsFlags`. |
| L-marengo-homing-05 | **CONFIRMED — documented** | `MARENGO_JOINT_SUBSET` narrows the loaded robot at construction, so excluded joints arrive at Ready aggregation as unbuilt inventory and intentionally do not block Robot Ready. | No behavior change. `aac9d904` documented the subset semantics on `robot_ready`; B14 carries the wording into `homing_facets.rs` with the `subset_excluded_joints_neither_block_ready_nor_become_targets` regression. |
| L-marengo-homing-02/03/04 (Hall) | **RESOLVED-BY-PRUNE** | Dormant Hall code with latent polarity/overlap/unknown-pin bugs, zero production callers. | `sensor.rs`, `check_sensor_health`/`sensor_health`/`set_sensor_health`, `method_requires_sensors` deleted per D-4 (B14, P-marengo-homing-03). Mechanical layout doc retained for the future GPIO adapter. |
| L-marengo-homing-06 | **RESOLVED-BY-PRUNE** | Non-atomic history writer (lead itself: "moot if writer pruned"). | `record_verification`/`persist`/`persist_record` deleted (B14, P-marengo-homing-01). |
| L-marengo-homing-07 | **RESOLVED-BY-PRUNE** | `mark_fault` discarded its message. | Legacy lifecycle (`mark_fault`, `set_state`, `all_verified`, `any_faulted`, `require_ready`, ...) deleted (B14, P-marengo-homing-02). Live fault evidence is `FaultAuthority`/`SafetySnapshot`, untouched. |
| L-marengo-homing-08 | **CONFIRMED intended** | Ready aggregation ignores `drive_active`. | D-9: the write-only `JointFacetInput.drive_active` field is deleted (B14, P-marengo-homing-07); per-joint drive state remains queryable via `Supervisor::joint_drive_active` for the wire facets. |
| L-marengo-config-15 | **NOT COVERED — out of WP-T ownership** | `load_commissioning_scope` treats `exists()==false` as absent. | No change. `commissioning_scope.rs` is outside the WP-T file ownership (no dead field resulted); routed to the config owner. |

## Prune B4 — Davout surface trim (commit `2a3f06b4`)

P-davout-01 (refusal stubs), P-davout-04 (`refresh_feedback` +
`feedback_poll_budget_us` key/validation; `drain_quiet` now checked against
the tick), P-davout-05 partial (`clear_motor_states`, `joint_velocity_rad`,
`clear_applied{,_joint}`, `active_reporting_applied`, pub `rebuild_limits`
deleted; `filter_mit_command` demoted to `#[cfg(test)]`), P-davout-06 (kept
`sync_active_reporting`, deleted the `tick_active_reporting_leases` alias),
P-davout-08 (duplicate `validate_mit_command`), P-marengo-config-09
(redundant validator calls), P-davout-15 (no-op `.min(torque_limit)` +
`bench_torque_cap_never_binds_tau_ff_max` equivalence test).

- **P-davout-07: KEPT, no code change.** No `DangerZoneAction` typed enum
  exists (WP-H deferred CS14); validation still admits only `clamp_velocity` /
  `clamp_torque`, so the `_ => {}` arm and the torque fallback stay until CS14
  lands. Danger-zone logic untouched.
- P-davout-05 remainder (`homing_registry`,
  `from_repo_with_physical_reference_and_record_path`, `commissioning_facets`,
  `joint_out_of_limits`) deferred to B14, where the history/out_of_limits
  fate is decided.

## Prune B14 — homing retirement (this commit)

P-davout-09 (history plumbing: `HomingRegistry` record load,
`MARENGO_CALIBRATION_RECORD` reads, `from_repo_with_calibration_record_path`
/ `from_simulation_with_calibration_record_path` /
`from_repo_with_physical_reference_and_record_path`), P-davout-10
(`request_enable` deleted; call sites migrate to `enable_targets` /
`disable_all`), P-marengo-homing-01 (verifier + writer + `chrono` dep),
-02 (legacy lifecycle), -03 (Hall sensors), -04 (`limb_ready`), -05
(proto-decode helpers), -06 (`effective_homing_for_robot`,
`HomingRegistry::new`), -07 (`drive_active`), -08 (fold remainder into
`davout::homing_facets`, delete the crate; workspace members, dependents and
docs updated).

- `homing.yaml calibration_record_path` is **kept**: nothing reads history,
  but the reserved location keeps the reference journal distinct and stays in
  the reference policy binding. No marengo-config field went dead (the
  reference transaction still reads the homing policy).
- `set_homing_complete` / `OperationalMode::Ready` are **kept**: live
  production path (`marengo-pi home`, berthier tests, gateway reads Ready).
- `commissioning_facets` / `joint_out_of_limits` stay `pub`: the former pins
  fault-visibility coverage in `fault_snapshot.rs`; the latter feeds the live
  wire facets. The deleted accessor is `homing_registry()`.
- `reference_transaction.rs` / `burst.rs` / `active_reporting.rs` sequencing
  logic untouched; only call-site migrations to deleted items (plus
  `normal_denials` shrinking 4 → 3 entries with `request_enable` gone).
- `crates/davout/tests/reference_history.rs` rewritten to pin the new
  contract: legacy files (corrupt, directory-shaped, well-formed, missing)
  never affect construction, never grant, create nothing.
- Gates: `cargo fmt` clean; `cargo check --workspace --all-targets` clean;
  `cargo clippy -p davout -p berthier -p marengo-config --all-targets
  -- -D warnings` clean (full-workspace clippy still fails on pre-existing
  `marengo-host-metrics` dead-code errors, untouched); `cargo test -p davout
  -p marengo-config -p berthier -p marengo-pi` green, including
  `physical_reference` (50) + `firmware_profile` (7).
