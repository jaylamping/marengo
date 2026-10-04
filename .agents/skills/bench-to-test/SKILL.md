---
name: bench-to-test
description: Turn a bug seen on the Marengo bench into a deterministic failing test before fixing it - choose the harness that can express it (FirmwareBus for wire, timing and sequencing; SimulationBus / ControlLoop::from_simulation for loop logic; the fitted-plant bench replay for motion quality; plain unit tests for pure functions), prove it red on the baseline, then green with the fix. Use as soon as a bench failure's cause is identified, and whenever a fix is proposed for behaviour that has only been seen on hardware.
---

# Bench to test

The repo's rule is that every bug fix ships with a test that fails on the baseline. Bench bugs are where that is hardest and matters most: the arm cannot run in CI, so the test is the only thing that keeps the bug from returning. The test reproduces the **mechanism** found on the bench, with numbers taken from the bench evidence.

## 1. State the mechanism

From the diagnosis (`fault-signatures`, `can-timeline`, `trace-forensics`), write the causal chain in one or two sentences, with the measured numbers. For example: "Two type-24 reports landed within 0.3 ms after the neutral solicit to pitch while four Enables were held; the 2-buffer FIFO dropped pitch's reply and Transport latched (soak `20261004T002637Z`, cycle 5)." A test cannot be written against a symptom, only against a mechanism.

## 2. Pick the harness

| Mechanism lives in | Harness | Examples to copy |
|---|---|---|
| Wire order, echoes, reply latency, post-SetZero blackout, reporting streams, RX FIFO overrun, drive reboot or silence | `FirmwareBus` firmware emulator, `crates/davout/tests/physical_firmware/mod.rs` (`Firmware::from_motors`, `Drive` knobs, `report_at`, `RxFifo` + `enforce`, `wire_time_stamps`) | `crates/davout/tests/exit_reporting.rs`, `drive_loss.rs`, `physical_reference.rs`, `receive_bounds.rs` |
| Supervisor or control-loop logic: mode transitions, refusals, fault latching, enable gating, intent handling | `davout::simulation::SimulationBus` with `Supervisor::from_simulation(…)`, or `ControlLoop::from_simulation*` (`crates/berthier/src/loop.rs`) | `crates/berthier/tests/feedback_bootstrap.rs`, `tick_error_intent.rs`, `degraded_hold.rs`; `crates/davout/tests/fault_authority.rs` |
| Motion quality: tracking, overshoot, stop error, drift, slip, zone caps | Production `PositionHold` against a plant fitted to the bench traces, scored like `--score-bench` | `crates/berthier/src/position_hold_tests/bench_replay.rs`; plant constants come from `trace-forensics` numbers |
| Pure maths or parsing: τ_g, friction, envelope, encode/decode, candump parsing | Unit test in the module's `x_tests/` file, or `MemoryBus` for frame tests | the crate's existing `#[path = "x_tests/…"]` modules |

`SimulationBus` never echoes, so echo-dependent sequencing belongs in `FirmwareBus`. A failure that comes from firmware timing outside the measured envelope first needs the envelope updated (`docs/commissioning/firmware/robstride-firmware-behavior.md`, `cargo test -p davout --test firmware_profile`). Done when the harness is chosen and you can name the knob or input that reproduces each link of the mechanism.

## 3. Write it red

- Set up only what the mechanism needs. Safety tests copy the master config and URDF into a `TestDirectory` (`crates/davout/tests/support/mod.rs`) and pass explicit calibration and journal paths; never let a test write into `config/` or `/opt`.
- Use the bench numbers: latencies, blackout windows, the friction curve, the inertia, the poses.
- Assert the behaviour the safety contract requires (e.g. "no reply is dropped", "Transport does not latch", "stop error ≤ 0.01 rad"), not the wording of a log line.
- Make it fast by construction (`test-speed`). Advance simulated time instead of sleeping through watchdogs, quiets or fuse budgets; use the smallest input that reproduces the mechanism; add the test to the crate's existing test binary instead of a new `tests/*.rs` file. A regression test the owner waits on every run must earn its seconds.
- Test-file conventions: a file-level `#![allow(clippy::expect_used)]`; leave the parent process environment untouched (re-exec a child for env-dependent cases); no hardware in default `cargo test` (hardware tests are `#[ignore]` behind the `socketcan` feature).

Run the narrowest command (`cargo test -p davout --test <file> <name>`, `cargo test -p berthier <module>::<name>`) on the unfixed code. It must fail, and fail for the mechanism's reason. Read the assertion message to confirm. Done when it is red for the right reason; record the red output.

## 4. Fix and go green

Size the fix to the best design, not the smallest diff. When the cause is in the control logic, load `control-rewrite`: a redesign of the function or module is welcome when you can make the case for it. Make the fix, rerun the test, then run the crate's suite and clippy with `-D warnings`. Note any change that alters physical tuning or a safety mechanism: tuning needs bench evidence, and a safety mechanism needs an ADR. Done when the test is green with the fix, red without it, and the crate suite passes.

## 5. Close the loop on the bench

A green test proves the model of the bug, not the robot. Deploy (`pi_sync_main`) and rerun the original bench session through `bench-run`, with the same question, tool and parameters. Update the `fault-signatures` entry with the fixing commit and the test name. In the commit message, name the bench session that exposed the bug.
