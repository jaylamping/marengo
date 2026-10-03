# Intent card: sim-harness

## 1. Header

| Field | Value |
|---|---|
| Crate | `sim-harness` |
| Path | `crates/sim-harness` |
| Kind | lib (no cargo features). The only dependency is `armee-kinematics`, declared as a path dep (Cargo.toml:14) |
| Baseline | `a2b55b3` |
| LOC src/tests | 113 in `src/lib.rs`: 39 lines of API (lib.rs:1-38) plus 73 lines of inline tests (lib.rs:40-113). No `tests/` directory. 5 tests (metrics/test-counts.md:93) |
| Coverage | 97.3% lines, 92.4% regions, 100% functions (metrics/coverage-by-crate.md:32, coverage-by-file.md:109). All coverage comes from its own tests |
| Sources | `src/lib.rs` `//!` 1-4; `README.md`; `codemap.md`; `src/codemap.md`; crates/AGENTS.md:25; AGENTS.md:120,282,290; ADR 0003; ADR 0005 Decision 3; ADR 0031; docs/roadmap.md:153; `sim/README.md`; `sim/scripts/smoke_test.py`; `scripts/check-sim.sh`; `scripts/validate-urdf.sh`; `scripts/urdf-to-mjcf.sh`; `.github/workflows/ci.yml:20-41,255-279`; justfile:31-33; prior review tooling.md T27/T28 and test-quality-plan.md:36,79; `git log` (9 commits, first `d7787d5` 2026-05-19, last code change `ce2e7ae` 2026-07-19); `cargo tree -i sim-harness` (no dependents). |

## 2. Intent

sim-harness is the **Rust-side half of ADR 0003's D1 simulation tier**. Python MuJoCo stepping runs in `sim/scripts/smoke_test.py`. This crate holds the Rust fixture-consistency checks and is the reserved home for "future in-process bindings" (lib.rs:1-4; README.md:3; ADR 0003 Decision table row D1 and Alternatives "In-process Rust MuJoCo bindings only: deferred"). Its actual delivered content is four tests asserting that each URDF and its paired MJCF declare the same number of actuated hinge joints: minimal fixture, production **Master URDF**, and the two archived 4-DOF arm slices (lib.rs:53-112). Its purpose is to catch an MJCF that falls out of step with a URDF edit (scripts/urdf-to-mjcf.sh:17-27). The roadmap expects it to grow: "Sim: regenerate MJCF from production URDF; extend `sim-harness` beyond minimal fixture" (roadmap.md:153, M6).

Relation to the three things called "simulation" in this repo:

| Thing | What it is | Relation to sim-harness |
|---|---|---|
| `just sim-check` → `scripts/check-sim.sh` | Docker `check-sim` (justfile:31-33) runs `smoke_test.py` on `MARENGO_SIM_MODEL` (default `sim/fixtures/minimal.xml`): 500 `mj_step`s, then asserts `nq ≥ 1` (smoke_test.py:13-21). Then it runs `cargo test -p sim-harness` (check-sim.sh:17-18) | sim-harness tests are the second half of the gate. CI pins the model to `minimal.xml` (ci.yml:277) |
| `sim/` | `fixtures/minimal.{urdf,xml}` + `scripts/smoke_test.py` (sim/README.md:5-8) | Reads `minimal.*` via `armee_kinematics::fixtures` (lib.rs:8-13,55-57) |
| Davout `simulation` module | `SimulationBus` (crates/davout/src/simulation.rs:156): a scripted in-memory CAN bus behind `Supervisor`/`ControlLoop::from_simulation*` closed constructors (ADR 0031) | **None.** No dependency in either direction (`cargo tree -i sim-harness` is empty; sim-harness depends only on armee-kinematics). No physics plant exists anywhere in Rust |

Conflicting statements:
- codemap.md:4,8 says it provides "MuJoCo or synthetic bus integration tests … works with … `MemoryBus` for CAN-free validation", and src/codemap.md:4 says "Synthetic bus and simulation loop helpers". The crate has no bus, no loop, and no robstride/davout dependency (Cargo.toml:13-14). `MemoryBus` lives in robstride (crates/robstride/src/bus.rs:1027).
- codemap.md:11 "Used by: crate integration tests" is wrong: there are no dependents (`cargo tree -i`).
- ADR 0003 D1 implies sim-harness participates in MuJoCo testing. It never loads MuJoCo; it counts `type="hinge"` substrings (lib.rs:36-38).

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| URDF↔MJCF DOF-count consistency for checked-in models | lib.rs:53-112 |
| MJCF path helpers for those models | lib.rs:10-28 |

| Must not | Evidence | Status |
|---|---|---|
| Hardware in default `cargo test` | Marengo rules ("Tests must not require hardware") | No violation |
| Heavy sim deps outside a feature gate | crates/AGENTS.md CONVENTIONS ("feature-gate … heavy sim deps") | No violation (none present) |

No explicit "must not" list exists in lib.rs; the crate is not in the AGENTS.md layer table.

## 4. Interface

| Group | Items | Consumers |
|---|---|---|
| Model paths | `default_model_path` (lib.rs:11), `production_model_path` (16), `arm_4dof_model_path` (21), `arm_4dof_right_model_path` (26) | **None outside the crate's own tests** (grep over crates/bins/tools) |
| Helpers | `model_exists` (31), `count_mjcf_hinge_joints` (36) | Same: own tests only |
| Gate entry points (non-Rust) | `cargo test -p sim-harness` | scripts/check-sim.sh:18; scripts/validate-urdf.sh:8; scripts/urdf-to-mjcf.sh:27 (single test by name); `cargo test --workspace` (check.sh:141 comment) |

Depth: the pub API is **shallow**. Every function is a one-line path join or `str::matches().count()`. Two path helpers duplicate `armee_kinematics::fixtures::{arm_4dof_mjcf, arm_4dof_right_mjcf}` (armee-kinematics lib.rs:75-78,85-88). The real value is the test bodies. No traits, so no seams.

## 5. Invariants owned

| Invariant | Enforcing code | Test |
|---|---|---|
| Minimal fixture exists | lib.rs:48-51 | `default_fixture_exists` |
| URDF actuated count == MJCF hinge count (minimal, production, arm_4dof, arm_4dof_right) | lib.rs:53-101 | the four `*_dof_match` tests |
| arm_4dof_right elbow is `right_elbow_pitch` on +Y in both models | lib.rs:102-111 | `arm_4dof_right_urdf_and_mjcf_dof_and_elbow_axis_match`. The MJCF check is a whole-file substring test: `contains("axis=\"0 1 0\"")` matches **any** joint with that axis (lib.rs:103), not the elbow specifically |
| Joint **names**, axes, limits, frames and inertials agree between URDF and MJCF (production) | none | **untested**. This is T28 (MJCF has 0 `<inertial>` elements: `grep -c '<inertial' assets/mjcf/marengo.xml` = 0 vs 6 in `assets/urdf/marengo.urdf`) |
| Production model actually steps in MuJoCo | none in the gate | **untested** (CI pins `minimal.xml`, ci.yml:277; T27) |

## 6. Inputs / outputs

- Files read (tests): `sim/fixtures/minimal.{urdf,xml}`, `assets/urdf/marengo.urdf`, `assets/mjcf/marengo.xml`, `assets/urdf/archive/seed-arm_4dof{,_right}/contributor.urdf`, `assets/mjcf/arm_4dof{,_right}.xml`, all resolved from `CARGO_MANIFEST_DIR` (lib.rs:22,27; armee-kinematics lib.rs:42-89).
- Env: `CARGO_MANIFEST_DIR` (compile time). `MARENGO_SIM_MODEL` is consumed by check-sim.sh:7, not by this crate.
- No config, proto, CAN or HTTP.

## 7. Prior review reconciliation

| Prior id | Topic | Status | Evidence |
|---|---|---|---|
| T27 (P2) | Green sim job exercises only a minimal model; no control/safety path | **open** | check-sim.sh:7 default and ci.yml:277 still `minimal.xml`; sim-harness is unchanged since `ce2e7ae` (2026-07-19); ledger T27 "open" |
| T28 (P2) | Production MJCF lacks the URDF inertials | **open** | 0 `<inertial>` in `assets/mjcf/marengo.xml`; ledger T28 "open" |
| test-quality-plan.md:79 | Replace hinge string-count tests as production acceptance; keep a minimal engine smoke | **open** | lib.rs:36-38 unchanged |
| test-quality-plan.md:36,110 | `validate-urdf.sh` redundant rerun | **partial** | Dropped from the primary check (implementation-roadmap.md:195-196); script unchanged (validate-urdf.sh:7-9) |
| ADR 0005 Decision 3 | Optional MuJoCo τ_g cross-check | **not implemented** | `sim/scripts/` contains only `smoke_test.py` |

## 8. Drift

| Statement | Reality |
|---|---|
| codemap.md:4,8; src/codemap.md:4 "synthetic bus", "MemoryBus", "simulation loop helpers" | None in the crate. The closest thing is davout `SimulationBus` (ADR 0031), which is unrelated |
| codemap.md:11 "Used by: crate integration tests, CI sim targets" | No crate dependents. Used only by scripts and CI (check-sim.sh:18, validate-urdf.sh:8, urdf-to-mjcf.sh:27) |
| lib.rs:3 "CI (`check-sim`)" | Correct; ci.yml:268-279 |
| sim/README.md:3; ADR 0003 "until production URDF is exported" | The production URDF exists (5-DOF right arm; armee-kinematics lib.rs:209-216) but CI still uses the minimal model |
| scripts/urdf-to-mjcf.sh:17 "2-DOF bench model" | Production MJCF has 5 hinges (`assets/mjcf/marengo.xml`) |
| Cargo.toml:14 path dep | Workspace convention is `workspace = true` (crates/AGENTS.md CONVENTIONS). Flagged by cargo-deny as a wildcard (metrics/deny-audit.md:1197-1200) |
| ci.yml:30-41 sim path filter | Omits `assets/**` and `config/**`, so an MJCF/URDF-only change skips the MuJoCo smoke. The Rust DOF tests still run in the workspace suite |

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting touches |
|---|---|---|---|
| `arm_4dof_model_path`, `arm_4dof_right_model_path` | Duplicate implementation of `armee_kinematics::fixtures::arm_4dof_mjcf`/`arm_4dof_right_mjcf` (which are themselves zero-use, metrics/pub-usage.md:11-12). Keep one copy | high | lib.rs:20-28,81,94 or armee-kinematics lib.rs:75-78,85-88 |
| Whole pub API (6 fns) as `pub` | Scaffold with no consumer: zero references outside own tests | high (make `#[cfg(test)]`/private) | lib.rs:10-38 |
| arm_4dof / arm_4dof_right URDF↔MJCF tests and `assets/mjcf/arm_4dof{,_right}.xml` | Superseded: bringup profiles retired (CONTEXT.md:38 "Bringup profile — Retired"); the paired URDFs live under `assets/urdf/archive/` and the master is 5-DOF | med. Archived seeds are still referenced by marengo-config (`profile_txn.rs`, `urdf_merge.rs`), tools/marengo-pi-mcp `sync-config.ts`, `scripts/bench-set-weighted-mass.sh` and armee-dynamics tests, so only the **MJCF** pair and its tests are cleanly removable | lib.rs:78-112; assets/mjcf/arm_4dof*.xml; armee-kinematics fixtures |
| Entire crate (fold the remaining production + minimal DOF tests into `armee-kinematics/tests/` or a future sim crate) | Scaffold: the crate exists for "future in-process bindings" (lib.rs:4) that ADR 0003 deferred | low. **User/roadmap decision**: roadmap.md:153 names sim-harness as the M6 extension point. Deleting it also touches check-sim.sh:17-18, validate-urdf.sh:8, urdf-to-mjcf.sh:27, ci.yml:33, AGENTS.md:120,282,290, crates/AGENTS.md:25, rust-patterns.md:32, Cargo.toml members | crate dir + listed scripts/docs |

## 10. Phase-B leads

| # | Location | Why suspicious |
|---|---|---|
| S1 | lib.rs:36-38 | `count_mjcf_hinge_joints` counts the substring `type="hinge"`. It misses hinges by MJCF default (no `type` attribute means hinge in MuJoCo [INFERENCE from MJCF semantics]), single-quoted attributes and `<default>` classes, and counts commented-out joints. A DOF mismatch can pass or fail spuriously |
| S2 | lib.rs:103 | The elbow-axis assertion is a file-wide substring match: it passes if *any* element has `axis="0 1 0"` |
| S3 | lib.rs:65-76 | The production check compares counts only: renamed, reordered, re-axed or re-limited joints pass. Runtime keys everything by joint name (robot.yaml), so a name mismatch is the realistic failure |
| S4 | scripts/check-sim.sh:7; ci.yml:277 | The MuJoCo smoke steps only the 2-joint fixture. Production `marengo.xml` is never stepped in CI (T27) |
| S5 | assets/mjcf/marengo.xml | No inertials means MuJoCo infers mass from geometry. Any future sim-based τ_g/plant check would validate against the wrong masses (T28) |
| S6 | ci.yml:30-41 | The sim job path filter excludes `assets/**`, so MJCF edits do not trigger the MuJoCo step |
| S7 | Gap | No Rust plant exists to drive davout `SimulationBus` / `ControlLoop::from_simulation*`. T27's fix (run the real control/safety path against a simulated plant) has no home. This crate is the natural owner, but nothing is assigned |
