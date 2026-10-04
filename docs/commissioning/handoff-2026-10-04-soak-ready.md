# Handoff 2026-10-04: deployed and CI-green, enable soak is next

Supersedes `handoff-2026-10-04-liveness-hardening.md`; that doc's history (soak table, RX-overrun causes A/B, open decisions, environment) still applies. Read this first.

## State

- **Local `main` = `57f91bb7`, NOT pushed.** `origin/main` = `93b494c3`. Push only after a soak PASS.
- **PR [#255](https://github.com/jaylamping/marengo/pull/255)** (`ci/liveness-hardening`) is at `57f91bb7`; all CI jobs pass (`check`, `vcan`, `sim`, `build-image`). It exists only to run CI; close or merge it once `main` is pushed.
- **Pi `.deploy-rev` = `57f91bb7`** (includes RX-overrun fix A `f49fd9be`). Gateway healthy; `marengo-pi` not running.
- **URDF** right-arm geometry from CAD landed earlier (`903c5a26`); the 2026-10-03 URDF handoff is done.
- User-owned, leave alone: `?? WATCHDOG.yml`, ` M .cursor/mcp.json`. Before `pi_sync_main`: `git stash push -m "local mcp.json host strip" -- .cursor/mcp.json`, pop after.

## Landed this session

|Commit|Change|
|---|---|
|`0431fcf0`|motor-repl `stop_independence` SIGTERM test sends the signal via `nix` (dev-dep) instead of the `kill` binary missing from the CI image. Fixed the PR #255 `check` failure.|
|`84da1838`|Bench session candump records kernel error frames: `candump -t z can0,#FFFFFFFF can1,#FFFFFFFF` (error mask only, keeps receive-all; no `-e`).|
|`54b81a4c`|marengo-candump parses the trailing `ERRORFRAME` marker (those lines were silently skipped).|
|`9e511f98`|`firmware-timing` JSON lists `kernel_error_frames`; error frames no longer perturb gap/load stats.|
|`7664b4e7`|`pi_enable_soak` report prints a `kernel error frames:` section. PASS rules unchanged.|

Merged as `57f91bb7`. MCP rebuilt (`just mcp-build`); the MCP server must be restarted (new omp session does this) to use the new candump.

## Next steps, in order

1. **Soak.** Get the operator to confirm the arm hangs limp at mechanical zero, supported, hands-off. Then `pi_enable_soak {confirm: true, set_zero: true, at_mechanical_reference: true, profile: "arm_attached", cycles: 20}`.
   - PASS = 20/20 clean, can0 `rx_over_errors` unchanged, `non_neutral_mit == 0`.
   - Then grep `bench-session.log` for `grant revoked` and `timestamp unusable` (expect 0), and run `pi_candump_summary`.
   - Cause B (receive blackout after Disabling a Run-mode drive) hit ~3 of 40 recent cycles, so a failure is likely. The `kernel error frames:` section now gives the overflow timestamps and controller flags; correlate them with the host Disable in the candump. Never relax the Transport latch (ADR needed).
2. **Push `main`** after PASS; close/merge PR #255.
3. **Cause B diagnostics** if it recurs (from the previous handoff): user's CANH–CANL resistance (expect ~60 Ω, power off) and can0 ground/shield tie — asked, not yet answered; scope CANH/CANL + MCP2515 INT across a finishing stop; IRQ tracing on CPU0.
4. **Gravity calibration** (`pi_gravity_calibrate`, operator present) and **bench re-check of RS03-tuned values** (velocity scale ±20 rad/s), as listed in the previous handoff.

## Notes

- MCP tests: one run in the main checkout showed 8 transient failures right after a heavy cargo build; three reruns were 210/210. Likely timing-sensitive bash-driven tests under load.
- Agent tooling: a user-level omp extension `~/.omp/agent/extensions/usage-bars.ts` now shows provider usage on the status row (`/usage-style` to change look). Not part of this repo.
