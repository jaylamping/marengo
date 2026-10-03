# Phase B — WP-H: Davout policy caps & limits

Branch `audit/wp-h`, against the assigned worktree baseline.

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-armee-dynamics-12 | **NEEDS-DECISION** | Bench evidence in the lead confirms a wrong gravity model can drive pitch off-target; no independent runtime model/sign plausibility gate is present. The wrong-sign watchdog is disabled in master config. No software-only regression can validate the safe threshold. | No change; options below. |
| L-davout-15 (CS10) | **NEEDS-DECISION** | MIT output caps `tau_ff`, `kp`, and `kd` separately, not their combined torque contribution; a large position error can produce PD torque beyond the bench torque limit. | No clamp added; options below. |
| L-davout-26 (CS14) | **NEEDS-DECISION** | Master `elevated_shoulder_pitch_fall` action is `clamp_velocity`; it changes only MIT velocity, cannot brake with `kd=0`, does not bound a kp-driven target, and can clamp motion in either direction. No safe threshold/action can be inferred without tuning. | No physical tuning or behavior change; options below. |
| L-davout-16 (CS16) | **CONFIRMED — fixed** | `limit_patch_rejects_measured_position_outside_new_bounds`: previously Set Limits looked in `last_feedback_samples`, which is cleared outside Active (when patches are allowed); now the test publishes q=1.0 and verifies a proposed [0.1, 0.9] hard range is refused. | Check the current published state before installing the proposed envelope; keep no-sample behavior unchanged. |
| L-marengo-config-12 | **CONFIRMED — fixed** | `homing_loader_rejects_invalid_defaults_and_overrides`: tolerance 0.11 rad and timeout 301 s are rejected; master homing config loads. The previously accepted unbounded values are now covered alongside NaN/negative/zero cases. | Bound `zero_verify_tolerance_rad` to (0, 0.1] rad and `search_timeout_s` to (0, 300] s. |
| L-marengo-config-16 | **CONFIRMED — fixed** | `all_master_configs_load_and_unknown_keys_are_rejected` loads all four master YAML configs then rejects injected unknown keys in robot, motors, control, and homing configuration. The pre-fix reproduction accepted an unknown key silently. | Add `deny_unknown_fields` to loaded config structs; keep the intentionally narrow minimal stop-target parser separate so extra motor config does not obstruct emergency disable. |
| L-davout-13 | **CONFIRMED — fixed** | `scoped_watchdog_ignores_cached_motor_fault_on_inactive_peer`: a stale fault on an inactive shoulder peer no longer blocks a scoped elbow session. Previously the scan covered all configured motors. | Scope the cached MotorFault backstop to active target addresses. |
| L-davout-30 | **ALREADY-FIXED; test gap closed** | `mit_filter_clamps_position_into_the_live_envelope` asserts the returned MIT position is clamped to the live envelope. Inspection confirms the core returns the clamped value and retains a hard-bound postcondition; the lead was absence of an assertion, not an observed faulty clamp. | Add regression coverage; no production change needed. |
| L-davout-14 | **NEEDS-DECISION** | Existing `seed_tau_ff_rate_limiter` seeds from measured joint torque, which includes PD terms; the measured seed is used on mode entry. The lead's possible feed-forward step is plausible, but whether to seed from measured total torque or prior commanded feed-forward is a control-policy decision. | No change; options below. |
| L-davout-31 | **CONFIRMED — fixed** | `public_mit_filter_refuses_mutated_invalid_policy_before_clamping` exercises the public filter after invalid policy mutation. It now validates command/config before mutation, so the formerly reachable invalid `f64::clamp` policy fails as an error instead of panicking. | Validate the public command and current safety policy before limiter/watchdog state changes. |
| L-marengo-config-18 | **CONFIRMED; NEEDS-DECISION** | `position_trajectory_accel_rad_s2` feeds both Berthier trajectory acceleration and Davout's envelope deceleration. One field therefore couples planner feel to safety margin, while serde fallback values differ materially from per-joint master tuning. | No tuning changed; options below. |
| L-marengo-config-20 | **REFUTED** | `Supervisor::apply_limit_patch` applies the velocity update to a candidate control config, validates it, rebuilds limits, and installs the rebuilt policy atomically. The overlay's old “gated until Davout limits rebuild” wording is stale, but the actual route does rebuild. | No code change in this WP. |
| L-davout-33 | **CONFIRMED — fixed** | `mit_gain_caps_are_checked_after_joint_to_motor_gear_scaling`: with gear ratio 2, a joint-space gain is divided by 4 before comparison to motor-space maxima; an over-cap motor-space gain is rejected. | Compare kp/kd caps in motor coordinates using the squared position scale; reject invalid scale. |
| L-davout-42 | **CONFIRMED; NEEDS-DECISION** | `min_opposition_ticks` and `grace_period_ticks` are tick-based; changing loop_hz rescales elapsed watchdog time. No validation binds them to loop_hz. | No timing behavior changed; options below. |
| L-davout-18 | **CONFIRMED — deferred** | Enable reloads robot configuration/commissioning scope and `MARENGO_JOINT_SUBSET` on the control thread; the loaded target scope can therefore differ from the installed model. The relevant enable sections are outside WP-H ownership. | No edit to enable/reference sequencing. Recommend using a startup-installed immutable target snapshot. |
| L-davout-36 | **REFUTED as a safety defect; doc corrected** | `filter_mit_core` selects the greater-magnitude commanded/measured velocity and preserves its sign. A falling arm with a small opposite command therefore uses the falling measured velocity and tightens the corresponding bound; this is the conservative behavior. | Amend ADR 0009 to describe installed behavior rather than command-only design intent. |
| L-davout-37 | **REFUTED** | `restore_limit_snapshot` calls `validate_safety_config`, which calls `validate_control_against_limits`, before `build_limits`; it refuses Active before mutation. | No code change. |
| L-marengo-config-17 | **REFUTED as unsafe behavior** | Zero torque/velocity caps are permitted and pinned by validation tests; zero is a fail-closed limit, not an unsafe omission. The lead is an operator-intent question, not evidence of an unsafe cap. | Keep zero accepted; no tuning changed. |
| L-marengo-pi-09 | **CONFIRMED — outside file ownership** | Pi SocketCAN setup uses the unfiltered motors list while Davout applies the configured joint subset. The discrepancy remains; WP-H does not own `bins/marengo-pi/src/main.rs`. | No cross-package edit; route to the Pi owner. |
| L-davout-19 | **CONFIRMED, fails closed** | `stop_motors` remains immutable while `motors` is public; mutating the latter can make enable/reference gates unsatisfiable. Result is refusal, not unintended motion. | No edit; changing public API/mutation ownership is outside requested policy fixes. |

## NEEDS-DECISION

### L-armee-dynamics-12 — runtime wrong-gravity defense
- **A (recommended):** Add an independent GravityComp admission guard using a measured/model torque residual with bounded freshness and explicit refusal on missing evidence. It addresses wrong sign/mass without relying on the currently disabled watchdog, but needs a defensible threshold and weighted-arm validation before enabling motion.
- **B:** Require operator sign-test plus the MCP gravity preflight as the only gate. It preserves current runtime behavior but leaves Consul/other enable paths able to bypass preflight.
- **C:** Restrict GravityComp to an explicitly validated arm/configuration profile. This is simpler to reason about but blocks uncommissioned profiles rather than detecting model errors.
- Recommendation: A, but keep GravityComp refused until residual threshold and false-positive policy are bench-qualified.

### L-davout-15 — CS10 total torque
- **A (recommended):** Bound the combined motor-space torque contribution (kp error + kd velocity error + feed-forward) against a defined total torque envelope, with explicit saturation/fault semantics and a position-error regression. This enforces a true total bound but changes impedance response and needs supported-arm validation.
- **B:** Keep separate kp/kd/feed-forward caps. It preserves current response but does not satisfy a total-torque cap.
- Recommendation: define whether the bound is a motor rating or configured bench limit and select saturation versus refusal before implementing A; do not infer a cap from the present feed-forward-only setting.

### L-davout-26 — CS14 danger zone
- **A (recommended):** Add a fail-closed `fault/disable` action for dangerous measured states; this acts independently of MIT `kd`, but is abrupt and requires recovery semantics.
- **B:** Use `clamp_torque` with a tuned torque cap; it can oppose gravity when `kd=0`, but needs a supported weighted descent test and cannot be selected from existing values.
- **C:** Restrict `clamp_velocity` rules to modes with validated nonzero damping and reject inert rules at config load. This prevents false assurance but does not cover GravityComp or kp-driven motion.
- Recommendation: add an explicit fault/disable action for the critical zone; tune and bench-validate any torque-clamp policy separately. The current master zone must not be described as a brake.

### L-davout-14 — measured torque limiter seed
- **A (recommended pending a weighted transition trace):** Keep measured total torque as the initial seed; it best represents actual load at transition, but may include PD torque and therefore produce a first feed-forward value unlike a zero-seeded ramp.
- **B:** Seed from the last commanded feed-forward (or zero if none); this bounds the commanded FF transition but may begin far from actual applied torque.
- Recommendation: retain A until a supported mode-transition trace establishes a safer alternative; clarify that this limiter bounds FF change, not total output torque.

### L-marengo-config-18 — shared planner acceleration / envelope deceleration
- **A (recommended):** Separate the planner acceleration and envelope deceleration fields, preserving current per-joint master values during migration. This removes accidental safety-envelope changes when tuning trajectory feel.
- **B:** Keep one shared field and document that changing planner acceleration also changes envelope margins. This avoids schema expansion but preserves coupling.
- Recommendation: A; do not use the current serde defaults as replacement physical tuning.

### L-davout-42 — tick-based watchdog timing
- **A (recommended):** Express opposition/grace durations in time units and convert using elapsed monotonic time. This keeps sensitivity stable across loop rates but changes config semantics.
- **B:** Retain tick counts and require a fixed validated `loop_hz`. This is smaller but constrains runtime configuration.
- Recommendation: A, with migration preserving the current 200 Hz elapsed durations.

### L-davout-18 — Enable target reload
- **A (recommended):** Snapshot the effective commissioning scope/subset when the supervisor/model is installed and use that immutable set during Enable; require a controlled reload for changes.
- **B:** Continue reloading at Enable and rebuild all related model/config state atomically. This supports live changes but adds disk/env I/O and control-thread latency.
- Recommendation: A; owned by the enable/reference work package and not changed here.

## Cross-package edits

None. L-marengo-pi-09 remains confirmed and belongs to `bins/marengo-pi/src/main.rs`, outside WP-H file ownership.

## Gate

Focused regressions run and green:
- `cargo test -p marengo-config --test safety_validation` — 12 passed.
- `cargo test -p davout --lib scoped_watchdog_ignores_cached_motor_fault_on_inactive_peer` — passed.
- `cargo test -p davout --lib limit_patch_rejects_measured_position_outside_new_bounds` — passed.
- `cargo test -p davout --lib mit_gain_caps_are_checked_after_joint_to_motor_gear_scaling` — passed.
- `cargo test -p davout --lib public_mit_filter_refuses_mutated_invalid_policy_before_clamping` — passed.
- `cargo test -p davout --lib mit_filter_clamps_position_into_the_live_envelope` — passed.

`cargo fmt --all -- --check` — passed after applying `cargo fmt --all` to the reported formatting differences.
`cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings` — passed.
`cargo test --workspace` — 1,154 passed, 1 ignored, 0 failed (122 suites).
`just check` — not completed: the recipe requires Docker (`docker compose build dev`), and `docker` is not installed in this workstation environment.

The Pi robot/motors/control/homing YAML files were read through the read-only Pi tool and match the master config content exercised by `all_master_configs_load_and_unknown_keys_are_rejected`; no Pi software was deployed. No physical config/tuning values were changed.
