# Intent card — `marengo-gateway`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-gateway` |
| Path | `bins/marengo-gateway` |
| Kind | bin (`src/main.rs`); no cargo features (`Cargo.toml:13-53`) |
| Baseline | `a2b55b3` |
| LOC | src 4023 / tests 3787 (inline `#[cfg(test)] mod` + `*_test(s).rs` split); metrics `loc.md:20` file-level split 7791/639. Coverage 68.2 % lines (`metrics/coverage-by-crate.md:12`); 62 tests (`metrics/test-counts.md:52`) |
| Sources | `README.md`, `codemap.md`, `src/codemap.md`, `bins/AGENTS.md`, `bins/codemap.md`, `AGENTS.md:35,61,175,223`, `codemap.md:11,22,63`, `CONTEXT.md:22,34,39-40,46-48`, ADR 0008/0011/0012/0017/0019/0032/0033, `docs/safety.md:88-111`, prior `2026-09-29/{finding-index.md,gateway.md,consul.md,tooling.md,test-quality-plan.md,implementation-ledger.json}`, `git log -- bins/marengo-gateway` (61 commits; first `99ea9fb` 2026-05-26 "Add marengo-gateway for Consul live telemetry over Chappe"; turns `1bfce54` log store, `129ba68` GUI restart, `f743825` Set Limits Durable wait, `9a7894e` Hardware commissioning/URDF, `73992cd` self-update, `78590c2`/`b5b16fa` observation invalidation, `df46849` runtime access ADR 0033), consumers in `consul/src/lib/{gateway-api,config-api,hardware-api,log-api,version-api,chappe-transport,chappe-config}.ts`, `tools/marengo-pi-mcp/src/{robot-state.ts,tools/logs.ts,tools/readonly.ts,tools/deploy-wait.ts}`, `scripts/{pi-remote.sh,cloud-pi-lib.sh,install-pi.sh:338}`, `metrics/*` |

## 2. Intent

The gateway is the **operator edge** between Consul (browser) and the Pi runtime: it ingests Chappe envelopes from `marengo-pi` over the Unix-socket IPC, caches the latest per-topic snapshot, and fans envelopes out over WebTransport and an HTTP length-prefixed stream, while relaying operator commands back over the same socket (ADR 0008 §Decision 1–4; `main.rs:1`; `README.md:3`). Since ADR 0011 it also owns the **SQL log writer** and log-archive HTTP API; since ADR 0012/0017 it is the HTTP face of the **Live limit patch** (waits for Pi `limit_patch` then `limit_patch_persist` ACKs before reporting success, `docs/safety.md:92`); since `9a7894e` it manages the **Pi URDF library** (staging, Accept URDF, archive, commissioning scope; `CONTEXT.md:34,39-40,46`); since `129ba68`/`73992cd` it runs management actions (restart `marengo-pi`, self-update). ADR 0033 makes it the single **access policy** for HTTP/HTTPS/WebTransport: credentials gate requests but "authorizing a gateway request is not motor authority" — Davout gates remain authoritative.

Conflicting statements of intent:
- ADR 0008 / `bins/AGENTS.md` "thin runtime … logic in crates" vs. ~4 kLOC of policy in the bin (URDF transaction, limit-patch orchestration, access policy, rate limiter). ADR 0019:80 calls extraction "appropriate".
- ADR 0019 (Proposed, `0019:1-4,30-33`) says gateway must **not** write competing master files nor infer permission from cached telemetry; current code does both (`hardware.rs:328-356`, `restart.rs:47-60`). CONTEXT.md:46 documents the current behavior as intended.
- ADR 0011 §5 calls the log token "optional"; ADR 0033 makes credentials mandatory for sensitive reads (superseded).

## 3. Owns / Must not

| Owns | Must not |
|---|---|
| IPC server endpoint + snapshot cache + envelope fan-out (`main.rs:159-184`, `state.rs:197-247`) | Command motors itself or bypass Davout (ADR 0033 ¶3; `bins/AGENTS.md` anti-pattern) — upheld: all commands go through `publish_command_envelope` → IPC (`state.rs:253-277`) |
| Access policy (5 capabilities, Origin policy) for all transports (`access.rs:12-235`, `http.rs:202-259`, `webtransport.rs:83-147`) | Treat auth as motor permission (ADR 0033 ¶3) — upheld; attestation/rate/allowlist checks remain (`http.rs:439-470`) |
| Live limit patch HTTP orchestration + ACK honesty (`limit_patch.rs:60-263`) | Write YAML for live limits (ADR 0012 §2: Pi owns write-behind) — upheld for limits; **violated in spirit for URDF**: gateway promotes master URDF itself (`hardware.rs:328-356`), the dual writer G09 |
| URDF staging/merge/activate/archive, commissioning-scope file (`hardware.rs:198-633`) | Infer management permission from stale telemetry (ADR 0019:32-33, Proposed) — **violated** (`restart.rs:53-59`, used by `deploy.rs:94`, `hardware.rs:255`) |
| Log ingest batch writer + log HTTP API (`logs.rs:203-568`) | Logic in bins (`bins/AGENTS.md` anti-patterns) — **violated** at scale (see §2) |
| Restart / self-update triggers (`restart.rs:72-217`, `deploy.rs:60-192`) | Expose sensitive streams anonymously (ADR 0033) — upheld (`http.rs:214-224,261-269`, `webtransport.rs:131-147`) |

## 4. Interface

| Group | Surface | Consumers | Notes |
|---|---|---|---|
| CLI | `--http-listen --https-listen --wt-listen --web-root --chappe-socket --demo --tls-cert --tls-key` (`main.rs:48-108`) | `scripts/install-pi.sh:338` (systemd ExecStart: `[::]:8080`, `[::]:8444`, `[::]:8443`), `README.md:17-19` (`--demo`) | HTTP 8080 is plaintext on the LAN |
| Public HTTP GET | `/health` (`http.rs:271`), `/tls/fingerprint`, `/snapshot/robot/{state,safety,heartbeat}`, `/snapshot/sensors/imu/torso`, `/snapshot/host/metrics/{pi,jetson}`, `/snapshot/actuator/limits`, `/config/snapshot`, `/hardware/completeness`, `/hardware/commissioning-scope`, `/version/status` (`http.rs:90-153`) | `/health`: Consul `gateway-api.ts`, `telemetry-source.ts`; MCP `readonly.ts:46,151`, `deploy-wait.ts:22`; `pi-remote.sh:57`. `/snapshot/robot/state`: MCP `robot-state.ts:104`. `/snapshot/actuator/limits`: `gateway-api.ts:38`. `/tls/fingerprint`: `chappe-transport.ts`. `/config/snapshot`: `config-api.ts:74`. completeness/scope: `hardware-api.ts`, `gateway-api.ts`. `/version/status`: `version-api.ts`. **No consumer**: `/snapshot/robot/{safety,heartbeat}`, `/snapshot/sensors/imu/torso`, `/snapshot/host/metrics/*` | `/command/home` → 410 (`http.rs:158,410-415`) |
| Stream | `GET /stream/chappe?topics=` (`http.rs:300-335`); WebTransport `/chappe` with `GatewaySubscribe` → `GatewaySubscriptionAdmission` (`webtransport.rs:111-189`); topic allowlist `state.rs:34-48` | `chappe-transport.ts`, topics from `chappe-config.ts:123-144` (8 topics). Not subscribed by any consumer: `robot/testing/telemetry`, `robot/testing/mit_command_batch`, `robot/audit/{tuning,action}`, `robot/actuator/limits` | Envelopes decoded+re-encoded per subscriber (`framing.rs:92-93`) |
| Commands (Control/Calibration) | `POST /command/{enable,testing_mit,set_zero,active_reporting_lease,motor_status_poll,actuator}` (`http.rs:155-168`) | all in `consul/src/lib/gateway-api.ts` | Chappe topics `robot/enable`, `robot/testing/mit_command_batch`, `robot/set_zero`, `robot/active_reporting_lease`, `robot/motor_status_poll`, `robot/actuator/command` |
| Configuration | `POST /config/patch` (`config.rs:185`), `/hardware/urdf{,/upload,/resolve-preview,/activate,/archive,/archive/{id},/archive/{id}/restore}`, `PUT/DELETE /hardware/commissioning-scope` (`http.rs:124-148`) | `config-api.ts:90`, `hardware-api.ts`, `gateway-api.ts` | GET `/hardware/urdf*` also needs Configuration (`http.rs:226-227`) |
| Management | `POST /control/restart-marengo-pi`, `POST /control/deploy` (`http.rs:149-154`) | `config-api.ts` (`restartMarengoPi`), `version-api.ts`; MCP `pi_restart_marengo_pi` uses its own helper, not this route | |
| SensitiveRead | `/logs/sessions*`, `/logs/structured`, `/snapshot/logs/recent`, `/settings` (`http.rs:107-123`) | `consul/src/lib/log-api.ts:143-217`; `scripts/cloud-pi-lib.sh:212-221` (legacy header); MCP `logs.ts:59,98` and `pi-remote.sh:82` curl **without** credential. **No consumer**: `/settings`, `/logs/sessions/{id}/download` | |

Depth: shallow adapter over `marengo-config`, `marengo-store`, `marengo-deploy`, `chappe`; deep spots are `access.rs` (small interface `authorize(headers, cap)`, hidden digest/Origin logic) and `limit_patch.rs`. Seams: `AppState` (one implementation; tests swap `AccessPolicy` via `with_access`, `state.rs:160-163`); `IpcListener` optional (`state.rs:67,271`) — test seam only (one adapter = hypothetical seam). No traits defined. Unused deps per `metrics/unused-deps.md:24-28`: `marengo-support`, `prost`, `quinn`, `thiserror` (confirmed: 0 uses in `src/`); `serde_yaml` used only in `hardware_tests.rs:281`. Zero-use pub item: `framing::read_length_prefixed` (`metrics/pub-usage.md:21`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) | Coverage of file |
|---|---|---|---|
| Unset/invalid/ambiguous credentials fail closed; capability bits do not escalate | `access.rs:70-119,178-210`; invalid env → empty policy (`state.rs:121-124`) | `access.rs:252,288,315`; `gateway_access_conformance_test.rs:58,195,265`; `gateway_access_public_test.rs:20` | access 93.5 % |
| Auth before body parse/publish/file change | middleware layer `http.rs:169-173,202-259` | `gateway_access_conformance_test.rs:58` (all protected routes) | http 72.7 % |
| Sensitive WT subscription needs credential in protobuf; bounded handshake (5 s, 16 KiB, 64 sessions) | `webtransport.rs:19-22,49-67,116-147` | `webtransport_access_tests.rs:98,158,196,254` | webtransport 49.6 % |
| IPC connection change retires all snapshots and emits typed invalidation | `state.rs:90-111`, `filter_topics` forces runtime topic `state.rs:326-340` | `state.rs:368,413` | state 80.3 % |
| Broadcast lag is surfaced as `RuntimeObservationGap` | `framing.rs:97-108` | `framing.rs:120` | framing 83.2 % |
| Set Zero needs confirm + sign attestation + allowlisted joint + global motion bucket | `http.rs:431-493`, `ratelimit.rs:108-110` | `http.rs:695,716,737`; `ratelimit.rs:190` | |
| Tuning: only kp/kd, clamped to live `kp_max/kd_max`, persist/firmware tiers need extra capability | `actuator.rs:59-125,148-187` | `actuator.rs:292,415,439,459,500,534` | actuator 90.6 % |
| Live limit patch HTTP success only after live ACK **and** Durable persist ACK | `limit_patch.rs:168-244` | ACK matcher only: `action_ack.rs:106,128,169,200`. **`apply_limit_patch_async` untested** | limit_patch **0 %** (`coverage-by-file.md:9`) |
| Restart/update/URDF activate refused while Active+fresh heartbeat, or persist Pending | `restart.rs:47-60,86-113`; `deploy.rs:90-119`; `hardware.rs:253-266` | `restart.rs:289,340`; `hardware_tests.rs:214`. Deploy gate **untested** | deploy **0 %** |
| Upload ids path-safe | `hardware.rs:136-159` | `hardware_tests.rs:180` | hardware 74.0 % |
| Commissioning scope widen requires confirm | `hardware.rs:597-625` | `hardware_tests.rs:491` | |
| Log ingest bounded (16 384 queue, drop counter on `/health`) | `logs.rs:28,254-257`, `http.rs:271-282` | untested (no test of drop path) | logs 36.8 % |
| Status poll globally ≤0.5/s burst 2 | `ratelimit.rs:20-21,112` | `http.rs:775`, `ratelimit.rs:239` | ratelimit 97.2 % |

## 6. Inputs / outputs

- **Env**: `MARENGO_GATEWAY_{OPERATOR,LOG,CONTROL,CALIBRATION,CONFIG,MANAGEMENT,READ}_TOKEN`, `MARENGO_GATEWAY_ALLOWED_ORIGINS` (`access.rs:72-117`); `MARENGO_CHAPPE_SOCKET` via `socket_path_from_env` (`main.rs:56`); `MARENGO_GATEWAY_TLS_EXTRA_SAN` (`webtransport.rs:253`); `MARENGO_ROOT` (TLS dir `webtransport.rs:298-303`, store/config roots via `marengo_store::resolve_marengo_root`, `main.rs:127`); `MARENGO_RESTART_MARENGO_PI_SCRIPT`, `MARENGO_RESTART_SKIP_SUDO` (`restart.rs:63,170`); `MARENGO_JOINT_SUBSET` (via `joint_subset_from_env`, `hardware.rs:538,565`); config-dir resolution via `marengo_config::resolve_config_dir` (`config.rs:88-90`).
- **Files**: `$ROOT/var/gateway/tls/{cert,key}.pem` (generated 13-day ECDSA, `webtransport.rs:264-296`); `assets/urdf/{marengo.urdf,staging/<id>/contributor.urdf,archive/<id>/{contributor,replaced_active}.urdf,manifest.json}` (`hardware.rs:28-113`); `var/commissioning-scope.yaml` (marengo-config default path); master `config/*.yaml` read; `marengo.db` via Store; session blob files (`logs.rs:561`); `pi-restart-marengo-pi.sh` in `marengo_deploy::paths::PRIVILEGED_HELPERS_DIR` (`restart.rs:69`).
- **Chappe in** (IPC, then snapshot/fan-out): `ALLOWED_TOPICS` `state.rs:34-48`; `robot/audit/action` also drives persist flags (`state.rs:203-219`) and ACK waits (`action_ack.rs:13-47`).
- **Chappe out** (IPC + local Bus): `robot/enable`, `robot/testing/mit_command_batch`, `robot/set_zero`, `robot/active_reporting_lease`, `robot/motor_status_poll`, `robot/actuator/command` (`http.rs:383,401,484,574,624`; `actuator.rs:137`; `limit_patch.rs:155`); synthesized `gateway/runtime_connection` (`state.rs:90-111`), `robot/testing/telemetry` (`state.rs:225-229`).
- **HTTP routes**: §4. **Processes**: `sudo -n <script> restart` (`restart.rs:174-189`); self-update enqueue via `marengo_deploy::enqueue_self_update` (`deploy.rs:164`); GitHub tip fetch (`deploy.rs:135`, `get_version_status(refresh)` `deploy.rs:50-58`).
- **DB writes**: `log_events` batches (`logs.rs:294-306`); `config_overrides` upsert `config.patch.<joint>` (`config.rs:231-245`).

## 7. Prior review reconciliation

| Id | Prior | Current status | Evidence |
|---|---|---|---|
| G01 | Pi telemetry queue unbounded | **fixed** (chappe, not this bin) | ledger verified (Batch22, PR merged `0166970`); bounded outbox `crates/chappe/src/ipc_outbox.rs:8-23` |
| G02 | tracing quota never refills | **fixed** (chappe) | PR213; `crates/chappe/src/tracing_layer.rs:19-55` atomic windowed quota |
| G03 | retention deadlock | **fixed** in store; note gateway no longer runs retention at all (purge only via `marengo-log-cli purge`, `scripts/systemd/marengo-log-maintenance.service:11`) | `crates/marengo-store/src/store.rs:269-285`; `crates/marengo-store/tests/retention.rs` |
| G04 | coalescing drops motor/URDF persist + ACKs | open (Pi side) | ledger `open`; gateway still waits on one `session_id` only (`limit_patch.rs:168-192`) |
| G05 | global persist flags | **open** | `state.rs:75-78,203-219` still two global atomics, not reset on IPC loss (`state.rs:90-111`); restart ignores `persist_degraded` (`restart.rs:103`) |
| G06 | missing/stale telemetry permits restart/update/URDF activation | **open** | `restart.rs:53-59` returns "allow" for missing mode, missing or stale heartbeat; callers `deploy.rs:94`, `hardware.rs:255`. Pinning test still present `restart.rs:363` (test-quality-plan:56 said REPLACE) |
| G07 | legacy routes bypass token; Vite tokens | **fixed** | PR245 / `df46849`; ADR 0033; `http.rs:202-259`; conformance tests `gateway_access_conformance_test.rs:58-195` |
| G08 | disk no-op reports Durable | **open** | `limit_patch.rs:107-118` compares disk `before` vs `after`, returns Durable without consulting Pi live state |
| G09 | URDF activation races Pi writer | **open** | `hardware.rs:328-356` writes `marengo.urdf.tmp` + rename with no generation/CAS and no `persist_pending` check |
| G10 | cached snapshots look current after Pi loss | **partial** | IPC disconnect now clears snapshots and emits invalidation (`state.rs:90-111`, `78590c2`, `b5b16fa`); still no per-topic age/boot id; `/health` always `ok:true` (`http.rs:277-281`). Ledger: open |
| G11 | Windows compile | open | `crates/chappe/src/lib.rs:18` exports `ipc` unconditionally; `ipc.rs` has 11 `std::os::unix` uses, no cfg |
| G12 | 13-day TLS never reloaded | **open** | `webtransport.rs:275` 13 days; loaded once (`main.rs:221-224,254`); regeneration only at startup (`webtransport.rs:204-215`) |
| G13 | sibling artifact NULLing | fixed (store) | ledger verified; `crates/marengo-store/tests/session_artifact_preservation.rs` |
| G14 | literal-Z import time | fixed (store) | ledger verified; `crates/marengo-store/tests/session_capture_chronology.rs` |
| G15 | interrupted migration | partial (store) | ledger `partial`; `crates/marengo-store/tests/migration_*.rs` |
| G16 | blocking unbounded log reads on Tokio workers | **open** | every handler calls sync `Store` on the worker (`logs.rs:324-541`), `std::fs::read` whole blob (`logs.rs:561`); hardware handlers sync fs (`hardware.rs:174-633`) |
| G17 | candump timestamp panic | fixed (candump) | ledger verified; `bins/marengo-log-cli/tests/candump_input_errors.rs:5` |
| G18 | CPU/iowait columns | fixed (host-metrics) | PR/merged `ad09ee4`; `crates/marengo-host-metrics/src/cpu.rs:72,106` |
| G19 | CAN/mount status fields | fixed (host-metrics) | PR238 / `f831e587` (ledger) |
| G20 | failed listener leaves gateway running | **open** | `main.rs:228-239` (HTTP bind error only logged in spawned task), `main.rs:255-264` (HTTPS serve error logged); process stays up on WT |
| G21 | activation 500 after promotion | **open** | `hardware.rs:358-361`: staging removed, then `completeness_report(...)?` can return 500 after master promoted |
| F05 (=G10 UI) | stream treated fresh | partial | as G10 |
| test-quality `restart_allows_active_with_stale_heartbeat_via_stub` | REPLACE | **open** | `restart.rs:363-410` unchanged |

## 8. Drift

- `codemap.md` "Adapter: IpcListener → in-memory Bus → HTTP/WT fan-out" and `state.rs:195-196` ("frames already reach the bus from … the IPC listener"): IPC server delivers frames by callback straight to `ingest_runtime_frame` (`main.rs:159-168`; `crates/chappe/src/ipc.rs:443-456`), never via the Bus; the Bus carries only gateway tracing + command echoes.
- `codemap.md` "enable/disable POST": there is no disable route (`http.rs:155-168`). "Depends on chappe, marengo-store, armee-proto": also marengo-config, marengo-candump, marengo-deploy (`Cargo.toml:18-41`). "IpcListener::bind": actual `spawn_server_with_lifecycle` (`main.rs:180`).
- `src/codemap.md` lists 9 modules; 6 are missing (`action_ack`, `actuator`, `deploy`, `hardware`, `limit_patch`, `ratelimit`).
- ADR 0008 table lists `/command/enable` only and "session accepted for any path under gateway in dev"; WT now refuses any path ≠ `/chappe` (`webtransport.rs:83-88`). ADR 0008 "Future: bearer token" superseded by ADR 0033.
- ADR 0011 §4 "log ring": `LogServices.ring` is write-only (`logs.rs:206,223,250`); §5 `MARENGO_LOG_ARCHIVE_DAYS` read nowhere (grep); "optional token" superseded by ADR 0033.
- `docs/safety.md:95` / ADR 0012 §3 "CAS `expected_revision` on the master config dir": Consul never sends a revision (`consul/src/lib/config-api.ts:34-46`); gateway reads disk revision at request time (`config.rs:212-213`) and compares it with disk again (`limit_patch.rs:86-106`), so CAS never detects a stale client.
- ADR 0012 Consequences "persist-degraded distinct banner": `persist_ok` (`config.rs:181`) is declared but never read by Consul (`config-api.ts:31` only).
- `CONTEXT.md:40` "Restoring an archive opens a **new** staged import": restore reuses the archive's own `upload_id` (`hardware.rs:458`), and the following activate overwrites that archive entry (`hardware.rs:305-326`).
- `actuator.rs:64` / `ratelimit.rs:11,29` reference "motion PR-5" — no such plan in ADRs/roadmap (grep) [INFERENCE: stale scaffold note].
- Consumers drifted from ADR 0033: MCP `pi_logs_list`/`pi_logs_archive_list` and `pi-remote.sh logs-list` curl `/logs/sessions` without a credential (`tools/marengo-pi-mcp/src/tools/logs.ts:63,101`; `scripts/pi-remote.sh:82-84`), so they always 401 and silently fall back to hot files.

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Deleting touches |
|---|---|---|---|
| `framing::read_length_prefixed` (`framing.rs:18-34`, `#[allow(dead_code)]`) | zero references incl. tests (`metrics/pub-usage.md:21`; grep) | high | nothing |
| `LogServices.ring` + preload (`logs.rs:131,206,210-224,250`) and possibly `marengo_store::LogRingBuffer` | zero readers (written only) | high | `logs.rs`; store `ring.rs` if no other user (grep shows none) |
| `robot/testing/telemetry` synthesis (`state.rs:26,44,223-229`) | zero subscribers in consul/tools/scripts (grep) | high | `state.rs`; halves RobotState broadcast traffic |
| Unused deps `marengo-support`, `prost`, `quinn`, `thiserror`; move `serde_yaml` to dev-deps (`Cargo.toml:27,29,30,36,35`) | zero references (`metrics/unused-deps.md:24-28`, grep) | high | `Cargo.toml` |
| Duplicate fingerprint set (`main.rs:222` vs `webtransport.rs:36`) | duplicate implementation | high | one line |
| `/command/home` 410 route (`http.rs:157-158,240-241,410-415`) + test `hardware_tests.rs:685` + Consul `postHomeCommand` (`gateway-api.ts:140-145`, only re-export `chappe-client.ts:14` + test mock) | superseded (retired; "use Hardware Set Zero") | med | clients get 404 not 410 |
| `PersistStatus::NotApplicable` + `"n/a"` mapping (`limit_patch.rs:27-30`, `config.rs:255`) | never constructed (`#[allow(dead_code)]`) | med | Consul type comment only |
| `LimitPatchResultJson` serde derive and fields `applied_live`, `revision`, `before`, `after` (`limit_patch.rs:33-45`) | only `ok/message/restart_required/persist_status` read by `config.rs:247-257`; never serialized | med | `limit_patch.rs` |
| `/settings` (`logs.rs:521-541`) and `/logs/sessions/{id}/download` (`logs.rs:543-574`) | no consumer outside conformance tests (consul/tools/scripts grep) | med | `http.rs:121,123`; conformance rows `gateway_access_conformance_test.rs:93,95` |
| `/snapshot/robot/{safety,heartbeat}`, `/snapshot/sensors/imu/torso`, `/snapshot/host/metrics/{pi,jetson}` HTTP routes (`http.rs:95-102,341-359`) | no HTTP consumer (grep); listed in ADR 0008 contract | low | ADR 0008 table; the internal `snapshot_*` accessors stay (restart/deploy use them) |
| Candump JSON legacy aliases `total_frames`, `delta_s`, `frame_count`, `bytes` (`logs.rs:75-77,84-85,125-126`) | "one-release" compat since `3e023f6` (2026-07-23); `bytes` unused; others used as fallbacks in Consul (`candump-frame-table.tsx:20`, `use-can-traffic-spectrum.ts:83`) | low | Consul must switch to `parsed_frames`/`offset_s` first |
| `config_overrides` audit upsert (`config.rs:231-245`) | write-only (no reader in repo) and last-write-wins per joint, so not an audit trail | low | ADR 0012 §5 mentions it; decide audit vs delete |
| `ConfigPatchJson.{device_id,can_interface,direction}` (`config.rs:62-64,202-210`) + Consul DTO fields (`config-api.ts:36-38`) | always rejected; superseded by ADR 0012 §4 | low | contract change |
| Test `restart_allows_active_with_stale_heartbeat_via_stub` (`restart.rs:362-410`) | test pinning unsafe implementation detail (test-quality-plan:56) | med | replace, not just delete |

Not prunable despite gaps: restart/deploy/activate refusal gates, access middleware, rate limits, observation invalidation (safety/stop-adjacent).

## 10. Phase-B leads

1. **Fail-open management gate** `restart.rs:53-59`: unknown mode, absent heartbeat or heartbeat older than 5 s ⇒ allow restart/self-update/URDF activation (`deploy.rs:94`, `hardware.rs:255`). Snapshots are cleared on IPC loss (`state.rs:91-93`), so a disconnected Pi always passes. G06.
2. **Sticky / global persist flags** `state.rs:203-219`: any `ActionEvent` with Pending sets `persist_pending`; lost/lagged Durable keeps restart/deploy refused forever; flags not reset on reconnect (`state.rs:90-111`). Restart ignores `persist_degraded` despite `docs/safety.md:94` (`restart.rs:103`). Activate ignores both (`hardware.rs:248-266`).
3. **CAS is disk-vs-disk** `config.rs:212-213` + `limit_patch.rs:97-106`; client never supplies revision → stale Set Limits overwrite. `before==after` no-op Durable from disk (`limit_patch.rs:107-118`, G08).
4. **`limit_patch.rs` 0 % coverage** (`metrics/coverage-by-file.md:9`): the ACK-honesty path (timeouts 8 s/12 s `limit_patch.rs:18-19`, persist timeout returns `ok:false` + Pending `:195-203`) has no test. `session_id` = joint + ms (`:120-127`) can collide → crossed ACKs.
5. **Config surface coupled to log DB**: `/config/snapshot` and `/config/patch` return 503 when the Store failed to open (`config.rs:178,189-193`), although they only use it for a swallowed audit write (`config.rs:239`).
6. **Blocking in async**: `publish_command_envelope` → `IpcListener::send_command` holds a std Mutex and blocks up to 1 s on a Unix write (`state.rs:271-272`; `crates/chappe/src/ipc.rs:210-214,512-536`) on Tokio workers; sync Store/fs in handlers (G16).
7. **Startup race** `main.rs:180-211`: IPC server is spawned before `state_holder` is filled; an early `on_connection_change(true)` and frames are dropped (`main.rs:162-165,173-176`), so no `RuntimeConnectionState{connected:true}` is emitted for the first peer.
8. **URDF restore→activate overwrites history**: same `upload_id` (`hardware.rs:458`) → activate rewrites `archive/<id>/replaced_active.urdf` and manifest (`hardware.rs:305-326`). `new_upload_id` is ms-based (`hardware.rs:128-134`): concurrent uploads share a staging dir. Abandoned staging dirs are never removed (no route).
9. **G21/G09** post-promotion 500 (`hardware.rs:358-361`); fixed tmp name `marengo.urdf.tmp` shared with any other writer (`hardware.rs:328`).
10. **`/command/enable` and `/command/testing_mit` have no rate limit and no gateway joint allowlist** (`http.rs:374-408`); bodies up to Axum 2 MiB. Davout is the only gate [INFERENCE: intended, but asymmetric with set_zero].
11. **Tuning bucket keyed by client-chosen `session_id`** (`ratelimit.rs:105`; `actuator.rs:98`): rotation bypasses 10/s and grows the HashMap (600 s TTL, `ratelimit.rs:24,132-134`). Requires Control credential.
12. **Unbounded HTTP stream subscribers / leaked pumps**: `/stream/chappe` spawns a pump per request with no cap (WT has 64) and the pump only notices disconnect on the next matching write (`http.rs:315-320`, `framing.rs:70-81`).
13. **Per-subscriber decode+re-encode** of every envelope (`framing.rs:92-93`) and double clone per ingest (`state.rs:222,228`) — avoidable allocation on the hot path.
14. **TLS key written with default umask** (`webtransport.rs:289-290`); HTTP 8080 bound on `[::]` (`scripts/install-pi.sh:338`) so Bearer credentials can cross the LAN in plaintext (`scripts/cloud-pi-lib.sh:212-221`).
15. **Anonymous outbound fetch**: public `GET /version/status?refresh=1` triggers GitHub fetch (`deploy.rs:50-58`) [INFERENCE: depends on marengo-deploy caching].
16. **Demo publisher writes into the real Store** at 20 Hz via `ingest_runtime_frame` → `ingest_log_event` (`webtransport.rs:510-535`, `state.rs:198-201`) and can coexist with a live IPC peer.
17. **`/health` is unconditional `ok:true`** (`http.rs:277-281`) — MCP `pi_wait_deploy` treats it as readiness (`tools/marengo-pi-mcp/src/tools/deploy-wait.ts:22`) even if HTTPS/Store failed (G20).
18. **Public config inventory**: `/config/snapshot` (CAN ids, directions, `config_dir`) is anonymous (`http.rs:124`, authorize_api only gates mutations) — ADR 0033 names only health and telemetry as public.
19. **Coverage gaps on management code**: `deploy.rs` 0 %, `main.rs` 0 %, `logs.rs` 36.8 %, `config.rs` 47.1 % (`metrics/coverage-by-file.md:8-10,19,21`) — gaps, not prune signals.
