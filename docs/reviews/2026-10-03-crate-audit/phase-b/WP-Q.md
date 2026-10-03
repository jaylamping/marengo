# Phase B — WP-Q: realtime loop budget (M06)

Branch `audit/wp-mq`; package `crates/chappe` publish path, `marengo-pi`
`main.rs` pacing/diagnostics, and the Berthier tick allocation in `loop.rs`.
Per the assignment: measure first, add a cheap overrun counter with no new
per-tick allocation, remove per-tick allocations and `format!` only where
mechanical and test-covered, attempt `ValidatedRobotConfig` only without
behavior change, and put the Chappe publish fast path on a read lock.

"Red" evidence below is either an observed baseline failure, the former
regression assertion/code path, or baseline code inspection with a recorded
measurement. Green evidence is from the listed regression tests and
acceptance gates.

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-berthier-08 (S3) | **CONFIRMED (partial)** | Per-tick `format!("{traj_phase:?}")` ran per joint per tick even when never logged, and the tick collected the active-joint set into a fresh `Vec` twice per tick. `HoldJointDiag.phase` spelling is pinned by new `position_trajectory::tests::phase_names_match_debug_spelling`; existing hold/law suites (219 lib tests) still pass. | Mechanical only: `HoldJointDiag.phase` is now the planner's `&'static str` (`TrajPhase::as_str`, CSV/log spelling unchanged) and the tick iterates the live active set without collecting it. Remaining allocations (`gravity_torques` string compares, `FrictionGains` clones, `position_hold` name strings) live in `armee-dynamics`/`position_hold` law code outside this package's `loop.rs` ownership — follow-up for WP-D/WP-E after Pi jitter measurement. No `dhat` profile was available in this environment; no wider optimization was attempted without measurement. |
| L-chappe-01 (S3) | **CONFIRMED** | `Bus::sender` took the `RwLock` write lock even for existing topics, so the 200 Hz loop, tracing layer, IMU, host-metrics, and IPC reader all contended on one write lock. New `chappe::tests::concurrent_publish_on_existing_topic_delivers_to_subscriber` passes (4 publishers × 10 sends, all 40 delivered). | Existing topics resolve under a read lock; the write lock is taken only to insert a new topic (double-checked after acquiring). Poison handling on the write path is unchanged (`into_inner`). |
| L-davout-17 (S3) | **CONFIRMED (inspection, measured)** | `reference_binding_valid` runs full `validate_safety_config` on every call: twice per tick via `poll_feedback` (Berthier drains feedback before and after `send_mit_batch`) plus once per send. Temporary instrumented run (300 iterations, master 5-joint config, since removed): mean **18.7 µs**, max 22.4 µs per validation → ≈57 µs/tick, ≈**1.1 % of the 5 ms budget** on the bench. Scales as O(joints²+groups²), so a 23-joint profile would be materially hotter. | No caching applied: `Supervisor` exposes `pub motors`/`pub control`/`pub homing_config` mutated directly by the overlay and tests, so a cached verdict cannot be invalidated without a structural install path — and a stale-valid cache would fail open. Behavior is unchanged; the measurement and the soundness argument are recorded here. See NEEDS-DECISION. |
| L-marengo-pi-12 (S3) | **CONFIRMED** | Sleep pacing (`period − elapsed`) with no catch-up and overruns visible only in a debug line that reset every second. New `ipc_wiring_tests::overruns_accumulate_as_lifetime_total` passes. | `LoopTimingWindow` keeps a saturating lifetime `total_overruns` across the 1 Hz windows and logs it with the per-window stats; the 1 Hz debug additionally logs `ControlLoop::tick_overruns` and `telemetry_failures`. Pacing is unchanged — no catch-up burst after an overrun is the correct realtime behavior; the gap was visibility, now covered. |
| L-robstride-12 (S3) | **REFUTED (inspection)** | The ≥200 µs sleep in `robstride::receive::drain` sits inside `if let Some(end) = deadline` and only runs after an entirely idle round with a positive budget. The tick path uses `drain_feedback` → `poll_feedback(Duration::ZERO)` (all ten Berthier call sites), giving `deadline == None` with default limits, so the sleep is unreachable in the 200 Hz loop. `refresh_feedback` (positive budget, may sleep) is the REPL/set-zero path, not the tick; all callers are synchronous. | No change: the premise does not hold on the current code, and moving the REPL-path sleep would change bench-tool behavior for no tick gain. A future async caller must revisit (noted here). |
| L-berthier-07 (S3) | **CONFIRMED** | The env-gated trace did a 512 KiB-buffered `write(2)` inside the tick with errors swallowed (`let _`), unbounded append growth, silently dropped open errors, and no flush on exit. New `position_trace::tests::healthy_trace_records_and_flushes`, `session_cap_stops_recording_without_error`, `open_refuses_already_huge_file`, `open_surfaces_unwritable_file`, and `write_failure_disables_trace_and_counts_once` pass. | `from_env` open failure warns instead of vanishing; tick writes are infallible by design (first failure disables tracing with one warning and is counted); a 256 MiB session cap plus refusal to append to already-huge files bounds disk use; `flush_position_trace` runs on owner shutdown, never in the tick. Remaining `format!` sites in `position_trace.rs` serve the env-gated CSV row itself, not the hot tick decision path. |

## M06 measurement summary

- `validate_safety_config` (5-joint master): mean 18.7 µs, max 22.4 µs (n=300, dev profile, host). Per-tick ≈3× ≈ 57 µs ≈ 1.1 % of the 5 ms period. No Pi (aarch64) measurement yet — bench verification should compare `tick_elapsed_avg_us`/`tick_elapsed_max_us` and `total_overruns` before/after deploy.
- Loop-budget telemetry now available every second at debug: `tick_elapsed_avg_us`, `tick_elapsed_max_us`, `overruns` + `total_overruns` (Pi window), `tick_overruns` + `telemetry_failures` (Berthier), and per-phase averages (`feedback/gravity/planner/compose/send/chappe/trace_us`).
- Tick overrun accounting costs one `Instant` read + compare per tick; no allocation (saturating `u64` counters only).

## NEEDS-DECISION

### L-davout-17 — ValidatedRobotConfig
- **A (recommended):** make the policy fields private behind one `install_control_overlay` (or generation-counted install) that validates once at install and revokes reference on failure; per-tick paths then trust the installed generation. Removes the 3×/tick revalidation soundly; touches `davout` public API, the `marengo-pi` overlay writer, and reference-revocation tests (WP-H/WP-J coordination).
- **B:** cache the last verdict keyed by a config revision, invalidated by explicit caller notice. Weaker: any missed invalidation fails open, so every writer must be audited — equivalent work to A with a worse failure mode.
- **C:** accept the per-tick cost on the 5-joint bench (≈1 %) and revisit with a 23-joint profile measurement. Leaves the humanoid scaling risk open.

### L-berthier-08 remainder — law/dynamics allocations
- **A (recommended):** after Pi jitter measurement shows budget pressure, let WP-D/WP-E own interned joint handles or index-keyed gravity evaluation to remove the per-tick string work. Needs law-level equivalence tests.
- **B:** optimize inside `loop.rs` only (done here). Caps what this package can do without crossing into law/dynamics ownership.

### L-berthier-07 — trace session cap
- The 256 MiB cap (≈30 min at full 5-joint 200 Hz) is a judgment value. Shrink it for SD-card benches or make it env-tunable if operators hit the cap mid-session.

## Cross-package edits

- None outside the package: no `davout`, `robstride`, `marengo-config`, `armee-dynamics`, or law-file behavior changes. `position_hold.rs`/`position_trajectory.rs` changes are limited to the `phase: &'static str` representation consumed by the tick diag path (spelling pinned by test). Gateway edits (shared topic consts) are reported under WP-M.
- `crates/chappe/src/ipc_outbox.rs`: topic lookup only; bounds, classes, expiry, and all counters are byte-identical (dropped-counter semantics belong to WP-L).

## Gate

- `cargo fmt --all -- --check` — pass.
- `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings` — pass.
- `cargo test --workspace` — pass (1205 passed, 0 failed).
- `cargo test -p chappe -p berthier -p marengo-pi` — pass (all suites, zero failures).
