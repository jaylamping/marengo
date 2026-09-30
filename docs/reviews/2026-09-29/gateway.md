# Marengo gateway, messaging, persistence, and diagnostics review

Reviewed September 29, 2026. Scope: gateway HTTP/WebTransport, Chappe, protobuf contracts, the SQL log store, candump inspection/CLI, local limit sync, host metrics, and the newer self-update/URDF HTTP surfaces.

## Source baseline and validation

- Findings use main at **`4bc77ba605834fdec04b436daa4bec67bca84fbb`** with one-based source line numbers. Source links are pinned to that revision. Chappe, Store, candump, metrics, and the relevant restart/state logic also match the older recovered branch (`c97aa96`); main-only surfaces are identified.
- All G findings remain unresolved. G11 duplicates CS22 in the [control appendix](control.md); G10 and F05 describe the same freshness failure across gateway and UI. See the [finding index](finding-index.md) for consolidated status and the [repository review](../2026-09-29-repository-review.md) for migration and completed cleanup.
- `cargo test -p marengo-store -p marengo-candump -p marengo-log-cli` on the recovered checkout, native Windows/Rust 1.88: **21 passed** (Store 4, candump 15 including contract tests, CLI 2). These files also match main.
- `cargo test -p marengo-deploy` on clean main: **15 passed** (11 unit, 4 job/script-shape contract tests).
- Native `cargo check -p chappe`, using the already-recovered `tools/protoc-28.3-win64/bin/protoc.exe` through `PROTOC`: **fails with seven E0433 errors** for unconditional `std::os::unix` references. An earlier attempt without `PROTOC` stopped at the missing-tool dependency; no installation was needed to reproduce the actual platform failure.
- Temporary, isolated reproductions confirmed the purge deadlock, log-artifact reference loss, bad capture start times, interrupted migration failure, candump duration panic, and logging limiter clock error. No source edits, live robot connections, deployments, service changes, or external messages were made. Reproduction data and raw evidence are retained locally with the migration recovery records.
- Gateway HTTP/QUIC integration tests were not run natively because Chappe does not compile on Windows. No browser, physical CAN, Pi GPIO, I2C, live certificates, or production DB was exercised.

## Architectural refresher for this scope

The actual motor path remains Berthier → Davout → robstride. Chappe is an in-process protobuf pub/sub bus plus a Unix-domain socket bridge to the gateway. Pi-side telemetry is published into Chappe, forwarded by a background thread to the gateway, cached in snapshots, and fanned out over WebTransport or an HTTP stream. Operator HTTP commands traverse the same IPC connection in the reverse direction. The gateway does not directly own CAN.

The latest main has moved past the recovered 4-DOF bringup branch: its documented source of truth is master `config/` and `assets/urdf/marengo.urdf`, with a **5-DOF right arm** and ephemeral/persisted commissioning scope. The older checkout still contains profile selection APIs and bringup-profile assumptions. Main contains Hardware URDF lifecycle/commissioning APIs and self-update APIs absent from the older checkout. The reviewed baseline still carries multiple stale directory maps and crate/bin counts; the separate migration/documentation changes update the top-level maps. The Jetson runtime is a scaffold, not a running planner/perception service.

Runtime limits have two representations: Davout/Berthier's live aggregate is authoritative while the Pi runs, and YAML/URDF are boot seeds written asynchronously. `marengo.db` is logging/audit/preferences, not authoritative motor config. This distinction is sensible, but several defects below break persistence acknowledgements and safe management gates.

For Windows/Mac development, the desirable split is native, shared-source domain/UI development plus a Linux robot/driver target. The current source-location dependency on WSL is unnecessary, but native Windows builds of the whole Rust workspace are currently blocked by Chappe's platform implementation.

## Confirmed and strongly supported findings

Priority P1 means address before relying on the affected control, recovery, or operating workflow; P2 means fix in the next reliability/platform pass. The review does not assign P0 based solely on hypothetical attack exposure or unavailable hardware evidence.

### G01 — P1: The Pi telemetry queue grows without bound while the gateway is offline

**Evidence:** [crates/chappe/src/ipc.rs:94](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/chappe/src/ipc.rs#L94) creates an unbounded `std::sync::mpsc::channel`; `:102-103` copies and enqueues every publish. `:107-120` attempts to connect before consuming this queue. `:128` can also block indefinitely in a synchronous socket write. Confidence: high, code-path confirmed; no OOM stress run.

**Trigger and consequence:** Leave the Pi runtime running with Chappe IPC configured while gateway startup fails, the gateway is stopped, or a connected peer stops reading. Telemetry continues to enqueue. Memory grows for the entire outage; reconnection then transmits historical state/logs before current state. This can exhaust memory in the process responsible for controlling the robot and delays operator feedback after recovery.

**Fix:** Bound the outbound queue and make control-path enqueue nonblocking. Coalesce latest-value telemetry by topic; use a separately bounded policy for logs/audit records. Expose dropped/coalesced counts, actual connection state, and queue age. Add socket write deadlines/cancellation and discard stale telemetry on reconnect. Retain priority for safety/current-state messages.

**Verification:** Run producers for minutes against an absent gateway and a non-reading peer; assert a fixed memory/queue ceiling and prompt control ticks. Reconnect and assert the first delivered state is recent rather than the beginning of the outage.

### G02 — P1: INFO/DEBUG/TRACE forwarding stops permanently after the first 40 events

**Evidence:** [crates/chappe/src/tracing_layer.rs:39](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/chappe/src/tracing_layer.rs#L39) computes `Instant::now().elapsed()` for every event. This measures elapsed time from an instant created immediately beforehand, yielding approximately zero. `:41-45` therefore never resets the rate-limit window. `:46-47` permits only WARN/ERROR once the count exceeds 40. Confidence: reproduced.

**Trigger and consequence:** Normal runtime startup and operation emit 40 tracing events. Subsequent informational/action diagnostics vanish from the Chappe/log-store/Consul path for the rest of the process lifetime, while journal/fmt output can continue. This explains unexplained gaps and removes evidence needed when returning to a robot after weeks away.

**Reproduction:** A temporary Rust executable using the exact clock/counter logic reported `accepted_first_40=40 accepted_after_1.1_seconds=false clock_ms=0` after waiting 1.1 seconds.

**Fix:** Store a monotonic start instant in the layer, or store/manage a real rolling timestamp. Make concurrent window resets coherent, and expose suppression totals so rate limiting is distinguishable from missing producers.

**Verification:** Test initial burst, suppression, elapsed-window refill, WARN/ERROR bypass, and multithreaded event emission. An injected clock avoids wall-clock sleeps.

### G03 — P1: Daily retention hangs as soon as an old session exists

**Evidence:** [crates/marengo-store/src/store.rs:291](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-store/src/store.rs#L291) holds `self.connection()`'s non-reentrant mutex guard; `:305` calls `self.get_session(id)` while that guard remains live; `get_session` attempts the same lock at `:393`. Scheduled path: [scripts/systemd/marengo-log-maintenance.service:11](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/systemd/marengo-log-maintenance.service#L11) invokes `marengo-log-cli purge --days 30`; [bins/marengo-log-cli/src/main.rs:254](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-log-cli/src/main.rs#L254) calls this method. Confidence: reproduced.

**Trigger and consequence:** The first session older than the configured cutoff causes `purge` to deadlock before deleting that session or its files. The daily maintenance service remains running and subsequent daily cleanup does not complete. Age retention eventually stops working.

**Reproduction:** Registered one isolated session with start time January 1, 2020, then ran `purge --days 30`; the helper still had not exited after three seconds and produced no output. The isolated helper was terminated; no production DB was touched.

**Fix:** Fetch session rows using the already-held connection, or fetch all required rows and release the lock before filesystem work. Use a transaction for DB changes and an explicit policy for unsuccessful blob removal. Do not call public lock-acquiring Store methods under its connection guard.

**Verification:** A bounded-time integration test must purge an old session with all three artifacts, keep a newer session, and verify both SQL rows and files. Also test missing/unremovable artifacts.

### G04 — P1: Coalescing can discard a pending motor/URDF persist and its ACK

**Evidence:** [bins/marengo-pi/src/limit_persist.rs:85-91](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/limit_persist.rs#L85) replaces the entire pending `PersistRequest`. The request includes optional motors, control, and one session/joint/parameter at `:57-66`. The worker writes motors+control+URDF only for `Some(motors)` (`:177-185`), and publishes a completion event only for the last surviving request (`:201-214`). `overlay.rs` allows persistent `ConfigOverlay` tuning and live `LimitPatch` to use this same queue. Confidence: high; control reviewer independently confirmed the request paths. No injected worker scheduling test was run.

**Trigger and consequence:** A limit patch applies live and queues motors+control+URDF. Before the worker takes it, a persistent gain edit queues a control-only request. The second request replaces the first, so the new motor hard limits/URDF are never persisted. The final control-only Durable event can still clear gateway persistence flags. Two limit changes also lose the first request's completion ACK even if the second full snapshot includes its data, producing a timeout after a successfully applied change.

**Fix:** Coalesce by a full authoritative configuration snapshot/generation, preserving the union of required artifacts and all request waiters. Every accepted live mutation must receive a terminal outcome tied to the generation that contains it. A control-only update must not erase dirty motors/URDF. A newer durable generation can acknowledge earlier included mutations explicitly.

**Verification:** Deterministically pause the writer; enqueue limit→gain and gain→limit, as well as different-joint limit patches. Verify YAML/URDF match the live aggregate and every caller receives the correct Durable/Failed outcome. Include restart and worker-failure cases.

### G05 — P1: Gateway persistence flags do not establish a safe restart/update state

**Evidence:** [bins/marengo-gateway/src/state.rs:148-159](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/state.rs#L148) updates two global booleans from any ActionEvent: any Durable clears both pending and degraded; Failed clears pending and sets degraded. The flags default to false at `:90-91`. Restart checks only pending at [restart.rs:127](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/restart.rs#L127); self-update does the same at [deploy.rs:110](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/deploy.rs#L110). Confidence: high, code-path confirmed. Both recovered checkout and main are affected, with deploy main-only.

**Trigger and consequence:** (a) A write-behind fails, so live config differs from disk; `pending=false` permits restart/update and reloads stale limits. (b) Request A becomes Durable while request B remains queued/writing; A clears pending too early. (c) A gateway restart loses the flag history while the Pi has pending or failed persistence. These management gates are therefore not authoritative. This conflicts with the repository safety guidance that restart while YAML/URDF are stale reverts live limits.

**Fix:** Have the Pi expose its current dirty/durable generations, queue state, and failed persistence state in a replayable snapshot or management RPC. Authorize restart/update only after the Pi atomically establishes a disabled, durable, quiescent state. Track request/generation identity rather than clearing global state on unrelated events; persist/recover unresolved failures. At minimum, block `persist_degraded` as well as pending until a real retry/recovery completes.

**Verification:** Pending A/Pending B/Durable A must remain pending; Failed A/Durable unrelated B must not erase an unresolved motors failure. Restart the gateway during a paused or failed writer and ensure management remains refused. Verify explicit recovery unlocks it.

### G06 — P1: Restart/update/URDF activation treat missing or stale telemetry as permission

**Evidence:** [restart.rs:73-79](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/restart.rs#L73) returns false for the refusal predicate if mode is not Active, if heartbeat is absent, or if the Active heartbeat is older than five seconds. [restart.rs:110-124](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/restart.rs#L110) uses that predicate before invoking the stop/restart script. [deploy.rs:93-107](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/deploy.rs#L93) and [hardware.rs:267-279](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/hardware.rs#L267) use the same check. [scripts/pi-restart-marengo-pi.sh](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/pi-restart-marengo-pi.sh) stops and pkills the process without a current Pi-side disabled handshake. Confidence: high. A gateway test explicitly enshrines allowing stale Active restart ([restart.rs:388](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/restart.rs#L388)).

**Trigger and consequence:** IPC/telemetry stalls while Pi control continues. The gateway sees a stale Active state and allows restart or update; URDF activation can also proceed. A communication failure is not evidence that motors are disabled. The control reviewer additionally found shutdown-order/CAN-disable reliability problems, which compound this path; those details belong in the control report.

**Fix:** Require a current Pi-side management handshake that atomically pauses new commands, verifies disabled drives/physical state, drains persistence, and then grants management permission. Refuse normal operations on unknown/stale state. If an emergency force-stop is needed, expose it as a separately explicit recovery operation with accurate consequences.

**Verification:** Simulate a live Active Pi with an interrupted telemetry bridge; restart/update/activate must refuse. Test clock skew and process boot IDs, plus a valid disabled handshake followed by an attempted concurrent enable.

### G07 — P1: The configured gateway token does not protect legacy motion/calibration routes

**Evidence:** [http.rs:288-321](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/http.rs#L288) decodes/publishes EnableRequest and MitCommandBatch without any header/auth gate. `:345-406` set-zero validates attestation/allowlist/rate but likewise lacks auth. In contrast, [actuator.rs:39-47](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/actuator.rs#L39) gates tuning through `authorize_logs`, and config/restart fail closed when the token is missing. [http.rs:72-75](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/http.rs#L72) permits all CORS origins/headers/methods and private-network requests. [scripts/systemd/marengo-gateway.service:13](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/scripts/systemd/marengo-gateway.service#L13) binds HTTP/HTTPS/QUIC on all interfaces. Confidence: high. The bench-auth deferral is documented, so this is both a known architectural security gap and a concrete bypass of the configured token's limited protection.

**Trigger and consequence:** Set `MARENGO_GATEWAY_LOG_TOKEN` and assume commands require it. A client can still post enable, Testing MIT targets/gains, and set-zero without the token. Testing MIT is especially relevant because the Pi reviewer confirmed it can automatically arm eligible joints. Live log subscriptions also bypass log-route authorization through `/stream/chappe` or WebTransport.

**Related frontend boundary:** The Consul reviewer verified that when a deployment sets `VITE_MARENGO_LOG_TOKEN`, clients reference it from build-time environment values ([consul/src/lib/config-api.ts:68-70](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/lib/config-api.ts#L68), `gateway-api.ts:36-38`, `log-api.ts:108-111`, `hardware-api.ts:87-89`, `version-api.ts:83-85`). `VITE_AUTO_LEARN_TOKEN` similarly feeds the Bearer header (`auto-learn-api.ts:21,55`). Vite exposes these values in the served JavaScript. The review build had tokens unset, so no actual secret value was observed; the exposure is conditional on building with these variables as described in deployment workflows. Public static assets therefore cannot safely carry the secret on which API authorization relies.

**Fix:** Use one fail-closed authentication/authorization middleware for every mutation and sensitive stream, with explicit roles/capabilities for control, calibration, configuration, and management. Restrict browser origins. Keep attestation and Davout checks as additional gates, not credentials. Provide a usable runtime sign-in/token-entry flow; embedding a reusable secret in static www assets is not a security solution.

**Verification:** Table-driven route tests must show missing/wrong credentials are rejected even when optional log auth is set. Exercise HTTP, HTTPS, fallback stream, and QUIC subscriptions. Verify authenticated requests still pass independent control/attestation/rate guards.

### G08 — P1: A disk-based no-op can falsely confirm that live limits were restored

**Evidence:** Main [limit_patch.rs:74-85](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/limit_patch.rs#L74) constructs `before` from disk; `:107-117` returns `ok=true, persist_status=Durable` without contacting the Pi when disk equals the requested patch. The recovered checkout has the same behavior in `profiles.rs:448-462` (exact branch positions differ). Async persist failures leave a successfully live-applied aggregate different from disk. Confidence: high, code-path confirmed.

**Trigger and consequence:** Disk contains hard upper bound 1 rad; a live patch widens it to 2 rad but write-behind fails. Request the original 1-rad bound to recover/tighten. The gateway says no changes/Durable because it compares to disk and never sends the 1-rad patch to the Pi. Live limits remain wider despite a successful response.

**Fix:** Let the Pi decide no-op based on its live aggregate and durable generation. A disk no-op cannot prove live equality or clean persistence. Use the runtime limit/config snapshot, or always send the patch and let the Pi validate/apply/persist it.

**Verification:** Inject persist failure after a live limit change, then request the original disk values. Assert the live limit really changes back and a terminal persistence outcome is truthful.

### G09 — P1/P2: URDF activation races the Pi config writer and can overwrite either result

**Evidence:** Main-only [hardware.rs:288-314](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/hardware.rs#L288) reads the master and builds a merge; `:342-357` writes/promotes the shared `marengo.urdf.tmp`. It does not coordinate with Pi persistence or check pending/degraded state. [crates/marengo-config/src/urdf_expand.rs:81-86](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-config/src/urdf_expand.rs#L81) uses the same temporary name during limit persistence; `:110-118` snapshots the old URDF and restores those old bytes if YAML writing fails. The control reviewer independently verified this race. Confidence: high; no concurrent failure-injection run.

**Trigger and consequence:** A Set Limits persist reads the old master; gateway activation promotes a new master; Pi persistence later promotes its old-master-plus-limit-expansion or restores its pre-activation backup after a YAML error. New geometry can be silently lost. Two activation requests can also share the same tmp path and race, without revision compare-and-swap.

**Fix:** Put all master-config/URDF mutations under one authoritative transaction owner and one cross-process lock/generation protocol. Make activation a Pi-coordinated durable transaction; include master revision in the preview and activation request. Use unique temporary files and do not roll back over a later generation.

**Verification:** Pause each operation after reading the URDF, interleave activate/limit persist, and inject YAML errors. Assert one coherent generation wins and the other gets a conflict, without lost geometry, limits, archive data, or lying success.

### G10 — P2: Gateway snapshots and health remain apparently successful after the Pi disappears

**Evidence:** [state.rs:175-189](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/state.rs#L175) retains snapshots indefinitely; `:222-254` decodes without age or IPC-session checks. [http.rs:282](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/http.rs#L282) returns HTTP 200 for every cached protobuf snapshot. [http.rs:185-195](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/http.rs#L185) always reports `ok=true`. WebTransport sessions and HTTP streams are tied to the gateway's broadcast, not Pi liveness. [bins/marengo-pi/src/host_metrics.rs:46](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/host_metrics.rs#L46) reports `ipc_configured()` as `ipc_connected`, which is only configuration existence ([chappe/src/lib.rs:165-174](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/chappe/src/lib.rs#L165)). Confidence: high; UI reviewer independently confirmed the consuming freshness gap.

**Trigger and consequence:** Pi exits/restarts or the bridge disconnects while gateway stays alive. Browser connection remains open; cached pose/safety/limits and connected status can remain indefinitely. The current safety state can be mistaken for the last one. Separately, reporting IPC configured as connected mislabels outages in diagnostics.

**Fix:** Track actual IPC peer/session liveness and per-topic monotonic receive age; invalidate or explicitly label cached snapshots on disconnect/new boot. Separate process liveness, robot readiness, telemetry freshness, and persistence health endpoints. Publish current connection status from the IPC implementation. UI must independently age its last samples/heartbeat.

**Verification:** Disconnect/restart the Pi under an open browser stream and assert telemetry becomes stale promptly, commands requiring current state are refused, and recovery clears old snapshots/session state. Keep cached historical data available only as explicitly stale information.

### G11 — P2, blocks requested platform workflow: Chappe cannot compile on native Windows

**Evidence:** [chappe/src/lib.rs:18](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/chappe/src/lib.rs#L18) unconditionally includes `ipc`; [ipc.rs:135,155,159,184,198,201,236](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/chappe/src/ipc.rs#L135) reference `std::os::unix`. [transport.rs](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/chappe/src/transport.rs) also embeds `IpcFanout` without a portable boundary. Confidence: native compiler reproduction, seven E0433 errors after supplying vendored protoc.

**Consequence:** This blocks the gateway, Pi simulation/control test dependency graph, and full native Windows workspace builds. The instruction text saying only Unix-socket tests fail on Windows understates the issue: the library itself fails to compile.

**Fix:** Separate the portable Bus/framing/contracts from a platform IPC backend. Gate Unix I/O and Linux-only runtimes properly. Offer a loopback TCP/named-pipe backend for local Windows integration tests, or make the Linux robot-target bins explicitly platform-only while portable libraries/tests compile natively. Add Windows and Mac CI for portable members; keep Linux CAN integration in containers/CI.

**Verification:** Run portable Rust tests on native Windows and Mac, then run Linux gateway/IPC/vcan tests on the robot-target environment. Build/demo a Windows gateway with the chosen portable backend if that is a supported workflow.

### G12 — P2: TLS certificates expire during normal long-running service operation

**Evidence:** [webtransport.rs:175-177](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/webtransport.rs#L175) generates a certificate valid for 13 days; [main.rs:200](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-log-cli/src/main.rs#L200) loads/generates it once at startup. Neither HTTPS nor QUIC has a scheduled renewal/reload path. `load_or_generate_tls` refreshes only on a later service startup. Confidence: high, code-path confirmed; no 13-day clock/uptime simulation.

**Trigger and consequence:** A healthy gateway runs longer than 13 days. HTTPS and WebTransport clients now encounter an expired certificate until the gateway is restarted. This is particularly relevant to returning after one or two months.

**Fix:** Rotate/reload TLS material before expiry for both listeners, or explicitly supervise a safe renewal/restart job. Handle fingerprint changes and browser reconnects coherently. Consider separate long-lived trusted HTTPS identity and short-lived WebTransport pinning material. `pem_valid_for_webtransport` should also actually check the key algorithm it claims to enforce.

**Verification:** Inject a clock and test before/after expiry, automatic renewal, matching HTTPS/QUIC keys/fingerprint, and open/new client sessions. Test invalid/non-P256 custom cert handling.

### G13 — P2: Importing sibling log files erases most artifact references

**Evidence:** [store.rs:634-674](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-store/src/store.rs#L634) imports one row per bench/candump/trace file with only that artifact present. `register_session`'s conflict update at `:335-337` assigns all artifact columns from those sparse rows, replacing the others with NULL. Confidence: reproduced.

**Trigger and consequence:** Import a normal session with three matching timestamp filenames. Only the last file's reference survives; UI/API falsely say the other two are absent. Archive later repairs only files beyond the keep count; the newest 50 sessions can remain broken.

**Reproduction:** `import-legacy` reported three imports for one session, but the resulting row had `bench_blob=null`, `candump_blob=null`, and only `trace_blob` set despite all files existing.

**Fix:** Group artifacts by session before registration, or merge optional artifact fields with existing columns rather than overwriting them with NULL. Distinguish an explicit remove operation from an omitted field. Report unique session and artifact counts accurately; preserve existing label/start/end metadata.

**Verification:** Import all three siblings in every order, repeat import, and add one missing artifact later. Assert all references/metadata survive and the reported session count is one.

### G14 — P2: Imported/archived session dates are replaced with the import time

**Evidence:** [store.rs:793-797](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-store/src/store.rs#L793) parses the session ID as `OffsetDateTime` using `[year][month][day]T[hour][minute][second]Z`. The final Z is a literal, with no parsed UTC offset. OffsetDateTime parsing fails for the normal capture ID. `:648` and `:497` then fall back to `now_ms()`. Confidence: reproduced.

**Reproduction:** The temp session ID `20260801T120000Z` imported `started_ms=1790724596205`, September 29, 2026, instead of August 1. The real harness generates this same timestamp format.

**Consequence:** Historical session sorting/date filters are wrong, and importing or archiving old captures makes them look new to the 30-day purge. Age retention can miss already-old data even after the deadlock is fixed.

**Fix:** Parse a `PrimitiveDateTime` and explicitly assume UTC, or include an offset component in the format. Reject/label unknown IDs deliberately rather than silently changing capture chronology. Preserve existing started/ended values while updating artifact locations.

**Verification:** Assert exact epoch milliseconds for real harness IDs, old captures, invalid IDs, and import/archive round trips. Verify retention uses capture time.

### G15 — P2: Interrupted schema migration makes the log store unable to reopen

**Evidence:** [store.rs:63-73](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-store/src/store.rs#L63) executes migrations and updates schema_version in separate autocommit operations. Migration 2 begins with a non-idempotent `ALTER TABLE ... ADD COLUMN fields_json` ([migrations.rs:63](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-store/src/migrations.rs#L63)), followed by FTS teardown/rebuild. Confidence: reproduced via a simulated interruption after the first v2 DDL.

**Trigger and consequence:** Power loss/error during v2 before schema_version is updated, or two first-time opens racing. Reopen reruns the ALTER against an already-added column and fails. Gateway then disables persistence; CLI maintenance fails. Pi SD/power interruptions make this a real recovery concern.

**Reproduction:** Built a v1 temp DB from the actual migration text, applied only the initial v2 ALTER, left version 1, and reopened with the actual CLI. It failed `duplicate column name: fields_json` with exit code 1.

**Fix:** Serialize and transactionally apply each version plus its version marker on one connection. Fail appropriately for a schema newer than this binary; maintain a backup/recovery strategy. Handle partially upgraded historic DBs deliberately.

**Verification:** Fault-inject between migration statements and race two openers. Each reopen must produce a coherent old or new schema with working insert/search/purge.

### G16 — P2: Paged log reads/downloads perform unbounded blocking work on Tokio workers

**Evidence:** [store.rs:800-820](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-store/src/store.rs#L800) reads the entire plain/decompressed bench/trace file into `Vec<String>` before returning a page. [logs.rs:424-466](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/logs.rs#L424) calls file and DB operations directly in async handlers; candump paging streams memory but scans the entire capture on every request. [logs.rs:603](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/logs.rs#L603) reads the whole download into RAM. Page limits for text endpoints are not clamped like candump limits. Confidence: high; no large-file stress run.

**Trigger and consequence:** A large capture, concurrent log browsing/downloads, or a high limit parameter occupies Tokio threads and can consume hundreds of MB or more. Gateway responsiveness, command requests, and telemetry fanout can stall. A gateway stall also activates G01's unbounded Pi queue risk.

**Fix:** Run file/SQLite work on a dedicated bounded blocking executor, stream file downloads, retain only requested text-page lines, cap all page/body/request sizes, and cache/index summaries where justified. Serialize archive mutation with readers or define snapshot semantics; preserve inexpensive control/health request capacity.

**Verification:** Use large plain/gzip captures with concurrent page/download requests; assert bounded memory, bounded blocking concurrency, and responsive telemetry/control/health. Verify page contents match existing contracts.

### G17 — P2: Candump accepts finite timestamp values that panic instead of returning an error

**Evidence:** [marengo-candump/src/scan.rs:307-310](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-candump/src/scan.rs#L307) checks finite/nonnegative only; `:207` converts a difference to Duration with `from_secs_f64`, which panics when too large. [lib.rs:127-137](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-candump/src/lib.rs#L127) has the same issue for JSON duration deserialization. Confidence: actual CLI reproduction.

**Reproduction:** A temp capture containing `(0.000000) can0 701#AA` and `(1e30) can0 701#AA` caused the CLI to panic `cannot convert float seconds to Duration` and exit 101. Main and recovered source are identical.

**Fix:** Use checked conversion (`try_from_secs_f64`) and a documented timestamp domain; return the crate's typed error or classify the line malformed. Enforce line/decompression limits to avoid malformed-input resource spikes. Validate ASCII DLC values/counts, which are currently recognized syntactically without consistency checks.

**Verification:** Test huge finite, NaN/infinity, regression, rounding edges, JSON offsets, malformed DLC counts, and truncated/corrupt gzip. The public seam must return a result without unwinding for untrusted captures.

### G18 — P2: CPU/iowait metrics use incorrect columns

**Evidence:** [marengo-host-metrics/src/lib.rs:244-245](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-host-metrics/src/lib.rs#L244) strips only `cpu` from `cpu0`, leaving the numeric CPU index in the parsed counters; `:278-280` then treats counters at the wrong offsets for per-core readings. Aggregate `iowait` uses `nums[5]` (IRQ), while actual iowait is `nums[4]`. Confidence: high, arithmetic checked against the primary kernel format.

**Consequence:** Per-core utilization and aggregate iowait charts misreport normal Pi load and can lead diagnosis away from SD I/O stalls. For a core with 10 user, 10 system, 80 idle ticks, the shifted calculation reports 10% rather than 20% usage.

**Fix:** Parse the CPU label separately, validate counter count without silently dropping malformed fields, use the documented indices, and key previous counters by actual CPU ID. Extract pure parsers for fixture tests.

**Verification:** Test aggregate and cpu0/cpu1 lines with known deltas, I/O wait versus IRQ, sparse/hotplugged CPU IDs, missing columns, and counter regression. See [Linux kernel /proc documentation](https://docs.kernel.org/filesystems/proc.html) for the source format.

### G19 — P2: CAN/read-only disk status fields can report false diagnostic states

**Evidence:** [marengo-host-metrics/src/lib.rs:455-460](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-host-metrics/src/lib.rs#L455) returns the last whitespace token on a `can state` line, which is commonly the `restart-ms` number rather than the state; lines containing CAN flags between `can` and `state` do not match. `:377` infers read-only by searching `df -B1` output for `ro`, although mount options are absent from that output. Confidence: high, verified against the primary SocketCAN output format.

**Consequence:** A CAN bus can show `0`/`100`/empty rather than ERROR-ACTIVE/BUS-OFF, and a read-only root filesystem can be labeled writable. Both are precisely the hardware/SD conditions the diagnostics panel should help identify.

**Fix:** Use structured netlink/`ip -json -details` data or parse the value after `state`, and derive mount flags from mountinfo/statvfs rather than df columns. Identify filesystem type separately from source device. Return unknown status explicitly when collection fails.

**Verification:** Fixture-test normal CAN, flagged CAN, ERROR-PASSIVE/BUS-OFF, and real mount-option lines. See [kernel SocketCAN diagnostics examples](https://docs.kernel.org/networking/can.html).

### G20 — P2: A failed HTTP/HTTPS listener leaves a nominally running gateway

**Evidence:** [bins/marengo-gateway/src/main.rs:207-218](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/main.rs#L207) spawns HTTP serving and only logs bind/serve failures. `:234-243` handles HTTPS similarly. The main task continues serving QUIC at `:246`, so a failed control/API/UI listener does not fail the process and systemd's `Restart=on-failure` cannot repair it. Confidence: high; no port-conflict integration run.

**Trigger and consequence:** One port is unavailable or a listener fails while QUIC remains alive. systemd shows the gateway running, but Consul/HTTP commands are missing or partly unusable. There is no consolidated listener readiness state.

**Fix:** Bind required listeners before declaring ready and supervise their tasks together. Unexpected termination of a required listener should fail the process or publish degraded readiness and retry according to a defined policy. Keep optional listeners explicitly optional.

**Verification:** Occupy each required port, fail one listener after startup, and verify startup failure/restart or intentional degraded state. `/health`/readiness must accurately reflect listener and robot dependencies.

### G21 — P2: URDF activation can return failure after already changing the master

**Evidence:** [bins/marengo-gateway/src/hardware.rs:357–370](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/hardware.rs#L357) promotes the merged master using `fs::rename`. [Lines 372–375](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/hardware.rs#L372) then remove staging and call `completeness_report`, mapping its error to HTTP 500. [crates/marengo-config/src/completeness.rs:52–59](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-config/src/completeness.rs#L52) can fail while loading robot/motors/control YAML, resolving the URDF path, or reading/parsing the resulting URDF. Confidence: high, verified source control flow; no injected post-promotion failure was executed.

**Trigger and consequence:** Promotion succeeds, but a required config file is missing, unreadable, invalid, or changes before the subsequent completeness read. The client receives failure even though the master is changed, the old master is archived, and staging has been removed. A retry can therefore get Not Found and cannot distinguish an unapplied request from an applied mutation whose response failed. The loaded Pi model remains the older model until restart. This is an outcome-reporting defect distinct from G09's competing writers.

**Fix:** Validate the prospective complete configuration and compute completeness before committing the master. Give every committed mutation an immutable receipt with request identity, resulting generation and checksum. Once promotion succeeds, return that applied receipt even if advisory reporting or cleanup later fails, and record those secondary failures separately. Retrying the same request should recover its receipt instead of reapplying or misreporting the result.

**Verification:** Inject a completeness/config-read failure around the promotion boundary; assert a failed precommit leaves the master unchanged, and a committed change returns or later recovers its correct receipt. Retry after a lost response and verify idempotence, checksum, archive identity and `restart_required` state.

## Additional design concerns and recommended fixes

These are separate from the above reproduced bugs or require a broader policy decision/test to determine priority.

1. **Mutation contracts are split across legacy and typed-command APIs.** Enable, Testing MIT, set-zero, tuning, limit patch, commissioning scope, URDF, restart, and deploy each have different auth, rate, confirmation, acknowledgement, and freshness semantics. Replace this with a small command-management module owning accepted/applied/durable outcomes, identity, idempotency, expiry, and rejection reasons. Keep routing thin and physical enforcement in Davout.
2. **No request identity robust enough for concurrent limit operations.** [limit_patch.rs:120-127](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/limit_patch.rs#L120) derives request/session IDs from joint plus milliseconds. [action_ack.rs:24](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/action_ack.rs#L24) matches session and action only; passed joint/operator are not validated. Two same-joint requests in one millisecond can consume the same ACK. Use unique opaque command IDs, boot IDs, monotonic command sequence, and exact resulting revision/config generation. Add collision/out-of-order/duplicate tests.
3. **CAS does not represent the client's edit baseline.** `ConfigPatchJson` has no expected-revision field; [config.rs:236-245](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/config.rs#L236) reads a fresh disk revision on the server. The live LimitPatch helper supports expected_revision internally, but the public route does not expose it. Two operators editing the same stale form can overwrite each other's changes without a client-visible conflict. Accept the client's live-generation/revision, not a just-in-time server disk token; keep optional backwards compatibility explicit.
4. **Log DB availability gates unrelated config reads.** [config.rs:185](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/config.rs#L185) returns 503 if log persistence is unavailable, despite only reading YAML for the snapshot. The mutation also hard-requires log services. Decouple read-only config visibility from logging, and decide explicitly whether audit failure should block mutations. When it does, report an audit-store reason rather than an ambiguous service outage.
5. **Log write failures are discarded and not counted as drops.** [logs.rs:298-305](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/logs.rs#L298) takes the batch then only warns on an insert failure. `/health.dropped_log_inserts` counts queue-send failures, so it can remain zero during sustained DB-write loss. Add writer health/last success/failure counters, bounded retries or a spool, and a graceful shutdown flush. Avoid recursive self-logging failure amplification.
6. **IPC peer and frame trust/lifecycle are implicit.** Every new connection replaces the command peer ([ipc.rs:208-213](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/chappe/src/ipc.rs#L208)) without producer identity; disconnected peers are not cleared by read_connection. Frame readers have no topic/payload bounds (`:263-277`), idle deadlines, or handshake/version checks. Future additional producers could steal the Pi command destination. Define role/boot identity, one control-runtime connection, protocol bounds, deadlines, and disconnection lifecycle. Local socket mode is useful but does not identify the correct producer.
7. **The durable store uses absolute paths and ambient default root resolution.** Session rows store full machine paths; relocating/restoring var/db to `J:/code` or another host can leave dangling artifact paths. `marengo-log-cli` also computes default DB path from environment independently of its explicit `--root` ([main.rs:220-222](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-log-cli/src/main.rs#L220)), and `Store::log_disk_usage_bytes` resolves an ambient DB path instead of the opened path ([store.rs:627](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-store/src/store.rs#L627)). Store root-relative artifacts and the actual opened DB path; resolve --db > environment DB > explicit root consistently; provide a verified migration/relink operation.
8. **Retention/budget settings currently overstate implementation.** `log_disk_budget_bytes` is persisted but not enforced; metrics hardcode 5 GiB and omit SQLite WAL/SHM. `MARENGO_LOG_ARCHIVE_DAYS` is documented but no runtime use was found, and the maintenance service hardcodes 30 days. Archiving/reads/import repeatedly scan/decompress whole data. Implement one typed retention policy, measured total disk accounting, an explicit budget action, and bounded cleanup. Do not represent unknown metrics as healthy zeros.
9. **Self-update status is mostly revision/file readiness, not service readiness.** `marengo-deploy::reconcile_job` promotes running to success from `.deploy-rev`; `ready_for_target` only checks matching SHA and www index presence. A successful marker does not verify gateway/Pi startup, commissioning state, or correct UI bundle version. Add a release manifest and post-install health handshake; keep physical enabling a separate action. On-disk job reconciliation/status writes should compare job IDs/generations under a lock so a reader cannot overwrite a newer enqueue.
10. **Timeout should cancel helper ownership.** [marengo-deploy/src/enqueue.rs](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/crates/marengo-deploy/src/enqueue.rs) wraps process output in a timeout without `kill_on_drop(true)`, unlike gateway restart's process helper. A timed-out enqueue can continue and start a job after an HTTP failure/retry. Define cancellation and job receipt semantics; use kill-on-drop where appropriate while distinguishing the intentionally detached systemd worker.
11. **Hardware upload/activation identity and archive immutability need strengthening.** Upload IDs use wall-clock milliseconds ([hardware.rs:129-134](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/hardware.rs#L129)); simultaneous uploads can share staging paths. Restoring a prior upload then activating reuses its archive directory and rewrites original replaced-active/manifest files. Use unique staging IDs and fresh immutable activation records, with source archive references and checksums. Validate/serialize master changes and handle staging cleanup/failed uploads.
12. **Proto-first has a partial duplicate-schema seam.** The binary Chappe path is truly protobuf, but HTTP config/log/deploy/URDF payloads are independent Rust serde/TS JSON types, while several log/candump protobuf messages duplicate them. Decide whether these HTTP schemas are generated from proto/OpenAPI or explicitly separate; test one canonical contract rather than maintaining parallel unused definitions. Envelope omits topic/boot identity; source role currently disambiguates HostMetrics, but this is fragile for future routing and replay.
13. **A 200 publish response should not be presented as an applied operation.** Non-limit commands currently acknowledge socket publication, not Pi acceptance. This is documented for leases/polls, but stronger typed command receipts would make enable/tuning/set-zero failures visible without relying on logs (which G02 suppresses). Frontend should await confirmed state or an operation result where the action matters.
14. **Module boundaries have grown shallow at the gateway.** Hardware lifecycle currently owns filesystem transactions/archive records inside a large HTTP bin module; config/state/persistence concerns span gateway and Pi bins. Move transaction/management/command receipts to deep library modules with simple interfaces and deterministic failure tests. `marengo-deploy` is a useful start, but its current fixture tests check JSON shapes, not execution of the scripts producing them.
15. **Documentation needs a source-of-truth reset after branch reconciliation.** Main changed master-config, 5-DOF, commissioning, and retired Home behavior, while the recovered branch retains 4-DOF profile APIs and Home. Update README, CONTEXT, root/crate/bin codemaps, ADR status, and runbooks together. Label scaffolds/unused contracts, and include an exact supported Windows/Mac development workflow that stores source only under `J:/code` on this machine.

## Focused fix sequence

1. Coordinate with the control report before further commissioning: close automatic/stale management authorization paths; establish truthful runtime connection/config generations; make failed persistence recoverable and blocking for normal restart/update.
2. Fix G04/G05/G08/G09 as one persistence architecture pass: full aggregate coalescing, completion ACKs for every request, one URDF/YAML writer, live-generation no-op/CAS, safe restart handshake, immutable records.
3. Fix G01/G02/G03 immediately: bounded IPC memory, real logging rate window, and deadlock-free retention. These are small core changes with strong reproductions.
4. Establish the native Windows/Mac development seam (G11), keep Linux CAN/runtime integration as a target, and complete checkout/path migration without coupling active source to WSL.
5. Repair the historical log store (G13/G14/G15), then stream/bound blocking I/O (G16), count actual persistence failures, and implement an honest retention/disk policy.
6. Add TLS renewal/listener supervision and real liveness/diagnostic parsers (G10/G12/G18/G19/G20), then harden network auth and request identity consistently if not addressed in step 1.

## Test gaps that matter

- No IPC outage/non-reading peer/backpressure/reconnect tests; existing IPC tests cover only encode/decode.
- No logging quota-refill/concurrency tests; existing tracing tests cover labels/JSON formatting only.
- No Store purge-with-old-session, import-sibling-artifact, capture-date, interrupted migration, parallel opener, move/restore, or disk-failure tests.
- No persistence sequence tests combining motors-bearing and control-only requests; no terminal ACK test for every coalesced requester or gateway restart during pending/failure.
- No safe management test for disconnected live Active robot, unknown boot state, failed persist, concurrent enable, or cross-writer URDF race. One existing restart test currently verifies unsafe stale-Active acceptance.
- No native Windows CI for the portable Rust dependency graph; the documentation's platform caveat is not validated by compilation.
- No certificate-expiry/renewal, listener-supervision, unauthorized legacy-route/stream, or large-log responsiveness tests.
- Metrics tests assert little more than hostname; no CPU, CAN, mount, or service-output fixtures.
- Deploy contract tests deserialize handwritten fixture shapes; they do not prove scripts write correct state under bootstrap failure, timeout, cancellation, crash/restart, concurrent status polls, or failed service readiness.

The passing tests are real validation of currently covered behavior. They do not establish that the robot is ready for live motion, that the runtime matches the repo, or that these uncovered recovery/failure paths are safe.
