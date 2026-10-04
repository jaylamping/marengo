# Handoff 2026-10-03: crate audit merged, enable soak PASS, pushed

> Superseded by [handoff-2026-10-04-liveness-hardening.md](handoff-2026-10-04-liveness-hardening.md).

## State

- `main` is pushed to `origin/main`. It contains:
  - all of the 2026-10-03 crate audit: Phase B WP-A through WP-T and prune batches B1–B16;
  - the schema-tolerant reference journal fix (`c6428152`);
  - the two enable-liveness fixes below (`ad1eb887`, `aa773418`), and the ADR 0036 note (`7dbc4870`).
- The workspace is 15 crates and 6 bins (`marengo-homing` was folded into Davout).
- Gate on `aa773418`: fmt and both clippy runs (host and `aarch64` marengo-pi) are clean, and
  `cargo test --workspace` passes. Both fixes were checked red on their baseline and green after.
- The Pi runs `.deploy-rev` = `aa773418…`, deployed with `pi_sync_main` (cross). The gateway is ready and the
  taught limits were preserved.
- `stash@{0}` ("local mcp.json host strip") was popped after the push. `?? WATCHDOG.yml` belongs to the user;
  leave it alone.

## Enable soak (`pi_enable_soak`, profile `arm_attached`, 20 cycles)

|Rev|Dir (`var/enable-soak/`)|Clean|can0 rx_over Δ|non_neutral_mit|Peak frames/10 ms|
|---|---|---:|---:|---:|---:|
|`84e80653`|`20261003T221010Z`|14/20|0|0|67|
|`7dbc4870` (`ad1eb887`)|`20261003T225034Z`|19/20|0|0|65|
|`aa773418`|`20261003T230806Z`|**20/20 PASS**|0|0|63|

Earlier reference points: 42 frames/10 ms in the `edbaebaf` soak and 66 in the `3b4e87e6` soak. No overruns
in any of these three runs, so the stop-burst pacing suspicion from `3b4e87e6` stays unconfirmed.

### Failure 1: the host read gap (fixed in `ad1eb887`)

Symptom: `enable blocked: … full-master Robot Ready` right after `homing verified`, in 6/20 cycles.

- Receive times are host read times.
- stdin/Chappe Enable runs the gravity preflight (64–96 ms on the Pi, 3125 samples) without reading CAN.
- Target resolution then judged grant liveness (`comm_watchdog_ms` = 100 ms) on reads from before the
  preflight. A drive whose post-SetZero blackout covered the last read lost its grant with its reports still
  queued.

The fix:

- `resolve_enable_targets` drains before it builds the facets.
- Non-Active drains judge liveness after reading the queue.
- Active drains are unchanged.

### Failure 2: owed type-24 On during admission (fixed in `aa773418`)

Symptom: `enable failed: … right_shoulder_pitch: no private current-reference permission` in cycle 14.

- Pitch's stream was Off, with its type-24 On held through `POST_SET_ZERO_QUIET`.
- The quiet ended during synchronous enable work: the preflight, then identity admission waiting out roll's
  blackout.
- No reporting sync ran, so the On was never written and the silence counted from the quiet's end passed
  100 ms.

The fix: `resolve_enable_targets` and each admission poll in `verify_physical_identities` run
`sync_active_reporting()` first. The liveness rule and the excuse window are unchanged.

Guard tests for both fixes prove that a drive that is genuinely silent still loses its grant. Details are in
`docs/safety.md` (*Host read gap*, *Owed On during Enable admission*) and in the ADR 0036 *Host-caused
silence* amendment.

## Liveness hardening (merged, not yet bench-soaked)

- Kernel SocketCAN RX timestamps (`c5dc5307`): frames carry the kernel wire time instead of the host read time.
- Solicited silence and bounded owed-On excuse (`1b8de9cf`, `fd78722b`): Active silence counts from the host's own solicitation; a held type-24 On is excused until written, bounded by `OWED_ON_WRITE_BOUND` (200 ms).
- Gravity preflight swept across control ticks (`6b9ded86`, `938aa5f7`): at most 128 samples and 2 ms per tick, Enable only after a full passing sweep.
- Firmware-timing analyzer stream-Off gaps (`d8f7be69`): unobservable post-SetZero gap samples no longer count as blackout silence.

## Next steps, in order

1. **Deploy and re-soak liveness hardening.** Deploy main to the Pi and re-run `pi_enable_soak`, then grep the bench log for `kernel receive timestamp unusable` and `physical reference grant revoked`.
2. **Gravity calibration sweep** (`pi_gravity_calibrate`). The operator must be present and must re-confirm.
   Weighted profiles also need `confirm_weighted_motion: true`.
3. **Bench re-check of RS03-tuned values.** The RS03 velocity scale was corrected from ±50 to ±20 rad/s in
   `1ceeeb5`, so the `config/control.yaml` values tuned under the old scale need re-checking on the bench:
   - Pitch: velocity 1.25, accel 4.5, kd 3.0, slew 0.15.
   - Roll: velocity 0.7, accel 4.0, slew 0.35.
   - Friction fc 0.08.
   - Danger zone `elevated_shoulder_pitch_fall` max_velocity 0.45.

## Open decisions for the user

- ~~**Residual stall risk, two options.**~~ Option (A) adopted on `wt/liveness-adr` (ADR 0036 amendment
  *Solicited silence and owed Ons*): a held On is excused until written, bounded by `OWED_ON_WRITE_BOUND`
  (200 ms). Option (B), a cheaper or interleaved preflight, needs no rule change and remains open.
- ~~**Active-mode drains still judge liveness before reading.**~~ Resolved on `wt/liveness-adr`: Active
  drains judge after reading, and an Active target's silence counts from the earliest write it has not
  answered (ADR 0036 amendment).
- **Kernel RX timestamps (`SO_TIMESTAMPNS`).** These would make receive times wire-accurate instead of read
  times. Adopting them needs an ADR.
- NEEDS-DECISION items live in `docs/reviews/2026-10-03-crate-audit/phase-b/WP-*.md` and `PRUNE-*.md`.
  Integrator decisions taken so far are in `docs/reviews/2026-10-03-crate-audit/decisions.md`.
- Proto types that B13 marked deprecated are still on the wire. Deleting them needs a `buf breaking` exception.
- Hardware E-stop (BCM 17) is not installed. Drive CanTimeout and the fault-clear frame are still open (see WP-I).
- **Robstride vcan tests not run.** `just check-vcan` was not run for the RX-timestamp change: there is no Linux host here. Run it on Linux before treating the kernel-stamp path as bench-proven.

## Firmware and analyzer observations (no action taken)

- **Post-SetZero silence for can0/4 (elbow) reached 66.1 ms** in the PASS soak. The profile documents 45–61 ms.
  The blackout ended at most 606 ms after the SetZero, so the 800 ms quiet still has about 194 ms of margin.
  Update `docs/commissioning/firmware/robstride-timing-profile.json` if this holds up.
- **The `firmware-timing` analyzer misreads stream-Off gaps** as post-SetZero silence. That produced the 117–128 ms
  pitch "silences" and the n=1 or n=15 pitch counts. When pitch's stream is Off, its real blackout is not
  observable. Fixing the analyzer is a separate piece of work.

## Known flakes

- Under full-workspace parallel load, berthier `cs24_one_code_crawl` and davout `physical_reference` tests can
  fail. The firmware emulator uses wall-clock timing.
- Rerun them in isolation (`cargo test -p davout --test physical_reference -- <name> --exact`) before treating a
  failure as a regression.

## Agent tooling (user config, not in this repo)

- Implementation subagents are routed by `~/.omp/agent/extensions/jev-router.ts`.
  - Only `task` items with `agent` omitted or `"task"` are routed. An explicit agent name skips Jev.
  - To force a tier and keep the routing log, put `route: <agent>` in `solutionSpace`.
- `~/.omp/agent/agents/verifier.md` now pins `model: "@task_low"` (gpt-6-luna). This gives the verifier a
  different model family from the Opus and Muse implementers.
