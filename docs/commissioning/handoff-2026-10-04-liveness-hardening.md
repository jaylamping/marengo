# Handoff 2026-10-04: liveness hardening merged locally, bench soak pending

Supersedes `handoff-2026-10-03-audit-soak.md`. Read this first in the next session.

## State

- **Local `main` is ahead of `origin/main` and NOT pushed.**
  - `origin/main` = `93b494c3`. That revision passed the 20/20 soak at `aa773418`.
  - Local `main` adds the liveness hardening, the RX-overrun fix and the doc/tooling fixes listed below.
  - Push `main` only after a bench `pi_enable_soak` PASS.
- **Draft PR [#255](https://github.com/jaylamping/marengo/pull/255)** (branch `ci/liveness-hardening`, at `1c2d323f`) exists only to run CI on these commits.
  - `vcan` passes; it runs the new kernel-timestamp tests on a real Linux kernel. `sim` passes.
  - `check` failed on the earlier push only on a pre-existing clippy `expect_used` in `marengo-host-metrics` Linux tests; that is fixed in `15912611`. Re-check its status.
  - Close the PR (or merge it) once `main` is pushed.
- **The Pi runs `.deploy-rev` = `9b1b3f8d`.** That deploy has the liveness hardening but NOT the RX-overrun fix `f49fd9be`.
- **User-owned items in the main checkout; leave them alone:**
  - `?? WATCHDOG.yml`
  - ` M .cursor/mcp.json`, the "local mcp.json host strip". Deploys need a clean tree, so before `pi_sync_main`, run `git stash push -m "local mcp.json host strip" -- .cursor/mcp.json`, then pop it afterwards.
- **Worktrees:** only `main`, plus the unrelated stale `~/.codex/worktrees/research-cache-await`. All slice worktrees and branches were merged and removed.

## What landed locally since `origin/main`

|Commit|Change|
|---|---|
|`c5dc5307`|robstride: SocketCAN frames and own-TX echoes carry the kernel RX time (`SO_TIMESTAMPNS`), mapped to `Instant` and clamped to the read time. Fallbacks (missing stamp, clock stepped backward, age over 1 s) are counted and logged as `SocketCAN kernel receive timestamp unusable`. Opening the socket fails if `SO_TIMESTAMPNS` can't be set.|
|`1b8de9cf`, `fd78722b`|Davout, ADR 0036 amendment *Solicited silence and owed Ons*:<br>- Liveness is judged after reading the queue, in Active too.<br>- An Active target's silence counts from the earliest host write it has not answered, and the comm watchdog follows the same rule.<br>- A held type-24 On is excused until it is written, bounded by `OWED_ON_WRITE_BOUND` = 200 ms.|
|`6b9ded86`, `938aa5f7`|marengo-pi: the gravity preflight is swept across control ticks (≤128 samples and ≤2 ms per tick; about 167 ms total on the Pi). Enable happens only after a complete passing sweep. Commands sent meanwhile are deferred, and discarded if the sweep is refused or voided (`enable refused: …`, `discarded N deferred command(s)`). AGENTS.md runtime contract updated.|
|`d8f7be69`|`firmware-timing` analyzer: stream-Off gaps are reported as unobservable, not as blackout silence. The firmware profile envelope is now blackout start 511.4–613.8 ms and length 44.5–66.1 ms; quiet margin 20 ms after the required 100 ms, hold margin 11.4 ms.|
|`f49fd9be`|Davout: while Enables are unwritten, targets without an Enable get no MIT frame, and bootstrap solicits are paced (one `BURST_GROUP_SPACING` group each) while a drive on that interface may still stream. This fixes overrun cause A below.|
|`b56de5d1`|MCP `pi_candump_summary` uses `/opt/marengo/bin/marengo-log-cli` (it was a PATH lookup that failed). This is already on `origin/main`; the MCP server needs a restart to load it, which a new session does.|
|`15912611`|`marengo-host-metrics`: allow `expect` in the Linux collector tests. This had been failing `main` CI since `edbaebaf`.|
|`e62c3037`|`.cursor/rules/windows-shell.mdc` is scoped to native Windows, and its host list is fixed per ADR 0018 (macOS and Windows; no WSL).|
|`1c2d323f`|`docs/pi-commissioning.md` overlay lines now match the bench Pi: `oscillator=16000000`, can0 `interrupt=23`, can1 `interrupt=25`.|

Docs updated along the way: ADR 0036 (three amendments), `docs/safety.md` (grant summary, *Host read gap*, *Owed On*, *Solicited silence while Active*, *Gravity preflight across ticks*, the RX-timestamp operator note, bootstrap solicit pacing), `docs/commissioning/firmware/*`, `AGENTS.md`.

## Enable soaks (`pi_enable_soak`, profile `arm_attached`, 20 cycles)

|Rev|Dir (`var/enable-soak/`)|Clean|can0 rx_over Δ|non_neutral_mit|Peak frames/10 ms|Failure|
|---|---|---:|---:|---:|---:|---|
|`84e80653`|`20261003T221010Z`|14/20|0|0|67|host read gap (fixed `ad1eb887`)|
|`7dbc4870`|`20261003T225034Z`|19/20|0|0|65|owed On in admission (fixed `aa773418`)|
|`aa773418`|`20261003T230806Z`|**20/20**|0|0|63|—|
|`9b1b3f8d`|`20261004T002637Z`|18/20|**+2**|0|68|RX overrun A (fixed `f49fd9be`) and B (open)|

In the `9b1b3f8d` soak: 0 `physical reference grant revoked` lines and 0 `timestamp unusable` warnings. The liveness hardening behaved; only the overruns failed.

## RX overrun causes (mcp251x keeps 2 RX frames)

- **A (cycle 5, fixed in `f49fd9be`, red→green RxFifo test):**
  - Berthier's neutral bootstrap MIT batch went to all five targets every tick while four Enables were held for the post-SetZero quiet.
  - Drives 2 and 3 were still streaming type-24 reports, which have the lowest CAN priority.
  - Two reports landed in one IRQ pass behind MIT 1, and pitch's reply was lost.
  - Residual: one paced write can still meet two coincident reports. Every single paced write shares this exposure.
- **B (cycle 16; also cycles 15 and 19 of the `3b4e87e6` soak): OPEN, not a host burst.**
  - A 3.8–5 ms receive blackout started 0.2–6.4 ms after the host Disabled a drive that was in Run (the reference target, at the end of its reference). Three drive frames then arrived in a window that holds two.
  - Pacing cannot cover a receive blackout, and stop bursts stay unpaced by design.
  - Read-only Pi checks found nothing wrong:
    - kernel 6.18.29 PREEMPT (not RT);
    - can0 has 0 bus errors, warnings or bus-off since boot;
    - no AER or throttling;
    - the mcp251x IRQ and the RP1 SPI IRQ are both on CPU0, and `irq/*-spi0.0` runs FIFO 50 on CPU2.
  - See also `docs/troubleshooting.md` (the Transport/overrun row) for the earlier ~0.7 ms receive-stall analysis.
  - Next diagnostics, in order:
    1. **Physical, needs the user:** the commissioning checklist's "CAN termination verified" box is unticked. With power off, measure CANH–CANL (expect about 60 Ω). Report whether can0's GND/shield is tied to the drives' supply ground.
    2. Run the soak candump with `-e`, so the overflow error frame and controller flags get kernel timestamps.
    3. Scope CANH/CANL and the MCP2515 INT line across a finishing stop. Trigger on the host Disable to the enabled target.
    4. If the board is clean, trace `irq/171-spi0.0` and the RP1 SPI IRQ on CPU0 during reference stops.

Do not relax the Transport latch; that needs an ADR.

## Next steps, in order

1. **Ask the user for the termination/ground measurements** (cause B, step 1).
2. **Deploy** current `main` with `pi_sync_main`. Stash `.cursor/mcp.json` first and pop it after.
3. **Soak:** first get the operator to re-confirm the arm is limp at mechanical zero, supported, hands-off. Then run `pi_enable_soak {confirm: true, set_zero: true, at_mechanical_reference: true, profile: "arm_attached", cycles: 20}`.
   - PASS = 20/20, can0 `rx_over_errors` unchanged, `non_neutral_mit == 0`.
   - Then grep `bench-session.log` for `grant revoked` and `timestamp unusable` (expect 0 of each), and run `pi_candump_summary` (it works after the MCP restart).
   - A cause-B overrun can still fail a cycle. If one does, go to the cause-B diagnostics; do not loosen anything.
4. **Push `main`** after a PASS, and close or merge PR #255.
5. **Gravity calibration sweep** (`pi_gravity_calibrate`). The operator must be present and re-confirm. Weighted profiles also need `confirm_weighted_motion: true`.
6. **Bench re-check of RS03-tuned values.** The velocity scale was corrected from ±50 to ±20 rad/s in `1ceeeb5`:
   - pitch: velocity 1.25, accel 4.5, kd 3.0, slew 0.15;
   - roll: velocity 0.7, accel 4.0, slew 0.35;
   - friction fc 0.08;
   - danger zone `elevated_shoulder_pitch_fall` max_velocity 0.45.

## Open decisions for the user

- **Local container gate on Apple Silicon.** `compose.yaml` pins `platform: linux/amd64`, so `just check` runs under Rosetta, about 7× slower. Berthier's wall-clock unit tests then time out against the 100 ms comm watchdog:
  - 19 failures at `93b494c3`, 2–3 now;
  - they all pass natively (macOS) and are expected to pass on CI's native amd64 runner.

  Options: (a) treat CI and native `cargo test` as the gate on this Mac; (b) add an arm64 dev image for local use; (c) make those berthier tests independent of wall-clock time.
- NEEDS-DECISION items in `docs/reviews/2026-10-03-crate-audit/phase-b/WP-*.md` and `PRUNE-*.md`. Integrator decisions taken so far are in `docs/reviews/2026-10-03-crate-audit/decisions.md`.
- Proto types that B13 marked deprecated are still on the wire. Deleting them needs a `buf breaking` exception.
- Hardware E-stop (BCM 17) is not installed. Drive CanTimeout and the fault-clear frame are still open (WP-I).

## Gate status on `1c2d323f`

- Native macOS (this session):
  - `cargo fmt --check` clean;
  - clippy clean, both host (excluding Linux-only crates) and `--workspace --all-targets --target aarch64-unknown-linux-gnu`;
  - `cargo test --workspace` passed 1246/0 on `1c2d323f`;
  - deps check exit 0; MCP tests 207/207.
- Container `just check`: everything up to `cargo test` passes; only the berthier wall-clock tests fail under Rosetta (see above).
- CI `check` on PR #255 (`1c2d323f`) fails one test, and the failure predates this branch:
  - `motor-repl` `stop_independence::sigterm_disables_every_drive_then_exits_143` panics at `bins/motor-repl/tests/stop_independence.rs:169` with `kill: NotFound`. The test shells out to a `kill` binary that the CI dev image does not have.
  - It was added in `edbaebaf`. `main` CI never reached it before, because the host-metrics clippy error (fixed in `15912611`) failed the job first.
  - Fix one of two ways: add `procps` to `docker/Dockerfile.dev`, or send the signal without the external binary (for example `nix::sys::signal::kill` as a dev-dependency). `vcan` and `sim` pass.

## Environment (new Mac, Apple Silicon)

- **Rust:** cargo lives in `~/.cargo/bin`. `~/.zshenv` now sources `~/.cargo/env`. The omp tool shell inherits its environment from the terminal that launched omp, so start omp from a new terminal. Until then, prefix commands with `PATH="$HOME/.cargo/bin:$PATH"`.
- **Docker Desktop:** installed (engine 29.8.1, `desktop-linux` context, Rosetta on, 18 CPUs, about 32 GB). `just build` works. The Docker settings file is not readable from the agent shell (macOS privacy), so resource changes go through the Docker Desktop UI.
- **SSH to the Pi:**
  - `~/.ssh/config` has `Host joey-robot.tail0b414.ts.net marengo.local`, `User joey`, key `~/.ssh/id_ed25519_marengo`.
  - The harness host `marengo-pi` is registered in `~/.omp/agent/ssh.json` and loads at session start.
  - Use it for read-only inspection only. Pi actions go through `pi_*` MCP tools.

## Agent tooling changes (user config, not in this repo)

- **Jev routing** (`~/.omp/agent/extensions/jev-router.ts`) routes only `task` items whose `agent` is omitted or `"task"`.
  - The rewrite is persisted: your own transcript later shows `"agent": "task-…"` even though you omitted it. The `Jev routing:` note and `~/.omp/agent/routing-log.jsonl` are the evidence.
  - Never set `task-*` yourself; force a tier with `route: <tier>` in `solutionSpace`. These notes are in `~/.omp/agent/APPEND_SYSTEM.md`.
- **New tier `task-extreme`** (`~/.omp/agent/agents/task-extreme.md`, role `task_extreme` = `anthropic/claude-fable-5-1:high`) is for the hardest problems only, such as a root cause still unknown after a `task-high` attempt.
  - Jev assigns it only on a confident pick (≥ 0.6); a low-confidence pick drops to `task-high`, and a tie-break never upgrades into it.
  - Safety overrides are now a floor (`task-high`), not a cap.
  - The escalation chain is low → medium → high → extreme.
  - The decision logic was smoke-tested on 8 synthetic Jev answers, all as expected.
- **Model resolution order** is `task.agentModelOverrides` → agent `model:` frontmatter → role/session fallback.
  - The overrides for `task-low`/`-medium`/`-high`/`-extreme` are now role aliases (`"@task_low"` and so on), so the thinking levels in `modelRoles` apply. Before this, plain model pins dropped them.
  - `verifier` is overridden to `anthropic/claude-sonnet-5-5`, which shadows `model: "@task_low"` in `verifier.md`. The override is the user's choice; the frontmatter is inert.
- **Agent templates** `~/.omp/agent/agents/task-{low,medium,high,extreme}.md` have a *Style* rule (no narration between tool calls) and capped reports: about 150 words for low and medium, about 350 for high and extreme. Long evidence goes to `local://<task-name>.md`.
- **`verifier.md`** tells it to use long command timeouts and to look for cargo in `~/.cargo/bin`. One verifier run lost its shell tools mid-check ("only GetDynamicTools and CallDynamicTool remain"); watch for this.
- **`config.yml` model roles** at handoff: `task_low` = `gpt-6-luna:high`, `task_medium` = `muse-spark-1.3-contributor:high`, `task_high` = `claude-opus-5-5:high`, `task_extreme` = `claude-fable-5-1:high`. The user also edits this file directly, so re-read it rather than trusting this list.
- **Worker hygiene:** run parallel slices in separate git worktrees. Workers edited the main checkout by mistake three times this session; check `git status` in the main checkout before merging.

## Known flakes

- Under full-workspace parallel load, berthier `cs24_one_code_crawl` and davout `physical_reference` can fail; the firmware emulator uses wall-clock timing. Rerun them in isolation before calling a failure a regression.
- Under Rosetta in the container, the berthier `loop` wall-clock tests fail (see Open decisions).
