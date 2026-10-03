# Handoff 2026-10-03: crate audit merged, soak pending, push held

## State

- Local `main` is at `84e80653` plus this doc. It is **88+ commits ahead of `origin/main` and NOT pushed.**
- It contains all of the 2026-10-03 crate audit: Phase B WP-A through WP-T, prune batches B1–B16, and the
  schema-tolerant reference journal fix (`c6428152`).
- The workspace is 15 crates and 6 bins (`marengo-homing` was folded into Davout).
- Gate on `84e80653`: fmt and both clippy runs (host and `aarch64` marengo-pi) clean, `cargo test --workspace`
  1209 passed / 0 failed, MCP 207/207, `scripts/deploy-config-lock.test.sh` 3/3. Consul tests and build were last
  run at `a6ed8997` (384 tests); no Consul source changed after that.
- Deployed to the Pi with `pi_sync_main` (cross): `.deploy-rev` = `84e80653…`, gateway ready, taught limits
  preserved. The deploy also deleted the stray `config/.marengo-profile.lock` from Pi staging, because the deploy
  now excludes it.
- Worktrees: only `main` (plus the unrelated, stale `~/.codex/worktrees/research-cache-await`).
- The main checkout has two items that belong to the user. Leave both alone:
  - `?? WATCHDOG.yml`
  - `stash@{0}` "local mcp.json host strip". Pop it after the soak; deploys need a clean tree.

## Why the last soak failed and what fixed it

The `pi_enable_soak` run on `3b4e87e6` (`var/enable-soak/20261003T205602Z`) failed 0/20:

- Every `right_shoulder_pitch` reference ended `journal Failed { reference codec: unknown field
  allow_firmware_speed_mode }`.
- WP-O removed that key, and `deny_unknown_fields` then rejected the historic journal rows that still contained
  it. `Database::validate_history` fully decoded every historic row against the live config types.

`c6428152` fixes this:

- History rows now get structural checks only: checksum, identity via `decode_identity`, ordering and capacity.
  History never grants.
- The full typed decode still runs for the row being written and for its readback.
- `inspect()` lists integrity-verified rows it can't decode as explicit legacy records.
- Red→green tests:
  - `older_schema_row_with_retired_control_key_still_opens_and_appends`
  - `corrupt_old_schema_row_is_refused_and_preserved`
- ADR 0036 has a 2026-10-03 amendment, and `docs/safety.md` has a one-line note under Physical reference grants.

That soak also showed two `rx_over_errors` increments, in cycles 15 and 19, each with a SafetyHazard invalidation.
Peak bus density was about 66 frames/10 ms, against 42 in the `edbaebaf` soak. The cause is still unexplained.
The suspects are unpaced abort/shutdown stop bursts (fault, E-stop and shutdown stops are never paced, by
design).

## Next steps, in order

1. **Operator check (physical).** Ask the user to confirm again that the arm hangs limp at its mechanical zero,
   is supported, and is hands-off. The soak runs SetZero on all 5 joints every cycle.
2. **Soak** via MCP:
   `pi_enable_soak {confirm: true, set_zero: true, at_mechanical_reference: true, profile: "arm_attached", cycles: 20}`
   - PASS = 20/20 clean, can0 `rx_over_errors` unchanged, and `firmware-timing` `non_neutral_mit == 0`.
   - After it, run `pi_candump_summary` and compare peak density with the 42 and 66 frames/10 ms figures above.
   - If `rx_over` grows again, diagnose the stop-burst pacing before doing anything else. Do not relax the
     Transport latch; that needs an ADR.
3. **Push** `main` only after a PASS. Then pop `stash@{0}`.
4. **Gravity calibration sweep** (`pi_gravity_calibrate`). The operator must be present and must re-confirm.
   Weighted profiles also need `confirm_weighted_motion: true`.
5. **Bench re-check of RS03-tuned values.** The RS03 velocity scale was corrected from ±50 to ±20 rad/s in
   `1ceeeb5`, so `config/control.yaml` values tuned under the old scale need re-checking on the bench. They are:
   - Pitch: velocity 1.25, accel 4.5, kd 3.0, slew 0.15.
   - Roll: velocity 0.7, accel 4.0, slew 0.35.
   - Friction fc 0.08.
   - Danger zone `elevated_shoulder_pitch_fall` max_velocity 0.45.

## Open decisions for the user

- NEEDS-DECISION items live in `docs/reviews/2026-10-03-crate-audit/phase-b/WP-*.md` and `PRUNE-*.md`.
  Integrator decisions taken so far are in `docs/reviews/2026-10-03-crate-audit/decisions.md`.
- Proto types that B13 marked deprecated are still on the wire. Deleting them needs a `buf breaking` exception.
- Hardware E-stop (BCM 17) is not installed. Drive CanTimeout and the fault-clear frame are still open (see WP-I).

## Known flakes

- Under full-workspace parallel load, berthier `cs24_one_code_crawl` and davout `physical_reference` tests can
  fail. The firmware emulator uses wall-clock timing.
- Rerun them in isolation (`cargo test -p davout --test physical_reference -- <name> --exact`) before treating a
  failure as a regression.

## Agent tooling changed this session (user config, not in this repo)

- Implementation subagents are routed by `~/.omp/agent/extensions/jev-router.ts`. It loads at session start.
  - A `task` call with `agent` omitted (or `agent: "task"`) is sent to TypeSafe Jev, which picks `task-low`,
    `task-medium`, `task-high`, `scout` or `designer`.
  - Safety overrides (motion/CAN/enable, auth, data loss) force `task-high`.
  - Low-confidence ties go to the stronger model.
- Each decision appears as a `Jev routing:` note and is appended to `~/.omp/agent/routing-log.jsonl`. If the
  first dispatch shows no such note, the extension did not load.
- `route: <agent>` in a brief forces a route.
