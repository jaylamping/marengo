# Test timing

How long the test loop takes and where the time goes. Maintained by the `test-speed` skill: measure with its recipe, add a dated row, and keep the opportunity list ranked by measured payoff.

## Measurements

| Date | Rev | Host | Scenario | Build | Run | Launch overhead | Test execution | Doc-tests |
|---|---|---|---|---:|---:|---:|---:|---:|
| 2026-10-04 | `2fb595fc` | Apple Silicon Mac, 18 cores, native | rerun, no changes | 0 s | 113 s | ~0 s | 91 s | 22 s |
| 2026-10-04 | `2fb595fc` | same | after `touch crates/robstride/src/lib.rs` | 93 s | 437 s | **322 s** (110 binaries, mean 2.9 s) | 91 s | ~22 s |
| 2026-10-04 | CI run 37225679705 | GitHub runner | `check` job | — | 12.2 min | — | — | — |

- Workspace: 125 test binaries, 1,346 tests.
- CI `check` job, 12.2 min in total: `check.sh` in the container 7.7 min, cargo cache restore 3.5 min, dev-image pull 0.8 min.
- Local `just check` runs the amd64 container under Rosetta (about 7× slower), which is why the full gate takes 20–30 min on this Mac.

## Opportunities, ranked by measured payoff

1. **First-launch latency: about 320 s per edit-then-test cycle.** Every freshly linked test binary waits 2–4 s before its first output; a second launch takes 0.00 s. Measured directly: a fresh `candump_cli` took 1.97 s to launch, then 0.00 s. The likely cause is macOS's first-launch assessment of new executables. Agent shells run under `/Applications/OpenChamber.app`.
   - **Owner action:** in System Settings → Privacy & Security → Developer Tools, add your terminal app and OpenChamber, then re-measure. This is unverified until measured.
   - Fewer binaries (item 3) also cuts it.
2. **Local gate under Rosetta.** An arm64 dev image (open decision (b) in `docs/commissioning/handoff-2026-10-04-liveness-hardening.md`) would bring `just check` close to native speed.
3. **125 test binaries.** Each is a separate link and a separate first launch. Integration-test files per crate:

   | Crate | Files |
   |---|---:|
   | davout | 23 |
   | berthier | 18 |
   | marengo-store | 17 |
   | robstride | 8 |
   | marengo-log-cli | 7 |
   | armee-dynamics | 4 |
   | chappe | 3 |
   | marengo-candump | 3 |

4. **Debug-build CPU in a few binaries (about 43 s of execution).** Measured execution time:

   | Binary | Time | Cause |
   |---|---:|---|
   | `marengo-candump/tests/input_limits.rs` | 16.3 s | gzips 256 MiB through unoptimised `miniz_oxide` |
   | `marengo-log-cli/tests/gravity_fit_wave_cli.rs` | 14.8 s | debug-build numerics in the gravity fit |
   | `berthier/tests/drive_loss_admission.rs` | 11.7 s | τ_g admission sweeps in a debug build |

   Fix with dependency `opt-level`, or with smaller inputs that still prove the contract.
5. **Real-time waits.**

   | Binary | Time | Notes |
   |---|---:|---|
   | `marengo_gateway` unit tests | 10.1 s | 80 tests |
   | `davout/tests/physical_reference.rs` | 8.9 s | 62 tests; FirmwareBus on real `Instant`s |
   | `berthier/tests/degraded_hold.rs` | 6.1 s | `thread::sleep(PERIOD)` loops |
   | `davout/tests/drive_loss.rs` | 3.3 s | |

   The structural fix is a clock seam in Davout and FirmwareBus (see `test-speed` lever 2).
6. **Doc-tests: 22 s over 15 crates.** Each doctest compiles and links separately on every run.
7. **Sequential binaries.** `cargo-nextest` would overlap all of the above. Not installed yet.
