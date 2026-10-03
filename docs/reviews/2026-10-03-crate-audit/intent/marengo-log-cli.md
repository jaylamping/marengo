# Intent card — `marengo-log-cli`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-log-cli` |
| Path | `bins/marengo-log-cli` |
| Kind | bin (`src/main.rs` + `src/gravity_fit.rs`); feature `robstride-enrichment` (default) → `marengo-candump/robstride-enrichment` (`Cargo.toml:17-19`) |
| Baseline | `a2b55b3` |
| LOC | src 1596 (main 399, gravity_fit ≈1196 before its test module at `gravity_fit.rs:1197`) / tests 1211 (`tests/*.rs` 1048 + inline 157); metrics `loc.md:26` 1752/1048. Coverage 89.0 % lines (`metrics/coverage-by-crate.md:15`): `main.rs` 71.8 %, `gravity_fit.rs` 91.7 % (`coverage-by-file.md:28,81`) |
| Sources | `codemap.md`, `src/codemap.md`, `main.rs:1-4`, `gravity_fit.rs:1-20`, `bins/AGENTS.md:12,27`, `bins/codemap.md:13`, ADR 0011/0029/0030, `docs/logging-taxonomy.md:45`, `docs/commissioning/limb-playbook.md:176-180`, `crates/marengo-candump/src/lib.rs:32`, `crates/armee-dynamics/{README.md:10,codemap.md:13}`, prior `2026-09-29/{gateway.md:161 (G13), finding-index.md (G03,G13,G17), tooling.md:178-182 (T14), batch12,batch16,batch18}`, ledger; `git log -- bins/marengo-log-cli` (14 commits: `1bfce54` 2026-06-15 store CLI; `e0b3ffd` candump summary/page; `e885fee` artifact preservation; `2b0aa57`/`3a7ee09` recovery; `5084ad0` bounded candump; `1fcb54c` 2026-10-03 gravity-fit); consumers `scripts/systemd/marengo-log-maintenance.service:9-11`, `scripts/{install-pi.sh:156-175,362-364,deploy-pi.sh:193,214,pi-native-build.sh:27,bench-log-archive.sh:13-24,pi-remote.sh:111-115}`, `tools/marengo-pi-mcp/src/tools/{logs.ts:105,128-132,motion.ts:58-67,gravity-calibrate.ts:743}` |

## 2. Intent

ADR 0011 §3 created it as the **shell/MCP entrypoint to the SQLite log store** ("no bash sqlite3"): register/finalize bench sessions, archive hot files into gzip blobs, purge by age, import the systemd journal nightly, and report disk use (`main.rs:1`; `codemap.md:4`). It is installed on the Pi (`install-pi.sh:175`) and driven daily by `marengo-log-maintenance.timer` (`install-pi.sh:362-364`; service `:9-11`) and per bench run by MCP motion tools (`motion.ts:58-67`). Two later responsibilities were added: **candump inspection** without a database (`e0b3ffd`; `marengo-candump` lib table "Operator summary/page with required `--timestamp`", `crates/marengo-candump/src/lib.rs:32`) and **explicit historical store recovery** `recover-known-v2` (ADR 0030 ¶2: dispatches before normal Store open). On 2026-10-03 it gained **`gravity-fit`** (`1fcb54c`): a workstation-only fitter that turns `pi_gravity_calibrate` session directories into a dated calibration record plus a proposed right-arm inertial URDF patch, never applying it (`gravity_fit.rs:1-20`; `limb-playbook.md:176-180`).

Conflicting statements of intent:
- `bins/AGENTS.md:12` ("Dev | Query archived bench sessions (SQL store)") and `bins/codemap.md:13` ("Query archived bench sessions"): there is **no query/list subcommand** (`main.rs:33-92`); session queries are served by the gateway (`/logs/sessions`). The bin is a Pi-installed maintenance tool plus a workstation fitter.
- `gravity-fit` is a calibration/dynamics tool living in a log CLI that also ships to the Pi (`deploy-pi.sh:193`), pulling `armee-dynamics` into the Pi maintenance binary (`Cargo.toml:22`). Nothing documents why it belongs here [INFERENCE: reuse of an existing workstation CLI].

## 3. Owns / Must not

| Owns | Must not |
|---|---|
| CLI parsing and dispatch to `marengo-store` (`main.rs:330-399`) | Implement schema/backup logic itself — upheld: `recover_known_v2` is a library call (`main.rs:253-273`; ADR 0030; `src/codemap.md:7`) |
| Candump inspect without DB (`main.rs:188-251`) | Open the normal Store for recovery/candump/gravity-fit — upheld (`main.rs:280-302`) |
| Gravity-fit pipeline: plan validation, trace parsing, steady-state extraction, friction pairing, record + patch writing (`gravity_fit.rs:204-1195`) | Apply a URDF change or touch the Pi (`gravity_fit.rs:17-19,329-330`) — upheld: writes only `<out_dir>/…` and `<dir>/proposed-marengo.urdf` |
| | Logic in bins (`bins/AGENTS.md` anti-patterns) — **violated**: ~1.2 kLOC of trace/steady-state/patch logic in `gravity_fit.rs`; only the parameter fit lives in `armee_dynamics::calibration` |
| | `println!` for runtime logs is acceptable: not a Chappe producer (`docs/logging-taxonomy.md:45`) |

## 4. Interface

| Subcommand | Code | Consumers | Tests |
|---|---|---|---|
| `session register` | `main.rs:96-109,340-358` | MCP `motion.ts:60-64` (per bench session) | `tests/session_cli.rs:51` (indirect) |
| `session finalize` | `main.rs:110-113,359-362` | MCP `motion.ts:65` | — |
| `session clear-artifact` | `main.rs:114-120,363-369` | none outside tests (operator repair, batch12) | `session_cli.rs:51,170` |
| `archive --keep` | `main.rs:40-44,371-374` | systemd `:10`; MCP `motion.ts:66`, `logs.ts:105`; `scripts/bench-log-archive.sh:24` | `session_cli.rs:65` |
| `purge --days` | `main.rs:45-49,375-378` | systemd `:11` (`--days 30`) | none in this bin (store `tests/retention.rs`) |
| `import-legacy --keep` | `main.rs:50-54,379-385` | none outside tests ("One-time import") | `session_cli.rs:63-64,161,192` |
| `disk-usage` | `main.rs:55-56,386-389` | **none** (grep: only codemap) | none |
| `journal-import` | `main.rs:57-58,390-393` | systemd `:9` | none |
| `recover-known-v2` | `main.rs:59-67,253-273` | operator (ADR 0030) | `tests/recovery_cli.rs:70,161` |
| `candump summary` | `main.rs:141-142,244-251` | MCP `logs.ts:132`; `scripts/pi-remote.sh:115`; `scripts/AGENTS.md:57` | `tests/candump_cli.rs:24,57`; `candump_input_errors.rs:5` |
| `candump page` | `main.rs:143-150` | none outside docs (`marengo-candump/src/lib.rs:32`) | none |
| `gravity-fit` | `main.rs:73-91,281-295,315-328`; `gravity_fit.rs:204` | MCP `gravity-calibrate.ts:743` (workstation `cargo run`); `limb-playbook.md:180` | `tests/gravity_fit_cli.rs:174,262,295,312`; inline `gravity_fit.rs:1239,1274,1298,1341` |

Global options `--root` (`MARENGO_ROOT`), `--db` (`MARENGO_DB_PATH`) (`main.rs:27-30`). Exit codes: 0/1 generally; gravity-fit 0 proposed / 2 refused / 1 error (`main.rs:314-328`; `gravity_fit.rs:93-101`).

Depth: store subcommands are a shallow CLI adapter over `marengo-store` (one call each). `gravity-fit` is the only deep module: one entry `run(&GravityFitArgs) -> Outcome` hiding plan/trace/patch formats; seam to `armee_dynamics::calibration` (`fit_gravity_params`, `cancel_friction`, `default_params`) — a real cross-crate seam with one adapter. No traits. Unused dep `tracing` (`metrics/unused-deps.md:34-35`; `Cargo.toml:30`; 0 uses in `src/`).

## 5. Invariants owned

| Invariant | Enforcing code | Test(s) |
|---|---|---|
| Recovery never touches the configured DB implicitly; explicit source/backup/output; receipt reported even if stdout fails | `main.rs:280-302,253-273` (+ library) | `recovery_cli.rs:70,161` |
| Candump commands never open the Store; huge timestamps are input errors, not panics | `main.rs:280-302`; `marengo_candump` checked conversion | `candump_input_errors.rs:5` (G17) |
| `--enrich` requires explicit `--config-dir` (no implicit runtime config) | `main.rs:163-164,188-205` | none |
| Clear-artifact removes only the selected reference, keeps files | store `clear_session_artifact` via `main.rs:363-369` | `session_cli.rs:51` |
| Gravity-fit refuses poses outside `control.yaml` soft ∩ `motors.yaml` hard (exit 2) | `gravity_fit.rs:209-232,365-418`; `is_refusal` `:93-101` | `gravity_fit_cli.rs:295`; `gravity_fit.rs:1341` |
| Fuse only sessions with byte-identical Pi URDF and same joint list | `gravity_fit.rs:234-255` (checks at `:238,246`) | none |
| Patch only right-arm links; only `<mass value>` / `<origin xyz>` inside `<inertial>` | `gravity_fit.rs:50,272-278,782-834` | `gravity_fit_cli.rs:312`; `gravity_fit.rs:1298` |
| Patch is round-trip verified (≤0.005 Nm) and only written when fit accepted; nothing applied | `gravity_fit.rs:48,285-296,836-851` | `gravity_fit_cli.rs:174,262` |
| Each pose needs both below and above approaches (friction cancel) | `gravity_fit.rs:651-695` | `gravity_fit.rs:1239` |

## 6. Inputs / outputs

- **Env/flags**: `MARENGO_ROOT`, `MARENGO_DB_PATH` (`main.rs:27-30`); fallbacks `marengo_store::resolve_marengo_root/resolve_db_path` (`main.rs:335-336`); defaults `DEFAULT_HOT_KEEP=50`, `DEFAULT_ARCHIVE_DAYS=30` (`crates/marengo-store/src/paths.rs:29-30`). DB setting `log_archive_days` is **not** read by `purge`.
- **DB**: `marengo.db` via `Store::open` (sessions, log_events, blobs); journal via `import_journal(&store, JOURNAL_UNITS)` (`main.rs:391`) [INFERENCE: shells out to `journalctl`, implemented in marengo-store].
- **Files read**: candump capture (plain/gzip) `--file`; `--config-dir/motors.yaml` for enrichment; gravity sessions `<dir>/{plan.json,position-trace.csv,pi-marengo.urdf,config/{robot,control,motors}.yaml}` (`gravity_fit.rs:52-55,209-233`); `--repo-urdf` (default `assets/urdf/marengo.urdf`, relative to CWD, `main.rs:89`).
- **Files written**: blobs under `var/log/blobs` (archive); `<out_dir>/<stem>.{json,md,urdf.patch}` (default `docs/commissioning/calibrations`, relative to CWD, `main.rs:86`); `<first dir>/proposed-marengo.urdf` (`gravity_fit.rs:287-288`).
- **stdout**: plain text receipts; JSON for candump `--format json` and recovery receipt.
- No Chappe, CAN, or HTTP.

## 7. Prior review reconciliation

| Id | Prior | Current status | Evidence |
|---|---|---|---|
| G03 | retention deadlock (purge path) | fixed in store; this CLI is now the only production purge caller | `crates/marengo-store/src/store.rs:269-285`; systemd `:11` |
| G13 | `import-legacy` NULLs siblings / miscounts | **fixed** | ledger verified; `main.rs:379-385` now prints sessions + artifacts from `import_legacy_hot_report`; `session_cli.rs:51-70` |
| G14/G15 | store dates / migration | fixed / partial (store-owned) | ledger |
| G17 | CLI panics (exit 101) on 1e30 timestamp | **fixed** | `5084ad0`; `tests/candump_input_errors.rs:5-26` asserts exit 1, no "panicked" |
| T14 | installed CLI not on MCP PATH | **open** (consumer-side) | `tools/marengo-pi-mcp/src/env.ts:13` PATH lacks `/opt/marengo/bin`; `logs.ts:128`, `motion.ts:59` still use `command -v marengo-log-cli` |
| batch12 / batch16 | clear-artifact; recover-known-v2 | delivered | `main.rs:114-120,59-67`; ADR 0030 |

## 8. Drift

- `bins/AGENTS.md:12`, `bins/codemap.md:13`: "Query archived bench sessions", host "Dev" — no query command; installed and timer-driven on Pi (§2).
- ADR 0011 §5 "30-day purge (`MARENGO_LOG_ARCHIVE_DAYS`)": no code reads that variable (grep); retention comes from the hard-coded `--days 30` in the unit, and the gateway's `/settings` exposes a DB `log_archive_days` that purge ignores.
- `src/codemap.md` lists "disk-usage" as a maintained command; it has no consumer or test.
- `crates/marengo-candump/src/lib.rs:32` advertises `page`; no caller and no test.
- `gravity_fit.rs:7-8` says the step window starts "after its `hold-at` until any joint's operator target changes"; code ends a step at the first tick whose **whole target map** differs (`gravity_fit.rs:571-573`) — same thing only if every row is present each tick [INFERENCE].

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Deleting touches |
|---|---|---|---|
| `tracing` dependency (`Cargo.toml:30`) | zero references (`metrics/unused-deps.md:34-35`) | high | `Cargo.toml` |
| `disk-usage` subcommand (`main.rs:55-56,386-389`) | zero references outside codemap; no test | med | `src/codemap.md:5`; `Store::log_disk_usage_bytes` (`crates/marengo-store/src/store.rs:655`) has no other caller, so it becomes dead too (host metrics compute disk use separately, `crates/marengo-host-metrics/src/lib.rs:158`) |
| `import-legacy` subcommand (`main.rs:50-54,379-385`) | "One-time import" (doc comment `main.rs:50`) with no production caller; scaffold of the 2026-06 migration | low | `tests/session_cli.rs:63-64,161,192`; keep if old Pis may still hold pre-store hot files |
| `candump page` (`main.rs:143-150,207-219`) | no consumer, untested; advertised in candump lib docs | low | `marengo-candump` doc table |
| `unreachable!` arms (`main.rs:394-396`) | dead branches forced by one shared `Commands` enum | low | split store vs non-store subcommands into separate enums so the match is total |

Do not prune `recover-known-v2` (ADR 0030) or `clear-artifact` (batch12 operator repair) despite having no scripted caller.

## 10. Phase-B leads

1. **Maintenance chain aborts**: oneshot unit runs `journal-import`, `archive`, `purge` as successive `ExecStart=` lines without `-` prefixes (`scripts/systemd/marengo-log-maintenance.service:9-11`); a journal-import failure skips archive and purge (systemd oneshot semantics [INFERENCE: not exercised]). No test covers `journal-import` or `purge` in this bin.
2. **T14 fallback deletes evidence**: when `command -v marengo-log-cli` fails on a standard install (PATH lacks `/opt/marengo/bin`, `env.ts:13`), MCP falls back to `benchLogPruneShell`, pruning hot logs without registering or archiving them (`motion.ts:58-69`).
3. **Empty artifact paths**: MCP passes `--candump "${CANDUMP:-}"` (`motion.ts:63`), i.e. `Some("")`, into `register_session` (`main.rs:349-356`); whether the store treats an empty path as "absent" is unverified [INFERENCE].
4. **Concurrent writer**: CLI opens the same DB as the gateway batch writer (ADR 0011 Consequences); SQLite busy timeout is 5 s (`crates/marengo-store/src/recovery.rs:286` for recovery; normal Store path unverified) — nightly runs may hit `SQLITE_BUSY` while the gateway writes.
5. **u64 underflow on non-monotonic trace time**: `last_ms - ticks[run_start].t_ms` and `window_s` subtraction (`gravity_fit.rs:614,645`) panic in debug / wrap in release if `t_ms` decreases within a step (concatenated or restarted traces); release wrap would pass the `run_s < MIN_WINDOW_S` check.
6. **Fused sessions use the first directory's limits only**: windows come from `first/config` (`gravity_fit.rs:209-232`); other dirs are checked only for joint list and URDF bytes (`:238,246`), not for control/motors equality.
7. **CWD-relative defaults**: `--out-dir docs/commissioning/calibrations` and `--repo-urdf assets/urdf/marengo.urdf` (`main.rs:86,89`); run outside the repo root silently writes records elsewhere and reports the local URDF comparison as unknown (`gravity_fit.rs:282`, `.ok()`).
8. **Writes into evidence dir**: `proposed-marengo.urdf` is written into the first session directory (`gravity_fit.rs:287-288`), mutating captured inputs; a second run overwrites it.
9. **Line-oriented URDF patching** (`gravity_fit.rs:782-834`): multi-line `<link>` or one-line `<inertial>` blocks fail to patch (fail-closed error) — acceptable but brittle; round-trip check (`:836-851`) is the guard.
10. **Trace rows grouped only when contiguous by tick** (`gravity_fit.rs:500-515`): interleaved writes would create duplicate ticks and split steps.
11. **`main.rs` 71.8 % covered** (`coverage-by-file.md:28`): `purge`, `journal-import`, `disk-usage`, `session finalize` and `candump page` paths are unexercised.
