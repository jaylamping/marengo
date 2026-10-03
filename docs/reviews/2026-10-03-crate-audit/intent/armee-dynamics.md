# Intent card: armee-dynamics

## 1. Header

| Field | Value |
|---|---|
| Crate | `armee-dynamics` |
| Path | `crates/armee-dynamics` |
| Kind | lib (no cargo features) |
| Baseline | `a2b55b3` (branch `audit/2026-10-03`) |
| LOC src | 1514 (`lib.rs` 336 incl. ~162 unit-test lines; `urdf_gravity.rs` 259; `calibration.rs` 919) |
| LOC tests | 556 (`analytic_gravity.rs` 79, `archived_arm_geometry.rs` 39, `gravity_calibration.rs` 438) + 61 fixture URDF lines |
| Sources | `src/lib.rs` `//!` 1-61; `codemap.md`; `src/codemap.md`; `README.md`; AGENTS.md:47-66; crates/AGENTS.md:11,35,43; docs/architecture.md:41; ADR 0005; ADR 0007:12; ADR 0015 (via config/control.yaml:214-217); CONTEXT.md:15,27,30; prior review control.md CS21/CS23 + item 6 (control.md:291), finding-index.md:40,42,139-142, ledger CS21/CS23/T27/T28/T30/M04/M05/M06/M07, test-quality-plan.md:7,54; `git log -- crates/armee-dynamics` (15 commits, first `5d44f5c` 2026-05-19 "adding gravity compensation logic"; CS23 repair `2f50de4` 2026-09-29; calibration `1fcb54c` 2026-10-03); `cargo tree -i armee-dynamics`. |

## 2. Intent

armee-dynamics exists to compute the **joint-space gravity holding torque τ_g(q)** that Berthier feeds forward under `GravityComp`, `Impedance` and Position hold. It does this in pure Rust, with no FFI and no `unsafe`, so it can run in the 200 Hz Pi tick and in CI (ADR 0005 Context/Decision; lib.rs:1-4; AGENTS.md:57-60). The method is a fixed-base virtual-work gradient: τ_i = ∂P/∂q_i with P = Σ m·9.81·z_COM. It is computed numerically from URDF link masses and COMs, with g = [0,0,-9.81] (lib.rs:23-35; urdf_gravity.rs:12-13,241-258). The crate deliberately owns only the gravity term. The `PureGravityTorque` newtype marks the output so that friction, damping and payload terms stay in Berthier's **MIT feedforward** composition (lib.rs:49-55; CONTEXT.md:15). Since `1fcb54c` it also owns **bench gravity calibration**: a MAP fit of per-link mass scale and principal-axis COM offset to friction-cancelled bench holding torques, which refuses unidentifiable, implausible or high-residual fits (calibration.rs:1-34). This serves the limb playbook "Gravity calibration after hardware changes" (docs/commissioning/limb-playbook.md:153) and **Payload critical gates** (CONTEXT.md:30).

Conflicting statements:
- ADR 0005 Decision 1 types the trait as `Result<Vec<f64>>`. The code returns `PureGravityTorque` (lib.rs:126).
- ADR 0005 Decision 3 promises a D1 MuJoCo τ_g cross-check script on `arm_4dof.urdf`. None exists: `sim/scripts/` holds only `smoke_test.py`.
- Some sources scope the crate to "`gravity_torques(q)` only" (crates/AGENTS.md:11; docs/rust-patterns.md:23). That scope predates the calibration module, which is now a second, offline responsibility (calibration.rs). [INFERENCE] The scope statements were not updated when `1fcb54c` landed.

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| τ_g(q) in joint space, in robot.yaml joint order | lib.rs:8,123-127; urdf_gravity.rs:163-176 |
| URDF → gravity model construction (joint-name validation, link-chain cache) | urdf_gravity.rs:33-69 |
| Drive-saturation pre-check helper `max_gravity_torque_over_range` | lib.rs:137-172 |
| Inertial calibration math (fit, identifiability, refusal verdicts, friction cancel) | calibration.rs:298-919 |

| Must not | Evidence | Status |
|---|---|---|
| CAN, commands, encoders | AGENTS.md:53; lib.rs:15; architecture.md:41 | No violation: deps are `armee-kinematics`, `nalgebra`, `thiserror`, `urdf-rs` only (Cargo.toml:10-14) |
| Safety policy, motor-space transform (Davout owns `direction`/`gear_ratio`) | lib.rs:3-4,37-47; crates/AGENTS.md:43-44 | No violation |
| Coriolis, mass matrix, contact, FK/IK for frames (needs ADR) | lib.rs:11-14 | No violation |
| File I/O beyond URDF load | codemap.md:85 says "open files (URDF loaded externally)" | **Drift**: `from_urdf` loads the file itself via `armee_kinematics::load_urdf` (urdf_gravity.rs:37) |

## 4. Interface

| Group | Items | Consumers |
|---|---|---|
| Gravity trait + output | `DynamicsModel` (lib.rs:124), `PureGravityTorque` (lib.rs:95, **pub field**), `DynamicsError` (lib.rs:73) | berthier `loop.rs:8,42,1141,1540-1541`; log-cli `gravity_fit.rs:30,839`; tests |
| Model | `UrdfGravityModel::from_urdf` (urdf_gravity.rs:33), `gravity_model_from_urdf` (lib.rs:130), `LinkInertial`, `link_inertial`, `with_link_inertial`, `links_downstream_of`, `point_mass_torques` (urdf_gravity.rs:72-151) | berthier `loop.rs:102,433`; log-cli `gravity_fit.rs:256,703,724,837`; log-cli tests `gravity_fit_cli.rs:37,43` |
| Saturation check | `max_gravity_torque_over_range` (lib.rs:146) | marengo-pi `main.rs:834`; motor-repl `main.rs:49` |
| Calibration | `InertialParam`(+`FromStr`), `GravitySample`, `JointWindow`, `FitOptions`, `CalibrationError`, `FitVerdict`, `GravityFit`, `assess_identifiability`, `default_params`, `fit_gravity_params`, `cancel_friction` (calibration.rs) | log-cli `gravity_fit.rs:26-30,265,280,670` only; `assess_identifiability` only in its own tests |

Depth:
- **Deep:** `gravity_torques`. A tiny surface (q → τ) hides URDF chain walking, the transforms and the numerical gradient. `fit_gravity_params` is also deep: one call returns the fit, identifiability, residuals and verdict.
- **Shallow:** `PureGravityTorque` exposes `pub Vec<f64>` (lib.rs:95), so any caller can construct one. The "type-level enforcement" claim (lib.rs:51-55,86-88) is a naming convention, not an invariant, as codemap.md:40 itself admits.
- **Seam count:** `DynamicsModel` has **one adapter** (`UrdfGravityModel`). Berthier stores the concrete type (`loop.rs:102`). The trait is used as `&dyn` only by `max_gravity_torque_over_range` (lib.rs:147). It is a hypothetical seam: no test double or second model exists.

**Metrics baseline** (`metrics/`, baseline `a2b55b3`): crate **90.3% / 91.9% / 91.3%** line/region/function (`coverage-by-crate.md`). Interface-bearing src: `calibration.rs` **89.2%**, `urdf_gravity.rs` **92.0%**, `lib.rs` **92.3%** lines (`coverage-by-file.md`). **9** listed tests on `armee_dynamics` binary (`test-counts.md`). No `pub` symbols from this crate appear in the zero-use heuristic list (`pub-usage.md`; macro/re-export false positives possible).

## 5. Invariants owned

| Invariant | Enforcing code | Test that fails if broken |
|---|---|---|
| Holding-torque sign/magnitude equals analytic dP/dq | urdf_gravity.rs:241-258 | `analytic_gravity.rs::pendulum_holding_torque_has_analytic_sign_and_magnitude` |
| COM transformed as a point (upstream joint-origin translations kept, CS23) | urdf_gravity.rs:188-191 | `analytic_gravity.rs::two_link_chain_accounts_for_distal_load_and_joint_coupling`; `archived_arm_geometry.rs` |
| Joint origin rotation respected | urdf_gravity.rs:204,228-234 | `analytic_gravity.rs::rotated_joint_origin_changes_the_gravity_reference` |
| Output order = configured joint order | urdf_gravity.rs:163-176 | `analytic_gravity.rs::configured_joint_order_defines_input_and_output_order` |
| `q.len()` must equal joint count | urdf_gravity.rs:164-169 | untested directly (exercised only via calibration `SampleDims` paths) |
| Configured joint names exist in the URDF | urdf_gravity.rs:38-44 | untested |
| Saturation scan rejects an out-of-range index | lib.rs:154-159 | `lib.rs::saturation_check_rejects_out_of_range_joint` |
| `point_mass_torques` × mass equals the link's τ_g contribution | urdf_gravity.rs:129-151 | `gravity_calibration.rs::point_mass_torques_reproduce_link_contribution` |
| Calibration refuses unidentifiable sets (σ_min, condition) | calibration.rs:838-860 | `gravity_calibration.rs::refuses_parameters_the_sweep_cannot_separate` |
| Calibration refuses high residual | calibration.rs:885-895 | `refuses_when_the_fit_cannot_explain_the_torques` |
| Calibration refuses poses outside soft∩hard + slack | calibration.rs:619-631 | `refuses_poses_outside_the_soft_window` |
| Calibration refuses implausible mass scale/COM | calibration.rs:871-884 | untested ([INFERENCE] from the test list, gravity_calibration.rs:176-416) |
| Non-finite samples rejected | calibration.rs:614-618 | untested |
| Wrong-sign τ_g detection at runtime | delegated to Davout watchdog (lib.rs:59-61) | Watchdog **disabled** in config (config/control.yaml:216-217). Gap, not owned here |

## 6. Inputs / outputs

- Files: the URDF path is supplied by the caller (berthier passes `resolve_urdf_path`, loop.rs:432-433; log-cli passes the Pi URDF, gravity_fit.rs:256). URDF `<inertial>` mass/origin and joint origin/axis/type are read (urdf_gravity.rs:74-77,181-187,203-221).
- Config: none read directly. Its tests read `config/robot.yaml`, `control.yaml` and `motors.yaml` through marengo-config, a dev-dep (gravity_calibration.rs:30-35; Cargo.toml:19-20).
- Proto/Chappe/CAN/HTTP/env: none.
- Output: `PureGravityTorque` (Nm, joint space), the `GravityFit` report, and `f64` max |τ_g|.

## 7. Prior review reconciliation

| Prior id | Topic | Current status | Evidence |
|---|---|---|---|
| CS23 (P1) | COM drops joint-origin translation | **fixed** | `2f50de4` / PR213; urdf_gravity.rs:188-191 uses `transform_point`; pinned by `analytic_gravity.rs` two-link test; ledger CS23 "verified" |
| CS21 (P2) | Ignored stale dynamics goldens | **partial: goldens fixed, production parity open** | `golden_tau_g.rs` and `link_chains_built_correctly` deleted in `2f50de4` (`git log --diff-filter=D`); no `#[ignore]` remains in the crate (grep). Production-model/independent acceptance still open (ledger CS21 "partial"; codemap.md:86) |
| control.md:291 item 6 / M05 | Prismatic/mimic/floating base silently unsupported; malformed models | **open** | link_transform still rotates only Revolute/Continuous (urdf_gravity.rs:210) and defaults absent joints to 0 (urdf_gravity.rs:209); no cycle/zero-axis/mass validation (urdf_gravity.rs:33-69); ledger M05 open |
| T30 (P2) | Partial gravity-preview vector → zeros | **open** (consumer side) | motor-repl `main.rs:436-448` still substitutes `vec![0.0; joint_count]` when too few args |
| T27 / T28 | Sim does not validate production gravity; MJCF lacks inertials | **open** | `assets/mjcf/marengo.xml` has 0 `<inertial>` (grep count); see sim-harness card |
| M04 | Wrong-sign policy coordinate | **open** | lib.rs:59-61 still points at the watchdog; config disables it (control.yaml:216-217) |
| M06 | Bound realtime work | **partial** (no change here) | gravity_torques allocates per call (see lead L1) |
| M07 | Model provenance / scope honesty | **open** | No provenance check in crate |

## 8. Drift

| Statement | Code reality |
|---|---|
| codemap.md:84 "Called by: berthier, motor-repl, tests" | Also marengo-pi (`main.rs:34,834`) and marengo-log-cli (`gravity_fit.rs`). Motor-repl uses `max_gravity_torque_over_range` and goes through Berthier `preview_gravity_torques` (main.rs:449), not the crate directly for preview |
| codemap.md:85 "Does not … open files" | `from_urdf` reads the URDF (urdf_gravity.rs:37) |
| lib.rs:51-55 "newtype enforces … at the type level" | Field is `pub` (lib.rs:95); nothing prevents construction |
| lib.rs:59-61 "rely on the Davout wrong-sign watchdog" | `wrong_sign_watchdog.enabled: false` (config/control.yaml:216-217) |
| ADR 0005 Decision 1 `Result<Vec<f64>>` | `Result<PureGravityTorque, DynamicsError>` (lib.rs:126) |
| ADR 0005 Decision 3 D1 MuJoCo τ_g comparison | No script (`sim/scripts/` = `smoke_test.py` only) |
| crates/AGENTS.md:11 "`gravity_torques(q)` only" | Crate also ships the 919-line calibration module |
| Cargo.toml:12 `nalgebra = "0.33"` direct | crates/AGENTS.md CONVENTIONS: deps in `[workspace.dependencies]` |

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting touches |
|---|---|---|---|
| `DynamicsModel` trait (fold into inherent methods) | Scaffold with no consumer (one adapter; Berthier holds the concrete type, loop.rs:102) | low (ADR 0005 Decision 1 names the trait; keep unless the ADR is amended) | lib.rs:124-127,147; urdf_gravity.rs:236; `use DynamicsModel` in berthier loop.rs:8, log-cli, 3 test files |
| `PureGravityTorque::as_slice` / `Index` impl | Duplicate implementation (`Deref<Target=[f64]>` already provides both, lib.rs:116-121) | med | lib.rs:98-114; callers `as_slice` at lib.rs:167 |
| `assess_identifiability` as **pub** | Zero external references (only gravity_calibration.rs and internal `default_params`); not flagged zero-use in `pub-usage.md` (name may appear in tests) | low (tests use it as an oracle; could stay `pub(crate)` + test-only) | calibration.rs:301; gravity_calibration.rs |
| In-crate unit tests on archived models (`bent_pose_nonzero_gravity`, `elevated_elbow_changes_elbow_torque`, `arm_4dof_right_elbow_torque_changes_with_pose`, `weighted_bench_right_heavier_at_pitch`) | Superseded by analytic oracles in `2f50de4` (test-quality-plan.md:54); assertions are weak (e.g. lib.rs:242-247 passes if abs-sums differ at all) | med | lib.rs:181-272; keep the saturation tests (lib.rs:281-335) |

Not prunable: `max_gravity_torque_over_range` is the enable-time saturation guard (marengo-pi main.rs:834). The calibration refusal paths are safety-relevant.

## 10. Phase-B leads

Metrics cross-check: metrics/pub-usage.md lists no zero-use items for this crate. Its heuristic scans only `crates/*/src`, so it does not see the bin consumers (log-cli, marengo-pi, motor-repl).

| # | Location | Why suspicious |
|---|---|---|
| L1 | urdf_gravity.rs:163-176,196-200,241-258 | Hot path at 200 Hz (loop.rs:1141; AGENTS.md:56). Each call clones every joint name into `q_map`, then runs 2n perturbations × L links. Each `link_transform` allocates a `Vec<&Joint>` and does a linear string `find` per chain joint (205-209). This is avoidable allocation and O(n²·L) string compares in a realtime tick (ledger M06 partial) |
| L2 | urdf_gravity.rs:209 | Joints not in `joint_names` are silently evaluated at q=0. If robot.yaml omits a URDF actuated joint, τ_g is wrong without error (control.md:291; M05) |
| L3 | urdf_gravity.rs:210 | Prismatic/mimic/floating joints are silently treated as fixed. No construction-time rejection (M05) |
| L4 | urdf_gravity.rs:52-59 | Chain walk has no cycle guard: a cyclic URDF hangs `from_urdf` (M05 acceptance "no hang") |
| L5 | urdf_gravity.rs:217 | `Unit::new_normalize` of a zero axis yields NaN torques. Construction does not validate axes (M05) |
| L6 | urdf_gravity.rs:38-44 | Only existence is checked. A fixed joint or a duplicated name in `joint_names` is accepted; τ for those entries is silently 0 (duplicate: `find` at 205-208 returns the first entry only) |
| L7 | marengo-pi main.rs:834; motor-repl main.rs:49 | `max_gravity_torque_over_range(..).unwrap_or(0.0)`: a model error reports 0 Nm and **passes** the saturation gate (fail-open). Consumer-side, but the API invites it |
| L8 | lib.rs:160-168 | The saturation scan holds all other joints at 0 (doc lib.rs:139-140). For coupled chains (elbow loaded by pitch) the worst case can sit at non-zero other joints, so this under-estimates the max |
| L9 | calibration.rs:481-493 | Gauss–Newton: on LU failure it `break`s silently (486-488), and hitting `MAX_ITERATIONS` is not reported. The verdict is computed on a possibly unconverged θ with no "not converged" refusal |
| L10 | calibration.rs:494-505 | Posterior σ comes from `normal` of the last iteration (pre-step), and is NaN-filled when singular. `sigma.max(0.0)` turns NaN into 0, so a singular posterior reports σ=0 (overconfident) |
| L11 | calibration.rs:913-919 | `cancel_friction` `zip`s silently: mismatched lengths truncate without error |
| L12 | calibration.rs:619-631 | Windows do not check `lower ≤ upper` or finiteness. A NaN window bound passes every sample |
| L13 | tests/gravity_calibration.rs:1-3,30-35 | The calibration oracle is "independent" only for the inertial path. Truth and fit share `link_transform`, so a CS23-class FK bug cancels out. Tests also bind to the live master URDF and config, which are mutable (test-quality-plan.md:54 asks for immutable fixtures) |
| L14 | lib.rs:95 | Public constructor of `PureGravityTorque` defeats the "pure gravity" contract that Berthier relies on (lib.rs:51-55) |
| L15 | §5 invariants vs `coverage-by-file.md` | Crate line coverage is high overall, but §5 rows marked **untested** (joint-name validation, implausible-mass refusal, non-finite samples) are coverage **gaps** on safety-adjacent refusal paths — not prune signals (`metrics/README.md` headline: low coverage on safety code = gap) |
