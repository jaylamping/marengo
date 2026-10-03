# Intent card: armee-kinematics

## 1. Header

| Field | Value |
|---|---|
| Crate | `armee-kinematics` |
| Path | `crates/armee-kinematics` |
| Kind | lib (no cargo features) |
| Baseline | `a2b55b3` |
| LOC src | 740 total (`lib.rs` 295, `limits.rs` 311, `expand.rs` 134). About 446 production plus about 294 inline `#[cfg(test)]` (lib.rs:195-295, limits.rs:182-311, expand.rs:71-134). No `tests/` directory |
| LOC tests | 0 integration; 17 inline unit tests, 1 `#[ignore]` (lib.rs:268) |
| Sources | `src/lib.rs` `//!` 1-20; `codemap.md`; `src/codemap.md`; `README.md`; AGENTS.md:63-66; crates/AGENTS.md:10; docs/architecture.md:42; ADR 0003 (D0 tier); ADR 0009; ADR 0017; CONTEXT.md:36,48; prior review repository-review.md:76, control.md:16,291, test-quality-plan.md:36,80,110; `git log` (20 commits; first `61fe36d` 2026-05-19; envelope `7dc010e` 2026-06-13; expand `f743825` 2026-07-22); `cargo tree -i armee-kinematics`. |

## 2. Intent

armee-kinematics is the **URDF fact reader and shared limit-envelope library**. It parses the **Master URDF** and answers "which joints are actuated, what are their hard and `safety_controller` soft bounds" for config validation, Davout and Berthier (lib.rs:1-16; crates/AGENTS.md:10; ADR 0003 D0). Its second job, added by ADR 0009 (`7dc010e`), is the single implementation of the velocity-scaled position envelope. Berthier uses it to plan (clamp hold targets, trajectory and approach speed) and Davout uses it to command and measure (clamp MIT position, measured-q fault) (ADR 0009 Decision 1-3; limits.rs). Its third job, from ADR 0017 (`f743825`), is the expand-only URDF hard-limit widening used by bench Set Limits / **Live limit patch** (expand.rs:1-9; CONTEXT.md:48). It is a deliberately leaf crate with no CAN, no control loop and no τ_g (lib.rs:3-4,9-13; codemap.md:80-82). Despite the name it is **not** an FK/IK solver; that is explicitly deferred (lib.rs:13; repository-review.md:76).

Conflicting statements:
- README.md:9 says it loads "joint limits and transforms" and is "used by planners … and sim collision checks". No transform/FK API exists. Talleyrand, its only planner dependent, is an empty crate.
- lib.rs:10 says actuated means "revolute/prismatic joints only". Code also counts Continuous (lib.rs:170-174).

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| URDF load (`urdf_rs::read_file` wrapper) | lib.rs:152-159 |
| Actuated joint enumeration | lib.rs:167-193 |
| Hard/soft bounds per joint | lib.rs:101-149; limits.rs:12-28 |
| ADR 0009 envelope math (`limit_margin_rad`, `effective_command_bounds`, clamps, `measured_position_fault`, `approach_velocity_cap`) | limits.rs:83-180 |
| ADR 0017 expand-only URDF widening | expand.rs:10-69 |
| Shared asset path helpers (`fixtures`) | lib.rs:42-89 |

| Must not | Evidence | Status |
|---|---|---|
| Control loop | architecture.md:42; lib.rs:3-4 | No violation |
| τ_g | lib.rs:11 | No violation |
| Filter commands / enable state (Davout) | lib.rs:12 | No violation, but the envelope **policy numbers** (`LimitMarginConfig::default`, limits.rs:41-52) duplicate config defaults owned by marengo-config (see Drift) |
| FK/IK | lib.rs:13 | No violation |

## 4. Interface

| Group | Items | Consumers |
|---|---|---|
| URDF load/facts | `load_urdf`, `UrdfError`, `joint_limits`, `JointLimits`, `joint_limit_bounds`, `actuated_joint_names`, `actuated_joint_count`, `joint_entry_count` | davout `lib.rs:148-149,583,2996-2997`; marengo-config `completeness.rs:5,56,137`, `urdf_merge.rs:8,87-89,388`, `urdf_expand.rs:6,25`; armee-dynamics `urdf_gravity.rs:6,37`; sim-harness tests (`actuated_joint_count`). `joint_entry_count` has **no external consumer** |
| Envelope (ADR 0009) | `JointLimitBounds`, `LimitMarginConfig`, `JointLimitPolicy`, `limit_margin_rad`, `effective_command_bounds`, `clamp_position_in_envelope`, `clamp_hold_target`, `measured_position_fault`, `approach_velocity_cap` | davout `lib.rs:75` (re-exports `JointLimitPolicy`), `lib.rs:2621,2814` (clamp), `feedback_consumer.rs:9,853` (fault), `lib.rs:3072-3085` (margin build); berthier `loop.rs:9,598-602` (hold target), `position_hold.rs:10-11,1098,1103,1229,1319`, `position_setpoint.rs:3,571-611`. `limit_margin_rad` is used only internally |
| Expand (ADR 0017) | `expand_urdf_joint_hard` | davout `limit_envelope.rs:3`; marengo-config `urdf_expand.rs:6` |
| Fixtures | `fixtures::{minimal_urdf, minimal_mjcf, production_urdf, production_mjcf, arm_4dof_urdf, arm_4dof_mjcf, arm_4dof_right_urdf, arm_4dof_right_mjcf}` | sim-harness lib.rs:8-18,55-97; armee-dynamics lib.rs:179-235; marengo-config lib.rs:1410 (`production_urdf`). `arm_4dof_mjcf` and `arm_4dof_right_mjcf` have **zero references** |

Depth:
- The envelope module is a **deep, pure** module: f64-in/f64-out with no state, consumed in five call sites across two crates. That is a good seam, since the same math is used by plan and command (ADR 0009 rationale "Davout-only clamp rejected").
- The URDF facts group is shallow pass-through over `urdf_rs` (callers still index `robot.joints` directly, e.g. lib.rs tests; davout reference model).
- `fixtures` is test-support shipped in the public production API (no `cfg(test)`/feature gate). It hard-codes `CARGO_MANIFEST_DIR/../..` (lib.rs:46-48), so the paths are only meaningful in a source checkout.
- No traits, so no adapter count.

**Metrics baseline** (`metrics/`, `a2b55b3`): crate **84.1% / 84.3% / 87.5%** line/region/function (`coverage-by-crate.md`). `limits.rs` (envelope API) **91.0%** lines; `lib.rs` **77.6%**; `expand.rs` **81.1%** (`coverage-by-file.md`). **17** inline unit tests (`test-counts.md`: `armee_kinematics`). Zero-use `pub` heuristic: `fixtures::arm_4dof_mjcf`, `fixtures::arm_4dof_right_mjcf` (`pub-usage.md`). One `#[ignore]` integration-style test (`suppressions.md`, `lib.rs:268`).

## 5. Invariants owned

Coverage behind these invariants (metrics/coverage-by-file.md:36,46,78): `limits.rs` 91.0%, `expand.rs` 81.1%, `lib.rs` 77.6%. The uncovered branches are the error and inverted-soft paths marked "untested" below. Treat them as **gaps**, not prune signals.

| Invariant | Enforcing code | Test(s) |
|---|---|---|
| Soft bounds clamped into hard | limits.rs:19-25 | indirectly via `slow_move_uses_min_margin` (limits.rs:262); no direct test of the clamp |
| Margin = min + k_v·\|v\| + k_stop·v²/(2a), deadband → v=0 | limits.rs:83-91 | `slow_move_uses_min_margin`, `fast_descent_margin_exceeds_hard_soft_gap` |
| Asymmetric envelope (only the approached bound shrinks) | limits.rs:99-104 | `asymmetric_envelope_moving_up` |
| Envelope ⊆ soft bounds | limits.rs:105-106 | `fast_descent_margin_exceeds_hard_soft_gap` |
| Envelope collapse → single point | limits.rs:107-110 | untested |
| Measured q fault at hard ± slack | limits.rs:152-155 | `measured_fault_respects_slack`; davout `lib.rs:3382` |
| Hold-at near a stop / zero-home not kinetic-clamped | limits.rs:126-149 | `hold_at_home_not_clamped_when_hard_lower_slightly_negative`, `hold_at_home_not_clamped_by_kinetic_margin`; berthier loop.rs:2872-2897,3059-3096 |
| Approach speed scaled to 0 at the wall | limits.rs:158-180 | `approach_cap_scales_near_lower_wall` (only checks 0<cap<v_max) |
| Expand never shrinks; rejects non-finite or inverted input; non-actuated rejected | expand.rs:16-38,40-48 | `never_shrinks`, `expands_lower_and_upper_only_outward`, `expands_past_current_hard`; invalid-input and non-actuated branches untested |
| Expand keeps soft inside new hard | expand.rs:50-66 | untested |
| Only actuated joints yield limits | lib.rs:110-118 | untested (non-actuated error branch) |

## 6. Inputs / outputs

- Files: any URDF path given (lib.rs:152). The fixtures point at `sim/fixtures/minimal.{urdf,xml}`, `assets/urdf/marengo.urdf`, `assets/mjcf/marengo.xml`, `assets/urdf/archive/seed-arm_4dof{,_right}/contributor.urdf` and `assets/mjcf/arm_4dof{,_right}.xml` (lib.rs:42-89).
- URDF elements read: `<joint type>`, `<limit lower upper velocity effort>`, `<safety_controller soft_lower_limit soft_upper_limit>` (lib.rs:119-124,141-145). Expand writes `<limit>`/`<safety_controller>` in memory only; persistence is the caller's (marengo-config urdf_expand.rs).
- Config: none read. `LimitMarginConfig` values are injected by Davout from `control.yaml` keys `position_limit_margin_{min_rad,k_v_s,k_stop}`, `position_trajectory_velocity_deadband_rad`, `position_limit_measured_fault_slack_rad`, `position_trajectory_accel_rad_s2` (davout lib.rs:3076-3083).
- Proto/CAN/HTTP/env: none (env only via dev-dep test `resolve_repo_root`).

## 7. Prior review reconciliation

| Prior id | Topic | Status | Evidence |
|---|---|---|---|
| test-quality-plan.md:80 | Ignored humanoid-DOF test with obsolete "placeholder" premise | **open** | Still `#[ignore = "marengo.urdf is placeholder until Brawner export…"]` at lib.rs:268; production URDF is the 5-DOF right arm (`loads_production_urdf`, lib.rs:209-216) |
| test-quality-plan.md:36,110 | `validate-urdf.sh` redundantly reruns kinematics/sim/config tests | **partial** | Removed from the primary check (implementation-roadmap.md:195-196); script still runs the cargo tests (scripts/validate-urdf.sh:7-9) and is the documented URDF validator (cad/README.md:91) |
| control.md:291 item 6 / M05 | Prismatic accepted as actuated, but dynamics omits its motion | **open** | lib.rs:170-174 still counts Prismatic; armee-dynamics urdf_gravity.rs:210 ignores it |
| repository-review.md:76 | "Not a general FK/IK solver" (scope note) | **unchanged / accurate** | lib.rs:13 |

No CS/G/T finding targets limits.rs directly (grep of control.md, gateway.md, tooling.md for `limits.rs`/envelope functions: none).

## 8. Drift

| Statement | Code reality |
|---|---|
| codemap.md:37 `JointLimits` has `contains(pos)` | No such method (lib.rs:91-98) |
| codemap.md:38 `JointLimitBounds` methods `hard_lower()` … | Methods live on `JointLimitPolicy` (limits.rs:64-80); `JointLimitBounds` has public fields only |
| codemap.md:39 policy includes "velocity cap, tau_ff max, LimitMarginConfig, danger zone rules" | No danger-zone fields (limits.rs:56-62) |
| codemap.md:40 `LimitMarginConfig` fields `kinetic_margin_rad, stop_margin_rad, …, linear_margin_rate` | Actual: `min_rad, k_v_s, k_stop, velocity_deadband_rad_s, measured_fault_slack_rad, decel_rad_s2` (limits.rs:32-39) |
| codemap.md:45 margin = linear·\|dq\| + stop | Code adds the quadratic stop-distance term (limits.rs:90; matches ADR 0009) |
| codemap.md:61-62 `fixtures::marengo_urdf()`, `invalid_urdf()`, `fixture_path(name)` | Do not exist (lib.rs:42-89) |
| codemap.md / src/codemap.md omit `expand` module | expand.rs exists (lib.rs:27) |
| codemap.md:81 "Depended upon by armee-dynamics, davout, berthier" | Also marengo-config (normal dep), sim-harness, talleyrand (`cargo tree -i`) |
| lib.rs:10 "revolute/prismatic joints only" | Includes Continuous (lib.rs:170-174) |
| README.md:9 "transforms … sim collision checks" | No transform API; no collision code |
| ADR 0009 table: slack default 0.03 | `LimitMarginConfig::default` slack 0.005, decel 0.20 (limits.rs:44-50) vs marengo-config default 0.03 (marengo-config lib.rs:563-567) |
| expand.rs:50-51 comment "expand soft outward only" | Code clamps soft **inward** to hard (expand.rs:53-60) |
| `UrdfError::Read { path, .. }` "failed to read URDF at {path}" | Reused for joint-not-found / not-actuated / invalid-envelope with `path = joint name` (lib.rs:106-117; expand.rs:17-20,27-29,35-37), which produces misleading messages |

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting touches |
|---|---|---|---|
| `fixtures::arm_4dof_mjcf`, `fixtures::arm_4dof_right_mjcf` | Zero references; **also** `pub-usage.md` zero-use heuristic; duplicate of sim-harness `arm_4dof_model_path`/`arm_4dof_right_model_path` (sim-harness lib.rs:21-28) | high | lib.rs:75-78,85-88 only |
| `joint_entry_count` | Zero references outside own test (lib.rs:204) | high | lib.rs:161-164,204 |
| `_q` parameter of `effective_command_bounds` | Unused parameter (limits.rs:94); misleads callers into thinking the envelope is position-aware | med (pub API change touches berthier position_hold.rs:1319, position_setpoint.rs:571-604 and clamp wrappers) | limits.rs:94,122; 6 call sites |
| `impl Default for LimitMarginConfig` | Duplicate implementation of config defaults with **different values** (limits.rs:41-52 vs marengo-config lib.rs:551-567). Only use is davout lib.rs:3084 `None` arm, which is unreachable at construction because coverage is validated first (davout lib.rs:570) | low (unverified for the live rebuild paths davout limit_envelope.rs:59,112; lib.rs:743) | limits.rs:41-52; davout lib.rs:3084 |
| `humanoid_urdf_actuated_joints_match_robot_config` (ignored) | Superseded: obsolete premise per test-quality-plan.md:80; `config/robot_humanoid.yaml` future scope | med | lib.rs:267-294; dev-deps `marengo-config`, `serde_yaml` (Cargo.toml:17-19) become removable |
| `fixtures` module in the public API | Scaffold/test-support in a production surface | low (move behind a `test-support` feature rather than delete; 3 crates use it) | lib.rs:42-89; sim-harness; armee-dynamics tests; marengo-config test |

Not prunable: `measured_position_fault`, the clamps and `approach_velocity_cap` are safety envelope code (ADR 0009). Their untested branches are listed as leads.

## 10. Phase-B leads

Metrics cross-check: metrics/pub-usage.md:11-12 independently flags `arm_4dof_mjcf` and `arm_4dof_right_mjcf` as zero-use. `joint_entry_count` is not in that list because the heuristic counts its own test (lib.rs:204).

| # | Location | Why suspicious |
|---|---|---|
| K1 | limits.rs:24-25 via lib.rs:146 | `f64::clamp(min,max)` **panics** if min > max or either is NaN. A URDF joint with `lower > upper` or a NaN limit panics inside `joint_limit_bounds`, which Davout calls (lib.rs:2997) *before* its own overlap check (lib.rs:2998-3003). This is a library panic in the Supervisor constructor path |
| K2 | limits.rs:138-142 | `lower_gate = max(hard_lower+0.005, 0.005)`. For any joint whose hard_lower is well below 0 (right pitch −0.9, ADR 0009 table), **every** hold target ≤ 0.005 rad approached from above bypasses the kinetic envelope, not just targets near the stop or at zero-home. The tests only pin hard_lower ∈ {0, −0.05} (limits.rs:211-259) |
| K3 | limits.rs:107-110 | Envelope collapse returns the soft-range **midpoint**, not something near q. A clamp then commands a possibly large jump toward mid-range. Untested |
| K4 | limits.rs:158-180 vs 105-106 | `approach_velocity_cap` measures distance to **hard** walls, while `effective_command_bounds` clips to **soft**. Inconsistent walls, so the speed cap may not taper before the soft clamp |
| K5 | limits.rs:123,152-155 | NaN propagation: `clamp(NaN)` returns NaN, and `measured_position_fault(NaN)` returns false (fail-open). Davout's post-clamp hard check `pos < lo \|\| pos > hi` is also false for NaN (davout lib.rs:2815). Finite-ness must be guaranteed upstream; verify where |
| K6 | lib.rs:119-124 | Continuous joints are "actuated" but return URDF `<limit>` values, often 0/0 for continuous. Davout fails closed (overlap check), but marengo-config completeness/merge may not |
| K7 | limits.rs:41-52 vs marengo-config lib.rs:563-567 | Two default tables (slack 0.005 vs 0.03; decel 0.20 vs `position_trajectory_accel_rad_s2`). Whichever path falls back to `Default` gets a 6× tighter measured-fault slack |
| K8 | lib.rs:128-149 | `joint_limit_bounds` does two linear lookups (via `joint_limits`) and reads soft from URDF only. Davout then overwrites soft from control.yaml (davout lib.rs:3005-3012), so URDF soft is a fallback whose precedence is not documented here |
| K9 | expand.rs:61-65 | If soft becomes inverted, it is reset to the full hard range, erasing operator soft bounds silently (`changed=true` only) |
| K10 | limits.rs §5 untested rows vs `coverage-by-file.md` | Envelope functions are **~91%** line-covered, but collapse/midpoint (`limits.rs:107-110`), non-actuated errors, and expand soft-clamp branches lack dedicated tests — **gap**, not evidence to prune envelope API |
