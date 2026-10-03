# Phase B — WP-M: Chappe IPC robustness

Branch `audit/wp-mq`; package `crates/chappe` (except the `ipc_outbox.rs`
dropped counter, which belongs to WP-L), `marengo-pi` `main.rs` loop/IPC
wiring, the topic-contract duplication (`marengo-gateway` `state.rs` topic
lists plus `marengo-pi` topic lists), and the Berthier tick publish path.

"Red" evidence below is either an observed baseline failure, the former
regression assertion/code path, or baseline code inspection where executing
the unsafe behavior would invoke live operations. Green evidence is from the
listed regression tests and acceptance gates.

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-berthier-24 (S2) | **CONFIRMED** | Baseline `tick_inner` used `?` on the Chappe publish (`loop.rs` reference-busy and steady-state paths), so any `BusError` became a tick error and `marengo-pi` ran `disable_all` on it; `publish_bytes` checked `receiver_count()==0` then `send`, so a subscriber detaching in between surfaced as `Err`. New `berthier::loop::tests::telemetry_failure_is_counted_not_propagated` and `chappe_telemetry_never_fails_the_tick` pass; `chappe::tests::publish_after_last_subscriber_dropped_succeeds` and `concurrent_publish_on_existing_topic_delivers_to_subscriber` pin the closed race. | Tick publish is now `publish_robot_state_counted`: failures increment `telemetry_failures` and `warn!`, never propagate. `Bus::publish_bytes` sends with `.ok()` — a gone receiver is success by construction, closing the check-then-send race. The 1 Hz Pi debug logs both counters. |
| L-chappe-03 (S2) | **CONFIRMED** | Baseline accept loop broke on the first non-`WouldBlock` error with only a `warn!`, leaving `send_command` failing with "no ipc peer" forever and no health signal. New `ipc::tests::accept_retry_gives_up_after_bounded_failures` and `accept_retry_resets_on_success` pass. | Transient accept errors retry with 250 ms backoff up to 20 consecutive failures; every failure increments `AcceptHealth::failures`, giving up sets `AcceptHealth::dead`, and `send_command` with no peer names a dead accept loop distinctly. |
| L-chappe-07 (S2) | **CONFIRMED (inspection)** | Freshness is `SystemTime` on both ends with a 1000 ms age window (`command_is_current`): an NTP step can refuse or admit commands, and a captured frame replays within the window. | No behavior change: a real fix needs a monotonic sequence or boot-epoch in the envelope (wire change, coordinated gateway+Pi deploy). The window and the NTP/replay exposure are now documented at the admission site. See NEEDS-DECISION. |
| L-chappe-02 (S3) | **CONFIRMED** | Baseline `WARN`/`ERROR` bypassed the tracing quota, so a per-tick warn flooded the in-process broadcast and the gateway DB writer; `connect_with_retry` logged `error!` every ~6 s while the gateway was down. New `tracing_layer::tests::warn_storm_is_capped_and_counted`, `suppression_counters_distinguish_severity`, and `quota_buckets_are_independent_per_severity` pass; the existing refill/concurrency quota tests still pass. | `WARN`/`ERROR` share a bounded 10/s budget independent of the 40/s info budget, with lifetime `dropped_normal`/`dropped_urgent` suppression counters. Reconnect failure now logs `warn!`, not `error!`. |
| L-chappe-09 (S3) | **CONFIRMED** | Topic strings were triplicated across chappe `ipc.rs`/`ipc_outbox.rs`, gateway `state.rs`, and `marengo-pi` `main.rs`; a one-file addition silently stopped forwarding. All pre-existing chappe integration suites (`ipc_backpressure`, `ipc_outage`, `ipc_peer_lifetime`) still pass against the shared constants, and they keep independent string literals so a changed value fails loudly. | Single source in `chappe::topics` (`LATEST_TELEMETRY_TOPICS`, `EVENT_TELEMETRY_TOPICS`, `COMMAND_TOPICS` plus one constant per topic). Gateway `state.rs` re-exports the shared values (gateway-only synthetic topics stay local), `http.rs` command publishes use them, and `marengo-pi` (`main.rs`, `overlay.rs`, `limit_persist.rs`, `imu.rs`) and the outbox/command allowlist consume them. |
| L-chappe-11 (S3) | **CONFIRMED (gap)** | Inbound bounds (topic ≤128 B, payload ≤64 KiB) existed in `take_frame` but were untested at reader level. New `ipc::tests::take_frame_refuses_oversize_topic_at_reader`, `take_frame_refuses_oversize_payload_at_reader`, `take_frame_waits_for_truncated_frame`, and `inbound_reader_closes_on_oversize_frame_without_publishing` pass. | Bounds pinned at the reader's frame splitter and at the connection reader: the valid frame ahead of an oversize frame is admitted, the oversize frame closes the connection without publishing. |
| L-marengo-pi-08 (S3) | **CONFIRMED** | A missing `MARENGO_CHAPPE_SOCKET` left the runtime moving motors with no gateway command/telemetry path behind a quiet start. New `ipc_wiring_tests::missing_socket_resolves_to_none_with_no_default` and `configured_socket_resolves_verbatim` pass. | Resolution is env-only with no silent default (`resolve_chappe_socket`), and a missing socket now `warn!`s loudly naming the consequence (no Consul commands, no gateway telemetry) instead of starting quietly headless. Headless bench stays supported, so this warns rather than refuses. |
| L-chappe-04 (S3) | **CONFIRMED** | Baseline unlinked any pre-existing socket path without checking it was a socket, and ignored `chmod` failure. New `ipc::tests::listener_refuses_to_replace_non_socket_path` passes. | Bind refuses a non-socket path instead of replacing it; `chmod` failure warns explicitly. Socket-group peer replacement (any group member can become the command peer) is unchanged: authenticating the peer needs `SO_PEERCRED`/handshake design. See NEEDS-DECISION. |
| L-chappe-06 (S3) | **CONFIRMED** | Both readers blocked in `read` forever; the client relied on the writer deadline, so an idle outbox with a hung gateway went unnoticed. New `ipc::tests::read_idle_timeout_classification` and `command_flow_survives_read_deadline` pass. | Both readers arm a 30 s read deadline. Idle timeouts (`WouldBlock`/`TimedOut`) resume the read with a `debug!` — commands are rare so client idle is normal — while every other read error still drops the connection. A hung-peer heartbeat/probe remains NEEDS-DECISION. |
| L-chappe-10 (S4) | **CONFIRMED** | `take_frame`/`read_connection` warned per frame on an unknown direction byte — a peer-controlled log-spam vector. New `ipc::tests::unknown_direction_frame_is_ignored_and_stream_survives` passes. | Unknown directions log at `debug!`; the frame is ignored and the stream survives. |

## NEEDS-DECISION

### L-chappe-07 — command freshness without wall clocks
- **A (recommended):** add a monotonic sequence or boot-epoch to the command envelope and admit on (sequence, boot) instead of wall age. Closes NTP and replay exposure; requires a coordinated gateway+Pi wire change and a mixed-version rollout plan.
- **B:** shrink the age window (e.g. 200 ms). Cheap, narrows replay, but keeps the NTP failure mode and risks refusing legitimate commands under scheduling jitter.
- **C:** keep the documented 1000 ms wall-clock window. Not recommended beyond the short term: the exposure stays silent without this record.

### L-chappe-04 — IPC peer authentication
- **A (recommended):** check `SO_PEERCRED` on accept against an allowlisted peer UID/binary identity before installing the command writer. Bounds the socket-group trust without a wire change; still machine-local trust.
- **B:** add a pre-shared handshake token for the command direction. Stronger, but introduces secret distribution and rotation.
- **C:** accept socket-group trust with the current hygiene (non-socket refuse, 0660 warn). Leaves any group member able to replace the command peer.

### L-chappe-06 — hung-peer detection with an idle outbox
- **A (recommended):** add a periodic keepalive frame on the runtime→gateway direction and an idle threshold on the gateway side that drops the peer. Detects a hung gateway even when telemetry is idle; small wire addition.
- **B:** have the client probe with a zero-length write on read-idle expiry. No wire change, but a hung peer may still ACK writes into socket buffers, limiting detection.
- **C:** keep deadline-only behavior (threads wake every 30 s; detection relies on the next write). Simplest; a fully idle hung peer still goes unnoticed.

## Cross-package edits

- `bins/marengo-gateway/src/state.rs`: topic constants re-export `chappe::topics` (single contract source); gateway-only synthetic topics unchanged. `http.rs`: four command publishes use the shared consts. No routing, auth, or health logic touched (WP-K owns those).
- `crates/berthier/src/loop.rs`: tick publish isolation only (`publish_robot_state_counted`, counters, overrun read). No law, fuse, or planner change.
- `bins/marengo-pi/src/main.rs` + `ipc_wiring_tests.rs`: socket resolution, overrun total, 1 Hz counter surfacing, trace flush on shutdown. No command-admission or motion-owner change (WP-F owns those).
- `crates/chappe/src/ipc_outbox.rs`: topic lookup only; `drop_publication` counting and all queue bounds are byte-identical (the dropped counter belongs to WP-L).
- Follow-up (WP-K): `AppState.ipc.accept_health()` is available for `/health` aggregation; command-path errors already surface the dead-accept-loop message in HTTP 500 bodies via `publish_command_envelope`.

## Gate

- `cargo fmt --all -- --check` — pass.
- `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings` — pass.
- `cargo test --workspace` — pass (1205 passed, 0 failed).
- `cargo test -p chappe` — pass (26 lib + 4 backpressure + 2 outage + 2 peer-lifetime).
- `cargo test -p berthier` — pass (219 lib + 16 integration suites).
- `cargo test -p marengo-pi` — pass (101).
