# gravity-comp-enhancement - Work Plan

## TL;DR (For humans)
<!-- Fill this LAST, after the detailed plan below is written, so it summarizes the REAL plan. -->
<!-- Plain English for a non-engineer: NO file paths, NO todo numbers, NO wave/agent/tool names. -->

**What you'll get:** Your robot's gravity compensation will be hardened with type-level mode isolation (so gravity comp changes can never silently break MIT, Torque, Impedance, or Position modes), corrected URDF accuracy (27% over-comp fixed), three new runtime safety guards (wrong-sign watchdog, torque saturation pre-check, mode-switch ramp), and a 10-test physical bench suite that proves it all works on your 700g weighted arm — with the results doc upgraded from "PASS WITH WARNINGS" to "PASS".

**Why this approach:** A three-judge Oracle panel scored the existing implementation against industry standards and found the algorithm is acceptable for 1-DOF but the validation is incomplete and mode isolation is enforced by developer convention, not by types or tests. The highest-risk finding is that tau_g is a shared summand in every control mode with no abstraction boundary — a single ill-scoped edit could silently corrupt Impedance/Position behavior. This plan closes that with a PureGravityTorque newtype and property tests first, then layers accuracy and safety enhancements, then validates everything on hardware.

**What it will NOT do:** Replace the numerical gravity algorithm with Pinocchio/RNEA (kept as future option at ≥8 DOF). Touch the full humanoid, left shoulder, Consul UI, Chappe wire types, or robstride CAN protocol. Include simulation-only tests.

**Effort:** Large — 21 todos across 5 waves (0-4), ~4 code waves (no hardware) + 1 bench wave (requires your 700g rig and operator attention for arm support on first enables).
**Risk:** Medium — the mode-switch kp/kd ramp (Todo 3) and wrong-sign watchdog (Todo 7) touch the safety-critical Davout path; mitigated by Wave 0 sign/golden tests as a safety gate, TDD on mode isolation, and incremental bench validation. Metis review caught a safety regression in the rate-limiter reset (clearing vs seeding) — fixed.
**Decisions to sanity-check:** (1) Numerical gradient retained (not RNEA) — judge flagged as below-standard-at-scale but acceptable at 1-DOF; FK caching (O(n·L·J)→O(n·L)) is a perf half-measure, not an algorithm upgrade. RNEA deferred to future ADR at ≥8 DOF. (2) Wrong-sign watchdog uses config-driven sign table (not tau_g recomputation in Davout) — preserves crate boundary, requires ADR 0015. (3) Mode-switch ramp is ~100ms (20 ticks at 200Hz) — may need tuning if it feels sluggish. (4) Rate limiter is SEEDED on transition (not cleared) to prevent unclamped torque step.

Your next move: approve to start execution, or run a high-accuracy Momus review first. Full execution detail follows below.

---

> TL;DR (machine): Large effort, Medium risk. 21 todos / 5 waves (0-4). Metis-reviewed: fixed rate-limiter safety regression (seed not clear), added Wave 0 sign/golden gate, config-driven watchdog + ADR 0015, proptest dep. 3 judge-scored gaps (mode isolation FAIL, validation FAIL, algorithm below-standard-at-scale) → PureGravityTorque newtype + ModeIsolation tests + kp/kd ramp + wrong-sign watchdog + saturation check + URDF COM fix + FK caching + friction fade + 10-test physical bench suite on 700g rig.

## Scope
### Must have
- Type-level enforcement that `gravity_torques()` returns pure gravity torque (not friction, not payload estimation folded in)
- Characterization tests proving each ControlMode (GravityComp, Impedance, Position, TorqueOnly) is independent — perturbing tau_g does not change non-gravity FF terms
- Mode-switch kp/kd ramp in `set_control_mode` to prevent transient torque steps (upright-pose fall vector)
- Rate-limiter state reset on mode transition (clear `last_tau_ff` when switching modes)
- Wrong-sign torque watchdog in Davout (runtime, not just manual Phase 0b)
- Pre-flight tau_g saturation check before enable (max(|tau_g|) over range vs tau_motor_max)
- URDF COM correction (18in→14in) in both bench URDFs
- FK chain caching in UrdfGravityModel (O(n³)→O(n²))
- Sign convention documentation in armee-dynamics lib.rs
- Friction-FF graded fade at Accelerate→Cruise boundary (closes Phase 2 smoothness)
- Physical bench test suite design document with detailed protocols for 700g weighted rig
- Bench test protocols: static torque sweep (corrected COM), gravity A/B tracking, friction ID sweep, mode-switch transient test, wrong-sign watchdog validation, payload robustness, negative-retarget descent gate, recovery formalization
- Acceptance gates with exact telemetry artifacts (bench-log, position-trace CSV, candump)
- Save findings to mem0 under `control/gravity-comp/*` and `decision/gravity-comp/*`

### Must NOT have (guardrails, anti-slop, scope boundaries)
- No Pinocchio/RNEA FFI replacement of numerical gradient (keep as optional future ADR when DOF≥8)
- No full humanoid dynamics (1-DOF bench only)
- No sim-only tests (physical bench tests only; sim cross-check is ADR 0005 D1, separate)
- No left shoulder pitch changes (right-only bench)
- No Consul UI changes
- No Chappe wire type changes
- No robstride CAN protocol changes
- No `unsafe` blocks (workspace forbids unless ADR exception)
- No `unwrap()`/`expect()` in crates/ library code (clippy warn)
- No friction/payload estimation folded INTO `gravity_torques()` — must stay in separate consumers
- No changes to the MIT frame encoding or motor space transform logic in robstride
- No Berthier opening CAN directly (must go through Davout)
- No `println!` in runtime code (use `tracing`)

## Verification strategy
> Zero human intervention - all verification is agent-executed.
- Test decision: TDD for mode isolation tests (write first, then implement); tests-after for enhancements (implement, then verify). Framework: Rust `cargo test --workspace` (no hardware in default tests).
- Evidence: `.omo/evidence/task-<N>-gravity-comp-enhancement.<ext>` — each task saves its proof (test output, bench log path, candump summary, position trace path) to this directory.
- Physical bench tests use `pi_hold_on`, `pi_bench_harness`, `pi_gravity_preview`, `pi_candump_summary`, `pi_logs_*` MCP tools. Bench artifacts on Pi: `/opt/marengo/var/log/bench-<TS>.log`, `position-trace-<TS>.csv`, `candump-<TS>.log`.
- `just check` (or `./scripts/check.sh` on cloud) must pass before any Rust commit.
- `pi_sync_main` deploys to Pi after Rust changes; `pi_sync_bench_config` for config-only changes.
- After any bench motion: `pi_candump_summary` + position trace analysis (CAN is wire truth).

## Execution strategy
### Parallel execution waves
> Target 5-8 todos per wave. Fewer than 3 (except the final) means under-split.

- **Wave 0 (Safety Gate — no hardware, MUST precede everything):** Todo 0. Cross-crate sign test + golden-value tests + proptest dep. Gates all tau_g changes.
- **Wave 1 (Mode Isolation — CRITICAL, no hardware needed):** Todos 1-5. Type contracts + tests + mode-switch ramp + rate-limiter seed. All cargo-test, no Pi.
- **Wave 2 (Accuracy & Safety Enhancements — mostly no hardware):** Todos 6-10. COM correction, wrong-sign watchdog (config-driven + ADR 0015), saturation check, FK caching, sign docs. Todos 6+10 are asset/doc only; 7-9 are Rust code verified by cargo test.
- **Wave 3 (Friction & Validation Code — no hardware):** Todos 10-12. Graded fade (closes Phase 2) + bench test suite doc + deploy. Cargo test + doc.
- **Wave 4 (Physical Bench Test Suite — REQUIRES HARDWARE):** Todos 13-20. Each is a bench protocol run on the 700g rig. Requires `confirm: true` + `confirm_weighted_motion: true`. Operator must support arm for first enables.

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| 0 (sign + golden tests + proptest) | — | 1, 6, 9 | 5 |
| 1 (PureGravityTorque newtype) | 0 | 2, 3, 7 | 6, 8, 10 |
| 2 (ModeIsolation test module) | 1 | — | 3, 6, 8, 10 |
| 3 (mode-switch kp/kd ramp) | 1 | 4 | 6, 8, 10 |
| 4 (rate-limiter SEED on transition) | 3 | — | 6, 8, 10 |
| 5 (sign convention docs) | — | — | all |
| 6 (URDF COM correction) | 0 | 13, 14 | 1-4, 7-10 |
| 7 (wrong-sign watchdog + ADR 0015) | 1, 0 | 18 | 6, 8, 9, 10 |
| 8 (saturation pre-check) | — | 19 | 6, 7, 9, 10 |
| 9 (FK chain caching) | 0 | — | 6, 7, 8, 10 |
| 10 (friction graded fade) | — | 16 | 6, 7, 8, 9 |
| 11 (bench test suite doc) | 6 | 13-20 | 7-10 |
| 12 (deploy + sync) | 0-10 | 13-20 | 11 |
| 13 (static torque sweep bench) | 6, 11, 12 | 14 | — |
| 14 (gravity A/B tracking bench) | 13 | — | — |
| 15 (payload robustness bench) | 13 | — | 14 |
| 16 (friction ID sweep bench) | 10, 12 | — | 14, 15 |
| 17 (mode-switch transient bench) | 3, 4, 12 | — | 14, 15, 16 |
| 18 (wrong-sign watchdog bench) | 7, 12 | — | 14-17 |
| 19 (negative-retarget descent bench) | 6, 12 | — | 14-18 |
| 20 (recovery + full suite re-run) | 13-19 | — | — |

## Todos
> Implementation + Test = ONE todo. Never separate.
<!-- APPEND TASK BATCHES BELOW THIS LINE WITH edit/apply_patch - never rewrite the headers above. -->
- [x] 0. Cross-crate tau_g sign test + golden-value tests — gates ALL tau_g changes
  What to do: FIRST task overall. (a) Add `crates/armee-dynamics/tests/sign_and_golden.rs` with: golden-value tests using bench data from `docs/bench-weighted-700g-results.md:20-28` (q=0→tau_g=0, q=0.3→0.9278, q=-0.3→-0.9278, q=1.57→3.1396 — NOTE: these are pre-COM-correction golden values; update after Todo 6). (b) Add a cross-crate sign test in berthier that verifies: at q=0.3 rad, tau_g has positive sign for right_shoulder_pitch, AND the wire-level torque after Davout's direction=-1 transform has the correct (negated) sign. Use MemoryBus mock. (c) Add `proptest` to `[workspace.dependencies]` in root Cargo.toml, run `cargo deny check` to verify advisories. These tests MUST exist before any tau_g change (COM correction, FK caching, newtype) so regressions are caught.
  Must NOT do: Do NOT change tau_g computation. Do NOT skip `cargo deny check` for proptest. Do NOT use hardware.
  Parallelization: Wave 0 (before everything) | Blocked by: — | Blocks: 1, 6, 9
  References: `docs/bench-weighted-700g-results.md:20-28` (golden values), `crates/armee-dynamics/src/urdf_gravity.rs:107-135` (gravity_torques), `crates/davout/src/lib.rs:1168-1183` (motor_position_scale with direction=-1), `Cargo.toml` (workspace deps)
  Acceptance criteria: `cargo test -p armee-dynamics --test sign_and_golden` passes. `cargo test -p berthier --test sign_cross_crate` passes. `cargo deny check` passes with proptest added. Golden values match bench data within 1%.
  QA scenarios: happy — golden values pass, sign test confirms direction transform. failure — if a future tau_g change flips sign, the sign test catches it before hardware. Evidence `.omo/evidence/task-0-gravity-comp-enhancement.txt`
  Commit: Y | test(armee-dynamics): golden-value and cross-crate sign tests gate tau_g changes

- [x] 1. PureGravityTorque newtype — type-level contract that gravity_torques() returns pure gravity
  What to do: Add a `PureGravityTorque(pub Vec<f64>)` newtype in `crates/armee-dynamics/src/lib.rs`. Change `DynamicsModel::gravity_torques` return type from `Result<Vec<f64>>` to `Result<PureGravityTorque>`. Update `UrdfGravityModel::gravity_torques` to wrap the output. Update all call sites in `crates/berthier/src/loop.rs` (L607: `let tau_g = self.dynamics.gravity_torques(&q)?;` → unwrap the newtype via `.0` or a `.as_slice()` method). Add a doc comment on PureGravityTorque: "Joint-space gravity holding torque τ_g(q) in Nm. Pure gravity only — no friction, no payload estimation, no velocity coupling. Motor-space transform τ_motor = τ_g / (direction·gear_ratio) applied by Davout." This is the type-level enforcement that prevents future "enhancements" from silently folding friction/payload into gravity_torques() and corrupting Impedance/Position modes.
  Must NOT do: Do NOT add any computation to the newtype — it is a transparent wrapper. Do NOT change the numerical gradient algorithm. Do NOT add friction or payload to gravity_torques.
  Parallelization: Wave 1 | Blocked by: — | Blocks: 2, 3, 7
  References: `crates/armee-dynamics/src/lib.rs:42-45` (DynamicsModel trait), `crates/armee-dynamics/src/urdf_gravity.rs:107-135` (gravity_torques impl), `crates/berthier/src/loop.rs:607` (call site), `crates/berthier/src/loop.rs:626,638` (GravityComp/Impedance tau_g usage), `crates/berthier/src/position_feedforward.rs:62` (Position tau_g usage)
  Acceptance criteria: `cargo build --workspace` compiles. `cargo test --workspace` passes. `cargo clippy --workspace --all-targets -- -D warnings` passes. PureGravityTorque has a doc comment documenting sign convention and "pure gravity" contract.
  QA scenarios: happy — `cargo test -p armee-dynamics` all 4 existing tests pass with new return type. failure — `cargo build` fails if someone tries `tau_g + tau_f` without unwrapping (type mismatch). Evidence `.omo/evidence/task-1-gravity-comp-enhancement.txt`
  Commit: Y | feat(armee-dynamics): PureGravityTorque newtype enforces pure-gravity contract

- [x] 2. ModeIsolation test module in berthier — property tests proving mode independence
  What to do: Create `crates/berthier/src/mode_isolation.rs` with `#[cfg(test)] mod mode_isolation_tests`. Add `mod mode_isolation;` to `crates/berthier/src/lib.rs`. Write tests: (a) For a fixed (q, dq, config), perturbing tau_g by ±50% must NOT change the Impedance-mode non-gravity FF (tau_f from friction_torque). Assert `impedance_tau_f == friction_torque(dq, fc, fv, fo, k)` independent of tau_g value. (b) Same perturbation must NOT change Position-mode non-gravity FF (tau_f + tau_d from compose_position_hold_feedforward). Assert position non-gravity component is invariant. (c) GravityComp mode tau_ff == tau_g (no extra terms). (d) TorqueOnly mode tau_ff == tau_g (same as GravityComp). Use the MemoryBus mock (`davout::MemoryBus::default()`) and `Controller::from_repo` for test setup. Load the `shoulder_pitch_right_only` bringup config from the repo root.
  Must NOT do: Do NOT require hardware. Do NOT use `unwrap()`/`expect()` in test code (use `expect` only in `#[cfg(test)]` per workspace convention). Do NOT test mode-switching here (that's Todo 3).
  Parallelization: Wave 1 | Blocked by: 1 | Blocks: —
  References: `crates/berthier/src/loop.rs:624-700` (mode dispatch), `crates/berthier/src/friction.rs:163` (friction_torque), `crates/berthier/src/position_feedforward.rs:21-64` (compose_position_hold_feedforward), `crates/davout/src/lib.rs` MemoryBus, `crates/berthier/src/lib.rs:102-122` (existing test pattern)
  Acceptance criteria: `cargo test -p berthier -- mode_isolation` passes with ≥4 tests. Each test asserts tau_g perturbation does not change non-gravity FF components. Tests run with no hardware.
  QA scenarios: happy — all 4+ mode isolation tests pass. failure — if someone folds friction into gravity_torques(), the Impedance test fails (tau_f would change with tau_g perturbation). Evidence `.omo/evidence/task-2-gravity-comp-enhancement.txt`
  Commit: Y | test(berthier): mode isolation property tests guard gravity-comp changes

- [x] 3. Mode-switch kp/kd ramp in set_control_mode — prevents transient torque steps
  What to do: In `crates/berthier/src/loop.rs` `set_control_mode` (L517-536), add a kp/kd transition ramp. When switching modes, store the previous mode's effective kp/kd and ramp toward the new mode's kp/kd over N ticks (default: ramp over ~100ms = 20 ticks at 200Hz). Add fields to ControlLoop: `mode_transition_ramp: Option<ModeTransitionRamp>` where ModeTransitionRamp holds {from_kp, from_kd, to_kp, to_kd, ticks_remaining}. In tick(), if a ramp is active, interpolate kp/kd between from and to values for the MIT batch. When ticks_remaining hits 0, clear the ramp. The ramp applies to the per-joint kp/kd before they enter the MIT batch. For GravityComp (kp=0,kd=0), the ramp smoothly reduces kp from the prior mode's value to 0. For Impedance, it ramps from 0 (or prior) to config kp/kd.
  Must NOT do: Do NOT ramp tau_ff (Davout's rate_limit_tau_ff already does that at L1051-1057). Do NOT change the mode-clearing logic for position state (L519-529 stays). Do NOT add a ramp when switching to Disabled (instant disable is correct for safety).
  Parallelization: Wave 1 | Blocked by: 1 | Blocks: 4
  References: `crates/berthier/src/loop.rs:517-536` (set_control_mode), `crates/berthier/src/loop.rs:62-98` (ControlLoop struct fields), `crates/berthier/src/loop.rs:624-700` (tick MIT batch construction), `crates/davout/src/lib.rs:1051-1057` (existing tau_ff rate limiter pattern), AGENTS.md "Upright-pose fall" anti-pattern
  Acceptance criteria: `cargo test -p berthier` passes. New test: `mode_switch_ramps_kp_over_transition` asserts kp interpolates between modes over N ticks. `just check` passes.
  QA scenarios: happy — switching Position→GravityComp ramps kp from 8.0→0.0 over 20 ticks, no instant step. failure — without ramp, kp steps 8.0→0.0 in one tick (test would catch this). Evidence `.omo/evidence/task-3-gravity-comp-enhancement.txt`
  Commit: Y | feat(berthier): kp/kd ramp on mode switch prevents transient torque steps

- [x] 4. Rate-limiter state SEED on mode transition — seed last_tau_ff (NOT clear)
  What to do: In `crates/berthier/src/loop.rs` `set_control_mode` (L517-536), after setting the new control mode, call `self.supervisor.seed_tau_ff_rate_limiter()` (new method on Supervisor). In `crates/davout/src/lib.rs` Supervisor, add `pub fn seed_tau_ff_rate_limiter(&mut self)` that SETS each joint's `last_tau_ff` entry to the CURRENT measured torque (`self.joint_torque_rad(joint)`) rather than clearing it. This ensures the rate limiter slews from the correct starting point on mode switch. CRITICAL: clearing last_tau_ff would cause `rate_limit_tau_ff` (L1149-1166) to use `unwrap_or(target)` → UNCLAMPED torque step. Seeding with measured torque prevents this safety regression. Add a per-transition ramp matrix doc comment: Position→GravityComp ramps kp/kd DOWN to 0; GravityComp→Position ramps kp/kd UP from 0; GravityComp→Disabled is instant (no ramp); Disabled→any is instant (enable path handles separately).
  Must NOT do: Do NOT clear last_tau_ff (causes unclamped torque step). Do NOT reset during normal tick-to-tick. Do NOT reset on Disabled→Active (enable path). Do NOT ramp to/from Disabled (instant for safety).
  Parallelization: Wave 1 | Blocked by: 3 | Blocks: —
  References: `crates/davout/src/lib.rs:1149-1166` (rate_limit_tau_ff — unwrap_or behavior), `crates/davout/src/lib.rs:166` (Supervisor), `crates/berthier/src/loop.rs:517-536` (set_control_mode), Metis gap C1
  Acceptance criteria: `cargo test -p davout` passes. New test: `rate_limiter_seeds_on_mode_transition` — set last_tau_ff to a known value, switch modes, assert last_tau_ff is updated to measured torque (not cleared to None). `just check` passes.
  QA scenarios: happy — switching modes seeds the rate limiter from current torque, no unclamped step. failure — if cleared, first post-switch tick passes tau_ff unclamped (test catches this). Evidence `.omo/evidence/task-4-gravity-comp-enhancement.txt`
  Commit: Y | fix(davout): seed tau_ff rate-limiter on mode transition (not clear)

- [x] 5. Sign convention documentation in armee-dynamics lib.rs
  What to do: Expand the module doc in `crates/armee-dynamics/src/lib.rs` (L1-21) to include an explicit "Sign Convention" section documenting: (1) `gravity_torques(q)` returns joint-space holding torque τ_g(q) in Nm — the torque required to hold the arm against gravity at pose q. (2) Positive τ_g means the motor must produce positive joint torque to counteract gravity. (3) Motor-space transform: τ_motor = τ_g / (direction · gear_ratio), applied by Davout `send_mit_joint`. (4) Gravity vector is [0, 0, -9.81] (Z-down). (5) Wrong τ_g sign is a safety issue — validate with `motor-repl gravity-preview` before bench enable (already noted, keep). (6) PureGravityTorque newtype (from Todo 1) enforces this is gravity-only, not friction or payload.
  Must NOT do: Do NOT change any code. Documentation only.
  Parallelization: Wave 1 | Blocked by: — | Blocks: —
  References: `crates/armee-dynamics/src/lib.rs:1-21` (current module doc), `crates/davout/src/lib.rs:1168-1183` (motor_position_scale), `docs/decisions/0005-dynamics-library.md` (ADR)
  Acceptance criteria: `cargo doc -p armee-dynamics --no-deps` generates without warnings. Doc comment includes all 6 points above.
  QA scenarios: happy — `cargo doc` clean, sign convention section present. failure — N/A (doc-only). Evidence `.omo/evidence/task-5-gravity-comp-enhancement.txt`
  Commit: Y | docs(armee-dynamics): document gravity torque sign convention and motor-space transform

- [x] 6. URDF COM correction (18in→14in) — close 27% over-compensation
  What to do: Edit `assets/urdf/shoulder_pitch_right_only.urdf` and `assets/urdf/shoulder_pitch_weighted.urdf`: change the right_upper_arm_stub link inertial origin z from -0.4572 to -0.36 (calibrated to ~14.2in effective COM from Phase 4 bench data). Add an XML comment: `<!-- COM calibrated 2026-06-19 from 700g bench Phase 4: effective COM ~14.2in vs original 18in. tau_meas/tau_g ratio was 0.73. -->`. Verify with `pi_gravity_preview` after sync: peak tau_g at π/2 should be ~2.5 Nm (down from 3.14). Sync to Pi via `pi_sync_bench_config` with `profile: shoulder_pitch_right_only` and `install_to_opt: true`.
  Must NOT do: Do NOT change mass (0.7 kg is correct per bench weighing). Do NOT change inertia diagonal (non-blocking for tau_g). Do NOT edit left-side URDFs.
  Parallelization: Wave 2 | Blocked by: — | Blocks: 13, 14, 19
  References: `assets/urdf/shoulder_pitch_right_only.urdf`, `assets/urdf/shoulder_pitch_weighted.urdf`, `docs/bench-weighted-700g-results.md:58` (finding: real COM ~14in), `scripts/bench-set-weighted-mass.sh`, mem0 `sdd/bench-weighted-700g-remediation/design` Workstream A
  Acceptance criteria: `pi_gravity_preview` with angles [0, 1.5708] returns tau_g ≈ 2.5 Nm for right_shoulder_pitch (was 3.14). XML comment present in both URDFs. `git diff` shows only the origin z and comment.
  QA scenarios: happy — tau_g at π/2 drops to ~2.5 Nm, within 10% of measured τ_meas. failure — tau_g still >3.0 Nm means COM not actually updated on Pi (sync issue). Evidence `.omo/evidence/task-6-gravity-comp-enhancement.txt` (gravity preview output)
  Commit: Y | fix(assets): calibrate right shoulder URDF COM to 14.2in from bench data

- [x] 7. Wrong-sign torque watchdog in Davout — config-driven sign table (GravityComp only)
  What to do: ARCHITECTURE NOTE (Metis M3): Davout does NOT know tau_g — it only sees torque_ff_nm. To check sign without breaking the crate boundary (Davout does not compute gravity) or changing proto wire types, use a CONFIG-DRIVEN SIGN TABLE. Add to `control.yaml` per joint: `wrong_sign_watchdog: { enabled: true, expected_sign_at_positive_q: -1, min_velocity_rad_s: 0.05, min_opposition_ticks: 10, grace_period_ticks: 20 }`. In `crates/davout/src/lib.rs` `filter_mit_command_at_tick` (L961-1060), AFTER computing out.torque_ff_nm (L1049): if control_mode == GravityComp AND watchdog enabled AND |dq_meas| > min_velocity: check if sign(torque_ff) opposes the expected sign for the current q_meas sign. Track opposition_ticks per joint. If sustained > min_opposition_ticks (after grace period), return `DavoutError::WrongSignWatchdog { joint }`. Add fields to Supervisor: `wrong_sign_state: HashMap<String, WrongSignState>`. M4: Apply ONLY in GravityComp mode (where torque_ff == tau_g). In Impedance/Position, torque_ff = tau_g + tau_f + tau_d — sign is composition-dependent, so disable the watchdog. Write a short ADR (0015) documenting the config-driven sign table approach and why Davout doesn't recompute tau_g.
  Must NOT do: Do NOT recompute tau_g in Davout (breaks crate boundary, lib.rs:22). Do NOT change proto wire types. Do NOT trip on zero velocity. Do NOT trip during grace period (first 20 ticks after enable). Do NOT apply to non-GravityComp modes. Do NOT trip on a single tick.
  Parallelization: Wave 2 | Blocked by: 1, 0 | Blocks: 18
  References: `crates/davout/src/lib.rs:961-1060` (filter_mit_command_at_tick), `crates/davout/src/lib.rs:166` (Supervisor), `crates/davout/src/lib.rs:22` (Davout does not compute gravity), `crates/berthier/src/loop.rs:626` (GravityComp tau_ff=tau_g), `config/bringup/shoulder_pitch_right_only/control.yaml`, `docs/decisions/` (ADR location), Metis M3/M4/D4
  Acceptance criteria: `cargo test -p davout` passes. New test: `wrong_sign_watchdog_trips_on_sustained_opposition` — 15 ticks of opposing sign, |dq|>0.05, assert DavoutError::WrongSignWatchdog. New test: `wrong_sign_watchdog_no_trip_in_impedance` — same condition but Impedance mode, assert no trip. New test: `wrong_sign_watchdog_grace_period` — 15 ticks opposition during grace, assert no trip. ADR 0015 written. `just check` passes.
  QA scenarios: happy — normal GravityComp (tau_g holds arm, dq≈0) never trips. failure — sign-flipped URDF causes sustained opposition → trip + disable within 50ms. Evidence `.omo/evidence/task-7-gravity-comp-enhancement.txt`
  Commit: Y | feat(davout): config-driven wrong-sign watchdog for GravityComp + ADR 0015

- [x] 8. Pre-flight tau_g saturation check — max(|tau_g|) vs tau_motor_max before enable
  What to do: Add a `check_gravity_saturation` method to `crates/armee-dynamics/src/lib.rs` DynamicsModel trait (default impl) or as a standalone function: `pub fn max_gravity_torque_over_range(model: &dyn DynamicsModel, joint_index: usize, range: RangeInclusive<f64>, steps: usize) -> f64`. Sample tau_g at N steps across the joint range, return max(|tau_g|). In `bins/motor-repl/src/main.rs` `enable` subcommand (L196-210) and `bins/marengo-pi/src/main.rs` `enable` stdin command (L297-308): before calling `request_enable(true)`, call the saturation check and compare against `motor_type_defaults.tau_ff_max_nm` (5.0 Nm for rs03). If max(|tau_g|) > 0.8 * tau_max, print a WARN. If > tau_max, refuse enable with error. Add `--force` flag to skip the check for diagnostics.
  Must NOT do: Do NOT block enable if tau_g < 0.8*tau_max (normal operation). Do NOT change the enable path for non-GravityComp modes. Do NOT compute tau_g over an unbounded range (use joint hard limits from motors.yaml bench.position_lower_rad/upper_rad).
  Parallelization: Wave 2 | Blocked by: — | Blocks: 19
  References: `crates/armee-dynamics/src/lib.rs:42-45` (DynamicsModel trait), `bins/motor-repl/src/main.rs:196-210` (enable), `bins/marengo-pi/src/main.rs:297-308` (enable stdin), `config/bringup/shoulder_pitch_right_only/motors.yaml:14-15` (bench limits -0.9 to 3.17), `config/bringup/shoulder_pitch_right_only/control.yaml:15` (tau_ff_max_nm: 5.0)
  Acceptance criteria: `cargo test -p armee-dynamics` passes. New test: `saturation_check_returns_max_over_range` — sample at 0, π/4, π/2, assert returns π/2 value (max for gravity). `cargo build --workspace` compiles with new --force flag. `just check` passes.
  QA scenarios: happy — enable at 700g with corrected COM: max tau_g ~2.5 Nm < 4.0 (0.8*5.0), no warning. failure — hypothetical 5kg payload: max tau_g >5.0, enable refused. Evidence `.omo/evidence/task-8-gravity-comp-enhancement.txt`
  Commit: Y | feat(armee-dynamics): pre-flight gravity saturation check before motor enable

- [x] 9. FK chain caching in UrdfGravityModel — O(n³)→O(n²)
  What to do: In `crates/armee-dynamics/src/urdf_gravity.rs`, precompute the FK chain at construction time. Add a field `link_chains: HashMap<String, Vec<usize>>` to UrdfGravityModel that maps each link name to an ordered vector of joint indices (root→leaf). In `from_urdf` (L21-37), after loading the robot, walk each link's parent chain once and store the joint index vector. In `link_transform` (L55-91), replace the `self.robot.joints.iter().find(...)` loop (O(n) linear scan per link) with a direct index lookup into the precomputed chain. This eliminates the repeated linear scans that make the current implementation O(n³).
  Must NOT do: Do NOT change the gravity_torques algorithm (still central-difference numerical gradient). Do NOT change the DynamicsModel trait. Do NOT introduce unsafe. Do NOT change DQ_EPS.
  Parallelization: Wave 2 | Blocked by: — | Blocks: —
  References: `crates/armee-dynamics/src/urdf_gravity.rs:15-91` (UrdfGravityModel struct + from_urdf + link_transform + link_com_world), `crates/armee-dynamics/src/urdf_gravity.rs:55-91` (link_transform with O(n) find loop)
  Acceptance criteria: `cargo test -p armee-dynamics` all 4 existing tests pass. New test: `cached_chain_matches_runtime_walk` — for the 4-DOF arm fixture, assert cached FK chain produces identical transforms to the old find-loop. `just check` passes.
  QA scenarios: happy — all existing gravity_torques tests pass with cached chains. failure — if cache is built wrong, bent_pose_nonzero_gravity test fails (different tau_g values). Evidence `.omo/evidence/task-9-gravity-comp-enhancement.txt`
  Commit: Y | perf(armee-dynamics): cache FK chains to reduce gravity_torques from O(n³) to O(n²)

- [x] 10. Friction-FF graded fade at Accelerate→Cruise boundary — close Phase 2 smoothness
  What to do: In `crates/berthier/src/friction.rs` `position_hold_friction` (L54-109), replace the hard scale=0 cliff at the Accelerate→Cruise boundary with a graded fade. Currently when measured dq outruns dq_traj in Cruise, friction drops to 0 abruptly (the trajectory_overspeed_fade function at L43-50 clips to 0). Add a smooth fade: instead of `(1.0 - (overspeed - deadband) / deadband).clamp(0.0, 1.0)`, use a smoother sigmoid or linear ramp that doesn't hit exactly 0 at the boundary. Concretely: extend the fade zone from `deadband` to `2*deadband` so the transition is gradual. This is the existing Workstream B from `sdd/bench-weighted-700g-remediation/design`. Add a unit test proving tau_f is continuous (no step discontinuity) at the Accelerate→Cruise transition.
  Must NOT do: Do NOT add Berthier-side tau_ff rate limiting (Davout already rate-limits at L1051-1057; source smoothing is the correct boundary). Do NOT change the friction model parameters (fc, fv, fo, k). Do NOT affect Impedance mode (it uses friction_torque at L163, not position_hold_friction).
  Parallelization: Wave 2 | Blocked by: — | Blocks: 16
  References: `crates/berthier/src/friction.rs:43-50` (trajectory_overspeed_fade), `crates/berthier/src/friction.rs:54-109` (position_hold_friction), `crates/berthier/src/friction.rs:111-138` (trajectory_velocity_friction), mem0 `sdd/bench-weighted-700g-remediation/design` Workstream B
  Acceptance criteria: `cargo test -p berthier` passes. New test: `friction_continuous_at_accelerate_to_cruise` — sample tau_f at dq values spanning the Accelerate→Cruise boundary, assert max single-step delta < 0.05 Nm (was ~0.72 Nm). Existing friction tests still pass. `just check` passes.
  QA scenarios: happy — jerk_rms drops below 800 gate (was 1295-1560) on bench replay. failure — if fade is too aggressive, friction assist is insufficient and arm doesn't reach target. Evidence `.omo/evidence/task-10-gravity-comp-enhancement.txt`
  Commit: Y | fix(berthier): graded friction fade at Accelerate→Cruise boundary for smoothness

- [x] 11. Bench test suite design document — detailed physical protocols
  What to do: Create `docs/bench-gravity-comp-test-suite.md` with detailed physical test protocols for the 700g weighted right-shoulder-pitch rig. For each test: prerequisites, step-by-step procedure, MCP tool invocations, pass/fail criteria, telemetry artifacts to collect, and safety preconditions. Tests to document: (T1) static torque sweep with corrected COM, (T2) gravity-on vs gravity-off tracking A/B, (T3) payload robustness (bare/700g/additional), (T4) friction identification sweep, (T5) mode-switch transient, (T6) wrong-sign watchdog validation, (T7) negative-retarget descent gate, (T8) recovery protocol, (T9) disable-drop behavior, (T10) full suite re-run. Each test must specify: operator safety preconditions (arm supported for first enable, E-stop reachable, confirm + confirm_weighted_motion flags), exact `pi_hold_on`/`pi_bench_harness`/`pi_gravity_preview` invocations with parameters, CSV columns to analyze (tau_g, tau_ff_cmd, tau_meas, dq_mit from position-trace CSV), candump verification, and quantitative pass thresholds.
  Must NOT do: Do NOT include sim-only tests. Do NOT test left shoulder. Do NOT require more than the existing 700g rig + one additional payload mass.
  Parallelization: Wave 3 | Blocked by: 6 | Blocks: 13-20
  References: `docs/bench-weighted-700g-results.md` (existing suite), `docs/bench-weighted-gravity-sign.md` (sign test), `docs/bench-position-tuning.md` (Layer 2 gate), `docs/bench-test-backlog.md`, `scripts/analyze-position-trace.py` (CSV analyzer), explore agent inventory (MCP tools, motor-repl, marengo-pi commands, config files)
  Acceptance criteria: Document exists with 10 test protocols, each with: prerequisites, procedure, MCP invocations, pass/fail criteria, telemetry artifacts. Reviewed by Momus for clarity/completeness.
  QA scenarios: happy — a worker can execute each test from the doc alone with no further questions. failure — Momus review finds a protocol with missing pass/fail criteria. Evidence `.omo/evidence/task-11-gravity-comp-enhancement.md`
  Commit: Y | docs(bench): gravity comp physical test suite protocols

- [x] 12. Deploy + sync to Pi — ship Waves 1-3 changes to hardware
  What to do: After Waves 1-3 are complete and `just check` passes, deploy to Pi. Run `pi_sync_main` (cross-build + deploy + install + gateway health poll). Then run `pi_sync_bench_config` with `profile: shoulder_pitch_right_only` and `install_to_opt: true` to sync the corrected URDF (Todo 6) and any control.yaml changes. Verify with `pi_health` and `pi_gravity_preview` at [0, 1.5708] to confirm corrected tau_g (~2.5 Nm) is live. Run `pi_motor_repl_status` to confirm binary is the new build. Run `pi_candump_once` to verify CAN is healthy.
  Must NOT do: Do NOT enable motors during deploy. Do NOT skip `pi_health` verification. Do NOT proceed to Wave 4 if deploy fails.
  Parallelization: Wave 3 | Blocked by: 1-10 | Blocks: 13-20
  References: `scripts/pi-remote.sh` (fallback), `tools/marengo-pi-mcp/src/tools/admin.ts` (pi_sync_main, pi_sync_bench_config, pi_wait_deploy)
  Acceptance criteria: `pi_health` returns healthy. `pi_gravity_preview` at [0, 1.5708] returns tau_g ≈ 2.5 Nm. `.deploy-rev` matches git SHA. Gateway `/health` 200.
  QA scenarios: happy — deploy succeeds, corrected tau_g live on Pi. failure — deploy fails or tau_g still 3.14 (sync issue), do NOT proceed to bench tests. Evidence `.omo/evidence/task-12-gravity-comp-enhancement.txt`
  Commit: N (deploy only, no commit)

- [ ] 13. Bench T1: Static torque sweep with corrected COM — RMS vs measured
  What to do: Run `pi_gravity_preview` at angles [0, 0.3, 0.785, 1.57, -0.3, -0.5] (6+ poses). Record tau_g for each. Then run `pi_hold_on` with `profile: weighted_single_arm`, `confirm: true`, `confirm_weighted_motion: true`, `position_rad` at each angle, `timeout_sec: 10`. For each hold, read the position-trace CSV (`pi_logs_tail` or `pi_read_file` on `position-trace-latest.csv`) and extract `tau_meas` at steady state (last 100 ticks). Compute RMS error: sqrt(mean((tau_g - tau_meas)²)). Compare against the pre-correction 27% error.
  Must NOT do: Do NOT exceed 2.0 rad (safety). Do NOT skip `confirm_weighted_motion: true`. Operator must support arm for first enable.
  Parallelization: Wave 4 | Blocked by: 6, 11, 12 | Blocks: 14
  References: `docs/bench-gravity-comp-test-suite.md` T1, `docs/bench-weighted-700g-results.md:20-29` (Phase 1 table), `pi_gravity_preview`, `pi_hold_on`
  Acceptance criteria: RMS error < 5% of max tau_g (target: <0.125 Nm with max ~2.5 Nm). Sign stable/symmetric. No faults. Evidence: bench log path + CSV tau_g/tau_meas table.
  QA scenarios: happy — RMS <5%, corrected COM fixed the over-comp. failure — RMS >10% means COM still wrong or mass incorrect. Evidence `.omo/evidence/task-13-gravity-comp-enhancement.md`
  Commit: N (bench test, save results to mem0)

- [ ] 14. Bench T2: Gravity-on vs gravity-off tracking A/B comparison
  What to do: Run the same trajectory (0→0.785→0 rad, v=2.0) twice: once with `gravity-on` (GravityComp mode via `pi_marengo_pi_script` with script ["home","enable bench","gravity-on","hold-at 0.785","hold-at 0","disable","quit"]), once with `gravity-off` (Position mode only, no gravity FF — use `impedance-on` with kp=8/kd=2 but tau_g zeroed via a temporary config or TorqueOnly with tau_ff=0). Extract steady-state error and tracking error from position-trace CSV. Compare: gravity-on should have 10x lower steady-state error than gravity-off.
  Must NOT do: Do NOT run gravity-off at elevated poses without arm support (upright-pose fall risk). Do NOT modify production code to zero tau_g — use a bench config overlay or TorqueOnly mode.
  Parallelization: Wave 4 | Blocked by: 13 | Blocks: —
  References: `docs/bench-gravity-comp-test-suite.md` T2, `pi_marengo_pi_script`, position-trace CSV columns (q, q_traj, settle_error, tau_g, tau_ff_cmd)
  Acceptance criteria: Gravity-on steady-state error < 0.02 rad. Gravity-off steady-state error > 0.1 rad (10x ratio). No faults. Evidence: both CSVs + comparison table.
  QA scenarios: happy — 10x improvement confirms gravity comp is effective. failure — <2x improvement means gravity comp isn't helping (URDF or sign issue). Evidence `.omo/evidence/task-14-gravity-comp-enhancement.md`
  Commit: N (bench test, save results to mem0)

- [ ] 15. Bench T3: Payload robustness — bare motor, 700g, additional mass
  What to do: Run T1 static torque sweep (Todo 13) with three configurations: (a) bare motor (no arm, remove weighted URDF → use `shoulder_pitch_right_only` with mass=0), (b) 700g weighted arm (current setup), (c) additional mass (add ~200g to the 700g rig, update URDF mass via `scripts/bench-set-weighted-mass.sh 0.9 right`). For each, compute RMS error vs measured. Verify gravity comp error < 5% for all three. This proves the gravity model generalizes across payloads.
  Must NOT do: Do NOT exceed motor torque limit (5.0 Nm) with additional mass — check saturation first (Todo 8). Do NOT change COM (only mass) for the additional-mass config.
  Parallelization: Wave 4 | Blocked by: 13 | Blocks: —
  References: `docs/bench-gravity-comp-test-suite.md` T3, `scripts/bench-set-weighted-mass.sh`, `pi_sync_bench_config`
  Acceptance criteria: RMS error < 5% for all 3 payloads. tau_g scales linearly with mass. No faults. Evidence: 3 sweep tables.
  QA scenarios: happy — gravity comp works across payloads. failure — error increases with mass → COM is mass-dependent (mass distribution changes). Evidence `.omo/evidence/task-15-gravity-comp-enhancement.md`
  Commit: N (bench test, save results to mem0)

- [ ] 16. Bench T4: Friction identification sweep — constant velocity, both directions
  What to do: Run constant-velocity moves at 3+ speeds (0.1, 0.5, 1.0 rad/s) in both directions (positive and negative) using `pi_marengo_pi_script` with position-hold at incremental targets. For each move, extract tau_meas and dq from position-trace CSV at steady cruise (middle 50% of move). Fit: tau_meas = fc * sign(dq) + fv * dq (after subtracting tau_g). Compute identified fc and fv. Compare against config values (fc=0.15, fv=0). Update config if identified values differ by >50%. Also verify the graded fade (Todo 10) fixed the Phase 2 smoothness failure: run `analyze-position-trace.py --gate layer2 --tau-ff-rate-limit 60` and assert jerk_rms < 800, tau_ff_slew < 120.
  Must NOT do: Do NOT identify friction at zero velocity (stiction, not Coulomb). Do NOT use Impedance mode for ID (it adds tau_f which contaminates). Use Position mode with gravity comp.
  Parallelization: Wave 4 | Blocked by: 10, 12 | Blocks: —
  References: `docs/bench-gravity-comp-test-suite.md` T4, `scripts/analyze-position-trace.py`, `docs/bench-weighted-700g-results.md:36-43` (Phase 2 Layer 2 failure), `config/bringup/shoulder_pitch_right_only/control.yaml:48-51` (friction config)
  Acceptance criteria: Identified fc within 50% of config (0.075-0.225). jerk_rms < 800 (was 1295-1560). tau_ff_slew < 120 (was 144). No faults. Evidence: friction ID table + Layer 2 gate output.
  QA scenarios: happy — friction identified, smoothness gate passes. failure — jerk still >800 → graded fade insufficient, need stronger smoothing. Evidence `.omo/evidence/task-16-gravity-comp-enhancement.md`
  Commit: N (bench test, save results to mem0; config update if needed is a separate commit)

- [ ] 17. Bench T5: Mode-switch transient test — verify kp/kd ramp
  What to do: Enable with gravity comp (gravity-on), hold at 0.5 rad. Then switch to Position mode (hold-at 0.5) via `pi_marengo_pi_script` with script ["home","enable bench","gravity-on","hold-at 0.5","status","hold-on","hold-at 0.5","status","disable","quit"]. The mode switch from GravityComp→Position should ramp kp from 0→8 over ~20 ticks. Extract from position-trace CSV: tau_ff_cmd before and after transition, kp column, dq_mit. Verify no torque step >0.5 Nm in a single tick (was potentially 0.72 Nm before ramp). Run the reverse: Position→GravityComp, verify kp ramps 8→0 smoothly.
  Must NOT do: Do NOT switch modes at elevated poses (>1.0 rad) without arm support. Do NOT switch to Disabled mid-test (that's T8).
  Parallelization: Wave 4 | Blocked by: 3, 4, 12 | Blocks: —
  References: `docs/bench-gravity-comp-test-suite.md` T5, `pi_marengo_pi_script`, position-trace CSV (kp, kd, tau_ff_cmd columns)
  Acceptance criteria: Max single-tick tau_ff_cmd delta < 0.5 Nm during transition. kp ramps over ≥10 ticks. No faults, no velocity trips. Evidence: CSV segment around transition + analysis.
  QA scenarios: happy — smooth ramp, no transient. failure — instant kp step → torque spike → velocity trip (the upright-pose fall vector). Evidence `.omo/evidence/task-17-gravity-comp-enhancement.md`
  Commit: N (bench test, save results to mem0)

- [ ] 18. Bench T6: Wrong-sign watchdog validation — trip on inverted tau_g
  What to do: This test validates the wrong-sign watchdog (Todo 7). With the arm at q≈0 and gravity-on active, temporarily invert the motor direction in motors.yaml (direction: -1 → 1) via a bench config overlay and sync. Enable with gravity-on. The watchdog should detect sign(tau_ff) opposing sign(dq) within ~50ms (10 ticks) and disable the drive with a WrongSignWatchdog error. Verify via `pi_logs_last_fault` that the watchdog message appears. Then revert the direction and verify normal operation (no trip). Also test the grace period: immediately after enable, the watchdog should NOT trip even if there's a brief dq (feedback bootstrap).
  Must NOT do: Do NOT leave the inverted config on the Pi — always revert. Do NOT test at elevated poses. Operator must support arm for the inverted-direction enable (arm may move unexpectedly). Run `pi_motor_recover` after the trip.
  Parallelization: Wave 4 | Blocked by: 7, 12 | Blocks: —
  References: `docs/bench-gravity-comp-test-suite.md` T6, `pi_logs_last_fault`, `pi_motor_recover`, `config/bringup/shoulder_pitch_right_only/motors.yaml:9` (direction: -1)
  Acceptance criteria: Watchdog trips within 50ms on inverted direction. Error message in bench log. Grace period respected (no trip in first 100ms). Normal operation (reverted direction) has no trip. Evidence: bench log + fault grep.
  QA scenarios: happy — watchdog catches wrong sign in <50ms, clean disable. failure — watchdog doesn't trip → arm runs away (velocity guard should catch as fallback). Evidence `.omo/evidence/task-18-gravity-comp-enhancement.md`
  Commit: N (bench test, save results to mem0)

- [ ] 19. Bench T7: Negative-retarget descent gate — verify no velocity trip
  What to do: After COM correction (Todo 6), run the C1 gate from the existing remediation plan: `pi_hold_on` with `profile: weighted_single_arm`, `position_rad: -0.3` (gentle negative move). Verify position-derived velocity stays < 1.5 rad/s (the C1 gate). If C1 passes, run the full negative ladder: hold-at -0.3, -0.5, -0.7, -0.85 (clamped to -0.6417 by limit envelope). Verify no velocity trip on any negative retarget. Extract max |dq| from position-trace CSV for each. Run `pi_candump_summary` after each move.
  Must NOT do: Do NOT exceed -0.9 rad (hard limit). Do NOT skip `pi_candump_summary` (CAN is wire truth). If C1 fails (velocity >1.5), do NOT proceed to the ladder — implement Workstream C2-C5 first.
  Parallelization: Wave 4 | Blocked by: 6, 12 | Blocks: —
  References: `docs/bench-gravity-comp-test-suite.md` T7, `docs/bench-weighted-700g-results.md:68` (Phase 6 negative-retarget trip), mem0 `sdd/bench-weighted-700g-remediation/design` C1 gate, `pi_hold_on`, `pi_candump_summary`
  Acceptance criteria: C1 gate: max |dq| < 1.5 rad/s on -0.3 move. Full ladder: no velocity trips, all < 2.0 rad/s. `fault=0x0000` every run. Evidence: position-trace CSV max |dq| table + candump summaries.
  QA scenarios: happy — COM correction fixed the negative-retarget trip. failure — C1 fails → negative-direction runaway persists, need descent gate (Workstream C). Evidence `.omo/evidence/task-19-gravity-comp-enhancement.md`
  Commit: N (bench test, save results to mem0)

- [ ] 20. Bench T8-T10: Recovery, disable-drop, full suite re-run + verdict
  What to do: (T8) Recovery: trigger a deliberate fault (e.g., push arm past velocity limit during gravity-on), run `pi_motor_recover`, verify RECOVER_OK, re-hold at 0. (T9) Disable-drop: hold at 0.5 rad, `pi_motor_disable`, verify arm sags to home, no torque spike, CAN freezes at last frame (expected). (T10) Full suite re-run: re-execute T1-T9 in sequence, update `docs/bench-weighted-700g-results.md` verdict from "PASS WITH WARNINGS" to "PASS" if all gates pass. Save all results to mem0 under `control/gravity-comp/bench-final-verdict`. Update `docs/bench-test-backlog.md` to mark items done.
  Must NOT do: Do NOT declare PASS if any gate fails. Do NOT skip the full suite re-run.
  Parallelization: Wave 4 | Blocked by: 13-19 | Blocks: —
  References: `docs/bench-gravity-comp-test-suite.md` T8-T10, `docs/bench-weighted-700g-results.md:75-84` (current verdict), `pi_motor_recover`, `pi_motor_disable`, `pi_logs_last_fault`
  Acceptance criteria: T8: RECOVER_OK, re-hold at 0 within 6 mrad. T9: arm sags, no spike, fault=0. T10: all T1-T9 pass, results doc updated to PASS. mem0 saved.
  QA scenarios: happy — full suite passes, verdict upgraded to PASS. failure — any test fails → verdict stays at PASS WITH WARNINGS, document the failure. Evidence `.omo/evidence/task-20-gravity-comp-enhancement.md`
  Commit: Y | docs(bench): gravity comp suite PASS — all gates green

## Final verification wave
> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.
- [x] F1. Plan compliance audit — verify every todo was executed, every acceptance criterion met, every reference file touched. Check `.omo/evidence/task-*` artifacts exist.
- [x] F2. Code quality review — `just check` (or `./scripts/check.sh`) passes clean. `cargo clippy --workspace --all-targets -- -D warnings` passes. No `unwrap()`/`expect()` in crates/. No `unsafe`. No rate-limiter clearing on mode switch (Metis C1 — must be seed, not clear). ADR 0015 exists for wrong-sign watchdog.
- [ ] F3. Real manual QA — physical bench suite (T1-T10) all PASS. `pi_candump_summary` healthy after every motion. `pi_logs_last_fault` clean. Results doc updated to PASS.
- [x] F4. Scope fidelity — verify no scope-creep: no Pinocchio/FFI, no full humanoid, no sim-only tests, no left shoulder, no Consul/Chappe/robstride changes. Verify mode isolation: run ModeIsolation tests and confirm they pass.

## Commit strategy
- Each Rust todo (1-5, 7-10) gets its own atomic commit with conventional commit format.
- Todo 6 (URDF) gets its own commit (asset-only).
- Todo 11 (test suite doc) gets its own commit (docs).
- Todo 12 (deploy) is not committed.
- Todos 13-20 (bench tests) are not committed individually — results saved to mem0.
- Todo 20 includes the final results doc update commit.
- All commits on a feature branch `gravity-comp-enhancement`, not main.
- `just check` must pass before every Rust commit.

## Success criteria
1. **Safety gate (Wave 0):** Cross-crate sign test + golden-value tests exist and pass BEFORE any tau_g change. proptest added and `cargo deny check` passes.
2. **Mode isolation proven:** PureGravityTorque newtype exists, ModeIsolation tests pass (property: perturbing tau_g does not change non-gravity FF), mode-switch ramp works. Rate-limiter is SEEDED (not cleared) on transition. Changing gravity_torques() cannot silently affect Impedance/Position modes.
3. **Gravity comp accuracy:** URDF COM corrected, RMS error <5% vs measured tau_g (was 27%). Golden-value tests updated and pass post-correction.
4. **Safety enhanced:** Wrong-sign watchdog (config-driven sign table, GravityComp only, ADR 0015) trips in <50ms. Pre-flight saturation check blocks over-limit enables. Mode-switch transients <0.5 Nm/tick. Rate-limiter seeded on transition (no unclamped step).
5. **Smoothness:** Layer 2 gate passes (jerk_rms <800, tau_ff_slew <120).
6. **Bench suite:** All 10 physical tests (T1-T10) PASS on the 700g rig. Results doc verdict upgraded to PASS.
7. **No regressions:** `just check` clean. `cargo test --workspace` passes with no hardware. All existing tests pass. MIT/Torque/Impedance/Position modes unaffected (proven by ModeIsolation tests).
8. **mem0 updated:** Findings saved under `control/gravity-comp/*`, `decision/gravity-comp/*`, `control/shoulder-pitch/gravity-comp-bench-final`.
