# Phase B — WP-NP: store/log I/O on async workers (G16) & archive integrity + deploy lifecycle

Branch `audit/wp-np`; baseline `223db4a0` (`git merge-base HEAD main`).
Fix commits: **D** = `ee613d6a` (marengo-deploy + Cargo.lock), **G** = `5bc62d14` (marengo-gateway),
**S** = `0e5c0d32` (marengo-store, marengo-candump), plus this report.
No Pi hardware behavior was exercised. `config/*.yaml` physical values untouched.
Network-dependent reds (live GitHub fetch) were not executed; negative-cache red is by code path + unit test.

"Red" = a new test, or a scratch reproduction using only baseline API, failed on a detached
worktree at the baseline (`/tmp/wpnp-base`, removed afterwards). Scratch probes injected into
baseline `mod tests` (private API) or run as integration tests are recorded as such; filtered
cargo invocations that found no matching test are **not** counted as red.

## Verdicts — WP-N

| Lead | Verdict | Evidence (red → green) |
|---|---|---|
| L-marengo-store-01 (G16 store) | **CONFIRMED-fixed** | All gateway log handlers (`logs.rs`) run Store reads on `spawn_blocking`; deploy reconcile/status/log-tail run on the blocking pool. No new test (concurrency posture); `cargo test -p marengo-gateway` green. Residual: page requests still rescan from line 1 within the blocking task (two-pass gzip decompress); a frame-index table remains future work. |
| L-marengo-deploy-03 (G16 deploy) | **CONFIRMED-fixed** | `load_reconciled_job` documented blocking-only; `post_control_deploy` + `current_version_status` call it via `spawn_blocking`; join failure serves `empty_reconciled` (display) / refuses enqueue fail-closed. |
| L-marengo-gateway-06 (blocking IPC send) | **CONFIRMED-fixed** | `publish_command_envelope` is `async` and runs `ipc.send_command` on the blocking pool; all six call sites (`http.rs`, `actuator.rs`, `limit_patch.rs`) await it. No new test. |
| L-marengo-log-cli-01 (maintenance skips on journal failure) | **OPEN-not-investigated** | Unit file unchanged (`ExecStart=-journal-import` dash-prefix predates baseline: journal failure already tolerated). The lead's premise (failure skips archive+purge) is **REFUTED** as written for exit failure; a hung journal-import still blocks the chain — no timeout was added. Journal-import *error reporting* improved (S: nonzero exit propagates instead of `Ok(0)` masking). |
| L-marengo-store-02 (read_text_page) | **CONFIRMED-fixed** | `text_page_streams_window_without_head_loss`: bounded window, no head-loss; totals saturate at `u32::MAX` instead of `as u32` wrap. Two-pass gzip cost documented. |
| L-marengo-candump-04 (out-of-order aborts) | **OPEN-not-investigated** | `TimestampRegression` still fails the whole inspection (pinned by `contract.rs::timestamp_regression_fails_inspection`). Multi-interface `-t z` merge and clock-step handling undecided. Only hardening done: single-open buffered magic peek (S, candump TOCTOU half of L-marengo-candump-08). |
| L-marengo-gateway-16 (demo writes real Store) | **CONFIRMED-fixed** | `--demo` points at an ephemeral per-process store dir; `demo_publisher_yields_to_a_live_peer` (connected → silent; disconnect → resumes). |
| L-marengo-log-cli-04 (busy timeout) | **CONFIRMED-fixed** | `store::tests::open_applies_bounded_busy_policy` asserts the 5 s ADR 0029 policy on the normal open path. SQLITE_BUSY under concurrent archive/writer load not bench-verified. |
| L-marengo-store-03 (purge) | **CONFIRMED-fixed** | Red (baseline API probe): old live unfinalized capture collected by age purge (`sessions == 1`, row gone). Green (S): purge keeps the newest unfinalized session, deletes files-then-rows in one transaction, reports file errors via tracing. `retention.rs` integration (`retention_removes_expired_sessions_and_artifacts_without_blocking`) extended with an abandoned-but-old session that must purge. |
| L-marengo-store-04 (COUNT/FTS) | **CONFIRMED-fixed** | `fts_query_with_quote_matches_literally` (quote no longer breaks MATCH); COUNT propagates errors and range-checks `u32` instead of `unwrap_or(0)`/`as u32`. |
| L-marengo-store-05 (journal import) | **CONFIRMED-fixed** | Streaming child-stdout (no buffered `output()`), `__CURSOR` resume with legacy-ms fallback, atomic `insert_log_events_with_cursor` batches of 500, 1 MiB line cap, parse skips fail-open per line but journalctl nonzero exit fails the import. Parser unit tests cross-platform; Linux I/O path not bench-run. Non-UTF-8 MESSAGE still lossy (`from_utf8_lossy`, cursor still advances) — documented in code, not a silent skip. |
| L-marengo-store-06 (archive) | **CONFIRMED-fixed** | `archive_leaves_no_temp_and_roundtrips` (unique pid+counter temps, file+dir fsync, temp cleanup on error); row-update-before-removal ordering kept but crash semantics documented (rows point at missing files → re-driven; never orphan blobs). Concurrent CLI+gateway archive collision removed. |
| L-marengo-store-07 (symlink loop) | **CONFIRMED-fixed (store side)** | Pre-existing `disk.rs` (not in this diff) uses `symlink_metadata` and never follows links, with a loop/out-of-tree test. The host-metrics copy named in the lead no longer exists (no `dir_size` in `marengo-host-metrics`). No change needed. |
| L-marengo-store-09 (session split) | **CONFIRMED-fixed** | `archive_keep_is_session_atomic`: keep-count applies to sessions ranked by newest artifact mtime, all kinds archive together. |
| L-marengo-store-12 (Store open failure) | **OPEN-not-investigated** | Gateway still warns and disables logs/config/audit on open failure (`main.rs:215`). No liveness/readiness signaling added. |
| L-marengo-candump-08 (double open/TOCTOU) | **CONFIRMED-fixed (half)** | Single-open buffered magic peek removes the metadata-then-open TOCTOU (S). Half-written last line still counts malformed while candump runs — no length-prefix protocol exists to distinguish tearing; remains OPEN for the tail-line half. |
| L-marengo-candump-07 (per-frame allocs) | **OPEN-not-investigated** | No allocation profiling or pooling done; still434 `interface.clone()` + split/collect/join per line on the request path. |
| L-marengo-candump-09 (gzip magic) | **CONFIRMED-no-action** | Two-byte magic is the documented, safe heuristic; single-open peek keeps it. No test beyond existing gzip fixtures. |
| L-marengo-store-08 (ring poison) | **OPEN-not-investigated** | Untouched. |
| L-marengo-store-10 (int casts) | **CONFIRMED-fixed** | `ms_to_sql` rejects out-of-range `u64`; `u64_from_sql` rejects negative stored values instead of wrapping; `out_of_range_timestamps_fail_closed`. |
| L-marengo-store-11 (public connection()) | **OPEN-not-investigated** | `Store::connection()` still public; nested `unchecked_transaction` still possible. Mitigated in practice by `insert_log_events_with_cursor` (journal no longer needs caller-held tx), but no visibility change. |

## Verdicts — WP-P

| Lead | Verdict | Evidence (red → green) |
|---|---|---|
| L-marengo-deploy-12 (coverage) | **CONFIRMED-fixed** | `enqueue.rs` + `upstream.rs` + orphan/timeout branches now tested (27 lib tests: `enqueue_reports_helper_success`, `enqueue_surfaces_helper_failure_output`, `enqueue_refuses_missing_helper`, token-config tests, negative-cache tests, CAS test, orphan/reconcile tests). Orphan branch covered via `reconcile_orphans_inactive_unit_without_success`. |
| L-marengo-deploy-01 (three writers/RMW) | **CONFIRMED-fixed** | Red (baseline): stale blind write of a previously-read copy clobbers a newer job id (demonstrated with baseline `write_job_file` semantics —since `reconcile_job`/`write_job_file` are not re-exported at baseline root, the probe used `fs::write` blind overwrites plus src-injected `reconcile_job` probes). Green (D): `job_file_lock_path` flock held across reconcile RMW and `compare_and_write_job` CAS on `job_id`; `compare_and_write_refuses_a_newer_phase`. Shell writers still use their own locks (enqueue `flock -n 9` on `/run` lock; self-update plain `mv`); cross-writer atomicity between Rust flock and shell `mv` is same-dir rename only, not a shared lock — see NEEDS-DECISION. |
| L-marengo-deploy-04 (negative cache) | **CONFIRMED-fixed** | `failed_fetch_is_negatively_cached` (failure stamps `fetched_at`, non-forced polls back off 60 s serving last-known-good); `fixed_override_still_wins_over_negative_cache`. No live-network red executed (sandbox has no GitHub access claim); red is the pre-fix `cache_is_fresh` requiring `sha.is_some()` path + code inspection. |
| L-marengo-deploy-07 (unparseable running wedges) | **CONFIRMED-fixed** | Red (src-injected baseline probe): `started_at = "not-a-timestamp"`, empty unit → `reconcile_job` returns false, stays Running. Green (D): `reconcile_unparseable_start_falls_back_to_mtime` (old mtime → Timeout), `reconcile_unparseable_start_without_mtime_fails_closed`, `reconcile_fresh_mtime_without_start_stays_running`, `reconcile_orphans_inactive_unit_without_success`. `UNIT_NAME=""` from MCP direct path now times out via the 120 s orphan horizon (mtime-backed) at the latest. |
| L-marengo-deploy-08 (MCP bypasses lock) | **OPEN-not-investigated (design present, not implemented)** | `pi_native` still runs `pi-self-update.sh` directly with no flock, no unit check, no gateway DEPLOY_LOCK. A shared file-lock bridge was designed (Rust `job_file_lock_path` sidecar exists) but MCP cannot `sudo` the enqueue helper from its SSH principal, and no `flock $JOB_FILE.lock` wrapper was added to `deploy.ts`/`pi-self-update.sh`. |
| L-marengo-deploy-09 (success = prefix + www) | **CONFIRMED-fixed (half)** | `promotion_match` + `ready_for_target` + enqueue current-check now require a full 40-hex installed SHA; `reconcile_never_promotes_on_short_installed_prefix`; `non_hex_first_token_is_not_a_revision`. No service/bundle verification added — still SHA + www only. |
| L-marengo-deploy-02 (corrupt does not block) | **CONFIRMED-fixed** | `corrupt_job_file_fails_closed_not_idle`; gateway `deploy_refuses_enqueue_on_corrupt_ledger` (409→CONFLICT). `read_job_file` coercion retained for display paths; ledger decisions use `JobFileRead::Corrupt`. |
| L-marengo-deploy-05 (token in argv) | **CONFIRMED-fixed** | `token_config_file_never_touches_argv` (0600 `-K` file, removed on drop); `token_with_line_break_is_refused`; `absent_token_needs_no_config_file`. `ps` red not executed (no live fetch); code-path evidence only. |
| L-marengo-deploy-06 (timeout w/o kill) | **CONFIRMED-fixed** | `kill_on_drop(true)` on enqueue helper and curl; 30 s/20 s timeouts map to `EnqueueTimeout`/curl-timeout errors. Slow-helper red not executed. |
| L-marengo-gateway-15 (anon refresh fetch) | **OPEN-not-investigated** | `GET /version/status?refresh=1` still unauthenticated-ly triggers `fetch_upstream_sha(true)` (forced fetch bypasses negative cache). Auth topology unchanged; refresh also still writes the cache file. Mitigated only by the failure backoff for non-forced polls. |
| L-marengo-deploy-10 (rev parse) | **CONFIRMED-fixed** | `non_hex_first_token_is_not_a_revision` (`"not-a-sha …"`, literal `\n`, `"xyz"`, `""` → empty sha). `MARENGO_DEPLOY_JOB_FILE` env honored by `resolve_job_file_path` (used in gateway test). |
| L-marengo-deploy-11 (fsync) | **CONFIRMED-fixed** | Job writes: file `sync_all` + dir `sync_all`, `create_new` unique temps, cleanup on error. Archive blobs: file + dir fsync. Power-loss red not executed. |

## Small finishes done in this pass

- Candump single-open reads (buffered magic peek; `scan.rs`).
- `install-pi.sh` adds `marengo` to `systemd-journal` so journal import can read (retained `|| true`; group effective on next login).
- Journal-import doc comment corrected (oversize-line skip does not advance the cursor — cursors unknown for skipped bytes).

## NEEDS-DECISION

- **L-marengo-deploy-01 residual — shared lock across runtimes.** Rust uses `deploy-job.json.lock` flock; `pi-enqueue-self-update.sh` uses `/run/marengo-self-update-enqueue/lock`; `pi-self-update.sh` uses plain `mv`. Options: (A) wrap all shell writes in `flock "$JOB_FILE.lock"` (recommended; MCP `pi_native` gets mutual exclusion without sudo); (B) route MCP through the root enqueue helper (needs a sudoers addition for `joey`); (C) accept rename-only semantics and keep CAS as Rust-side only.
- **L-marengo-deploy-08 — MCP `pi_native` vs Consul Update concurrency.** Same as above. Also `deploy.ts` never checks the gateway DEPLOY_LOCK or the systemd unit; recommend (A) plus a pre-flight `systemctl is-active marengo-self-update` check over SSH.
- **L-marengo-candump-04 — timestamp policy.** Whole-inspection failure on regression vs per-interface sort vs skip-and-count. Archive import inherits the decision. Recommend per-interface monotonic checks with a skipped-regression counter, after confirming bench `candump -t z can0 can1` interleave behavior.
- **L-marengo-store-11 — `connection()` visibility.** Recommend `pub(crate)` + narrow public query methods; audit current external callers first (gateway uses Store methods, not the raw connection, per this diff).
- **L-marengo-store-12 — degraded-logs signaling.** Warn-only today. Recommend a `/health` degraded flag or refuse-to-serve-logs 503 with a distinct code so Consul can surface it.
- **L-marengo-gateway-15 — refresh auth/rate.** Recommend requiring Management (or at least rate-limiting) for `refresh=1`, since it forces a GitHub fetch and cache write.
- **L-marengo-log-cli-01 follow-up — hung journal.** Recommend `TimeoutStartSec=` on the service or a `timeout(1)` wrapper so a hung journalctl cannot starve archive+purge.
- **L-marengo-store-08 — ring poison.** Recommend failing `push` loudly or auto-reseeding instead of silent permanent loss.

## Cross-package edits and gate

- `marengo-deploy`: flock RMW, CAS demote, strict corrupt refusal, mtime-backed reconcile timeout, promotion-grade SHA, negative cache, token `-K` file, `kill_on_drop`, durable job writes. New dep `fs2` (Cargo.lock).
- `marengo-gateway`: `spawn_blocking` for all Store/deploy/systemctl/log reads, async IPC send, ephemeral demo store + live-peer yield, corrupt-ledger 409→CONFLICT test.
- `marengo-store`: busy timeout, transactional purge/archive, session-atomic keep, streaming cursor journal import, sanitized FTS, closed casts, bounded paging. New dep `tracing`.
- `marengo-candump`: single-open magic peek (no TOCTOU).
- `scripts/install-pi.sh`: `systemd-journal` group for `marengo`.
- Untouched as designed: `tools/marengo-pi-mcp` (no MCP `npm test` needed), `scripts/systemd` unit (dash-prefix already tolerates journal failure), `marengo-host-metrics` (no `dir_size` remains), `config/*.yaml` values.

Gate (final, on committed branch): `cargo fmt --all -- --check` clean;
`cargo clippy --workspace --all-targets --exclude marengo-host-metrics --exclude marengo-pi -- -D warnings` clean;
`cargo clippy -p marengo-pi --all-targets --target aarch64-unknown-linux-gnu` clean
(after fixing the journal `Read` import, `MAX_LINE_BYTES` cfg, and exhaustive cursor match);
`cargo test --workspace` **1204 passed, 0 failed** (includes a fixed deploy-test serialization flake and
one transient berthier timing failure that passes in isolation and in the full rerun).
Focused suites green: `cargo test -p marengo-store -p marengo-deploy -p marengo-gateway -p marengo-log-cli -p marengo-candump`
(all `ok`, zero failures); `scripts/deploy-rev.test.sh` 6/6; `scripts/deploy-job-contract.test.sh` 34/34
(with `cargo` on PATH).
