# Intent card: marengo-support

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-support` |
| Path | `crates/marengo-support` |
| Kind | lib |
| Baseline | `a2b55b3` (audit/2026-10-03) |
| LOC | src 10 (`src/lib.rs`), tests 0 (metrics/loc.md:29) |
| Deps | `tracing-subscriber` only (`Cargo.toml:13-14`) |
| History | 3 commits: `5106357` 2026-05-19 "more bug fixes" (creation), `dd2ed3e` README, `5b4f222` codemap. No functional change since creation. |
| Sources consulted | `src/lib.rs`, `README.md`, `codemap.md`, `src/codemap.md`, `crates/AGENTS.md:21`, `crates/codemap.md:20`, `bins/AGENTS.md:3,32`, `bins/codemap.md:27`, `AGENTS.md:176`, `docs/rust-patterns.md:7,375`, `.cursor/rules/marengo.mdc:15`, `crates/chappe/src/tracing_layer.rs:205-224`, all `bins/*/Cargo.toml` + `main.rs`, metrics/unused-deps.md, git log |

## 2. Intent

The crate gives every non-Chappe binary one shared way to install a `tracing` fmt subscriber filtered by `RUST_LOG`. Bins call it once from `main`, so CLI and scaffold logs look the same (`src/lib.rs:1-10`; `README.md:3`; `docs/rust-patterns.md:7`). That is all the code does.

**Conflicting statements of intent.** Some docs claim more than the code does:
- `crates/AGENTS.md:21` says it owns "`init_tracing()`, repo-root resolution, shared utils".
- `codemap.md:4,8` says "workspace lint policy overrides, repo root resolution utilities" and "`#![forbid(unsafe_code)]` … enforced here and re-exported".
- `crates/codemap.md:20` says "workspace lint overrides".

None of those exist in the code. Repo-root resolution is `marengo_config::resolve_repo_root` (`crates/marengo-config/src/lib.rs:209`). The unsafe-code lint is set workspace-wide in the root `Cargo.toml:77-78`, not in this crate.

## 3. Owns / Must not

| Owns | Evidence |
|---|---|
| Global fmt subscriber with `EnvFilter::from_default_env()` for CLI/scaffold bins | `src/lib.rs:6-10` |

| Must not | Evidence | Status |
|---|---|---|
| Install the subscriber for Chappe producers (`marengo-pi`, `marengo-gateway`) | `docs/rust-patterns.md:7`; `bins/AGENTS.md:3` | Respected: `bins/marengo-pi/src/main.rs:1111` and `bins/marengo-gateway/src/main.rs:122` use `chappe::tracing_layer::init_subscriber` |
| Contain logic beyond bin bootstrap | `Cargo.toml:4` "Shared runtime helpers for Marengo binaries" | Respected |

No layer violation was found.

## 4. Interface

| Group | Surface | Consumers |
|---|---|---|
| Tracing bootstrap | `pub fn init_tracing()` (`src/lib.rs:6`) | `bins/motor-repl/src/main.rs:128`, `bins/marengo-log-cli/src/main.rs:20,276`, `bins/imu-probe/src/main.rs:8,99`, `bins/marengo-jetson/src/main.rs:4`, `bins/probe/src/main.rs:2`, `bins/teleop/src/main.rs:4`, `bins/wave-demo/src/main.rs:2` (7 callers) |
| Declared but unused dependency | `marengo-support` in `bins/marengo-pi/Cargo.toml:32` and `bins/marengo-gateway/Cargo.toml:27` | No call sites (grep). `cargo machete` flags both (metrics/unused-deps.md:24-33). |

Depth: this is a shallow module with a 1-function interface and 3 lines of body. It has no trait and no seam, and no cargo features. It hides one decision, the subscriber shape. The same decision is also coded in `chappe::tracing_layer::init_subscriber(None, _)` (`crates/chappe/src/tracing_layer.rs:218-222`: registry + EnvFilter + fmt layer), so it is duplicated.

**Metrics baseline** (`metrics/`, `a2b55b3`): **100% lines** on 10 LOC. Regions and functions are reported as 0.0 (`coverage-by-crate.md:20`, `loc.md:29`); the mismatch is a summary artifact worth checking. There are **0** tests in the crate, so coverage comes from the bins' test runs. `pub-usage.md` does not list `init_tracing` as zero-use (7 bin call sites). Unused-dependency entries: `marengo-pi`, `marengo-gateway` (`unused-deps.md:24-33`).

## 5. Invariants owned

| Invariant | Enforcing code | Test |
|---|---|---|
| A subscriber is installed at most once per process (`.init()` panics on a second global set) | `src/lib.rs:9` (tracing-subscriber `init`) | untested. The coverage-by-crate figure of 100% (metrics/coverage-by-crate.md:20) comes from the bins' runs, not from tests in this crate. |

This crate owns no safety invariants.

## 6. Inputs / outputs

| Kind | Item |
|---|---|
| Env var | `RUST_LOG`, via `EnvFilter::from_default_env`. When it is unset, tracing-subscriber 0.3.23 falls back to an ERROR-only default (`tracing-subscriber/src/filter/env/mod.rs:270-289`). The `src/lib.rs:5` doc mentions "`.env`", but nothing loads a `.env` file. |
| Output | stdout/stderr fmt logs |
| Config, proto, CAN, files, HTTP | none |

## 7. Prior review reconciliation

No 2026-09-29 finding concerns this crate. A grep of finding-index.md and implementation-ledger.json for `marengo-support`/`init_tracing` matches nothing. G02 (tracing quota) is in the gateway's Chappe log layer, not here.

| Prior ID | Status | Evidence |
|---|---|---|
| none | — | — |

## 8. Drift

| Doc claim | Code reality |
|---|---|
| `crates/AGENTS.md:21`: "repo-root resolution, shared utils" | Only `init_tracing`. Repo root lives in `marengo-config/src/lib.rs:209`. |
| `codemap.md:4,8`: lint policy overrides; `#![forbid(unsafe_code)]` enforced here and re-exported | Not in `src/lib.rs`. The lint is set in root `Cargo.toml:77-78`, and lints cannot be re-exported. |
| `crates/codemap.md:20`: "workspace lint overrides" | Not present. |
| `codemap.md:11`, `bins/codemap.md:27`: "Consumed by all `bins/*`" / "All bins use … init_tracing()" | 7 of 9 bins call it. `marengo-pi` and `marengo-gateway` declare the dependency but use Chappe's init. `marengo-limit-sync` installs no subscriber. |
| `README.md:3`: "from each `bins/*` main" | Same as above. |
| `src/lib.rs:5`: "`RUST_LOG` / `.env` filter" | No `.env` loading. |

## 9. Prune candidates

| Candidate | Evidence class | Confidence | Deleting touches |
|---|---|---|---|
| `marengo-support` dependency in `bins/marengo-pi/Cargo.toml:32` and `bins/marengo-gateway/Cargo.toml:27` | Zero references; `cargo machete` (`metrics/unused-deps.md`) | high | 2 Cargo.toml lines and Cargo.lock |
| Doc claims of repo-root/lint ownership | Superseded or untrue | high | `crates/AGENTS.md:21`, `codemap.md`, `crates/codemap.md:20` |
| The crate itself: fold into Chappe or inline | Duplicate of `chappe::tracing_layer::init_subscriber(None, _)` (`tracing_layer.rs:218-222`) | low | 7 bins + workspace `Cargo.toml:8,57`. The tradeoff is below. |

**Is the crate worth keeping?** Keep it, but slim the docs. The alternatives cost more than they save:
- (a) Call `chappe::init_subscriber(None, name)` from the 7 bins. Six of them (all except marengo-jetson) do not depend on chappe today, and chappe pulls in `tokio` (`crates/chappe/Cargo.toml:17`).
- (b) Inline 3 lines into 7 bins. That adds a `tracing-subscriber` dependency to each and creates 7 copies.

The cheaper deduplication is the reverse direction: chappe's `None` branch calls `marengo_support::init_tracing()`, or support grows an optional-layer hook. Separately, four of the seven callers are ≤8-line scaffold mains (`probe`, `teleop`, `wave-demo`, `marengo-jetson`). They belong to the bins audit. If those bins are pruned, the crate's justification shrinks to 3 callers.

## 10. Phase-B leads

| Lead | Where | Why suspicious |
|---|---|---|
| ERROR-only logging when `RUST_LOG` is unset | `src/lib.rs:8` | `info!` lines in motor-repl, imu-probe and log-cli are silent unless the env sets `RUST_LOG`. Check whether `/etc/marengo/env` or the MCP launchers set it. |
| Two subscriber implementations can diverge | `src/lib.rs:7-9` vs `crates/chappe/src/tracing_layer.rs:211-222` | Format and filter changes must be made twice. Low risk. |
| `marengo-limit-sync` installs no subscriber | `bins/marengo-limit-sync/src/main.rs` (no `tracing` usage) | Likely harmless: its library path (`marengo-config`) has no `tracing` dependency (`crates/marengo-config/Cargo.toml:13-19`). Docs claiming that every bin initializes tracing are wrong (§8). Low. |
| 100% llvm-cov on 10 LOC | `metrics/coverage-by-crate.md` | Full coverage is inherited from bin test runs, not crate-local tests; does not prove double-`init()` behavior (§5). Not a safety surface. |
