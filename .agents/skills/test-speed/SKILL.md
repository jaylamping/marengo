---
name: test-speed
description: Keep Marengo's test loop fast and keep making it faster - iterate on the narrowest tests, measure where cargo test time goes (build, first launch, test execution, doc-tests), fix the biggest cost you can each time you touch tests, and write new tests that are fast by construction. Use whenever you run or wait on cargo test or just check, write or review a test, notice a slow test, build or CI job, or the owner mentions test or CI time.
---

# Test speed

The owner spends a large share of their time waiting on tests. Treat test time as a defect with a measured cost. Every session that runs the suite should leave it faster, or at least leave a measured, ranked list of what to fix next in `docs/test-timing.md`. "It has always taken this long" is a finding to fix, not a fact of life.

## While iterating: climb the narrowest ladder

1. `cargo check -p <crate> --tests`: type errors, with no linking and nothing to launch.
2. The behaviour you changed: `cargo test -p <crate> --lib <module::path>`, or `cargo test -p <crate> --test <file> <name>`.
3. The crate's suite, then its direct dependents (`cargo test -p davout -p berthier`).
4. `cargo test --workspace`, once, at the end.
5. The merge gate (`just check`), once. On Apple Silicon it runs an amd64 container under Rosetta, about 7× slower, so it is never an iteration loop.

Go up a rung only when the one below is green. Every rung you skip saves link time and launch time for every binary it would have rebuilt.

## Measure before changing anything

The realistic case is edit-then-test. A rerun with no changes hides the build, link and first-launch costs, so measure after touching a low-level crate:

```bash
touch crates/robstride/src/lib.rs
/usr/bin/time -p cargo test --workspace --no-run                  # build + link
cargo test --workspace 2>&1 | python3 -u -c 'import sys,time
t0=time.time()
[sys.stdout.write(f"{time.time()-t0:8.2f} {l}") for l in sys.stdin]' > /tmp/test-run.log
python3 .agents/skills/test-speed/scripts/rank_test_binaries.py /tmp/test-run.log
```

- The rank script lists execution seconds per binary. A binary's own `finished in` excludes its startup.
- In the timestamped log, the gap between a `Running …` line and that binary's `running N tests` line is launch latency.
- To time single tests inside a slow binary, run them by name with `time`, or use nextest's per-test report if it has been adopted.

Record the numbers in `docs/test-timing.md`: date, rev, scenario, build, run, launch overhead, doc-tests, and the top binaries.

## Levers

`docs/test-timing.md` holds the current measured ranking. Pull the biggest lever you can.

1. **First-launch latency (macOS).** Each freshly linked binary waits 2–4 s on its first launch; a second launch takes 0.00 s. The usual fix is to list the launching app under System Settings → Privacy & Security → Developer Tools: the owner's terminal app, and `OpenChamber.app` for agent shells. Only the owner can toggle it. Fewer binaries (lever 3) shrinks this cost too.
2. **Simulated time.** Tests that sleep or spin through a real watchdog (`comm_watchdog_ms` 100 ms), the post-SetZero quiet (800 ms), enable completion (2 s) or a fuse budget (2000 ms) pay those seconds on every run. Advance time virtually instead. `SimulationBus::elapse_reference_clock` covers the reference clock. Davout's other timers read `Instant::now()` directly (dozens of sites in `crates/davout/src/lib.rs`), and the FirmwareBus emulator runs on real `Instant`s, so a clock seam on the supervisor and the emulator is the structural fix. Propose it, using `control-rewrite`'s standard of argument, when a slow test needs it.
3. **Fewer test binaries.** Every `tests/*.rs` file is its own linked executable, with its own link step and first launch. Merging a crate's integration tests into one binary (`tests/it/main.rs` with modules) cuts both. Keep separate binaries only where isolation is the point: process environment, signals, `stop_independence`.
4. **Debug-build CPU.** Compression, numeric sweeps and fits run unoptimised under `cargo test`. Optimised dependencies (`[profile.dev.package."*"] opt-level = 2`, or per heavy package) speed them several-fold, at the cost of a one-time dependency rebuild; measure both sides. Better still, prove the contract with a smaller input: a configurable limit in a unit test plus one small integration check, rather than generating a production-size input.
5. **Parallelism across binaries.** Plain `cargo test` runs binaries one after another. `cargo-nextest` runs every test from every binary in parallel and reports per-test times. Adopting it is a tooling change (install, `scripts/check.sh`, CI); bring measurements when proposing it.
6. **Doc-tests.** Under edition 2021 each doctest is compiled and linked as its own executable on every run. Mark illustrative examples `no_run` or `ignore`, or move behavioural examples into unit tests.
7. **Gate and CI.** The local container runs under Rosetta (an arm64 dev image is an open decision in `docs/commissioning/handoff-2026-10-04-liveness-hardening.md`). In CI, cache restore is a large share of the `check` job.

## Writing a test

- Make time pass by advancing simulated time. A test that genuinely measures real timing stays under 100 ms of real waiting and says why in a comment.
- Use the smallest input that proves the contract.
- Add the test to the crate's existing test binary rather than a new `tests/*.rs` file, unless process isolation is the point.
- A new test that takes over 1 s needs a written justification, or speeding up before merge.

## Changing a test for speed

A speed change must keep every test asserting what it asserted. The same tests pass, the count does not drop, and a test moved onto simulated time still trips on the defect it guards: reintroduce the defect briefly, or show explicitly why the same assertion still bites. Record before and after numbers in `docs/test-timing.md` and in the commit message.

## Standing duty

After any workspace run, compare the rank output with `docs/test-timing.md`. A binary over 5 s that is not listed there gets added with its cause. Leave each session with the suite no slower than you found it.
