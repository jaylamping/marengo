# Intent card: `marengo-store`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-store` |
| Path | `crates/marengo-store` |
| Kind | lib |
| Baseline | `a2b55b3` |
| LOC | src 2,755 (`recovery.rs` 1,185, `store.rs` 963, `migrations.rs` 212, `journal.rs` 153, `ring.rs` 92, `model.rs` 62, `lib.rs` 34, `paths.rs` 31, `error.rs` 23); tests/ 6,810 in 16 files, 29 `#[test]` fns (`migration_recovery.rs` 1,354 alone) |
| Sources consulted | `src/lib.rs` //! docs; `codemap.md`, `src/codemap.md`; crates/AGENTS.md:23; crates/codemap.md:21; ADR 0011, 0012 §5, 0028, 0029, 0030; gateway.md G03/G13/G14/G15/G16 + design concerns 5, 7, 8; finding-index.md:52-65; implementation-ledger.json; `git log -- crates/marengo-store`; consumers `bins/marengo-gateway/src/{main,logs,config}.rs`, `bins/marengo-log-cli/src/main.rs`, `scripts/systemd/marengo-log-maintenance.service` |

## 2. Intent

`marengo-store` is the Pi's **durable historical record** for operator diagnostics (crate doc `lib.rs:1-10`; ADR 0011). It covers three things:
- Structured log events, with FTS search.
- Bench capture sessions: bench log, candump and position-trace artifacts as hot files or gzip blobs, with SQL metadata.
- Small operator settings and an audit table.

ADR 0011 created it so logs survive Consul refresh and can be searched across sessions. The gateway is the primary writer and `marengo-log-cli` the maintenance and archive entrypoint (ADR 0011 §1-6, Consequences). Since 2026-10-01 its dominant concern is **refusing to corrupt or reinterpret history**:
- Capture chronology must come from the canonical UTC session ID or existing rows, with unknown ends left NULL (ADR 0028).
- Each schema step commits atomically with its marker (ADR 0029).
- One recognized historic schema state can be recovered only through an explicit, backed-up operation (ADR 0030).

The crate explicitly does not authorize motion or repair arbitrary corruption (`lib.rs:6-8`). ADR 0012 §5 confirms `settings` / `config_overrides` are audit/prefs, **not** the limit source of truth.

Conflicting statements of intent:
- crates/codemap.md:21 and `codemap.md:4` say "time-series **key-value** store for **telemetry replay**". `src/codemap.md:4` says "telemetry samples". No telemetry sample table exists (`migrations.rs:7-102`). The crate stores logs, sessions, settings and overrides.
- ADR 0011 §5 says retention is "30-day purge (`MARENGO_LOG_ARCHIVE_DAYS`)". No code reads that env var (zero grep hits in bins/crates/scripts). The systemd unit hardcodes `purge --days 30` (`scripts/systemd/marengo-log-maintenance.service:11`). The DB setting `log_archive_days` is seeded (`migrations.rs:135-151`) but only displayed (`bins/marengo-gateway/src/logs.rs:530-538`).

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| SQLite schema v1→v3 and per-step atomic migration with version marker; refusal of OFF journal, future, invalid and unversioned-nonempty DBs | `migrations.rs:5-212`; ADR 0029 |
| Log events + FTS5 external-content index + triggers; structured query (filters, FTS prefix, paging ≤5000) | `migrations.rs:14-43,66-98`; `store.rs:116-267` |
| Session registry with sparse-update semantics; candump stats invalidation on reference change; explicit finalize; reference clearing | `store.rs:317-379`; ADR 0028 |
| Hot→blob archive (gzip, `blobs/<UTC date>/`), legacy hot import, capture ID → UTC date parsing | `store.rs:440-538,664-721,792-840` |
| Retention `purge_before` / `purge_older_than_days` (rows and blob files) | `store.rs:269-315` |
| Text paging for bench/trace blobs; candump inspection delegated to `marengo-candump` | `store.rs:540-653,842-863`; commit `5deae83` |
| Journal import (`journalctl` → `log_events`, `systemd:` target), Linux only | `journal.rs:1-129` |
| In-memory `LogRingBuffer` (10k) | `ring.rs:1-68` |
| Known-v2 recovery with verified backup + separate output | `recovery.rs:35-237`; ADR 0030 |
| Default paths `$MARENGO_ROOT/var/marengo.db`, `var/log`, `var/log/blobs` | `paths.rs:1-31` |

| Must not | Evidence | Violation? |
|---|---|---|
| Authorize robot motion / be limit SoT | `lib.rs:6-7`; ADR 0012 §5 | None. No control-stack deps (`Cargo.toml:13-24`). |
| Repair arbitrary damaged DBs | `lib.rs:7`; ADR 0030 | None. Recovery is profile-locked (`recovery.rs:35`). |
| Run recovery during normal open | ADR 0030 §Decision | None. Normal open refuses (`migrations.rs:120-130`). Recovery is only reachable via CLI `recover-known-v2` (`bins/marengo-log-cli/src/main.rs:258`). |
| Own request scheduling / HTTP | `lib.rs:8` | None. The sync API is called directly from async Axum handlers (§10 lead 1). The defect is the gateway's, but the crate offers no async or bounded API. |
| Ring buffer in a store crate | (no doc assigns it) | Layer smell, not a violation. `LogRingBuffer` is a gateway snapshot concern (`ring.rs:8`) with no SQL relation. |

## 4. Interface

| Concept | Public surface | Consumers |
|---|---|---|
| Open / migrate | `Store::{open, open_with_candump, open_default, migrate, connection, marengo_root}` | gateway `main.rs:147` (`open_with_candump`); log-cli `main.rs:335-336` (via `resolve_*`); `connection()` only in tests (`bins/marengo-log-cli/tests/recovery_cli.rs:185,354` + crate tests). `open_default` and external `migrate` have **zero** refs. |
| Settings / audit | `set_setting`, `get_setting`, `set_config_override` | gateway `logs.rs:536` (get, keys `schema_version`, `log_archive_days`, `log_disk_budget_bytes`); gateway `config.rs:239` (`set_config_override`, result discarded with `let _`); `set_setting` is called only by `journal.rs:126` and tests |
| Log events | `insert_log_events`, `recent_log_events`, `query_structured_logs`, `StructuredLogQuery`, `LogEventInsert`, `LogEventRow` | gateway `logs.rs:210,300,326,495` |
| Sessions | `register_session`, `clear_session_artifact`, `finalize_session`, `list_sessions`, `get_session`, `latest_session`, `SessionArtifact`, `LogSessionRow` | log-cli `main.rs:349-364`; gateway `logs.rs:364,551`. `latest_session`: **zero** refs. |
| Artifact reads | `read_bench_page`, `read_trace_page`, `read_candump_page`, `read_hot_candump_page`, `candump_summary`, `hot_candump_summary` | gateway `logs.rs:408-469` |
| Maintenance | `archive_hot_sessions`, `import_legacy_hot`, `import_legacy_hot_report`, `purge_older_than_days`, `purge_before`, `log_disk_usage_bytes`, `LegacyImportSummary` | log-cli `main.rs:372-387`; `import_legacy_hot` (compat wrapper) is used only by crate tests; `purge_before` only by tests (deliberate seam per ADR 0028) |
| Journal | `import_journal`, `JOURNAL_UNITS` | log-cli `main.rs:391`; systemd `marengo-log-maintenance.service:9` |
| Recovery | `recover_known_v2`, `RecoveryReceipt` | log-cli `main.rs:258` |
| Ring | `LogRingBuffer`, `DEFAULT_RING_CAPACITY` | gateway `logs.rs:14,131,206,210` |
| Paths/consts | `resolve_db_path`, `resolve_marengo_root`, `log_dir`, `blob_dir`, `default_db_path`, `DEFAULT_ARCHIVE_DAYS`, `DEFAULT_HOT_KEEP`, `DEFAULT_LOG_DISK_BUDGET_BYTES`, `now_ms` | gateway `main.rs:127-128`, `config.rs:243`; log-cli `main.rs:17-18,42-52`. `blob_dir`, `default_db_path`, `DEFAULT_LOG_DISK_BUDGET_BYTES`: zero external refs. |
| Re-exports | `CandumpFrame`, `CandumpSummary` (`lib.rs:23`) | **zero**. Gateway imports `marengo_candump` directly (`bins/marengo-gateway/src/logs.rs:136,160`). |

Cargo features: none. rusqlite features `bundled`, `backup`, `hooks` (`Cargo.toml:16`).

Depth (codebase-design):
- `Store` is **wide and shallow-to-medium**: about 30 pub methods, many thin SQL wrappers. The deep parts are migration (`migrations::migrate`, private) and `recover_known_v2`, which is one call hiding 1,185 lines.
- `connection()` leaks the raw `rusqlite::Connection` past the mutex owner. ADR 0029 acknowledges that callers can disable journaling through it.
- Seams: `marengo_candump::Candump` is injected (`open_with_candump`, plain vs Robstride-enriched): **2 adapters, real seam**. No trait seams. Filesystem and clock are ambient (`now_ms()`, `resolve_db_path()`).
- Two parallel "maintenance" paths exist: `import_legacy_hot_report` calls `archive_hot_sessions` internally (`store.rs:706`).

**Metrics baseline** (`metrics/`, `a2b55b3`, coverage measured on aarch64-apple-darwin): crate lines/regions/functions **81.7 / 79.4 / 76.4 %** (`coverage-by-crate.md`). By file: `migrations.rs` 89.7 %, `recovery.rs` 84.0 %, `store.rs` 80.4 %, `ring.rs` 72.1 %, `journal.rs` 0.0 % (0/4; only the non-Linux stub compiles on macOS, so the Linux importer is **not measured at all**) (`coverage-by-file.md`). `test-counts.md` lists 4 tests for `marengo_store` (lib unit tests only; the 29 integration tests are counted per test binary). Zero-use `pub` heuristic: `latest_session`, `open_default`, `LogRingBuffer::push_batch`; test-only: `import_legacy_hot` (`pub-usage.md`). `cargo machete` flags `tracing` as unused (`unused-deps.md`; confirmed: no `tracing::` use in `src/`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) |
|---|---|---|
| Each schema step and its marker commit in one Immediate transaction; failed marker rolls back that step | `migrations.rs:113-169` | `tests/migration_atomicity.rs:591,599`; `tests/migration_step_conformance.rs:440` |
| Version is re-read under the writer reservation (competing opener) | `migrations.rs:114-117` | `tests/migration_competing_writer.rs:317`; `tests/migration_marker_progress.rs:349` |
| Refuse journal_mode OFF before any logical write | `migrations.rs:105-110` | `tests/migration_admission.rs:133` |
| Refuse unversioned nonempty DB (incl. `sqliteX…` names) | `migrations.rs:201-210` | `tests/migration_admission.rs:96` |
| Refuse future/invalid marker without change | `migrations.rs:188-198` | `tests/migration_step_conformance.rs:540` |
| Refuse historic partial v2 (marker1 + `fields_json`) in normal open | `migrations.rs:120-130` | `tests/migration_recovery.rs` (historical refusal control, `historical_refusal` helper at :221) |
| Current v3 open preserves settings timestamps; defaults inserted only if absent | `migrations.rs:134-151` | `tests/migration_step_conformance.rs:504`; `tests/migration_recovery.rs:1225` |
| Recovery: source unchanged, verified standalone backup before output, no overwrite, owned staging cleanup | `recovery.rs:45-237,786-868,1027-1185` | `tests/migration_recovery.rs:395,661,918,1032` |
| Retention does not re-enter the Store mutex (G03) | `store.rs:283-295` | `tests/retention.rs:13` (child-process deadline) |
| Retention cutoff fits i64 before deletion | `store.rs:277-278` | `tests/session_capture_retention.rs` |
| Sparse registration preserves siblings/label/start; changed candump ref nulls stats | `store.rs:330-352` | `tests/session_artifact_preservation.rs:240`; `tests/session_candump_replacement.rs:156` |
| New capture without parseable UTC ID is refused before any write/gzip/removal | `store.rs:448-455,684-687,714-721` | `tests/session_capture_refusal.rs` |
| Capture start = canonical `YYYYMMDDTHHMMSSZ` (opt. `profile-`), ≥ epoch; unknown end = NULL | `store.rs:331-332,820-840` | `tests/session_capture_chronology.rs:117`; `tests/session_capture_unknown_end.rs`; `tests/session_capture_compatibility.rs` |
| Clearing a reference keeps file, siblings, metadata | `store.rs:358-371` | `tests/session_artifact_operations.rs:231,320` |
| FTS query applies filters to count and rows | `store.rs:189-239` | `store.rs:905` |
| Ring drops oldest at capacity | `ring.rs:28-31` | `ring.rs:75` |
| Journal cursor monotonic; no duplicate rows after crash | `journal.rs:100-103,124-127` | **untested** (only `journal_priority_maps`, Linux only, `journal.rs:148`) |
| Archive publish atomicity (gzip tmp→rename, then row, then hot delete) | `store.rs:466-514` | partially (`session_artifact_operations.rs:381`); crash between steps **untested** (ADR 0028 Consequences defers it) |
| `query_structured_logs` paging/offset/limit clamp, `list_sessions` clamp 1..500 | `store.rs:157,404` | **untested** |

## 6. Inputs / outputs

- **Env:** `MARENGO_ROOT` (default `/opt/marengo`), `MARENGO_DB_PATH` (`paths.rs:19-29`).
- **Files:**
  - `$ROOT/var/marengo.db` (+`-wal`/`-shm`; WAL, `synchronous=NORMAL`, `store.rs:46-48`).
  - Hot captures `$ROOT/var/log/{bench-<id>.log, candump-<id>.log, position-trace-<id>.csv}`, with `latest` excluded (`store.rs:792-808`).
  - Hot candump `var/log/candump-latest.log` (`store.rs:579,619`).
  - Blobs `var/log/blobs/<YYYY-MM-DD|unknown>/<name>.gz` (`store.rs:495-514`).
  - Recovery source/backup/output paths with sidecars (`recovery.rs:19`).
- **Tables:** `settings`, `log_events`, `log_events_fts`, `log_sessions`, `config_overrides` (`migrations.rs:7-63,66-98`); `candump_frame_index` dropped in v3 (`migrations.rs:100-102`).
- **Settings keys:** `schema_version`, `log_archive_days`, `log_disk_budget_bytes` (`migrations.rs:135-151,158-162`), `journal_import_ts_ms` (`journal.rs:17`).
- **Process:** `journalctl --output json --since @<s> -u marengo-pi -u marengo-can -u marengo-gateway` (`journal.rs:60-72`).
- No Chappe, CAN or HTTP. The gateway maps HTTP routes (ADR 0011 §HTTP).

## 7. Prior review reconciliation

| Prior ID | Status | Evidence |
|---|---|---|
| G03 retention deadlock | **Fixed** (PR213) | `store.rs:283-295` uses the held guard; `tests/retention.rs:13` |
| G13 sibling artifacts erased on import | **Fixed** (PR226) | `store.rs:330-352` COALESCE; `tests/session_artifact_preservation.rs:240`, `tests/session_import_preservation.rs` |
| G14 literal-Z dates → import time | **Fixed in software** (ledger: verified, PR227 impl `cabfe94`). finding-index.md:63 still "Open" (drift). | `store.rs:820-840`; ADR 0028; `tests/session_capture_chronology.rs:117`. Historic rows written before the fix keep their fallback dates (ADR 0028: retrospective repair deferred). |
| G15 interrupted migration unopenable | **Partial** (ledger). Normal path and known-v2 recovery fixed (PR230/232). Other historic prefixes, interruption and crash/power-loss remain. | `migrations.rs:104-170`; `recovery.rs`; ADR 0029/0030 "G15 remains partial" |
| G16 unbounded blocking page reads on Tokio | **Open** | `read_text_page` reads the whole (decompressed) file into a `Vec` per page (`store.rs:842-862`). Gateway calls it directly from async handlers (`bins/marengo-gateway/src/logs.rs:401-469`), not via `spawn_blocking`. |
| Design concern 7: ambient DB path in disk usage; absolute artifact paths | **Open** | `log_disk_usage_bytes` uses `resolve_db_path()`, not the opened path (`store.rs:657`). Rows store absolute paths (`store.rs:348-350,535`). |
| Design concern 8: budget not enforced; `MARENGO_LOG_ARCHIVE_DAYS` unused; WAL/SHM omitted | **Open** | Budget is only seeded and displayed (`migrations.rs:141`; gateway `logs.rs:534`). No enforcement code. Env var has zero refs. Disk usage counts the db file only, not `-wal`/`-shm` (`store.rs:657-659`). |
| Design concern 5: log write failures not counted | **Open** (gateway-owned) | `bins/marengo-gateway/src/logs.rs:300-305` only warns |

## 8. Drift

| Doc | Code |
|---|---|
| crates/codemap.md:21, `codemap.md:4` "time-series key-value store for telemetry replay"; `src/codemap.md:4` "telemetry samples" | Logs, sessions, settings, overrides. No telemetry table (`migrations.rs`). |
| `codemap.md:8` "Store struct: session-scoped writes" | Writes are global (logs, settings), not session-scoped. |
| ADR 0011 §5 `MARENGO_LOG_ARCHIVE_DAYS` | Unused. Systemd hardcodes 30 (`marengo-log-maintenance.service:11`). |
| ADR 0029 "Retain the existing bounded five-second busy policy" | `Store::open` sets no `busy_timeout` (`store.rs:46-48`). It relies on rusqlite's default open busy timeout [INFERENCE: rusqlite sets 5 s on open; not verified in this audit]. |
| crates/AGENTS.md:23 "Bench session / log archive SQL store" | Accurate. Table omits `marengo-candump` (a dependency) and the header says "16 Rust crates" (`crates/AGENTS.md:3`); `ls crates` shows 18. |
| finding-index.md:63 G14 "Open" | Ledger: verified (PR227). |
| `paths.rs` `DEFAULT_LOG_DISK_BUDGET_BYTES` (5 GiB) | Duplicated as a literal in `marengo-host-metrics/src/lib.rs:91,201`. |

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting it touches |
|---|---|---|---|
| `Store::open_default` | Zero references (incl. tests/bins/tools) | high | `store.rs:58-62` |
| `Store::latest_session` | Zero references | high | `store.rs:427-438` |
| `LogRingBuffer::push_batch` | Zero references (`metrics/pub-usage.md`; gateway pushes single events) | high | `ring.rs:34-38` |
| `tracing` dependency | Unused dependency (`metrics/unused-deps.md`) | high | `Cargo.toml:24` |
| `CandumpFrame` / `CandumpSummary` re-exports | Zero references | high | `lib.rs:23` |
| `blob_dir`, `default_db_path`, `DEFAULT_LOG_DISK_BUDGET_BYTES` as `pub` | Zero external references; used internally/tests | med (make `pub(crate)`; `blob_dir` used by 14 test sites) | `lib.rs:26-29` |
| `Store::migrate` as `pub` | Zero external callers; `open` always migrates. ADR 0029 "keep public open/migrate interfaces" | low (ADR keeps it) | `tests/migration_*` use it |
| `Store::import_legacy_hot` (compat wrapper) | Superseded by `import_legacy_hot_report` (commit `e885fee`); only crate tests call it (9 sites) | med | tests in `session_import_preservation.rs`, `session_capture_*` |
| `config_overrides` table + `set_config_override` | Write-only: no reader anywhere. The single writer discards errors (`bins/marengo-gateway/src/config.rs:239`). ADR 0012 §5 calls it audit. | low (needs a decision: delete or give it a reader; schema change needs a v4 step) | migrations, gateway `config.rs:232-245` |
| `LogRingBuffer` in this crate | Belongs to the gateway (only consumer); no SQL relation | low (move, not delete) | `lib.rs:33`; gateway `logs.rs` |
| Settings `log_archive_days` / `log_disk_budget_bytes` seeding | Feature never enabled: seeded and displayed, never enforced | low (gap vs prune: either enforce or delete) | `migrations.rs:134-151`; gateway `logs.rs:530-534` |
| Duplicated test harness helpers (`observe` ×13, `bounded` ×4, `literal_controls`/`ordinary_settings`/`optional_bytes`/`download`/`capture` ×3, …) across `tests/*.rs` | Duplicate implementation | med (consolidate into a `tests/common/` module; most of the 6.8k-line weight) | all 16 test files |
| Very large qualification tests that assert literal SQLite internals (e.g. `migration_recovery.rs` 1,354 lines for 5 tests; `/proc` stage scanning `:1020`) | Test pinning implementation detail (staging names, `/proc` probing) | low; they also encode ADR 0030 acceptance, so review before trimming | `migration_recovery.rs` |
| `recover_known_v2` + `recovery.rs` (1,185 lines) | One-shot historic recovery for a single legacy profile. Prunable only once no field DB can be marker1+fields_json. | low (keep until the Pi DB is confirmed ≥v2 consistent) | log-cli `recover-known-v2`, `tests/migration_recovery.rs`, `bins/marengo-log-cli/tests/recovery_cli.rs` |

## 10. Phase-B leads

1. **Blocking SQL/file I/O on Tokio workers.** Every Store call takes a `std::sync::Mutex<Connection>` (`store.rs:64-66`). Gateway read handlers call Store directly in `async fn` (`bins/marengo-gateway/src/logs.rs:326,364,406-469,495,536,551`), while the batch writer holds the same mutex inside `spawn_blocking` (`logs.rs:300`). A long insert or page read stalls HTTP workers (G16).
2. **Whole-file page reads.** `read_text_page` decompresses and collects every line to return one page (`store.rs:850-862`); `total` is `as u32` (`:859`). Large gz blobs mean O(file) memory and time per request.
3. **Non-transactional purge.** Log delete, per-session file removal and row delete run in autocommit (`store.rs:279-311`). File removal errors are swallowed (`let _ = fs::remove_file`, `:305`), giving deleted rows with orphaned blobs or the reverse. Selection is by `started_ms`, so a session started 31 days ago but still capturing is purged.
4. **Swallowed count errors.** `query_structured_logs` maps a COUNT failure to `0` (`store.rs:222-227,246-251`) while rows may still return. Bad FTS syntax (`q` with quotes/operators → `MATCH 'q*'`, `:216`) probably errors at prepare/count [INFERENCE]; the gateway returns 500 with no message.
5. **Unbounded `journalctl` read.** Full stdout (24 h lookback on first run) is buffered in memory (`journal.rs:72-84`). Entries with byte-array `MESSAGE` (journald JSON for non-UTF-8) fail to deserialize and are silently skipped (`:89-92`). The cursor uses `ts_ms <= since_ms`, so entries in the same millisecond after the cursor are dropped (`:100`). Insert and cursor update are not atomic, so a crash between them re-imports duplicates (`:125-126`).
6. **Archive non-atomicity.** gzip→`update_session_blob`→candump inspect→`remove_file(hot)` (`store.rs:472-489`). A failure after the row update leaves both hot and blob, and the next run re-archives with `.gz` overwrite via rename. `gzip_to_blob` names `<name>.gz.tmp` deterministically, so concurrent CLI and gateway runs collide (`:503-504`). No fsync before rename (`:505-512`).
7. **Recursive `dir_size` follows symlinks** (`is_dir()` follows, `store.rs:865-878`). A symlink loop under `var/log` (e.g. a `latest` dir link) means infinite recursion and stack overflow [INFERENCE]. The host-metrics copy has the same bug (`marengo-host-metrics/src/lib.rs:210-224`).
8. **Ring poison = silent permanent loss.** `push` returns on a poisoned lock (`ring.rs:24-27`), and `recent` returns empty (`:41-44`). Consul backfill goes empty with no signal.
9. **`archive_hot_sessions` keep-count is per artifact kind and by mtime** (`store.rs:443-450`). A touched old file survives while a newer one is archived. Bench/candump/trace of one session can split between hot and blob.
10. **Integer casts.** `ts_ms as i64` / `row.get::<i64> as u64` (`store.rs:129,727,740-746`) wrap silently for values above `i64::MAX` or negative stored values.
11. **`unchecked_transaction` while other code holds the guard.** Safe today (single guard), but `connection()` being public lets callers start a nested transaction (`store.rs:64,121`).
12. **Gateway open failure means persistence disabled and only WARN** (`bins/marengo-gateway/src/main.rs:147-156`). A migration refusal (correct per ADR 0029) silently disables logs, config reads and audit (gateway.md design concern 4).
13. **Coverage gaps on the paths behind these leads** (`metrics/coverage-by-file.md`): `journal.rs` 0 % measured (lead 5's importer is Linux-only and was not compiled in this macOS baseline; its only test is Linux-gated, `journal.rs:142-153`), `ring.rs` 72 % (poison branches of lead 8 likely uncovered [INFERENCE]), `store.rs` 80 % (purge/page/count-error branches of leads 2–4). These are gaps, not prune signals.
