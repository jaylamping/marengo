# Intent card: `chappe`

## 1. Header

| Field | Value |
|---|---|
| Crate | `chappe` (codename Chappe, the message bus) |
| Path | `crates/chappe` |
| Kind | lib |
| Baseline | `a2b55b3` (worktree `audit/2026-10-03`) |
| LOC | src 1,776 (`lib.rs` 254, `ipc.rs` 760, `ipc_outbox.rs` 324, `tracing_layer.rs` 347, `transport.rs` 91; inline unit tests in 4 files); tests/ 490 (`ipc_backpressure.rs` 277, `ipc_peer_lifetime.rs` 128, `ipc_outage.rs` 85) |
| Sources consulted | `src/lib.rs` //! docs; `README.md`; `codemap.md`, `src/codemap.md`; AGENTS.md:17,35,54,168-178,246; crates/AGENTS.md:12,37; bins/AGENTS.md:3,31; docs/rust-patterns.md:7,66-87,375; ADR 0001, ADR 0008, ADR 0013; prior review gateway.md (G01, G02, G10, G11, design concern 6), control.md CS22, finding-index.md, implementation-ledger.json; `git log -- crates/chappe`; consumers in `bins/marengo-pi`, `bins/marengo-gateway`, `crates/berthier`, `bins/marengo-jetson`; `cargo check -p chappe` on macOS (clean). |

## 2. Intent

Chappe is the **transport-only** pub/sub bus that carries binary protobuf `Envelope` bytes between runtime components (crate doc `src/lib.rs:1-16`; ADR 0001; AGENTS.md:17,54). In-process it is a topic→`tokio::broadcast` map (`lib.rs:42-54`). Cross-process it is a single Unix-domain-socket bridge, added by ADR 0008: `marengo-pi` forwards telemetry to `marengo-gateway`, and the gateway sends operator commands back on the same socket (`ADR 0008 §Decision.4`; `ipc.rs:1-5`). Since the 2026-10-01 G01 repair it also owns **bounded, non-blocking** runtime→gateway admission. State topics are coalesced and expire after 1 s. Audit and log events go to a bounded FIFO (`ipc_outbox.rs:1-23`; commits `5fc366f`, `b5b16fa`). Its third job is forwarding `tracing` events as rate-limited `LogEvent`s on `logs/structured` for Chappe producers (`tracing_layer.rs:1`; ADR 0013; bins/AGENTS.md:31).

Conflicting statements of intent:
- `src/lib.rs:3` says it connects "Pi, Jetson, Consul UI, and tools", and codemap.md:20 lists `marengo-jetson` and Consul `chappe-client.ts` as consumers. In the code the only cross-process peer is the gateway. Jetson declares the dependency but never uses it (`bins/marengo-jetson/src/main.rs` has 6 lines and no `chappe::` use). Consul talks to the gateway, not Chappe (ADR 0008).
- README.md:15 and ADR 0008:58 plan "NATS/MQTT backends" behind the same bytes. The `Transport` trait (`transport.rs:11`) is the only artifact of that plan, and it has no consumers (§9).
- AGENTS.md:246 states the IPC implementation is Unix-only. `Cargo.toml:21-22` declares an empty `cfg(unix)` dependency block, but `lib.rs:18` includes `pub mod ipc` unconditionally, so non-Unix targets do not compile (CS22/G11).

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| Topic broadcast of `Vec<u8>` envelope bytes; envelope wrap with `timestamp_ms`, `source_node`, `message_type` | `lib.rs:100-147` |
| Unix socket framing `u8 dir + u32 topic_len + topic + u32 len + payload`, bounds (topic ≤128 B, payload ≤64 KiB) | `ipc.rs:3-5,24-25,44-99,586-617`; `ipc_outbox.rs:8` |
| Runtime outbox classes (7 latest-value topics, 3 event topics), 1 s state expiry, 1 s write deadline, reconnect | `ipc_outbox.rs:11-23,130-237`; `ipc.rs:165-237` |
| Gateway→runtime command allowlist (7 topics) and freshness (age ≤1000 ms, not future) | `ipc.rs:276-301` |
| Gateway listener peer lifecycle (single peer, generation retirement, shutdown join) | `ipc.rs:333-543` |
| `tracing` → `LogEvent` layer (40 events/s for INFO/DEBUG/TRACE, WARN/ERROR exempt, `fields_json` ≤2048 B) | `tracing_layer.rs:15-20,51-75,132-154,166-203` |
| Socket path default `/run/marengo/chappe.sock` and `MARENGO_CHAPPE_SOCKET` | `ipc.rs:27-34` |

| Must not | Evidence | Violation? |
|---|---|---|
| Command motors / control policy | `lib.rs:13`; AGENTS.md:54 | None found. Command topics are only re-published onto the local bus (`ipc.rs:265-267`). Freshness gating of commands (`ipc.rs:276-301`) is transport admission, not motion policy. |
| Parse URDF / load config | `lib.rs:14` | None (deps: armee-proto, serde_json, thiserror, tokio, tracing, tracing-subscriber; `Cargo.toml:13-19`). |
| Carry raw CAN | `lib.rs:16` | Not enforced in code: any topic string can be published in-process. IPC admission is allowlist-only (`ipc_outbox.rs:149-154`). |
| JSON on the wire | AGENTS.md:193; ADR 0001 | `serde_json` is used only for `LogEvent.fields_json` inside the protobuf (ADR 0013). This is not a violation. |

Layering note: Berthier depends on Chappe directly and publishes `robot/state` from the control loop (`crates/berthier/src/loop.rs:11,46,1535`). AGENTS.md:61 places that publish in `marengo-pi`. This is drift, not a boundary violation: Chappe stays policy-free.

## 4. Interface

| Concept | Public surface | Consumers |
|---|---|---|
| In-process bus | `Bus::{new, default, publish, publish_bytes, subscribe, last_publish_ms, set_ipc_fanout (unix), ipc_queue_stats (unix)}`, `BusError` (`lib.rs:35-187`) | marengo-pi (`main.rs:44,702,755,1111-1131`, `overlay.rs`, `imu.rs`, `limit_persist.rs`, `host_metrics.rs:46-47`); berthier (`loop.rs:11,1535`); gateway (`state.rs:10`, `main.rs:30`) |
| Bus extras with no production consumer | `Bus::recv_envelope` (`lib.rs:150`; used only in chappe unit tests), `Bus::ipc_configured` (`lib.rs:166`; zero refs) | none |
| Runtime IPC client | `ipc::IpcFanout::{spawn_client, forward_runtime_to_gateway, queue_stats, subscribe_connection, shutdown}` (`ipc.rs:102-157`), `ForwardOutcome`, `IpcQueueStats`, capacity consts (`ipc.rs:14-17`) | marengo-pi `main.rs:1113-1120`; `host_metrics.rs:56-64` (consts); `subscribe_connection` only in chappe tests |
| Gateway IPC listener | `ipc::IpcListener::{spawn_server, spawn_server_with_lifecycle, shutdown, send_command}` (`ipc.rs:334-537`) | gateway `main.rs:180`, `state.rs:9,67,183,272`; `spawn_server` (no lifecycle) has zero refs |
| Framing | `ipc::{encode_frame, decode_frame, DIRECTION_*}` (`ipc.rs:22-87`) | chappe tests only |
| Socket path | `ipc::{default_socket_path, socket_path_from_env}` | gateway `main.rs:56` (falls back to the default); marengo-pi `main.rs:1113` (env only, **no default**) |
| Tracing | `tracing_layer::{init_subscriber, ChappeLogLayer, TOPIC_LOGS}` | marengo-pi `main.rs:1111`; gateway `main.rs:122`. Gateway redefines `TOPIC_LOGS` instead of importing it (`bins/marengo-gateway/src/state.rs:22`) |
| Transport seam | `transport::{Transport, SharedBus, TransportError}` (`transport.rs:11-91`) | **zero** consumers outside the crate |

Depth (codebase-design):
- `Bus` is **deep**. Its small interface hides channel creation, no-subscriber success and optional IPC fan-out.
- `IpcFanout` and `IpcListener` are **deep**. The interface is spawn, forward, send, shutdown. The concurrency behind it (generations, deadlines, bounded classes) is substantial.
- `Transport` is a **hypothetical seam**. It has 2 adapters (`Bus`, `SharedBus`) and 0 callers. `SharedBus` duplicates `Bus::set_ipc_fanout` (`transport.rs:72-78` vs `lib.rs:108-113`).
- The topic contract is a **shallow, triplicated seam**. Topic strings live in chappe `ipc.rs:277-285` (commands) and `ipc_outbox.rs:11-19,143-147` (forwarded), in gateway `state.rs:17-60`, and in marengo-pi `main.rs:1123-1129`. No shared constant module exists.
- Cargo features: none. The `cfg(unix)` gating is partial: `lib.rs:52,76,108,166,178` are gated, `lib.rs:18` is not.

**Metrics baseline** (`metrics/`, `a2b55b3`): crate **80.7% / 81.0% / 64.4%** (`coverage-by-crate.md`). `ipc.rs` **81.2%**, `ipc_outbox.rs` **95.7%**, `tracing_layer.rs` **80.6%**, `lib.rs` **82.8%**; **`transport.rs` 0.0%** (47 lines, no exercised tests) (`coverage-by-file.md`). **11** tests (`test-counts.md`). Zero-use `pub` heuristic: `TransportError`, `ipc_configured`, `ipc::spawn_server`, `with_ipc_fanout` (`pub-usage.md`). `marengo-jetson` lists unused `chappe` dep (`unused-deps.md`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) that fail if broken |
|---|---|---|
| Publish without subscribers succeeds (headless Pi) | `lib.rs:115-117` (commit `5979340`) | `lib.rs:229` `publish_without_subscribers_succeeds` |
| Envelope wire contract for typed publish | `lib.rs:124-142` | `lib.rs:198`, `lib.rs:236` |
| Runtime admission never blocks the publisher (try_lock, drop on contention) | `ipc_outbox.rs:156-160` | `ipc_outbox.rs:306` `publisher_finishes_while_writer_still_owns_queue_guard` |
| Bounded outbox: ≤7 latest + 128 events / 512 KiB; payload ≤64 KiB | `ipc_outbox.rs:8-23,149-191` | `tests/ipc_backpressure.rs:12` `absent_peer_has_bounded_classes_and_honest_counters` |
| State older than 1 s is never replayed after reconnect | `ipc_outbox.rs:23,225-229` | `ipc.rs:700` `reconnect_sends_only_eligible_state_under_injected_monotonic_time`; `tests/ipc_outage.rs:11` |
| Socket writes bounded by a 1 s deadline; failure forces reconnect | `ipc.rs:197-200,210-237,525-531` | `tests/ipc_backpressure.rs:67,190` |
| Close refuses later admission | `ipc_outbox.rs:149,161` | `ipc_outbox.rs:274` |
| Commands: allowlisted topic, decodable Envelope, 0 ≤ age ≤ 1000 ms | `ipc.rs:276-301` | `tests/ipc_peer_lifetime.rs:17` `stale_future_and_unknown_commands_never_reach_runtime_bus` |
| Replaced or retired gateway peers cannot deliver frames or own the command writer | `ipc.rs:415-471` | `tests/ipc_peer_lifetime.rs:71` |
| Listener shutdown joins without peer cooperation | `ipc.rs:423-426,487-510` | `ipc.rs:644` |
| Inbound frame bounds (topic ≤128, payload ≤64 KiB); refusal closes the connection | `ipc.rs:246-248,555-557,586-617` | **untested** at reader level (only outbound `encode_frame` roundtrip `ipc.rs:626`) |
| Tracing quota 40/s refills; WARN/ERROR always forwarded | `tracing_layer.rs:51-75` | `tracing_layer.rs:244`; concurrent quota test (`tracing_layer.rs` tests module, `concurrent_tracing_events_share_one_quota`) |
| `fields_json` ≤2048 B with `_truncated` marker | `tracing_layer.rs:132-154` | `serialize_fields_truncates_large_payload` (tracing_layer tests) |

## 6. Inputs / outputs

- **Env:** `MARENGO_CHAPPE_SOCKET` (`ipc.rs:33`); `MARENGO_LOG_SESSION_ID` read **per log event** (`tracing_layer.rs:196`); `RUST_LOG` (`tracing_layer.rs:211`).
- **Files:** Unix socket (default `/run/marengo/chappe.sock`, `ipc.rs:29`). The listener deletes any pre-existing file at that path (`ipc.rs:369-371`), creates parent directories, and sets mode 0660 best-effort (`ipc.rs:377-380`).
- **Topics forwarded runtime→gateway:**
  - Latest-value: `robot/safety`, `robot/heartbeat`, `robot/state`, `sensors/imu/torso`, `host/metrics/pi`, `host/metrics/jetson`, `robot/actuator/limits` (`ipc_outbox.rs:11-19`).
  - Event: `logs/structured`, `robot/audit/action`, `robot/audit/tuning` (`ipc_outbox.rs:143-147`).
- **Topics accepted gateway→runtime:** `robot/enable`, `robot/homing`, `robot/set_zero`, `robot/active_reporting_lease`, `robot/motor_status_poll`, `robot/testing/mit_command_batch`, `robot/actuator/command` (`ipc.rs:277-285`).
- **Log topic:** `logs/structured` (`tracing_layer.rs:15`).
- **Threads:** `chappe-ipc-client` (plus one reader thread per connection, `ipc.rs:119-121,181`), `chappe-ipc-server` (plus one reader per peer, `ipc.rs:387-388,455`).
- No CAN, HTTP or config.

## 7. Prior review reconciliation

| Prior ID | Prior claim | Current status | Evidence |
|---|---|---|---|
| G01 (P1) | Unbounded IPC telemetry queue | **Fixed.** Ledger: verified PR236. finding-index.md:50 still says "Open", which is drift. | Bounded `Outbox` (`ipc_outbox.rs:8-23,130-237`), write deadline (`ipc.rs:210-237`); commits `84de1b2`, `5fc366f`, `b5b16fa`; tests `tests/ipc_backpressure.rs:12,67,106,190`, `tests/ipc_outage.rs:11` |
| G02 (P1) | Tracing quota never refills | **Fixed** (PR213) | Atomic bucket+count (`tracing_layer.rs:26-75`); test `tracing_layer.rs:244`. Recommended "suppression counters" not implemented: dropped INFO events are uncounted (`tracing_layer.rs:172-174`). |
| G11 / CS22 (P2) | Unix-only types prevent non-Unix compile | **Open** | `lib.rs:18` `pub mod ipc;` unconditional; `ipc.rs:211,239,303,335,546` use `std::os::unix` unguarded; ledger status open. macOS compiles (Unix), so the issue is Windows only. |
| G10 (P2) | Snapshots/health look current after Pi disappears | **Open** (gateway-owned; Chappe contributes) | `Envelope` has no boot/sequence identity (`lib.rs:132-140`). The connection signal now exists (`IpcListener::spawn_server_with_lifecycle`, `ipc.rs:350-361`), but staleness policy belongs to the gateway. Ledger: open. |
| gateway.md design concern 6 | Peer trust/lifecycle implicit; no frame bounds/deadlines/handshake | **Partially fixed** | Bounds `ipc.rs:24-25,586-617`, generation retirement `ipc.rs:415-471`, write deadline `ipc.rs:210`. Still missing: producer identity/handshake/version, idle read deadline (`read_connection` blocks indefinitely, `ipc.rs:551-552`). Any local process that can open the 0660 socket replaces the command peer (`ipc.rs:427-430`). |
| gateway.md design concern 12 | Envelope omits topic/boot identity | **Open** | `transport.rs:22-30`, `lib.rs:132-140` |

## 8. Drift

| Doc says | Code says |
|---|---|
| codemap.md:8 "Typed helpers `publish_proto` / `subscribe_proto`" | No such functions (zero grep hits). Typed publish is `Bus::publish` (`lib.rs:124`). |
| codemap.md:9 "`IpcListener` / `IpcClient`" | No `IpcClient`; the client is `IpcFanout` (`ipc.rs:102`). |
| codemap.md:10 "optional OpenTelemetry integration" | No OpenTelemetry. `tracing_layer` publishes `LogEvent` on Chappe (`tracing_layer.rs:1`). |
| codemap.md:16 "`IpcListener` fans out to connected Unix socket clients"; src/codemap.md:16 "IpcListener::accept → subscribe to SharedBus topics → write framed envelopes" | The listener is gateway-side, ingests runtime frames, and writes only commands (`ipc.rs:333`). It does not subscribe to `SharedBus`. One peer at a time (`ipc.rs:427-430`). |
| codemap.md:19-20 consumers include `marengo-jetson`, Consul `chappe-client.ts`; topics `safety/state`, `heartbeat` | Jetson has no Chappe use. Actual topics are `robot/safety`, `robot/heartbeat` (`ipc_outbox.rs:12-13`). |
| src/codemap.md:15 "hash topic" | `HashMap` lookup under a **write** lock on every publish (`lib.rs:83-95`). |
| AGENTS.md:61 / ADR 0008:8 "`marengo-pi` publishes `RobotState`" | Publisher is Berthier's control loop (`crates/berthier/src/loop.rs:1535`, source_node `"berthier"`). |
| ADR 0008:63 "`marengo-pi` sets `MARENGO_CHAPPE_SOCKET`" | `marengo-pi.service` does not set it. It comes only from optional `/etc/marengo/env` (`scripts/systemd/marengo-pi.service:13`; `scripts/env.example:25`). Gateway service sets it (`marengo-gateway.service:12`). |
| ADR 0008:40 WebTransport allowlist (7 topics) | Gateway `ALLOWED_TOPICS` has 13, including `robot/testing/telemetry`, which no producer publishes and Chappe would not forward (`bins/marengo-gateway/src/state.rs:26,37-51`; absent from `ipc_outbox.rs:11-19`). |
| `lib.rs:160` "last **successful** publish" | Timestamp is stored before the send result is known (`lib.rs:101-107`). |
| `ipc_outbox.rs:1` "command topics never enter this outbox" | They are refused, but each refusal increments `dropped` (`ipc_outbox.rs:149-154,206-208`). Every command the runtime re-publishes from IPC (`ipc.rs:267` → `lib.rs:108-113`) inflates `IpcQueueHealth.dropped_total`. |
| finding-index.md:50 G01 "Open" | Ledger G01 = verified PR236. |

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting it touches |
|---|---|---|---|
| `transport.rs` whole module: `Transport`, `SharedBus`, `TransportError` | Zero references including tests/bins/tools; **0% line coverage** (`coverage-by-file.md`); `pub-usage.md` flags `TransportError`, `with_ipc_fanout`; duplicate of `Bus` + `set_ipc_fanout` (`transport.rs:72-78` vs `lib.rs:108-113`); scaffold for ADR 0008:58 "future NATS" | med (high on SharedBus/TransportError; ADR 0008 names a future NATS mode, so confirm the plan is dropped first) | `lib.rs:21,30` re-exports; codemap rows |
| `Transport::publish` default body | Duplicate implementation of `Bus::publish` (`transport.rs:14-32` ≡ `lib.rs:124-142`) | high (goes with the module) | same |
| `Bus::ipc_configured` | Zero references; `pub-usage.md` zero-use | high | `lib.rs:165-175` |
| `Bus::recv_envelope` | Scaffold with no production consumer (chappe unit tests only) | med | 2 tests in `lib.rs:198-253` would decode inline |
| `IpcListener::spawn_server` (no lifecycle) | Zero references (gateway and tests use `spawn_server_with_lifecycle`) | high | `ipc.rs:341-346` |
| `ipc::decode_frame` / `encode_frame` as `pub` | Used only by tests and internally; could be `pub(crate)` + test helper | low | `tests/ipc_peer_lifetime.rs:5` |
| `"host/metrics/jetson"` latest slot | Scaffold with no producer: Jetson binary is a 6-line scaffold without Chappe IPC (`bins/marengo-jetson/src/main.rs`) | low (roadmap keeps Jetson) | `ipc_outbox.rs:17` |
| `chappe` dependency in `bins/marengo-jetson/Cargo.toml:18` | Zero use in that bin | high | Cargo.toml only |
| `[target.'cfg(unix)'.dependencies]` empty block | Dead manifest stanza; implies gating that does not exist (`Cargo.toml:21-22`) | high | Cargo.toml |

Not prunable: write deadline, outbox bounds, command freshness gate, shutdown joins. These are stop/transport safety bounds.

## 10. Phase-B leads

1. **Publish race can fail the control tick.** `publish_bytes` checks `receiver_count()==0`, then `send` (`lib.rs:115-119`). If the last receiver drops in between, `send` returns `Err`, which Berthier propagates with `?` from the control loop (`crates/berthier/src/loop.rs:46,1535`). [INFERENCE] A telemetry subscriber detaching could surface as a control-loop error. Check how the tick handles `ControlLoopError::Chappe`.
2. **Write lock on every publish.** `Bus::sender` takes the `RwLock` **write** lock even for existing topics (`lib.rs:84-95`). The 200 Hz control loop, the tracing layer (any thread), IMU, host-metrics and the IPC reader all contend on it. Poison is swallowed (`into_inner`).
3. **WARN/ERROR are exempt from the quota** (`tracing_layer.rs:52-54`), so a warn storm floods `logs/structured`. The event FIFO bounds IPC (`ipc_outbox.rs:177-181`) but not the in-process broadcast or the gateway's DB writer. `connect_with_retry` logs `error!` every ~6 s while the gateway is down (`ipc.rs:303-331`, loop at `ipc.rs:166-173`).
4. **Listener thread dies silently** on any non-`WouldBlock` accept error (`ipc.rs:399-402`). After that the gateway never accepts a runtime again, `IpcListener` still exists, and `send_command` returns "no ipc peer". There is no health signal.
5. **Asymmetric socket defaults.** The gateway falls back to `/run/marengo/chappe.sock` (`bins/marengo-gateway/src/main.rs:56`). marengo-pi enables IPC only if the env var is set (`bins/marengo-pi/src/main.rs:1113`). A missing `/etc/marengo/env` line therefore gives "gateway healthy, no telemetry", logged only at INFO.
6. **Socket path hygiene.** `remove_file` on any existing path, with no check that it is a socket (`ipc.rs:369-371`). Permission setting is ignored on failure (`ipc.rs:377-380`). Any member of the socket group can replace the command peer (`ipc.rs:427-430`).
7. **Misleading `dropped` counter:** inbound commands and unknown topics count as dropped (`ipc_outbox.rs:149-154`, `ipc.rs:267`). This feeds `HostMetrics.chappe.ipc_queue.dropped_total` (`bins/marengo-pi/src/host_metrics.rs:61`).
8. **Reader has no idle deadline.** `read_connection` and `read_inbound_commands` block on `read` forever (`ipc.rs:243,552`). The runtime side relies on the writer's deadline to shut down the stream (`ipc.rs:203`). If the outbox is idle and the gateway hangs without closing, the client never notices [INFERENCE].
9. **Command freshness uses wall clocks.** `SystemTime` on both ends (`ipc.rs:292-300`), with no monotonic or sequence field. NTP steps on the Pi can refuse or admit commands. A 1000 ms age window also allows replay of a captured frame within that second.
10. **Stale-state drop counts but does not report a topic.** `next()` silently discards expired state (`ipc_outbox.rs:225-229`). Fine for transport. Still, check whether the gateway can tell "expired" from "never sent" (G10).
11. **Topic contract triplication** (§4). Adding a topic requires 3 files; a miss causes silent non-forwarding (`ipc_outbox.rs:151`).
12. **`take_frame` ignores direction until decode.** An unknown direction is warned per frame (`ipc.rs:579`), which a misbehaving peer could use to spam warnings.
13. **Inbound frame bounds (§5) vs coverage.** `ipc.rs` is ~81% line-covered, but the §5 row "untested at reader level" for oversize inbound frames is a **coverage gap** on transport safety bounds — not a signal to delete bounds code (`metrics/coverage-by-file.md`).
