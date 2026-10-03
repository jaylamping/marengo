# Phase B — WP-AC: Robstride wire protocol and CAN transport

Branch: `audit/wp-ac`. The behavior fixes and B6 API pruning share overlapping Robstride implementation and test files, so they are recorded as one combined code commit rather than implying a clean hunk-level split. No Davout enable/reference sequencing, timing constants, or Transport latch behavior was changed.

## Verdicts

| Lead | Verdict | Evidence | Result |
|---|---|---|---|
| L-robstride-01 | **FIXED (already present)** | RS03 scale is ±20 rad/s; `rs03_velocity_scale.rs` checks the bench ramp through the bounded feedback-report API and checks an encoded cruise command. | Retained the RS03 scale; removed only unused `MitRanges` convenience bounds. |
| L-robstride-17 | **CONFIRMED, FIXED** | Write encoders refuse `ZeroSta`/`AddOffset`; the bus refuses raw type-22 save frames. Coverage is in `params.rs` and `command_validity.rs`. | Ordinary parameter writes remain typed and guarded. A type-4 fault-clear encoder exists but is not wired into an operational API. |
| L-robstride-15 | **NEEDS-DECISION** | RS00 ±50 rad/s / 17 Nm remain disputed against the manual's ±33 rad/s / 14 Nm; no moving device-ID-5 capture was available. | No RS00 scale change. |
| L-robstride-02 | **NEEDS-DECISION** | The vendor documents type-4 `Byte[0]=1`, but fault-class clearing behavior and the resulting drive mode are not bench-qualified. Ordinary Disable is not fault clear; MCP wording now says so. | Keep power-cycle recovery; do not expose type-4 fault clear without explicit recovery-policy approval. |
| L-robstride-08 | **NEEDS-DECISION** | No capture of mode bits and drive behavior during an enabled RunMode write was available. | Capture the transition before adding a driver guard; do not guess a guard that could conflict with Davout's sequencing. |
| L-robstride-16 | **CONFIRMED, FIXED** | `finite_mit_values_outside_wire_range_are_rejected` and command-validation tests reject finite out-of-range MIT values before encoding/batch delivery. | No saturation of out-of-vendor-range command fields. |
| L-robstride-14 | **NEEDS-DECISION** | Type-21 fault/warning bytes remain raw; no known-fault capture confirmed the drive's byte order. | Preserve raw bytes and avoid naming bits pending a known-fault bench capture. |
| L-robstride-03 | **NEEDS-DECISION** | Echo attribution is still by host ID and exact frame, not socket origin; the current read path does not expose socket-origin metadata. | Preserve sole-CAN-owner discipline and echo gates; do not claim socket-level attribution. |
| L-robstride-09 | **NEEDS-DECISION (short-write seam)** | The Linux/vcan coverage exercises own-message echo and duplicate-address refusal; the atomic short-write path is not independently proven by an injectable seam. | Keep behavior unchanged; require Linux/vcan coverage for echo, short writes, and duplicate-address rejection, and change behavior only if that evidence fails. |
| L-robstride-18 | **NEEDS-DECISION** | SocketCAN queue overflow/drop counts are not reported; no read-only flood comparison was run. | Quantify kernel drops against a read-only candump baseline before adding overflow reporting. |
| L-robstride-06 | **CONFIRMED, FIXED** | FD conversion fails closed; short, remote, and malformed traffic is retained as malformed evidence rather than terminating the poll. The receive path counts skipped raw traffic against bounded work. | Foreign FD/RTR traffic no longer becomes a whole-poll receive failure solely because it is not an eight-byte classic Data frame. |
| L-robstride-07 | **CONFIRMED, FIXED** | `echo_only_positive_budget_poll_is_not_a_receive_timeout` exercises the feedback report path with only an own echo and a positive budget. | Echo-only completion no longer masquerades as a receive timeout. |
| L-davout-34 | **REFUTED** | `mit_control_all_at` validates the full batch and routes before sending; invalid commands/addresses and duplicate command identities are rejected before any frame is emitted. `command_validity.rs` covers preflight/no-prefix behavior. | No Davout change; Transport latch semantics unchanged. |
| L-robstride-11 | **CONFIRMED, FIXED** | Unknown communication-type traffic is traced as `unknown-comm-type` instead of being silently discarded. | No protocol action is taken for unknown types. |
| L-robstride-10 | **CONFIRMED, FIXED** | `SocketCanBus::open` maps `CanSocket::open` failures to `BusError::Driver`. | Open failures remain distinguishable from send failures. |
| L-robstride-13 | **CONFIRMED, FIXED (diagnostic gap)** | Ambiguous/unconfigured routing drops are traced. Matching remains fail-closed when an interface-less synthetic frame could match multiple configured buses. | No frame is guessed onto a bus; drops are now diagnosable. |
| L-robstride-19 | **REFUTED by the addressed cutover** | Unaddressed `MotorBus` command aliases were removed. `SocketCanRouter::send_frame` rejects unaddressed writes with multiple interfaces; a single-interface route is unambiguous. | Davout uses addressed `_at` methods. |

## B6 pruning

Completed the requested candidates from `prune-candidates.md`:

| Candidate | Result |
|---|---|
| P-robstride-01 | Removed unused `MitRanges::{p_min,p_max,v_min,v_max,t_min,t_max}` after retaining the RS03 ±20 scale. |
| P-robstride-03 | Removed `send_motion` and `JointMotion`. |
| P-robstride-04 | Removed the free `send_mit` function; updated Davout's data-flow comment and Rust-pattern example to the real addressed batch path. |
| P-robstride-05 | Removed unaddressed `MotorBus` command methods and `send_encoded_frame`; migrated callers/tests to addressed `_at` methods. Kept addressed Disable/stop paths. |
| P-robstride-06 | Removed legacy raw/timed receive projections and `RawReceiveReport::into_result`; callers/tests use `recv_raw_report` or the lossless feedback report. |
| P-robstride-07 | Removed `recv_all*`, projection helpers, and `FeedbackObservation::update_state`. Migrated receive/fault safety assertions to inspect ordered report events and retained delivered prefixes instead of deleting their safety coverage. |
| P-robstride-08 | Removed `mit_tx_id` and `mit_rx_id`. |
| P-robstride-09 | Removed `decode_mit_feedback`; the RS03 capture regression now decodes through `recv_feedback_report`. Updated the Robstride protocol ADR and codemap. |
| P-robstride-11 | Removed duplicate `MitFeedback.fault`; Davout's compatibility `MotorState.fault` is derived from `status_flags`. |
| P-robstride-12 | Removed private `FaultReport.device_id` and the unreachable fault decode branch after envelope/type/length validation. |

Preserved `ZeroSta` and `AddOffset` parameter identifiers/read behavior, `ParameterId::CanTimeout` and `LimitTorque`, and all addressed Disable/stop behavior. Updated `crates/robstride/codemap.md`, the Robstride intent map, ADR 0002, ADR 0021, `docs/rust-patterns.md`, and affected caller documentation.

## NEEDS-DECISION

- **L-robstride-02:** Keep power-cycle recovery. Do not wire the vendor type-4 `Byte[0]=1` fault-clear frame until a qualified recovery policy approves which faults may clear and how Reset/re-enable is handled. MCP Disable wording has been corrected because Disable is not fault clear.
- **L-robstride-03:** Keep sole-CAN-owner discipline and all echo gates. `read_frame` cannot attribute a received echo to a particular local socket, so do not treat host-ID matching as socket proof.
- **L-robstride-05:** Measure the 64-frame/256-attempt headroom with own-message echo enabled under the 23-motor workload, including a delayed poll and type-24 traffic, before changing either cap.
- **L-robstride-08:** Capture mode bits and drive behavior around RunMode writes while enabled, including Reset transitions. Do not add a speculative driver guard that could break Davout sequencing.
- **L-robstride-09:** Keep the short-write seam unproven; add/retain Linux/vcan evidence for echo, atomic short writes, and duplicate-address rejection. Make no behavior change unless that evidence fails.
- **L-robstride-14:** Keep type-21 words raw until a known-fault bench capture confirms byte order; do not assign fault/warning names from the SDK alone.
- **L-robstride-15:** Keep RS00 scales pending an ID-5 moving capture compared against position differentiation and a known torque command.
- **L-robstride-18:** Run a read-only SocketCAN flood comparison against candump to quantify kernel queue drops before adding overflow reporting.

## Gate

- `cargo test -p robstride -p davout` — **PASS**, 374 tests across 32 suites.
- `cargo test --workspace` — **PASS**, 1,142 tests across 121 suites, 1 ignored, and 10 compiler warnings.
- `cargo fmt --all -- --check` — **PASS**.
- `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings` — **PASS**.
- `just check` — **BLOCKED** on this macOS host: recipe invokes `docker compose build dev`, but `docker` is not installed (`sh: docker: command not found`).
