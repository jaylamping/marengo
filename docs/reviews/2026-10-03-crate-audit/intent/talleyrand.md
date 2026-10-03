# Intent card: talleyrand

## 1. Header

| Field | Value |
|---|---|
| Crate | `talleyrand` |
| Path | `crates/talleyrand` |
| Kind | lib (scaffold; no features) |
| Baseline | `a2b55b3` |
| LOC src/tests | 1 / 0 (`src/lib.rs:1` is a single `//!` line; metrics/loc.md:34). 0 tests |
| Sources | `src/lib.rs:1`; `Cargo.toml`; `README.md`; `codemap.md`; `src/codemap.md`; AGENTS.md:20,186; crates/AGENTS.md:15,45; docs/architecture.md:26,38; docs/rust-patterns.md:27; docs/roadmap.md:28,166,174; ADR 0014 §4,§5,§8,§11, Consequences, Implementation order 7; ADR 0007:57; ADR 0009 Consequences; repository-review.md:89; implementation-roadmap.md:117; metrics/unused-deps.md; `git log` (4 commits, all docs/housekeeping: `61fe36d`, `5106357` 2026-05-19; `dd2ed3e` 2026-06-17; `5b4f222` 2026-06-23). No code commit since creation. |

## 2. Intent

Talleyrand is the reserved home of the **motion planner**: Cartesian goals, labeled poses or motion primitives turned into **joint-space trajectories with timing**, which Berthier then executes. Berthier stays a joint-space executor (README.md:7-9; architecture.md:38 "Cartesian primitives → joint trajectories (future)"; ADR 0007:57 "Talleyrand (future) owns Cartesian → joint timing; Berthier executes whatever joint refs it receives"). ADR 0014 gives it concrete scope: subscribe to `navigator/intent` (`NavigatorIntent` from Fouché), resolve `target_label` against a labeled-pose catalog relative to URDF `base_link`, run **IK per phase**, and emit joint trajectories to Berthier, with Berthier and Davout unchanged (ADR 0014 §4, §5, §11, Consequences). Its architectural purpose today is boundary-keeping: "Talleyrand owns IK and multi-joint timing. Berthier does not" (crates/AGENTS.md:45). This keeps the LLM and perception path from bypassing the joint-space envelope (ADR 0014 Alternatives "LLM directly emits joint targets … rejected"; ADR 0009 Consequences). The roadmap gate is "After collision meshes + stable full URDF" (roadmap.md:166). Gait/stepping is M8 (roadmap.md:174).

Conflicting statements of intent:

| Topic | Source A | Source B |
|---|---|---|
| Host node | ADR 0014 §8 table "Control / IK / motors → Pi" and §11 "Pi side … Talleyrand subscribes to `navigator/intent`" | roadmap.md:28 "Jetson (planner + perception)"; codemap.md:11 "Future consumer: `bins/marengo-jetson`"; `bins/marengo-jetson/Cargo.toml:21` depends on it; `bins/marengo-jetson/src/main.rs:1` "vision, planner" |
| Transport to Berthier | README.md:9 "Publishes setpoints for Berthier over Chappe" | ADR 0014 §4 diagram "[in-process on Pi] Berthier joint-space executor" |
| Scope wording | crates/AGENTS.md:15 "IK + multi-joint timing" | ADR 0007:57 "Cartesian → joint timing"; Berthier already owns per-joint trapezoid timing (AGENTS.md:182; CONTEXT.md:14 "Position hold") |
| Berthier relation | crates/berthier/README.md:9 "Consumes planner setpoints from Talleyrand" | berthier has no dependency on talleyrand (`cargo tree -i talleyrand` → only marengo-jetson) |

[INFERENCE] ADR 0014 is the newest and most specific statement (Accepted 2026-06-19). The Jetson-host statements predate it and were not updated.

## 3. Owns / Must not

| Owns (future) | Evidence |
|---|---|
| Cartesian/labeled-pose → joint trajectory, IK, multi-joint timing | crates/AGENTS.md:15,45; ADR 0014 §5,§11 |
| Collision-aware paths (collision meshes) | README.md:9; lib.rs:1 |

| Must not | Evidence | Status |
|---|---|---|
| CAN, MIT encode | architecture.md:38 | No code, so trivially satisfied |
| Bypass Berthier/Davout | ADR 0014 §1, Alternatives | n/a |

## 4. Interface

No public items. `lib.rs` is one doc comment. Declared deps `armee-kinematics`, `thiserror`, `tracing` (Cargo.toml:13-16) are all **unused** per `cargo machete` (metrics/unused-deps.md). The only dependent is `marengo-jetson`, which does not use it either (metrics/unused-deps.md flags `talleyrand` in marengo-jetson; `bins/marengo-jetson/src/main.rs:3-6` only logs "scaffold"). Depth: none. Seams: none.

## 5. Invariants owned

None in code. The intended invariant (planner output stays inside the ADR 0009 joint envelope; LLM cannot reach motors except via Talleyrand → Berthier → Davout, ADR 0014 Alternatives) is enforced today by the absence of any path. Untested.

## 6. Inputs / outputs

None today. Planned (ADR 0014 §7, §11): input topic `navigator/intent` (`NavigatorIntent`), optional `perception/frame` (`PerceptionFrame`); output joint trajectories to Berthier. Neither proto message exists yet (grep of `proto/marengo/v1/marengo.proto`: none). Planned input assets: `assets/meshes/collision/` (README.md:9), which today contains only `.gitkeep`.

## 7. Prior review reconciliation

| Prior id | Topic | Status | Evidence |
|---|---|---|---|
| repository-review.md:89 | Planning/perception are library scaffolds, no working planner | **unchanged (accurate)** | lib.rs:1 |
| implementation-roadmap.md:117 | Do not grow planning/perception scaffolds to satisfy unrelated findings | **respected** | no code commits since `5b4f222` |
| ledger M07 | Identify scaffolds as unsupported (scope honesty) | **partial** | AGENTS.md:20, codemap.md:4 say "scaffold"/"in development"; berthier README.md:9 and talleyrand README.md:9 describe behaviour as if present |

## 8. Drift

- berthier README.md:9 "Consumes planner setpoints from Talleyrand": no such dependency or code path.
- talleyrand README.md:9 "using armee-kinematics and collision meshes … Publishes setpoints … over Chappe": no code, no meshes (`assets/meshes/collision/.gitkeep` only), and the transport conflicts with ADR 0014 §4.
- codemap.md:11 / marengo-jetson Cargo.toml:21 place it on the Jetson; ADR 0014 places it on the Pi.
- crates/AGENTS.md:15 vs ADR 0007:57 on "multi-joint timing" (see §2).
- `scripts/daily-audit/audit.py:403` maps `talleyrand` path changes to a "whole-body control impedance" research query. Harmless, but it is a tooling reference to keep or remove with the crate.

## 9. Prune candidates (classification only; deleting roadmap scaffolds is a user decision)

| Candidate | Evidence class | Confidence | Keep cost | Delete cost / touches |
|---|---|---|---|---|
| Crate `talleyrand` | Scaffold with no consumer (0 pub items; the only dependent does not use it) | n/a (user decision) | One workspace member and lock entry; three unused deps compiled for marengo-jetson (metrics/unused-deps.md); ongoing doc drift (4 conflicting statements above) | Root Cargo.toml:14,66; bins/marengo-jetson/Cargo.toml:21; README.md:64; AGENTS.md:20,186; crates/AGENTS.md:15,45 (the IK boundary rule loses its named owner); crates/codemap.md:26; codemap.md:33; docs/architecture.md:26,38; docs/rust-patterns.md:27; docs/roadmap.md:166,174; ADR 0007:57, ADR 0009, ADR 0014 (would need a superseding note); docs/portraits/talleyrand.jpg + README row; scripts/daily-audit/audit.py:403. Recreating later costs one Cargo.toml + lib.rs |
| Unused deps `armee-kinematics`, `thiserror`, `tracing` in talleyrand/Cargo.toml:13-16 | Scaffold with no consumer (machete) | high | — | Cargo.toml only; also removes the `talleyrand → armee-kinematics` edge that kinematics docs do not list |
| `talleyrand` dep in marengo-jetson | Scaffold with no consumer (machete) | high, but it conflicts with ADR 0014 §11 (Pi host) either way | — | bins/marengo-jetson/Cargo.toml:21; bins/marengo-jetson/codemap.md:11 |

## 10. Phase-B leads

| # | Location | Why suspicious |
|---|---|---|
| T1 | crates/AGENTS.md:45 vs Berthier | The boundary "Berthier does not own multi-joint timing" has no enforcing code. Berthier already owns per-joint trapezoid/cosine timing (ADR 0007; CONTEXT.md:14). Phase B should decide whether coordinated multi-joint timing (e.g. compound/Wave tests, `39e8a8a`) has crept into Berthier |
| T2 | ADR 0014 §11 vs marengo-jetson Cargo.toml:21 | Host ambiguity. If built on the Jetson as wired today, Talleyrand would emit trajectories across the network, contradicting "Jetson … never control" (ADR 0014 §1) |
| T3 | roadmap.md:166 gate | Gate prerequisites are unmet (no collision meshes; master URDF = 5-DOF right arm only). Any planner work now violates the roadmap's own gate |
