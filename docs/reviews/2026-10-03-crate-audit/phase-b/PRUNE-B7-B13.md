# Phase B — PRUNE-B7-B13: config surface, pi internals, proto deprecations, Jetson remnants

Branch `audit/prune-b7b13`; baseline `main` (no commits ahead at start; all work below is
uncommitted → committed as B7 / B13 / Jetson-remnants). A previous agent left 37 uncommitted
files; that partial work was reviewed file-by-file, kept where correct, and completed here.
No Pi hardware behavior was exercised. Ownership stayed inside
`crates/marengo-config`, `bins/marengo-limit-sync`, `bins/marengo-pi` internals named by the
B13 rows, `proto/marengo/v1/marengo.proto`, Consul Jetson UI, and the gateway Jetson
topic/route. The two `marengo-host-metrics` edits are minimal cross-package fallout of the
proto deprecation (see § Cross-package edits).

## B7 — marengo-config surface and limit-sync duplicates

| Row | Verdict | Evidence / fix |
|---|---|---|
| P-marengo-config-01 | **Deleted** | `resolve_desired_joint_velocity_cap` folded into `resolve_joint_velocity_cap` (that name kept); alias removed from `lib.rs` and its tests. ADR 0010 amendment notes the fold. |
| P-marengo-config-02 | **Deleted** | `add_joint_from_source`, `joint_in_motors`, `joint_in_profile_urdf`, `membership_slugs_for_joint`, `AddJointResult` + their `profile_txn` tests removed. Zero references; bringup profiles retired. |
| P-marengo-config-03 | **Deleted** | `upsert_joint_limits`, `UpsertLimitResult`, `check_revision` removed. Berthier `feedback_bootstrap` fixture migrated to validate → `ensure_soft_inset` → apply motor+control → `write_motors_control_and_urdf` (same validate-then-commit incl. URDF expand). |
| P-marengo-config-05 | **Deleted** | `is_actuated` (`urdf_merge`) removed; zero refs, was `allow(dead_code)`. |
| P-marengo-config-06 | **Deleted** | `load_urdf_from_str` temp-file round trip → `urdf_rs::read_from_string`. Also closes the L-marengo-config-09 temp-file leak on the merge path. |
| P-marengo-config-07 | **Deleted (narrowed), 2 adjustments** | ~15 zero-external-consumer items narrowed: sub-validators, `control_config_path`, consts, merge helpers (`merge_preview_from_robots`, `apply_merge_xml`, `validate_merged_urdf_xml`), `validate_joint_subset`, `actuator_group_for_joint`, `limit_margin_fields_valid` → `pub(crate)`; `write_profile` → `#[cfg(test)]` (rollback oracle, no production caller left). Adjustments: (1) `validate_joint_gains_against_motor_type` stays `pub` — validators are never pruned and it has no internal production caller (`apply_joint_config_param` does not invoke it; that gap is not this wave's scope). (2) `expand_urdf_file_to_cover_motors` / `write_motors_and_control` / `write_motors_control_and_urdf` stay `pub` (live callers). Verified zero external users of every narrowed item. |
| P-marengo-config-08 (D-8) | **Deleted** | `robot.bench.max_joint_velocity_rad_s` removed from schema, `validate_robot_config`, both robot YAMLs, gateway test fixture, and config tests. ADR 0010 amendment spends the ":35 commissioning documentation" allowance; `docs/safety.md:14` no longer lists the field. `motors.yaml bench.velocity_limit_rad_s` and URDF `limit.velocity` remain as non-capping documentation. |
| P-marengo-config-10 | **Deleted** | `robot.name` removed from schema, `validate_robot_config`, both robot YAMLs, gateway fixture. C03 holds: no other reader in scope; MCP has zero readers; Consul `name` hits are all URDF `<robot name>` XML parsing (unrelated). |
| P-marengo-config-11 | **NEEDS-DECISION** | `motor_type` duplicated in `control.joints` + `motors.yaml`, validated equal (`lib.rs:776-781`), control copy read at `berthier loop.rs:994`. Deriving the control copy needs Berthier to read the motors copy via Davout — a behavioral change outside this ownership. See § NEEDS-DECISION. |
| P-marengo-config-12 | **Deleted** | `homing.defaults.method` (+ serde default fn use + `Default` impl field + `config/homing.yaml` defaults key) removed. C21 holds (zero readers). Per-entry `method` is untouched — it IS read (`effective_joint_ref`, Davout reference path). |
| P-marengo-limit-sync-02 | **ALREADY-FIXED on main** | No `ensure_soft_inset` call remains in `bins/marengo-limit-sync/src/main.rs`; the single call lives inside `apply_local_limit_patch` (`urdf_expand.rs:162`). Nothing to delete. |
| P-marengo-limit-sync-04 | **Deleted** | Consul `profile` field + plumbing removed from `persist-joint-limits.ts`; `tools/limit-sync-local/server.ts` never read it. Bringup profiles retired. |

WP-J non-regression: `profile_txn` still exposes `write_motors_and_control` + `limit_patch_from_motor`;
`atomic_file` / `config_revision` untouched; `config_revision.rs` CAS untouched. `marengo-config`
and gateway suites green (see gate).

## B13 — marengo-pi internals and dead proto surface

| Row | Verdict | Evidence / fix |
|---|---|---|
| P-marengo-pi-01 | **Deleted** | `robot/homing` subscription, `HomingComplete` compat drain, and `homing_rx` plumbing removed from `main`, `ControlLoopRuntime`, `drain_chappe_commands`, `motion_owner_chappe_tests`, `safety_publication_tests`, `shutdown_tests` (incl. `ControlLoopRuntime` literal sites), and the chappe `COMMAND_TOPICS` allowlist (7→6). Zero `robot/homing` refs remain anywhere; `HomingComplete` Rust use is one gateway comment. Proto message kept compiling with `deprecated = true` for wire compat (step 1 of deprecate-then-reserve). |
| P-marengo-pi-04 | **Deleted (deduped)** | Overlay's duplicate `TOPIC_AUDIT_ACTION` const + `publish_action_event` removed; single home in `limit_persist` (`pub(crate)`). Overlay re-exports the topic `#[cfg(test)]`; `motion_owner` imports from `limit_persist`. |
| P-marengo-pi-06 | **Deleted (migrated)** | `drain_commands` → `drain_commands_until_shutdown` (2 call sites), `wait_persist_idle` → `persist.wait_idle` (2 sites), `_owner_shutdown` param dropped from `spawn_with_test_hooks` (13 sites), `is_busy`/`wait_idle` → `#[cfg(test)]`, `wait_idle_for_test` removed (3 sites → `wait_idle`). `spawn_with_test_hooks` keeps its `#[cfg(test)]` gate (a missing gate broke non-test builds; caught by clippy). |
| P-marengo-pi-07 | **NEEDS-DECISION** | `main` still loads control/motors/robot at startup. Those loads feed SocketCAN open + `ControlLoop` construction args, not pure duplicates; removal needs the subset decision (L-marengo-pi-09, WP-H) and touches WP-MQ run-loop wiring. See § NEEDS-DECISION. |
| P-marengo-pi-08 | **Replaced with G04 regression** | `persist_queue_coalesces_to_latest_draft` rewritten: gates the first write, enqueues a limit_patch (motors: Some, bounds −1.23/2.34, soft −1.1/2.2) + a later control-only draft (kp 44.0), asserts disk keeps the queued bounds with the newer kp. Red→green below. |
| P-armee-proto-02 | **Deprecated (step 1 of 2)** | 9 dead types marked `option deprecated = true` with B13 comments: `LogSessionMeta`, `LogSessionList`, `StructuredLogEntry`, `StructuredLogList`, `CandumpTimestampMode`, `CandumpIdCount`, `CandumpInterfaceSummary`, `SessionStartRequest`, `SessionStartResponse`. NOT deprecated (kept): `ModeChange`, `HoldCommand`, `PresetCommand`, `EnableChange` — zero direct name refs, but live via the `ActuatorCommand` oneof arms the overlay matches (`overlay.rs:253-257`, P-marengo-pi-03 KEEP). Nothing deleted, so no `reserved` yet and `buf breaking` FILE is clean; deletion + `reserved` numbers/names needs the P26 breaking exception (follow-up). |
| P-armee-proto-03 (D-11) | **Kept, documented** | Tuning path (`TuningTier`, `TuningChangeEvent`, `/command/actuator`, `robot/audit/tuning`) kept per D-11; "currently UNWIRED" notes added in `marengo.proto` and `gateway/actuator.rs`. Do not remove without a new D-11 decision. |
| P-armee-proto-04 | **Deprecated in place** | `MitJointCommand.velocity`/`torque_ff`, `MitCommandBatch.timestamp_ms`, `OperatorCommand.seq`, `JointActuatorLimit.wired` marked `[deprecated = true]`. Writes stay during the window so wire bytes do not change; targeted `#[allow(deprecated)]` with B13 comments at: gateway `apply_limit_patch_async` (seq), gateway test helpers `seed_limits` (wired) + `operator_envelope` (seq), pi `build_limit_snapshot` (wired), pi test helpers `gain_batch` / `limit_patch_op` / `tuning_operator` / `motion_operator` / `command` + 3 test fns reading `wired`/`seq`. All allows must go at the reserve step. Residual: Consul `use-compound-playback.ts` still writes `velocity: 0, torqueFf: 0` (zeros the Pi ignores; TS deprecation is JSDoc-only, build green) — stop those writes at reserve. |

## Jetson remnants (D-1) + stragglers

- Proto Jetson surface deprecated (reserve deferred with P-armee-proto-02): `HOST_NODE_ROLE_JETSON` value,
  `HostMetrics.jetson` field 21, `JetsonPlatformMetrics` message. No producer exists; nothing deleted.
- Gateway: `TOPIC_HOST_METRICS_JETSON` const, allowlist entry, `Snapshots.host_metrics_jetson` slot +
  ingest arm + accessor, `/snapshot/host/metrics/jetson` route + handler, and the demo `host_jetson`
  publisher (incl. `JetsonPlatformMetrics` import) deleted. `host_metrics_pi` path untouched (P-gateway-10).
- Consul: `jetson-host-card.tsx` deleted; `SectionCards` grid, `hostMetricsStore` (`jetsonMetrics`),
  `use-chappe-telemetry` (all metrics now publish as Pi), `chappe-config` topics, `host-metrics.ts`
  dummy data deleted; both card tests updated. Remaining `jetson` strings are simulation-flavor copy
  (`sim-session-card`, `simulation.ts` host label), not telemetry.
- Portraits: `docs/portraits/fouche.jpg` + `talleyrand.jpg` deleted with their README table rows.
  Zero live references (remaining READMEs point at other portraits; main README:21 already records the
  scaffold removal). ADR 0014 stays the design record, untouched.
- ADR 0007 + ADR 0009: amendment notes appended (history above untouched) recording the `talleyrand`
  crate removal per D-1/B15.
- `vcan` cargo feature alias (P-robstride-02, P-motor-repl-06; survived merged B1): removed from
  `crates/robstride/Cargo.toml` + `bins/motor-repl/Cargo.toml`. Compose/CI/justfile/scripts `vcan`
  hits are the docker vcan profile / `vcan-up.sh` / `--features socketcan` — unrelated, no caller
  updates needed. The `vcan` *module* (P-robstride-13 KEEP) is untouched.
- `.slim/codemap.json` (tracked): removed 49 file + 18 folder keys for paths deleted from the repo
  (probe/wave-demo/teleop/marengo-jetson bins, fouche/talleyrand crates, `config/bringup/*`,
  `config/network.yaml`, `deploy-jetson.sh`, chappe `transport.rs`, store `ring.rs`, candump/robstride
  leftovers, Consul glass-shell deletions from earlier waves). Kept `consul/src/gen/*` keys (generated,
  will re-materialize). Remaining stored hashes are unverifiable — no in-repo generator (stale since
  June, Windows root) — so only dead-path keys were pruned, no hashes refreshed.

## NEEDS-DECISION

1. **P-marengo-config-11 (`motor_type` duplication).** Options: (a) keep both copies + equality
   validation (status quo, validated at `lib.rs:776-781`); (b) derive the control copy and have Berthier
   read the motors copy via Davout (needs a Berthier/Davout owner, touches construction + `loop.rs:994`
   + `davout lib.rs:508`). Recommendation: (a) until a Berthier owner schedules (b); the duplication is
   validated, not silent.
2. **P-marengo-pi-07 (main config loads).** Options: (a) keep — loads feed `RuntimeBus::socketcan_from_motors`,
   loop Hz/state Hz, and URDF resolution, while Davout's re-read is the supervisor's own admission;
   (b) plumb loaded configs into `ControlLoop` construction to read once (touches WP-MQ run-loop wiring
   and the L-marengo-pi-09 subset story owned by WP-H). Recommendation: (a); revisit with the subset decision.

## Cross-package edits

- `crates/marengo-host-metrics/src/lib.rs`: `#[allow(deprecated)]` + B13 comments on `sample` and
  `sample_services` for the deprecated `HostNodeRole::Jetson` match arms (kept mapping Jetson → none,
  as B15 left it). No behavior change. (Host-only `sample_state` dead-code warnings are pre-existing,
  platform-gated, and outside the gate, which excludes this crate.)

## Red → green

`persist_queue_coalesces_to_latest_draft` (P-marengo-pi-08, G04): with `carry_pending_limit_patch`
temporarily stubbed to `Ok(())` (old replace-with-None semantics) the test **FAILS** at the
discriminating assertion (`left: -0.41035…` fixture default vs `right: -1.23` queued bound,
`overlay_tests.rs:531`); with the WP-J carry restored it **passes**. Stub removed afterwards
(`git diff` shows no trace). Berthier `feedback_bootstrap` fixture migration is behavior-preserving
by construction (same validate → inset → apply → write-all sequence the removed upsert performed);
`marengo-config`, gateway, and pi suites green.

## Gate

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings`: clean.
- `cargo clippy -p marengo-pi --all-targets --target aarch64-unknown-linux-gnu -- -D warnings`: clean.
- `cargo test --workspace`: 121 suites `ok`, zero failures (two full runs).
- `buf lint proto`: clean. `buf breaking proto --against main`: clean (deprecations only, no deletions).
- Consul: `npm run gen:proto` → `proto-checksum.sh: ok` (`.checksum` updated to `d2ce5daa…`),
  `npm test -- --run`: 75 files / 373 tests pass; `npm run build`: success.
- MCP tests not run: `tools/` untouched.
- Worktree isolation: main checkout shows only `M .cursor/mcp.json` + `?? WATCHDOG.yml` throughout.
