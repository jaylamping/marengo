# Phase B — WP-K: gateway authority & health

Branch `audit/wp-k`; package `marengo-gateway`, Consul API contract, MCP archived-log callers, and `pi-remote.sh`.

"Red" evidence below is either an observed baseline failure, the former regression assertion/code path, or baseline code inspection where executing the unsafe behavior would invoke external or live operations. Green evidence is from the listed regression tests and acceptance gates.

## Verdicts

| Lead | Verdict | Evidence (red → green) | Fix |
|---|---|---|---|
| L-marengo-gateway-01 (G06) | **CONFIRMED** | Baseline restart predicate treated missing/invalid or stale mode/heartbeat as permission and permitted stale Active; the old stale-Active-via-stub test explicitly expected HTTP 200. New `restart::tests::management_state_requires_fresh_valid_disabled_or_ready_evidence`, `restart_refuses_active_with_stale_heartbeat`, and `deploy::tests::deploy_refuses_when_runtime_evidence_is_missing` pass. | Shared gate now allows only fresh, valid Disabled/Ready evidence; unknown, invalid, stale, future-dated, or Active evidence refuses restart, deploy, and URDF activation. |
| L-marengo-gateway-17 (G20/G10) | **CONFIRMED** | Baseline `/health` returned unconditional `ok:true`; deploy-wait trusted that result. `state::runtime_transition_stream_tests::listener_health_requires_bound_http_and_required_https` passes; MCP `deploy-wait` test asserts the health JSON listener contract. | Health reports listener, IPC, Store, and dropped-log state; `ok` requires HTTP and any configured HTTPS listener. HTTPS health is set only after bind/server construction succeeds and cleared when serving stops. `pi_wait_deploy` checks `ok`, HTTP, and required HTTPS readiness. |
| L-marengo-gateway-02 (G05) | **CONFIRMED** | Baseline disconnect reset persistence flags despite no Durable evidence; restart also ignored degraded status and activation ignored both flags. New `state::ipc_snapshot_tests::reconnect_does_not_clear_unresolved_persist_flags` passes. Gate paths now check pending/degraded state. | Disconnect no longer clears pending/degraded evidence. Restart, deploy, and activation refuse pending or degraded persistence. **Unresolved recovery semantics are in NEEDS-DECISION**: a lost terminal event cannot safely be inferred as Durable. |
| L-marengo-gateway-18 (ADR 0033 / R08) | **CONFIRMED** | Baseline inventory routes were anonymous. `gateway_access_conformance_test::all_protected_routes_refuse_before_body_files_publication_or_subscription` now exercises `/config/snapshot`, `/hardware/completeness`, and `/hardware/commissioning-scope` against missing, wrong, foreign-origin, and valid credentials. Consul `config-api.ts` and `gateway-api.ts` send configuration credentials. | Configuration capability required for config inventory and hardware inventory/scope routes; Consul callers already send credentials. |
| L-marengo-gateway-19 | **CONFIRMED** | Baseline had no deployment-gate test and restart’s unsafe stale-Active test pinned the old behavior. Added missing-evidence deploy-route regression; replaced stale-Active success with refusal and expanded restart predicate coverage. | Tests cover restart and deploy management admission; state health and startup-callback lifecycle also receive regressions. |
| L-marengo-gateway-21 | **REFUTED** | Current Consul exports `postTestingMitCommandBatch`, which posts to the real `/command/testing_mit` endpoint; no `postMitCommandBatch` or `/command/mit` reference exists in current `gateway-api.ts` / `chappe-client.ts`. | No dead export to delete; keep the live Testing MIT API. |
| L-marengo-gateway-23 | **REFUTED** | Current gateway conformance tests subscribe to actual allowed command topics; no `robot/limits/patch` entry exists. | No vacuous nonexistent-topic assertion remains in the current test. |
| L-marengo-gateway-05 | **CONFIRMED** | WP-J inspection confirmed `config.rs` requires `state.logs` for snapshot/patch and uses Store only for the audit upsert (`post_config_patch` lines 189–193, 229–234). | Not changed here: `config.rs` belongs to WP-J. Cross-package owner confirms it remains unchanged in WP-J; see cross-package note. |
| L-marengo-gateway-14 | **CONFIRMED** | Baseline generated the TLS private key through `std::fs::write`, applying process umask. `webtransport::tests::tls_material_retains_certificate_chain_and_matching_key` now asserts Unix key mode `0600` after generation. | Key creation uses mode `0600` and reapplies restrictive permissions before writing. Plaintext HTTP `:8080` remains **NEEDS-DECISION**. |
| L-marengo-gateway-07 | **CONFIRMED** | Baseline callbacks dropped initial IPC frames/connection transitions while `state_holder` was `None`. New `ipc_bootstrap_tests::callbacks_before_state_installation_are_replayed_in_order` passes. | Buffer callbacks until AppState is installed, then replay them in order under the holder lock. |
| L-marengo-gateway-12 | **CONFIRMED** | Baseline `/stream/chappe` had no concurrent subscriber bound. Existing `http` stream-cap regression asserts the 65th stream is refused with 429 after 64 permits. | A bounded 64-permit semaphore caps HTTP stream pumps; permit lifetime follows the pump. |
| L-marengo-gateway-22 | **CONFIRMED** | Baseline `/config/patch` authorized Configuration alone; new `role_matrix_and_body_limits_preserve_independent_admission` proves both Configuration-only and Control-only credentials receive 403. | Config patch now requires both Configuration and Control capabilities. |
| L-marengo-gateway-13 | **CONFIRMED (inspection)** | `ingest_runtime_frame` clones topic/payload for broadcast and decodes/re-encodes per subscriber as described; no profile evidence demonstrates material impact on this 5-joint bench. | No optimization made without measurement. Profiling and representation choices are in NEEDS-DECISION. |
| L-marengo-gateway-24 | **CONFIRMED** | Baseline allowed `robot/testing/mit_command_batch` as a subscribable topic and bus fanout echoed the gateway’s outbound command to subscribers; no consumer exists. `state::ipc_snapshot_tests::testing_mit_command_is_not_a_subscribable_runtime_topic` passes. | Removed the command topic from runtime/stream `ALLOWED_TOPICS`; the control command still goes to the Pi over IPC. |
| L-marengo-gateway-11 | **CONFIRMED (inspection)** | The tuning limiter keys its bucket by caller-provided `session_id`; rotating ids bypasses the per-session bucket and expands the map. | No change pending a principled caller identity and bucket-bound decision; see NEEDS-DECISION. |
| L-marengo-gateway-20 (assigned MCP/script follow-up) | **CONFIRMED** | Baseline archived-session curls omitted the gateway token, causing `/logs/sessions` 401 and hot-file fallback. MCP logs tests cover token forwarding; the Pi remote shell preamble already sources `/etc/marengo/env`. | `pi_logs_list`, `pi_logs_archive_list`, and `pi-remote.sh logs-list` use `MARENGO_GATEWAY_LOG_TOKEN` via `x-marengo-log-token` when configured, retaining existing fallback behavior. |

## NEEDS-DECISION

### L-marengo-gateway-02 — safely recovering persistence state when terminal evidence is lost
- **A (recommended):** add an authoritative reconciliation path keyed by persistence operation/session, where the Pi reports terminal state before the gateway clears a pending/degraded marker. This preserves fail-closed behavior while avoiding unrelated Durable events clearing other operations.
- **B:** require explicit operator acknowledgement/restart of the gateway to clear uncertain state. Simpler, but recovery is less observable and may discard unresolved evidence.
- **C:** clear on IPC reconnect or any Durable event. Not recommended: reconnect and unrelated Durable evidence do not prove the lost operation was persisted.

### L-marengo-gateway-14 — plaintext HTTP listener
- **A (recommended):** bind HTTP to loopback and use HTTPS for remote/browser access; preserves local readiness probes but changes direct LAN MCP/Consul callers.
- **B:** keep LAN HTTP but provision/use TLS for all credential-bearing callers; requires migrating clients and certificates.
- **C:** retain plaintext LAN HTTP with documented credential exposure. Not recommended because Bearer credentials traverse the LAN unencrypted.

### L-marengo-gateway-13 — fanout allocations
- **A (recommended):** profile representative stream subscriber counts and payload rates first, then optimize only measured hot allocations.
- **B:** immediately move the broadcast payload to shared immutable bytes/serialization to avoid per-subscriber copies; adds representation and lifetime complexity without measured bench impact.

### L-marengo-gateway-11 — tuning rate-limit identity
- **A (recommended):** key buckets by an authenticated credential/principal identity and cap/expire that map; rotating client `session_id` then cannot evade admission.
- **B:** use one global tuning bucket, bounded and simple but shared operators contend.
- **C:** keep client-session buckets and only bound their map; prevents memory growth but leaves the bypass.

### L-marengo-gateway-05 — Store availability versus config availability (WP-J ownership)
- **A (recommended):** decouple snapshot and config mutation from Store availability; make the audit write best-effort while surfacing its failure separately.
- **B:** require Store availability for config mutation and document the dependency. Not recommended: the config disk/Pi path does not need Store to perform the operation.

## Cross-package edits

- WP-J inspected `bins/marengo-gateway/src/config.rs` and confirmed L-marengo-gateway-05; no change was made because that file belongs to WP-J.
- No Consul source files were modified: current callers already use the valid `/command/testing_mit` endpoint, and config/hardware inventory callers already attach credentials.

## Gate

- `cargo fmt --all -- --check` — pass.
- `cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings` — pass.
- `cargo test --workspace` — pass, 1,155 passed across 122 suites, 1 ignored (10 existing warnings).
- `cargo test -p marengo-gateway` — pass, 78 tests.
- `cd tools/marengo-pi-mcp && npm test` — pass, 200 tests (installed locked dependencies with `npm ci`; the first test attempt lacked `tsx`).
- Consul `npm test` / `npm run build` not run; no Consul files were changed.
