# Research: v1 high-impact URDF fields for sim fidelity

**Ticket:** [Research: v1 high-impact URDF fields for sim fidelity](https://github.com/jaylamping/marengo/issues/99)  
**Parent map:** [Wayfinder: Consul Config/Setup — URDF SoT completeness gate](https://github.com/jaylamping/marengo/issues/96)  
**Date:** 2026-08-09  
**Scope:** Research only — defines the initial warn-only checklist for the Config/Setup URDF completeness gate. Does not implement Consul UI.

---

## Summary

Marengo’s runtime reads a **narrow slice** of URDF today. The highest-impact gaps for **safety-relevant accuracy** are link **mass + COM**, **kinematic chain geometry** (joint origins and axes), and **hard position limits** on actuated joints — these feed `armee-dynamics` gravity feedforward and Davout’s limit envelope. Secondary but still v1-worthy checks cover **config↔URDF joint coverage**, **effort caps**, and **placeholder mass detection**.

MuJoCo sim uses **hand-maintained MJCF** paired with URDF (`sim-harness` DOF parity tests); collision meshes, full inertia tensors, and URDF friction/material tags are **not consumed** by the live control stack and are deferred. Consul URDF preview uses **visual primitives only** (box/cylinder/sphere) — no mesh loading yet.

Per map #96 Notes: the gate **warns/informs only**; it never hard-blocks the operator.

---

## Prioritized v1 checklist (warn-only)

| Priority | Check ID | Warn when | Default severity |
|----------|----------|-----------|------------------|
| P0 | `inertial-mass-com` | Any link on the actuated kinematic chain has missing `<inertial>`, `mass ≤ 0`, or COM at link origin with mass above a stub threshold | **critical** |
| P0 | `joint-kinematics` | Any actuated joint missing `<origin>` or `<axis>`, zero-length axis, or axis/origin inconsistent with `hardware/docs/kinematics.md` / profile expectations | **critical** |
| P0 | `hard-position-limits` | Any revolute/prismatic joint in `robot.yaml` lacks `<limit lower upper>` or hard bounds are placeholder-wide (e.g. ±π factory defaults on bench joints) | **critical** |
| P0 | `config-joint-coverage` | A joint listed in profile `robot.yaml` / `motors.yaml` is absent from URDF (today checked loosely via string match in `joint_in_profile_urdf`) | **critical** |
| P1 | `urdf-motors-limit-overlap` | URDF hard `[lower, upper]` and `motors.yaml` bench envelope do not overlap (Davout `build_limits` would fail at init) | **high** |
| P1 | `effort-limit-present` | Actuated joint `<limit effort="…">` missing or zero while motors/robot bench expect non-zero torque cap | **high** |
| P1 | `soft-limit-source` | No `<safety_controller>` soft bounds and no corresponding `control.yaml` soft inset for a joint that has asymmetric hard limits | **medium** |
| P1 | `placeholder-provenance` | URDF header or link comments indicate PROVISIONAL/ballpark/placeholder, or all chain masses are round stub values (0.001, 0.3, 0.5, …) | **medium** |
| P2 | `mjcf-dof-parity` | Paired MJCF exists but actuated DOF count or named hinge axes differ from URDF (`sim-harness` tests) | **medium** |
| P2 | `visual-geometry` | Link on actuated chain has no `<visual>` (Consul preview renders empty; operator cannot visually verify pose) | **low** |
| P2 | `link-length-sanity` | Sum of segment lengths implied by joint origins deviates strongly from `kinematics.md` link dimensions (FK/gravity lever arms wrong) | **medium** |

Severity labels are suggestions for Consul copy/styling; all remain non-blocking.

---

## Why each item (Marengo consumer)

### P0 — safety-critical path

#### `inertial-mass-com`

- **Consumer:** `armee-dynamics` [`UrdfGravityModel`](../../crates/armee-dynamics/src/urdf_gravity.rs) → Berthier `ControlLoop` gravity FF (`crates/berthier/src/loop.rs`).
- **Fields read:** `<link><inertial><mass value="…">`, `<origin xyz="…">` (COM in link frame). Inertia tensor **is not used** for τ_g.
- **Behavior:** Links with `mass ≤ 0` are **silently skipped** — gravity torque is under-estimated with no runtime error.
- **Why here:** Wrong COM/mass is a documented **safety issue** ([ADR 0005](../../docs/decisions/0005-dynamics-library.md), [docs/bench-gravity-comp-test-suite.md](../../docs/bench-gravity-comp-test-suite.md)): incorrect τ_g causes arm fall in GravityComp/Position hold; Davout wrong-sign watchdog is a backstop, not a substitute for correct URDF mass properties.
- **Placeholder signal:** `assets/urdf/arm_4dof_right.urdf` header: *"PROVISIONAL: masses/COM/link lengths are ballpark until CAD export."*

#### `joint-kinematics`

- **Consumers:** `armee-dynamics` FK (`link_transform`, joint `origin` + `axis` + type), Consul FK viz ([`consul/src/urdf/forward-kinematics.ts`](../../consul/src/urdf/forward-kinematics.ts), [`urdf-scene.tsx`](../../consul/src/components/dashboard/urdf-preview/urdf-scene.tsx)), `sim-harness` elbow-axis regression tests.
- **Fields read:** `<joint><origin xyz rpy>`, `<axis xyz>`, `type`, `parent`/`child`.
- **Why here:** Virtual-work gravity depends on COM world positions; wrong axis or link offset rotates/translates τ_g incorrectly. Bench tests assert elbow axis +Y ([`sim-harness/src/lib.rs`](../../crates/sim-harness/src/lib.rs)).

#### `hard-position-limits`

- **Consumers:** Davout [`build_limits`](../../crates/davout/src/lib.rs) via `armee-kinematics::joint_limits` / `joint_limit_bounds`; homing verify receives limits derived from this policy (`crates/marengo-homing/src/verify.rs`).
- **Fields read:** `<limit lower upper …>`, optional `<safety_controller soft_*>` (soft defaults to hard when absent).
- **Behavior:** Effective hard envelope = **URDF hard ∩ `motors.yaml` bench**; non-overlap fails supervisor init. Bench Set Limits may **expand-only** widen URDF hard ([ADR 0017](../../docs/decisions/0017-bench-set-limits-urdf-expand.md)).
- **Why here:** Limits define Davout clamping, measured-position fault, and approach velocity cap ([`armee-kinematics/src/limits.rs`](../../crates/armee-kinematics/src/limits.rs)). Placeholder ±π limits on a real joint are a safety envelope lie.
- **Not URDF for velocity:** Command velocity caps resolve from `control.yaml` only ([docs/safety.md](../../docs/safety.md), ADR 0010) — do **not** warn on URDF `<limit velocity>` for v1.

#### `config-joint-coverage`

- **Consumers:** `marengo-config::joint_in_profile_urdf`, gateway profile add-joint paths (`bins/marengo-gateway/src/profiles.rs`); future Config/Setup chain view.
- **Why here:** Runtime loads URDF path from `robot.yaml` (`marengo-config::resolve_urdf_path`). A motor row for a missing URDF joint breaks limit build or gravity model init (`UnknownJoint`).

### P1 — high impact (sim + partial safety)

#### `urdf-motors-limit-overlap`

- **Consumer:** Davout `build_limits` overlap check (same path as hard limits).
- **Why here:** Prevents “config looks fine but Enable fails” when bench teaches wider range than URDF allows — the failure mode ADR 0017 addressed for live Set Limits.

#### `effort-limit-present`

- **Consumer:** Davout `build_limits` sets `JointLimitPolicy.effort` and `tau_ff_max` as `min(URDF effort, motor bench torque, robot bench max)`.
- **Why here:** Missing/zero URDF effort weakens torque cap derivation; gravity pre-flight saturation uses motor limits but policy effort still feeds Davout filtering.

#### `soft-limit-source`

- **Consumers:** `joint_limit_bounds` reads URDF `safety_controller`; `control.yaml` can override soft bounds. ADR 0009 velocity-scaled envelope uses soft⊂hard gap (~27 mrad default inset via Set Limits).
- **Why here:** Missing both URDF soft and control soft on asymmetric joints collapses soft≡hard — operator loses ADR 0009 margin behavior.

#### `placeholder-provenance`

- **Consumer:** Operator trust / process signal for Config/Setup (no code reader today).
- **Why here:** Explicit PROVISIONAL headers and stub mass patterns correlate with large τ_g error documented on bench ([`hardware/docs/kinematics.md`](../../hardware/docs/kinematics.md): *"Masses and inertials … estimates until CAD export"*).

### P2 — sim fidelity & operator viz

#### `mjcf-dof-parity`

- **Consumers:** [`sim-harness`](../../crates/sim-harness/src/lib.rs) URDF/MJCF DOF tests; [`scripts/urdf-to-mjcf.sh`](../../scripts/urdf-to-mjcf.sh) (manual MJCF maintenance per [ADR 0003](../../docs/decisions/0003-simulation-testing.md)).
- **Why here:** MuJoCo D1 smoke steps MJCF, not URDF directly. DOF/name/axis drift between URDF and `assets/mjcf/*.xml` breaks sim as a τ_g cross-check ([ADR 0005](../../docs/decisions/0005-dynamics-library.md) D1 validation intent).

#### `visual-geometry`

- **Consumer:** Consul [`parse-urdf.ts`](../../consul/src/urdf/parse-urdf.ts) — box/cylinder/sphere only; no `<mesh>` yet.
- **Why here:** Config/Setup “view active URDF” needs visuals to validate link frames; absence is non-fatal but blocks human verification.

#### `link-length-sanity`

- **Consumers:** Same FK chain as gravity; cross-check against [`hardware/docs/kinematics.md`](../../hardware/docs/kinematics.md) link dimensions.
- **Why here:** COM lever arm errors from wrong segment lengths affect τ_g magnitude even when masses are correct.

---

## Explicitly deferred / evolving later

| Field / concern | Reason deferred |
|-----------------|-----------------|
| **Inertia tensor** (`ixx`…`izz`) | `UrdfGravityModel` uses mass + COM only; tensor matters for MuJoCo dynamics but not live τ_g path |
| **Collision geometry** (`<collision>`, `assets/meshes/collision/`) | Not read by Davout/Berthier; Talleyrand planning scaffold only ([`crates/talleyrand/README.md`](../../crates/talleyrand/README.md)) |
| **URDF material friction** | Berthier friction from `control.yaml` (`friction.fc/fv/fo/k`), not URDF tags ([docs/tuning.md](../../docs/tuning.md)) |
| **Mesh visuals** (`<mesh filename="…">`) | Consul parser supports primitives only; warn after mesh loader lands |
| **Mimic / passive joints** | Not in active 4-DOF bench profile; add when humanoid URDF lands |
| **Full humanoid actuated-joint set vs `robot_humanoid.yaml`** | Test exists but `#[ignore]` until Brawner export ([`armee-kinematics` humanoid test](../../crates/armee-kinematics/src/lib.rs)) |
| **Automated URDF→MJCF** | `scripts/urdf-to-mjcf.sh` documents hand-maintained MJCF; auto-converter is a separate pipeline ticket on map #96 |
| **Golden τ_g vectors / MuJoCo τ_g compare** | ADR 0005 D1 optional cross-check — future CI hardening, not v1 gate |
| **Joint velocity in URDF** | Command caps from `control.yaml` only (ADR 0010) |

Industry practice (Pinocchio/MuJoCo importers expect full inertial + collision + limits) aligns with deferred items; v1 checklist intentionally matches **what Marengo code reads today** plus MJCF parity for the sim tier we actually run.

---

## Suggested warn severity labels

| Label | Use for | Consul UX hint |
|-------|---------|----------------|
| **critical** | Wrong τ_g or wrong/failed limit envelope if deployed as-is | Red badge; link to gravity/limit docs |
| **high** | Init failures or weakened torque policy; fix before weighted/elevated motion | Amber badge |
| **medium** | Sim cross-check or operator-margin degradation | Yellow badge |
| **low** | Viz-only gaps | Gray/info badge |

All severities: single-line “what’s wrong”, “why Marengo cares”, “suggested fix” (CAD export, bench measure, Set Limits, edit URDF field X). No disable buttons, no deploy blocks.

---

## Key code paths (reference)

| Area | Path |
|------|------|
| Gravity from URDF | `crates/armee-dynamics/src/urdf_gravity.rs` |
| URDF parse + limits | `crates/armee-kinematics/src/lib.rs`, `limits.rs` |
| Davout limit build | `crates/davout/src/lib.rs` (`build_limits`) |
| URDF expand (Set Limits) | `crates/marengo-config/src/urdf_expand.rs`, ADR 0017 |
| Sim URDF/MJCF parity | `crates/sim-harness/src/lib.rs` |
| URDF validation (CI) | `scripts/validate-urdf.sh` (parse tests only) |
| Consul URDF parse/viz | `consul/src/urdf/parse-urdf.ts`, `urdf-scene.tsx` |
| Kinematic SSOT doc | `hardware/docs/kinematics.md` |
| Active bench URDF example | `assets/urdf/arm_4dof_right.urdf` |

---

## Decision for map #96

**Lock v1 gate on P0 + P1 rows above** (nine distinct checks across four P0 and five P1/P2 items as listed). P2 items (`visual-geometry`, `mjcf-dof-parity`, `link-length-sanity`) ship if cheap; otherwise follow immediately after P0/P1. Deferred table items stay out of v1 warnings until a consumer lands.
