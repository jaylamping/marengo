# Intent card — `teleop`

## 1. Header

| Field | Value |
|---|---|
| Crate | `teleop` (`bins/teleop`), bin, baseline `a2b55b3` |
| LOC | src 8 (`main.rs`), tests 0 (`metrics/loc.md:35`); coverage 0 % (`metrics/coverage-by-crate.md:19`) |
| Sources | `Cargo.toml`, `main.rs`, `codemap.md`, `src/codemap.md`, `bins/AGENTS.md` ("Dev, Teleoperation input, Scaffold"), `docs/roadmap.md:175` (M8), ADR 0004 (OpenArm teleop reference link), `git log` (code commit `5d44f5c` 2026-05-19 only) |

## 2. Intent

Classification: **roadmap placeholder (deferred future work)**. `docs/roadmap.md:175` (M8 "Locomotion and whole-body behaviors", status *later*): "Teleop ([bins/teleop]) — explicitly **after** unilateral arm G-comp on hardware". Code logs "implement leader/follower after unilateral G-comp (see docs/tuning.md)" (`main.rs:1-7`). **Conflicting intent:** Cargo/`main.rs` say bilateral **leader/follower** teleop (`Cargo.toml:4`, `main.rs:1`), while codemap says **gamepad/joystick** input mapped to joint commands (`bins/teleop/codemap.md`).

## 3. Owns / Must not

Owns nothing yet. When built: must go through the owner process (Chappe → marengo-pi), not open CAN (`bins/AGENTS.md`; ADR 0036 process-local grants).

## 4. Interface

None; deps `marengo-support`, `tracing`. Consumers: roadmap link only.

## 5. Invariants owned

None.

## 6. Inputs / outputs

`RUST_LOG` only.

## 7. Prior review reconciliation

`2026-09-29-repository-review.md:94` (name ≠ product) — **unchanged**.

## 8. Drift

Leader/follower vs gamepad intent conflict (§2). `src/codemap.md` "Input device read loop and command publish" — absent.

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Touches |
|---|---|---|---|
| Whole crate `bins/teleop` | scaffold with no consumer; roadmap M8 defers it behind unmet hardware gates | med (roadmap links it; owner may want the placeholder) | root `Cargo.toml:31`, `docs/roadmap.md:175` link, bins docs tables |

## 10. Phase-B leads

None (no logic). Decision lead: resolve leader/follower vs gamepad intent before any implementation.
