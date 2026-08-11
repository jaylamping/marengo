# Active MIT modes and existing acceptance gates

**Wayfinder research** for [marengo#154](https://github.com/jaylamping/marengo/issues/154) (map [marengo#150](https://github.com/jaylamping/marengo/issues/150)).

**Snapshot date:** 2026-08-11 (branch `research/active-mit-modes-acceptance-gates`, `origin/main`).

**Primary sources:** [ADR 0004](../decisions/0004-control-modes-and-mit.md), [ADR 0007](../decisions/0007-bench-position-trajectory-control.md), [ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md), [ADR 0010](../decisions/0010-actuator-velocity-cap-resolution.md), [ADR 0015](../decisions/0015-wrong-sign-watchdog.md), [CONTEXT.md](../../CONTEXT.md), [docs/tuning.md](../tuning.md), [docs/safety.md](../safety.md), per-joint `docs/bench-*.md`, `proto/marengo/v1/marengo.proto`, `crates/davout`, `crates/berthier`, `config/control.yaml`.

---

## Summary

Marengo exposes **five Berthier `ControlMode` values** on the MIT production path (`run_mode=0`); only four are used for bench motion when Davout `OperationalMode::Active`. Acceptance criteria today are **fragmented** across ADR policy, a gravity-comp numeric suite (T1–T9), position-hold Layer 2 analyzer gates, and per-DOF commissioning docs (roll/yaw/elbow/smoke) that mostly exercise **`Position` holds** with implicit `GravityComp` prerequisites. **`Impedance` and `TorqueOnly` have almost no standalone numeric gates.** Coupled multi-joint gravity, full-arm mode cycling, and limb-level sign-off are explicitly deferred or missing.

---

## 1. Active MIT control modes (inventory)

### 1.1 Wire and code enum

Protobuf and Rust agree on five motion-related modes plus boot default:

| Mode | Proto | Berthier / Davout | MIT send when `Active`? |
|------|-------|-------------------|-------------------------|
| `Disabled` | `CONTROL_MODE_DISABLED` | Default at boot | No |
| `GravityComp` | `CONTROL_MODE_GRAVITY_COMP` | `kp=0`, `kd=0`, `τ_ff=τ_g(q)` | Yes (rate-limited `τ_ff`) |
| `Impedance` | `CONTROL_MODE_IMPEDANCE` | Gains from `control.yaml` `impedance`; `τ_ff=τ_g+τ_f`; `q_des=q` | Yes (full MIT limits) |
| `Position` | `CONTROL_MODE_POSITION` | Position-hold executor (`PositionHold`); not a separate wire mode | Yes (planner + impedance gains) |
| `TorqueOnly` | `CONTROL_MODE_TORQUE_ONLY` | `kp=0`, `kd=0`; **currently aliases GravityComp** (`τ_ff=τ_g`) | Yes (torque caps only) |

Sources: [ADR 0004](../decisions/0004-control-modes-and-mit.md), `proto/marengo/v1/marengo.proto` (`ControlMode`), `crates/davout/src/lib.rs` (`ControlMode`), `crates/berthier/src/mit_feedforward.rs`.

### 1.2 Terminology (CONTEXT.md)

| Term | Meaning |
|------|---------|
| **Position hold** | Berthier motion primitive for `ControlMode::Position` (`hold-on` / `hold-at`); not a sixth mode |
| **MIT feedforward** | Packing path for `GravityComp` / `Impedance` / `TorqueOnly` via `MitFeedforward` |
| **GainRuntime** | Testing overrides + **20-tick (~100 ms) kp/kd ramp** on non-Disabled mode transitions (`gain_runtime.rs`) |
| **Joint Ready / Limb Ready / Robot Ready** | Homing/reference aggregation — prerequisite to enable, not a control mode |

Source: [CONTEXT.md](../../CONTEXT.md).

### 1.3 Firmware vs Marengo modes

All Marengo bench modes use **Robstride MIT Mode 0** (`run_mode=0`). Marengo `Position` does **not** switch drives to firmware Position mode (`run_mode=1`). Firmware Speed mode (`run_mode=2`) is diagnostic-only and gated by `control.bench.allow_firmware_speed_mode` (default `false` in master `config/control.yaml`).

Sources: [docs/tuning.md](../tuning.md), `config/control.yaml`, `crates/robstride/README.md`.

### 1.4 Mode composition (Berthier)

| Mode | Module | Key MIT fields |
|------|--------|----------------|
| `GravityComp` / `TorqueOnly` | `mit_feedforward` | `τ_ff=τ_g`, `q_des=q`, `v_des=0`, wire kp/kd from `gravity_comp` (zeros) |
| `Impedance` | `mit_feedforward` | `τ_ff=τ_g+τ_f`, `q_des=q`, `v_des=0`, wire kp/kd from `impedance` YAML |
| `Position` | `position_hold` | Trapezoid planner, lead-bounded `q_des`, `kd_mit=0`, damping via FF ([rust-patterns.md](../rust-patterns.md) §7) |
| `Disabled` | — | No batch |

Testing gain overrides apply only in `Impedance` / `Position`; cleared on enter to `GravityComp` / `TorqueOnly` / `Disabled`.

Sources: `crates/berthier/src/mit_feedforward.rs`, `crates/berthier/src/gain_runtime.rs`, [docs/rust-patterns.md](../rust-patterns.md).

### 1.5 Operator mode commands (marengo-pi / motor-repl)

| Operator phrase | Maps to |
|-----------------|---------|
| `gravity-on` | `GravityComp` |
| `gravity-off` | `TorqueOnly` (zero composed FF in scripts — see T2) |
| `hold-on` / `hold-at` | `Position` (latched setpoint) |
| `mode position` / `mode gravity` / `mode impedance` | Direct mode set (scripted in T5) |

Sources: [ADR 0004](../decisions/0004-control-modes-and-mit.md), [docs/bench-gravity-comp-test-suite.md](../bench-gravity-comp-test-suite.md) T2/T5.

---

## 2. Per-mode existing acceptance gates

Gates below are **already documented** with numeric or procedural criteria. Global preconditions (homing Verified, `fault=0`, CAN up, deploy rev match, E-stop, arm support on first elevated enable) are shared across suites — see [docs/safety.md](../safety.md) and each suite’s “Standard pre-flight.”

### 2.1 `Disabled`

| Gate | Source |
|------|--------|
| Default at boot; no MIT send | [ADR 0004](../decisions/0004-control-modes-and-mit.md) |
| `disable_on_exit: true` in master config | `config/control.yaml` |
| Clean disable: no torque spike, CAN feedback freezes ≤50 ms (T9) | [bench-gravity-comp-test-suite.md](../bench-gravity-comp-test-suite.md) T9 |
| Recovery from velocity fault: `RECOVER_OK` &lt;10 s; re-hold \|q−0\| &lt;6 mrad (T8) | same |

### 2.2 `GravityComp`

**Policy (ADR / safety — absorb as playbook prerequisites)**

| Gate | Criterion |
|------|-----------|
| Required on bench until impedance signed off | [ADR 0004](../decisions/0004-control-modes-and-mit.md) |
| No position-only holding in elevated poses | same, [docs/safety.md](../safety.md) upright incident |
| Sign pulse before full `τ_g` | [docs/safety.md](../safety.md) |
| Upright release: no free-fall | [docs/safety.md](../safety.md), [docs/bench-weighted-gravity-sign.md](../bench-weighted-gravity-sign.md) |
| `gravity-on` at q≈0: backdrivable, no runaway | [bench-weighted-gravity-sign.md](../bench-weighted-gravity-sign.md), [bench-weighted-700g-results.md](../bench-weighted-700g-results.md) |

**Numeric suite — [bench-gravity-comp-test-suite.md](../bench-gravity-comp-test-suite.md) (single-joint pitch, weighted 700 g)**

| ID | Gate | Threshold |
|----|------|-----------|
| T1 pre-flight | `max(\|τ_g\|)` over pose sweep | &lt; 5.0 Nm (motor cap) |
| T1 COM | `τ_g(π/2)` | ∈ [2.30, 2.65] Nm (700 g, 14 in COM) |
| T1 tracking | RMS `(τ_meas − τ_g)` | &lt; 5% of `max(\|τ_g\|)` |
| T1 per-pose | `\|τ_meas − τ_g\|` | &lt; 0.15 Nm each angle |
| T2 gravity-on dwell | `\|mean(q − 0.785)\|` | &lt; 0.02 rad |
| T2 gravity-off dwell | `\|mean(q − 0.785)\|` | &gt; 0.10 rad; ratio err_off/err_on &gt; 10 |
| T3 payload | RMS / per-pose / mass scaling | &lt;5% RMS; &lt;0.20 Nm residual; linear mass ±5% |
| T4 friction ID | `fc` vs config | 0.075–0.225 Nm (config 0.15) |
| T6 wrong-sign | trip latency | &lt; 50 ms after grace; GravityComp-only |
| T7 negative retarget | peak `\|dq\|` | &lt;1.5 rad/s (−0.3); &lt;2.0 rad/s ladder |
| T5 mode switch | kp ramp | 10–30 ticks; `Δτ_ff` &lt;0.5 Nm/tick; slew &lt;60 Nm/s |

**Per-DOF sign-only (elbow)**

| ID | Gate | Criterion |
|----|------|-----------|
| E6 | GravityComp sign | Modest pitch + flexed elbow, then Wave pose — supported arm, no runaway |

Source: [docs/bench-elbow-test-suite.md](../bench-elbow-test-suite.md).

**Runtime watchdog — [ADR 0015](../decisions/0015-wrong-sign-watchdog.md)**

| Parameter | Default | Note |
|-----------|---------|------|
| `min_opposition_ticks` | 10 (50 ms) | GravityComp only |
| `grace_period_ticks` | 20 (100 ms) | |
| `min_velocity_rad_s` | 0.05 | |

**Master 5-DOF config:** `wrong_sign_watchdog.enabled: false` during elbow bring-up (`config/control.yaml` comment). Playbook must not claim watchdog coverage when disabled.

**Pre-flight saturation API:** `armee_dynamics::max_gravity_torque_over_range`; enforced on enable path in `motor-repl` (`preflight_gravity_saturation`). Referenced as enhancement #7 in gravity suite.

### 2.3 `Position` (hold-at / hold-on)

**Tuning layers — [docs/bench-position-tuning.md](../bench-position-tuning.md), [ADR 0007](../decisions/0007-bench-position-trajectory-control.md)**

| Layer | Gate | Threshold |
|-------|------|-----------|
| Layer 2 (small move) | Operator smoothness | Required even if analyzer passes |
| Layer 2 | `fault` | 0 |
| Layer 2 | `lead_sat` | &lt; 10% |
| Layer 2 | `jerk_rms` | &lt; 800 rad/s² (approach) |
| Layer 2 | `τ_ff` peak slew | &lt; 2× `tau_ff_rate_limit_nm_per_s` (60 → 120 Nm/s) |
| Layer 2 | `τ_f` sign flips | ≤ 2 on approach |
| Layer 3+ distance | Staged after Layer 2 PASS | 0.3 → 0.8 → 1.57 rad (weighted pitch) |
| ADR 0007 hardware sequence | Staged limit sweep | 0→0.1→0, then −0.3, −0.85, 1.57, 3.10 — each segment clean |

**Signed-off 2-DOF baseline — [docs/bench-2dof-right-smoke.md](../bench-2dof-right-smoke.md)**

| Parameter | Value |
|-----------|-------|
| impedance kp/kd/ki | 18 / 3 / 5 |
| `position_slew_rad_s` | 0.15 |
| `position_slew_max_lead_rad` | 0.12 |
| `position_trajectory_velocity_rad_s` | 1.25 |
| `position_trajectory_accel_rad_s2` | 4.5 |
| Group `velocity_max_rad_s` | 2.5 (pitch, roll) |

| Test | Pass |
|------|------|
| D1 pitch hold 0.3 rad | No fault |
| D2 roll hold 0.785 rad | No fault |
| D3 cross-talk | Passive joint \|Δq\| &lt; 0.03 rad (30 mrad) |

**Per-DOF commissioning (Position holds, other joints at 0)**

| Suite | Hold accuracy | Other |
|-------|---------------|-------|
| Roll R3 | ±50 mrad per target | Limits 0…π |
| Yaw Y3 | ±50 mrad | ±1.57 rad envelope |
| Yaw Y4 | Pitch hold ±50 mrad of 0.3 while yaw moves | Elevated pitch |
| Elbow E4 | Stable hold 0.3 rad | Inside soft limits |

Sources: [bench-roll-test-suite.md](../bench-roll-test-suite.md), [bench-yaw-test-suite.md](../bench-yaw-test-suite.md), [bench-elbow-test-suite.md](../bench-elbow-test-suite.md).

**90° round trip (operator acceptance, weighted pitch)**

| Criterion | Notes |
|-----------|-------|
| Mechanical 90° = `hold-at 1.484` rad | [bench-90deg-calibrated-roundtrip.md](../bench-90deg-calibrated-roundtrip.md) |
| Return settle | ~0.031 rad (~1.8°) accepted visually ~1° |
| Requires `gravity-on` before outbound at q≈0 | Documented sessions |

**Mode-switch transients involving Position** — T5 in gravity suite (see §2.2).

**[docs/tuning.md](../tuning.md) operator check (no analyzer)**

| Step | Criterion |
|------|-----------|
| Small push | ~±0.1 rad; verify return without oscillation or limit fault |
| Gain sweep | Back off kp 20% from first oscillation |

### 2.4 `Impedance`

| Existing gate | Source |
|---------------|--------|
| Use only after G-comp sign test | [ADR 0004](../decisions/0004-control-modes-and-mit.md), [docs/tuning.md](../tuning.md) |
| RS03 kd up to 100; RS02/RS00 kd max 5 | [docs/tuning.md](../tuning.md) |
| kp sweep: back off 20% from first oscillation | [docs/tuning.md](../tuning.md) |
| Small push ±0.1 rad return test | [docs/tuning.md](../tuning.md) |
| Layer 4 tuning guidance (kp/kd symptoms) | [docs/bench-position-tuning.md](../bench-position-tuning.md) — **no numeric pass table** |
| Deferred in weighted results | Impedance push test not run ([bench-weighted-700g-results.md](../bench-weighted-700g-results.md)) |

**No dedicated Impedance commissioning suite or harness profile with `commissioning_criteria_met`.**

### 2.5 `TorqueOnly`

| Existing gate | Source |
|---------------|--------|
| Diagnostics / `gravity-off` contrast leg | T2 gravity-off steady-state error &gt; 0.10 rad |
| Aliases GravityComp in code today | `mit_feedforward.rs` — playbook should treat as **diagnostic**, not independent torque command path |
| Torque caps via Davout | [ADR 0004](../decisions/0004-control-modes-and-mit.md) |

**No standalone TorqueOnly numeric acceptance table.**

---

## 3. Cross-mode and Davout gates (mode-agnostic when `Active`)

These apply regardless of `ControlMode` and should be **absorbed** into every playbook chapter:

| Layer | Gate | Source |
|-------|------|--------|
| Enable FSM | Homing **Verified** → `Ready` → operator `Enable` → `Active` | [docs/safety.md](../safety.md) |
| Velocity cap | `resolve_joint_velocity_cap` from `control.yaml` only | [ADR 0010](../decisions/0010-actuator-velocity-cap-resolution.md) |
| Limit envelope | `effective_command_bounds`; measured `q` beyond hard+slack → fault | [ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md) |
| `tau_ff` rate limit | 60 Nm/s (master config) | `config/control.yaml` |
| Comm watchdog | 100 ms → `Disabled` | `config/control.yaml` `comm_watchdog_ms` |
| Danger zone | `elevated_shoulder_pitch_fall`: q&gt;0.5 and dq&lt;−0.1 → clamp descent to 0.45 rad/s | `config/control.yaml` |
| Motor-type `tau_ff_max` | rs03 5.0 Nm; rs02/rs00 3.0 Nm | `config/control.yaml` `motor_type_defaults` |
| Mode transition ramp | 20 ticks kp/kd interpolation | `crates/berthier/src/gain_runtime.rs` |
| Harness smoke | `pass_kind: "smoke"` — faults/watchdog only, **not** ±50 mrad | [bench-yaw-test-suite.md](../bench-yaw-test-suite.md), [bench-elbow-test-suite.md](../bench-elbow-test-suite.md) |

---

## 4. Gaps for full-arm / coupled commissioning

| Gap | Evidence |
|-----|----------|
| **Coupled gravity compensation** | [bench-2dof-right-smoke.md](../bench-2dof-right-smoke.md) explicitly excludes “coupled gravity preview, dual gravity-on, coordinated sky poses” |
| **Multi-joint τ_g acceptance** | T1–T3 are single-joint pitch; no numeric coupled τ_meas vs τ_g gates |
| **Impedance chapter** | No numeric gates; map #150 lists “coupled G-comp / Impedance / TorqueOnly numeric gates” as not yet specified |
| **TorqueOnly chapter** | Diagnostic only; no operator torque command gates |
| **Full Active MIT mode suite on one limb** | Map #150 spine: Reference → Limits → Sign → G-comp → Position ladder → Impedance → TorqueOnly → Payload → sign-off — only fragments exist per mode |
| **Limb-parameterized harness** | Per-DOF profiles (`roll_attached`, `yaw_attached`, `elbow_attached`, `arm_2dof_smoke`, `weighted_single_arm`) are joint-count-specific, not `right_arm` limb schema |
| **5-DOF master tree vs bringup profiles** | Commissioning docs still reference `config/bringup/arm_*` paths; master `config/` is 5-DOF SoT — playbook must unify |
| **Wrong-sign watchdog** | Disabled on master profile during mixed-direction bring-up |
| **Weighted vs unweighted** | Map calls for unweighted then standard payload; most numeric gates are weighted single-joint pitch |
| **Speed ladder / near-limit stress** | ADR 0007 staged sweep exists for pitch; no limb-wide speed ladder doc |
| **Cross-talk at full arm** | D3 is 2-DOF 30 mrad; no 5-DOF coupled cross-talk matrix |

---

## 5. Absorb vs replace (playbook migration)

### 5.1 Absorb (keep criteria, consolidate location)

| Current artifact | Playbook action |
|------------------|-----------------|
| [ADR 0004](../decisions/0004-control-modes-and-mit.md) mode table + bench policy | Canonical mode definitions and ordering |
| Standard pre-flight (health, CAN, homing, fault, deploy rev) | Single “Reference” pre-chapter |
| Sign probe pattern (R1, Y1, E3, safety sign pulse) | Unified “Sign” gate per joint |
| GravityComp T1–T3, T7 τ/velocity gates | “G-comp per-joint” + payload chapter (limb-parameterized angles) |
| T5 mode-switch ramp/slew | “Mode transition” sub-gate under full MIT suite |
| Layer 2 analyzer + operator smoothness | “Position ladder” first rung (small move) |
| ±50 mrad hold gates (roll/yaw/elbow) | Position ladder per joint |
| D3 / Y4 cross-talk thresholds | Coupled Position gates (extend to N-DOF) |
| ADR 0009/0010/0015 Davout limits, velocity, watchdog | Global safety appendix (not per-mode prose) |
| 2-DOF signed-off gains table | Instance baseline for `right_arm` DOF1–2 unless superseded |
| `analyze-position-trace.py --gate layer2` | Reuse as harness/analyzer hook |

### 5.2 Replace or supersede (structure, not necessarily delete history)

| Current artifact | Why replace |
|------------------|-------------|
| Separate `docs/bench-roll-*.md`, `bench-yaw-*.md`, `bench-elbow-*.md`, `bench-2dof-*.md` | Map #150: one limb-parameterized playbook |
| `weighted_single_arm` / `arm_3dof_right` profile sprawl | Single limb config injection (`right_arm` master tree) |
| Harness `pass_kind: "smoke"` as commissioning complete | Explicit `commissioning_criteria_met` per chapter ([bench-yaw-test-suite.md](../bench-yaw-test-suite.md) already documents the gap) |
| Per-suite duplicate pre-flight / safety contracts | One operator safety contract in playbook |
| Bringup-only config paths in suite headers | Point at master `config/` + commissioning scope |
| T6 wrong-sign as mandatory gate | Optional when `wrong_sign_watchdog.enabled: false` |
| [bench-gravity-comp-test-suite.md](../bench-gravity-comp-test-suite.md) as **the** authority | Becomes one joint/payload instance of G-comp chapter |

### 5.3 Net-new (map #150 — not in repo yet)

- Limb parameterization schema (joint list, envelope injection)
- Coupled G-comp numeric gates (multi-joint τ alignment)
- Impedance and TorqueOnly numeric acceptance tables
- Standard-payload fixture identity and critical gate subset
- Multi-speed ladder through **taught** ROM (Set Limits envelope — see [#155](https://github.com/jaylamping/marengo/issues/155))
- Limb sign-off record linking all chapters

---

## 6. Suggested playbook mode spine (from map #150, grounded in today’s gates)

| Chapter | Primary mode(s) | Existing gates to lift |
|---------|-----------------|------------------------|
| Reference | — | Pre-flight, Joint Ready, commissioning scope ([CONTEXT.md](../../CONTEXT.md)) |
| Limits confirm | — | Set Limits readback + near-limit probe (map decision) |
| Sign | `GravityComp` (pulse) | safety.md, R1/Y1/E3, T6 optional |
| G-comp per-joint | `GravityComp` | T1 thresholds, E6, weighted sign doc |
| G-comp coupled | `GravityComp` | **Gap** — only policy in safety.md |
| Position ladder | `Position` | Layer 2, ±50 mrad, D1–D3, ADR 0007 staged sweep |
| Impedance | `Impedance` | tuning.md qualitative only — **needs numeric gates** |
| TorqueOnly | `TorqueOnly` | T2 contrast only — **needs diagnostic gates** |
| Payload critical | `GravityComp` + `Position` | T3, weighted sign, optional 90° round trip |
| Limb sign-off | All | T10-style re-run + health sweep — **limb-wide variant TBD** |

---

## References (quick index)

| Topic | Path |
|-------|------|
| Mode ADR | `docs/decisions/0004-control-modes-and-mit.md` |
| Position trajectory ADR | `docs/decisions/0007-bench-position-trajectory-control.md` |
| Limit envelope ADR | `docs/decisions/0009-dynamic-position-limit-envelope.md` |
| Velocity cap ADR | `docs/decisions/0010-actuator-velocity-cap-resolution.md` |
| Wrong-sign ADR | `docs/decisions/0015-wrong-sign-watchdog.md` |
| Master control config | `config/control.yaml` |
| Gravity suite | `docs/bench-gravity-comp-test-suite.md` |
| Position tuning / Layer 2 | `docs/bench-position-tuning.md` |
| 2-DOF smoke | `docs/bench-2dof-right-smoke.md` |
| Roll / yaw / elbow suites | `docs/bench-roll-test-suite.md`, `docs/bench-yaw-test-suite.md`, `docs/bench-elbow-test-suite.md` |
| Proto enum | `proto/marengo/v1/marengo.proto` |
| MIT compose | `crates/berthier/src/mit_feedforward.rs` |
| Mode ramp | `crates/berthier/src/gain_runtime.rs` |
