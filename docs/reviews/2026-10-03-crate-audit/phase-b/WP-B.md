# Phase B — WP-B: Davout TX pacing and enable/reference sequencing

Branch: `audit/wp-b`. The sequencer proposal is design-only in [WP-B-enable-sequencer-design.md](WP-B-enable-sequencer-design.md); no production sequencer rewrite was made. Existing mainline pacing/blackout/reference fixes were audited in place. One additional fix addresses L-davout-35.

“Red → green” is recorded only for the new lease regression test below. Existing emulator coverage is cited as current-code evidence; this work did not claim historical pre-fix runs for those existing tests.

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-davout-27 | **ALREADY-FIXED** | `physical_firmware::receive_error_after_arming_stops_every_drive_and_clears_the_reservation` injects an RX error after Enable and asserts every configured address is Disabled after the last Enable, with reservation cleared. `advance_reference` now catches propagated errors and calls `finish_reference`; `calibrate_joint_zero` maps that error to `HomingVerify` after the owner cleanup. | Existing fix; calibrate path verified. |
| L-davout-01 | **ALREADY-FIXED** | `reference_write_bursts_are_spaced_on_the_bus`, `identity_admission_requests_leave_one_spacing_apart`, and `status_solicit_disables_leave_one_spacing_apart` exercise 2 ms per-interface groups. `active_reporting` writes use the 5 ms slot. Fault/E-stop/cancel/shutdown stop path remains unpaced by design. | Existing `BurstPacer` covers baseline/normal finish stop groups, reporting Off, identity admission and status solicitation. No emergency-stop pacing added. |
| L-davout-25 | **CONFIRMED** | Timing rules/anchors remain distributed among Supervisor, feedback consumer, physical-reference grant tracking, active reporting, and reference transaction code. Design review is captured in `WP-B-enable-sequencer-design.md`, including R0–R15/P1 clocks, anchors, transitions, migration and emulator/soak strategy. | Design only as assigned; no rewrite. |
| L-davout-02 | **ALREADY-FIXED** | Emulator tests `enable_right_after_the_last_reference_is_held_past_the_set_zero_blackout` and `every_grant_is_valid_at_every_offset_after_the_last_reference` exercise the 450–800 ms no-Off/Enable rule; active reporting defers type-24 writes for held blackout addresses. | Existing active-reporting hold and 800 ms quiet gate. |
| L-davout-03 | **ALREADY-FIXED** | `a_grant_survives_the_gap_between_the_enable_echo_and_the_first_run_reply` and `a_drive_that_died_while_the_host_held_its_stream_off_loses_its_grant` cover first-feedback grace and liveness after host-withheld stream silence. Grant timing is independent of the diagnostics flag; type-24 stale retry is 200 ms while grant silence is counted from the proper anchor. | Existing post-SetZero liveness/withheld-silence fix. |
| L-davout-08 | **CONFIRMED** | `verify_physical_identities` still synchronously retries and waits up to the identity deadline; `calibrate_joint_zero` synchronously advances acquisition and commit, sleeping between polls. In marengo-pi these operations run on the control-owner call path, so they can block ticks. | Not changed: moving acquisition off the owner thread changes ownership/cancellation and stop semantics; see decision D-B1. |
| L-davout-21 | **CONFIRMED** | The physical-firmware emulator now tests numerous wire-level echo/blackout/pacing rules, but SimulationBus/MemoryBus remain non-echoing and do not exercise echo-path interactions end-to-end with Berthier/marengo-pi, including R7. The 20/20 Pi enable soak is bench coverage, not that missing deterministic integration seam. | Emulator coverage is partial mitigation; see D-B2. |
| L-davout-24 | **ALREADY-FIXED** | `identity_request_dropped_in_post_set_zero_blackout_is_asked_again` demonstrates a dropped reference identity request is retried and acquisition proceeds only on a post-watermark response. | Existing retry in reference acquisition. |
| L-davout-28 | **CONFIRMED** | `abort_reference_for_hazard`, `cancel_reference_for_shutdown`, and `latch_control_fault` still discard cleanup/stop errors (`let _ = …` / `.ok()`), so the caller cannot distinguish a completed stop from failed stop delivery. The fault authority does latch the hazard; no changed code here upgrades the cleanup-delivery result. | Not changed because these public hooks are currently void/terminal interfaces and alterable stop-error propagation needs an explicit contract; see D-B3. |
| L-davout-05 | **CONFIRMED** | Reference `ArmTarget` writes `enable_drive_at` without a preceding explicit RunMode::Mit command; unlike normal session enable, it depends on the drive’s persisted run mode. Emulator does not prove behavior after external Motor Studio mode change. | Not changed: extra mode-setting frame changes reference wire ordering/timing; supported-hardware compatibility qualification required; see D-B4. |
| L-davout-06 | **REFUTED** | Physical acquisition’s AwaitAck accepts only a type-2 frame popped after SetZero (receive order); `stale_pre_set_zero_status_is_not_an_ack` proves a delayed earlier status cannot qualify. Separate post-request mechPos readback remains required. | No change. |
| L-davout-04 | **CONFIRMED** | Timing values remain constants, not derived from `loop_hz`/`comm_watchdog_ms`: type-24 5 ms slot, identity 100 ms, 800 ms SetZero quiet and 2 ms burst spacing. The test design explicitly pins the observed physical profile rather than proving arbitrary configuration combinations. | No timing retune without profile evidence; see D-B5. |
| L-davout-32 | **CONFIRMED** | Physical-specific blackout/Off-settle gates are conditional on `echoes_transmissions()`. A physical reference backend over a non-echoing bus can therefore skip gates that are meaningful only with echo evidence. No production SocketCAN route currently uses a non-echoing bus, but the owner abstraction permits it. | Recommend refuse physical acquisition/session admission when the transport cannot meet the echo contract; see D-B6. No fallback to weakened gates. |
| L-davout-07 | **CONFIRMED** | Public operational mode/drive-active reporting can become Active before all staggered Enable writes have been emitted; `enable_writes_pending` separately exposes that pending state to the first-feedback grace logic. | Preserve deployed operator-visible mode semantics for now; separate `AdmissionPending` status needs compatibility decision, D-B7. |
| L-davout-40 | **CONFIRMED** | Existing tests cover stale SetZero acknowledgements, readback tolerance/status, coordinate discontinuity and failed stop evidence, but trusted-coordinate mapping and selected Transport latch distinctions (Delivery/Backend versus stop/reporting failures) are not asserted end-to-end in the firmware suite. | Coverage gap remains; test seam additions should assert observed evidence and latched fault class, not internal wiring. |
| L-davout-09 | **CONFIRMED — FIXED** | New `physical_reference::identity_request_transport_failure_latches_and_stops`: red because failed type-0 admission returned a Bus error without latching Transport; after the fix it asserts the fault latch, no Enable transmission, failed TX evidence and all-address stop. | `enable_targets_inner` now routes identity-admission errors through `stop_after_runtime_error`; transport failures latch and stop immediately, while non-transport identity refusals remain fail-closed. |
| L-davout-11 | **REFUTED** | Off/Enable strictness is based on echo state and per-address write/echo timestamps; prior-session echoes are consumed before an enable gate is opened and cannot satisfy the current reporting-Off gate. `missing_reporting_off_echo_withholds_enable_and_fails_closed` and the blackout wire-order tests exercise the gate. | No change. |
| L-davout-35 | **CONFIRMED — FIXED** | New `lease_inputs_are_canonical_and_ttl_overflow_is_rejected`: red on baseline lease code with `overflow when adding duration to instant`; green after fix. It also proves acquire with padded IDs can be released using normalized ID. | `checked_add` rejects unrepresentable TTLs; acquire/renew/release consistently trim IDs. `InvalidTtl` maps to a normal Davout lease error. |
| L-davout-39 | **REFUTED** | `observe_position`’s return is intentionally observed through the next `reference_binding_valid`; permission-requiring paths (`ensure_reference_for`, active-set checks, command/tick admission and homing-state query) call binding validation before acting. An observed discontinuity therefore fails closed before the next protected operation. `coordinate_discontinuity_after_grant_revokes` covers revocation. | No change. |
| L-davout-10 | **REFUTED** | The guard is defensive only: ordered AwaitReportingOff/ArmTarget transitions prevent its invalid phase from occurring through a valid transaction. The guard does not change a reachable normal sequence. | No change. |
| L-davout-41 | **CONFIRMED (latent)** | `u64` epoch/generation increments still saturate, and receive-order clamp remains defensive rather than a refusal. These require counter exhaustion / invalid internal order, not normal bench operation; virtual-only initial coverage is separate from physical permission. | No change absent a concrete reachable failure; retain fail-closed boundary and track through sequencing refactor. |

## NEEDS-DECISION

### D-B1 — blocking identity/reference work on control-owner thread (L-davout-08)
- **A (recommended):** make admission/reference an explicit asynchronous owner transaction, advanced in bounded loop work. This preserves single-owner bus I/O and responsive ticks but changes public operation completion/cancellation APIs and must retain same-call cleanup on abort.
- **B:** keep synchronous behavior and document a measured maximum blocking budget; simpler, but control-loop watchdog headroom remains consumed by the wait.
- Recommend A, staged after the sequencer design, not by moving CAN I/O to another thread.

### D-B2 — echo-path integration seam (L-davout-21)
- **A (recommended):** extend the deterministic firmware bus/emulator into the owner integration tests for Berthier grace, pending enable writes and strict Run feedback; keep SimulationBus explicitly non-echoing.
- **B:** add a second configurable SimulationBus echo mode, at the cost of duplicate firmware/echo behavior and potentially misleading simulation fidelity.
- Recommend A; the firmware emulator remains the wire-conformance authority.

### D-B3 — cleanup stop failure observability (L-davout-28)
- **A (recommended):** return or record structured cleanup outcome from hazard/shutdown hooks while preserving immediate stop attempt and latched fault authority.
- **B:** retain void hooks and rely only on logs/fault snapshots; callers remain unable to tell stop delivery failed.
- Recommend A, with transport latch/terminal evidence preserved; never defer the stop.

### D-B4 — force MIT mode during reference arm (L-davout-05)
- **A (recommended after hardware qualification):** explicitly command MIT RunMode before reference Enable and await its required evidence if firmware supports it.
- **B:** retain persisted-mode assumption and enforce/document a commissioning precondition.
- Recommend A only after confirming Robstride firmware behavior and measuring sequencing; do not add a guessed frame to the enabled path.

### D-B5 — derive timing policy from config/profile (L-davout-04)
- **A (recommended):** centralize validated timing policy and derive loop-relative slots/deadlines from it while keeping measured firmware blackout/quiet and burst bounds explicit.
- **B:** keep constants pinned to the qualified 200 Hz bench profile and reject unsupported loop/watchdog combinations.
- Recommend B until alternate configurations are actually supported and measured; do not casually derive the 800 ms physical quiet from a loop rate.

### D-B6 — non-echoing physical owner (L-davout-32)
- **A (recommended):** refuse physical admission before any arm/SetZero if the transport cannot provide required echo evidence.
- **B:** allow only an explicitly virtual/non-physical backend to use the non-echoing branch.
- Recommend A for any physical backend; no physical gate may be silently skipped.

### D-B7 — Active reporting during staggered enable (L-davout-07)
- **A (recommended):** preserve current public `Active` semantics for compatibility while publishing pending-target detail separately.
- **B:** delay Active/drive_active until every target’s Enable write/echo/Run succeeds; clearer status but changes monitoring and admission behavior.
- Recommend A until consumers and status semantics are migrated together.

## Cross-package edits

None. `lib.rs` changes are within Davout ownership. No physical tuning values or enable/reference timing behavior were changed.

## Gate

Targeted regression red → green:
- L-davout-35 red: `cargo test -p davout active_reporting::tests::lease_inputs_are_canonical_and_ttl_overflow_is_rejected -- --exact` failed with `overflow when adding duration to instant` on baseline `now + ttl`; green: same command passed after `checked_add` and normalized lease IDs (1 passed).
- L-davout-09 red: `cargo test -p davout --test physical_reference identity_request_transport_failure_latches_and_stops -- --exact` failed because identity send failure did not latch a fault; green: after routing identity errors through `stop_after_runtime_error`, the test passed with no Enable, latched Transport fault, failed-TX evidence, and all-address stop.

Final required gates: `cargo fmt --all -- --check` passed; `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings` passed; `cargo test --workspace` passed (1,151 passed, 1 ignored, 10 warnings). Warnings were the known macOS dead-code warnings in host-metrics and marengo-pi host_metrics. The MCP was untouched, so `npm test` was not run; `cargo clippy -p marengo-pi` was not applicable.
