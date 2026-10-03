# Phase B — WP-R: build, codegen, CI and sim hygiene

Branch `audit/wp-r`, verified against main `223db4a0`.
Fix commits: **S** = scripts/CI/codegen gates, **M** = sim-harness + MJCF,
**B** = build.rs + proto inclusion, **T** = cross-package gap tests,
**D** = docs/drift. Plus this file.

"Red" = the new test (or a scratch reproduction using only baseline API)
failed on a detached worktree at main (`/tmp/wp-r-red`, removed afterwards)
before the fix. Coverage-only additions note the gap as the red.

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-sim-harness-04 (T27 path filter + production smoke) | **CONFIRMED** | `scripts/check-sim.sh` now smokes `assets/mjcf/marengo.xml` after the fixture model; `ci.yml` sim filter gains `assets/**`, `config/**`. No local MuJoCo here — load validity rides the new `sim-harness` parity tests plus the CI sim job | S: `check-sim.sh`, `ci.yml:30-42` |
| L-sim-harness-05 (MJCF has no inertial) | **CONFIRMED** | `sim-harness::production_mjcf_body_masses_match_urdf`. Red scratch: `FAILED … production MJCF has no <inertial> masses` on baseline XML | M: `assets/mjcf/marengo.xml` inertials copied from the URDF CAD values (no invented numbers), mass-parity test |
| L-armee-proto-04 (breaking only on PR CI) | **CONFIRMED** | `check.sh` runs `buf breaking` on local runs too when `origin/main` (or `BUF_BREAKING_AGAINST`) materializes; warn-and-skip locally when unavailable, still fail-closed in CI | S: `check.sh:45-83` |
| L-berthier-20 (no plant, T27 homeless) | **CONFIRMED** | No plant built (design work — see NEEDS-DECISION). In-ownership part: `sim-harness` crate docs now name it the reserved plant home over `davout::simulation::SimulationBus` | M (docs) |
| L-marengo-candump-06 (Pi log-cli lacks enrichment) | **REFUTED** | `marengo-log-cli` default features include `robstride-enrichment` and no Pi build passes `--no-default-features` (cargo unifies `--features` across the deploy set, so enrichment stays on). Locked by `scripts/deploy-log-cli-enrich.test.sh` (wired into `check.sh`) and `log-cli/tests/store_cli_gaps.rs::cli_candump_enrich_resolves_the_catalog` | S + T (guards) |
| L-marengo-host-metrics-05 (stale build SHA) | **CONFIRMED** | Scratch: empty commit → baseline rebuild `Finished in 0.06s` (build script never reran, SHA stale); fixed rebuild `Compiling marengo-host-metrics` after the same commit | B: `build.rs` watches the absolute git dir `HEAD` + branch ref (or `packed-refs`); SHA logic untouched |
| L-marengo-imu-09 (driver 61.9 %) | **CONFIRMED**, partially closed | New `driver::tests`: `poll_with_no_packets_returns_none`, `send_packet_rejects_an_invalid_channel`, `oversize_packet_header_is_a_protocol_error` (16/16 green). `initialize`/`enable_feature` retry/timeout paths still uncovered (need scripted multi-transaction mocks + seconds of sleeps) | T (cross-package) |
| L-armee-proto-01 (new .proto never reruns build) | **CONFIRMED** | Scratch, warmed cache: added `proto/marengo/v2/ping.proto` → baseline rebuild `Finished in 0.04s`, no recompile; fixed rebuild `Compiling armee-proto` | B: `build.rs` adds `rerun-if-changed` on the whole `proto/` tree |
| L-armee-proto-03 (local checksum self-blessing) | **CONFIRMED** | `scripts/proto-checksum.test.sh`. Red scratch: `error: proto-checksum.sh exited 0 with a missing checksum` on baseline; green here. Missing checksum now fails everywhere with the regen command | S: `proto-checksum.sh` + test wired into `check.sh` |
| L-sim-harness-03 (counts-only parity) | **CONFIRMED** | `production_urdf_and_mjcf_names_axes_and_ranges_match` (exact name sets, per-joint axes, MJCF range inside URDF hard limits) + `renamed_mjcf_joint_breaks_name_parity` (rename keeps counts — old check blind). Green 12/12 | M: `sim-harness` |
| L-marengo-log-cli-11 (main.rs 71.8 %) | **CONFIRMED**, partially closed | New `tests/store_cli_gaps.rs` (5/5): CLI `session finalize`, `purge`, `candump page`, non-Linux `journal-import` stub, `--enrich` guard. `disk-usage` already pruned by merged B9; `recover-known-v2`/`import-legacy` wait on D-7 field-DB state | T (cross-package, behavior unchanged) |
| L-marengo-store-13 (journal/ring/store gaps) | **CONFIRMED**, partially closed | Extracted pure `parse_journal_line` (same skip semantics) with 3 cross-platform tests; `journal_priority_level`/`JournalEntry` ungated. Red scratch: `error[E0425]: cannot find function parse_journal_line`. Note: `ring.rs` no longer exists (now `disk.rs`, renamed in B8) — that third of the lead is stale. `store.rs` 80 % left open (owner wave merged) | T (cross-package) |
| L-armee-proto-05 (caret ranges flip checksum) | **REFUTED** as a defect | A lock refresh can change codegen output, but it cannot slip through: the committed `.checksum` gate fails closed in CI and locally, and `buf breaking` now also runs locally. No pin change made | — |
| L-berthier-21 (one-point isolation proptest) | **CONFIRMED** | Full-input proptest (gains/kd/velocities/bands/lead/age/all 4 phases/Option friction/bools). Mutant red: `tau_d += tau_g*0.5 when kd>15` passes the old test, fails the new one, clean tree passes | T (cross-package, test-only) |
| L-marengo-limit-sync-06 (zero Rust coverage) | **CONFIRMED**, partially closed | New `tests/cli_smoke.rs`: `--help` documents both soft flags; one-sided `--soft-lower` refused (also pins the L-marengo-limit-sync-02 fix). Bin now has Rust coverage | T (cross-package) |
| L-marengo-support-02 (two subscribers) | **CONFIRMED** | Real divergence surface, no behavior bug — see NEEDS-DECISION | — |
| L-marengo-support-03 (limit-sync has no subscriber) | **CONFIRMED** | `bins/AGENTS.md` already required `init_tracing` of non-producer bins. Fixed: `init_tracing()` + the three `eprintln!` → `tracing` (new `marengo-support`, `tracing` deps) | T (cross-package, minimal) |
| L-marengo-support-04 (100 % artifact) | **ALREADY-FIXED** | `marengo-support` now carries a real unit test (`unset_filter_defaults_to_info_and_explicit_directives_override_it`); the regions/functions note was a metrics-presentation quirk of the 10-line crate | — |
| L-repo-02 (stale codex worktrees) | **CONFIRMED**, D-10 executed | Removed 10 empty `~/.codex/worktrees/*` husks (only `.codex-worktree-name` inside). Kept: `research-cache-await` worktree + local `codex/research-cache-await` branch (6+ unpushed commits) and all remote `codex/*` branches, per D-10 | — (home-dir cleanup) |
| L-repo-03 (llvm-cov homing flake) | **REFUTED** | `metrics/README.md:55`: the failing run was a `target/llvm-cov-target/` artifact race (`No such file or directory` on the test binary); the subsequent full-workspace run completed with no `--exclude`. No test bug identified; `marengo-homing` is being folded into Davout per D-3 anyway | — |
| L-repo-04 (deny/audit native) | **ALREADY-FIXED** | Ran `scripts/check-dependencies.sh` natively on macOS just now: exit 0 (one allowed `paste` unmaintained warning). `scripts/AGENTS.md` already documents it host-native | — |
| L-sim-harness-01 (substring hinge count) | **CONFIRMED** | Scratch baseline-API repro: commented-out `<joint type="hinge"/>` counted → `FAILED` (returned 1, expected 0). Fixed: comment stripping, real type resolution incl. `<default>` classes, `slide` excluded (`commented_joints_are_not_counted`, `slide_joints_are_not_hinges`, `default_class_joint_types_resolve`) | M |
| L-sim-harness-02 (file-wide elbow axis) | **CONFIRMED** | `elbow_axis_is_per_joint_not_file_wide`: doctored MJCF with the axis string elsewhere but a wrong elbow axis fails per-joint, would have passed `contains` | M (parity test asserts the elbow joint's own axis) |
| L-armee-proto-02 (v2 silently unreachable) | **CONFIRMED** | New `tests/proto_inclusion.rs` fails when a compiled module is not `include!`d (scratch: added v2 package → `FAILED`). The test is the fix: silence becomes a CI failure | B (test-only) |

## Drift fixed (all in WP-R ownership unless noted)

- `check-println-crates.sh`: `rg`-absent hosts fell through to an empty hit list (silent pass). Now falls back to `grep -rEl` over the same file set.
- `check.sh` aarch64 smoke: `--workspace -p` combined (cargo tolerates it and builds just the `-p` set, but the flag lies). Now builds exactly `-p marengo-pi -p imu-probe`. Package set unchanged.
- `.tool-versions`: aligned to `mise.toml` (`rust 1.88.0`, `nodejs 24.16.0`, `protoc`/`buf` unchanged). Root `AGENTS.md` tool table updated (it called the file stale, and its Python cell now names pytest).
- `.github/workflows/deploy.yml`: deleted (placeholder mentioning Jetson targets; D-1 removes Jetson plumbing, nothing referenced the workflow).
- `scripts/test_analyze_position_trace.py` (8 pytest) + `scripts/test_preserve_taught_limits.py` (5 unittest): wired into `check.sh` with fail-loud guards for pytest/pyyaml; `docker/Dockerfile.dev` installs both; `scripts/AGENTS.md` stdlib line amended. Both suites pass.
- `tools/compound-auto-learn` (`vitest run` + `typecheck`): wired into the `check.sh` node-tooling section.
- armee-kinematics `#[ignore]` humanoid test (cross-package): it asserted the 23-joint future template against the 5-DOF bench URDF — run with `--ignored` on baseline it FAILS (`left: 5 joints, right: 23`). Replaced with `bench_robot_config_joints_match_bench_urdf` (live `robot.yaml` vs live URDF), green.
- `docs/rust-patterns.md` + `crates/AGENTS.md`: dropped the nonexistent `sim` feature; `rust-patterns.md` "deterministic seeds" → deterministic `sim-harness` fixtures.
- Counts: `codemap.md` 18+10 → 16+6; `bins/AGENTS.md` 9 → 6 binaries (added the missing `marengo-limit-sync` row; `anyhow::Result` → `ExitCode` since no bin uses anyhow); `bins/codemap.md` tracing line fixed, `motor-repl` row de-jogged, new `marengo-limit-sync/codemap.md`; `crates/AGENTS.md` gained the missing `marengo-candump` row and the `Supervisor` line ref 166 → 367.

## NEEDS-DECISION

### S2 — the two tracing subscribers (`support::init_tracing` vs `chappe::init_subscriber`)
- **A (recommended): keep both, document the split.** Producers publish `LogEvent` on `logs/structured` (needs the Chappe bus); CLIs log to stdout/journal. Unifying forces a `chappe` dependency into every CLI to gain nothing operator-visible. Cost: one paragraph in `logging-taxonomy.md`.
- **B: unify on `chappe::init_subscriber(None)`** and delete `support::init_tracing`. Cost: dependency + call-site churn in 7 bins for an S4; contradicts the KEEP on `marengo-support` (P-marengo-support-02).
- Needs no hardware; needs the logging-taxonomy owner to accept the paragraph.

### B20 — where the Berthier plant lives (L-berthier-20)
- **A (recommended): build it in `sim-harness`** (D-5 keeps the crate as the M6 extension point; this wave documented it as the reserved home and fixed sim masses so a plant sees CAD gravity). Start: deterministic 1-DOF gravity + viscous plant driving `ControlLoop` through `davout::simulation::SimulationBus`. Needs a small design (golden-state determinism rules, no second CAN path).
- **B: grow the plant inside `berthier`/`davout` test modules.** Less discoverable; splits plant logic across crates.
- Needs no hardware; needs a design pass before code.

## Cross-package edits (all minimal, test-only except where noted)

- `crates/armee-proto/tests/proto_inclusion.rs` (new), `build.rs` dir watch is in-ownership.
- `crates/armee-kinematics/src/lib.rs`: ignored-test replacement only (parses
  via `marengo-config::load_robot_config`, no `serde_yaml`); `Cargo.toml`
  drops the `serde_yaml` dev-dep to match B11.
- `crates/berthier/src/mode_isolation.rs`: proptest strategies only.
- `crates/marengo-store/src/journal.rs`: pure-function extraction (identical skip semantics) + tests.
- `crates/marengo-imu/src/driver.rs`: 3 new unit tests only.
- `bins/marengo-log-cli/tests/store_cli_gaps.rs` (new, no src change).
- `bins/marengo-limit-sync/{src/main.rs,Cargo.toml}`: subscriber + `tracing` conversion (3 lines); `tests/cli_smoke.rs` (new).
- Peers `WaveC2_Prune_B5_B11` (sim-harness, kinematics) and `WaveC2_Prune_B7_B13` (limit-sync) were notified before editing; my regions avoid their prune targets (arm_4dof fns, fixtures module, limit-sync logic).

## Behaviour that changes on the bench / at startup

- Nothing on the bench. `marengo-limit-sync` log lines move from stderr `eprintln!` to `tracing` (same console via `init_tracing`).
- `check.sh` is stricter: local `buf breaking` (when `origin/main` is fetchable), `proto-checksum` fails closed without a checksum file, and the two Python suites + compound-auto-learn + two contract tests are new gates (need `pytest`/`pyyaml` outside the container).
- `check-sim.sh` additionally steps the production MJCF; CI sim now triggers on `assets/**` and `config/**`.
- `marengo-host-metrics` rebuilds (once) after every commit so `MARENGO_GIT_SHA` never goes stale.
- Production MJCF now carries CAD inertials: sim gravity matches the dynamics model instead of MuJoCo geom inference.

## Gate

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings`; `cargo clippy -p marengo-pi --all-targets --target aarch64-unknown-linux-gnu -- -D warnings`; `cargo test --workspace`. (consul/MCP untouched — their gates do not apply.)

Results on this branch (2026-10-03, macOS, after rebase onto main `5bf98252`):
fmt OK; workspace clippy OK; pi aarch64 clippy OK; `cargo test` run
per-package (single workspace run exceeds the 30 s shell cap here) — every
suite green, zero failures, including the new sim-harness (10, post-B11),
proto inclusion, journal parse (4), log-cli gap (5), limit-sync smoke (2),
IMU (16) and widened Berthier isolation tests.
`scripts/check-dependencies.sh` also passes natively. Container-only steps
(npm suites, MuJoCo production smoke) ride the CI jobs they gate.

`check-println-crates.sh` note: wiring the grep fallback exposed that the
guard (dead in CI — no `rg` in the dev container) would fail on ~25
pre-existing `println!` in peer-owned `tests/`/`examples/` (davout, berthier,
store, homing). The guard is scoped to lib sources (`crates/*/src`), where it
passes clean: test/example output is not operator-visible runtime logging.
Widening it to tests is a separate decision for the owning waves, not this one.

## Integration vs prune B11 (done on rebase)

Prune batch B11 (peer `WaveC2_Prune_B5_B11`, merged to main as `5ba8b849`)
deletes the `arm_4dof*` MJCF pair, their sim-harness tests, and the ignored
kinematics test this wave replaced. This branch rebased onto main after that
merge: the two `arm_4dof*_model_path` fns plus their tests are dropped from
`sim-harness/src/lib.rs` (the parser and production-parity tests stay, now
all `#[cfg(test)]` per B11's test-only gating — 10 tests, not 12), B11's
`armee-kinematics` dependency line with `features = ["test-support"]` is
kept, and `bench_robot_config_joints_match_bench_urdf` parses exclusively
through `marengo-config` (`load_robot_config`) with no `serde_yaml` dev-dep.
