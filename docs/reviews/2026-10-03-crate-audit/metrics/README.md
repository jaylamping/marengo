# Crate audit metrics (Phase A baseline)

**Git baseline:** `a2b55b3` on branch `audit/2026-10-03`  
**Worktree:** `/Users/joseph/code/marengo-wt/audit`  
**Raw artifacts:** `/tmp/audit-cov.json`, `/tmp/audit-cov.log`, `/tmp/audit-machete.txt`, `/tmp/audit-deny.txt`, `/tmp/audit-audit.txt`, `/tmp/audit-test-list.txt`

## Method

1. Toolchain `1.88.0` + `llvm-tools-preview`; install locked `cargo-llvm-cov`, `cargo-machete`, `cargo-deny`, `cargo-audit`.
2. Coverage: `cargo llvm-cov --workspace --json --summary-only --output-path /tmp/audit-cov.json` (full workspace build+test; JSON finalized with `cargo llvm-cov report --json --summary-only` after the instrumented run). No crate excludes required.
3. `cargo machete` → `unused-deps.md`.
4. `cargo deny check` + `cargo audit` → `deny-audit.md`.
5. LOC, suppressions, test list, pub-usage heuristics via `/tmp/audit_metrics_gen.py` and `/tmp/audit_pub_usage.py`.

## Tool versions

| Tool | Version |
|------|---------|
| rustc (1.88.0) | 1.88.0 (6b00bc388 2025-06-23) |
| llvm-tools-preview | installed (aarch64-apple-darwin) |
| cargo-llvm-cov | 0.9.1 |
| cargo-machete | 0.9.2 |
| cargo-deny | 0.20.2 |
| cargo-audit | 0.22.2 |

## Headline numbers

| Metric | Value |
|--------|------:|
| Source LOC (`crates/` + `bins/` `src/`) | 68,643 |
| Test LOC (integration + `tests/`) | 27,131 |
| Listed tests (`cargo test --workspace -- --list`) | 980 |
| Workspace line coverage (instrumented tree) | **81.7%** (33,291 / 40,734 lines) |
| `src` files ≥50 LOC in coverage report | 120 |
| `cargo deny check` | **PASS** (`advisories ok, bans ok, licenses ok, sources ok`; wildcard/duplicate warnings) |
| `cargo audit` | **PASS** (exit 0; 1 allowed warning: `paste` unmaintained RUSTSEC-2024-0436) |
| `cargo machete` unused dep entries | 10 packages flagged (see `unused-deps.md`) |
| Pub items in `crates/*/src` (heuristic) | zero-use 24, test-only 19 |

## Artifacts in this directory

| File | Contents |
|------|----------|
| `coverage-by-crate.md` | Line / region / function % per crate and `bin:*` |
| `coverage-by-file.md` | All `src` files ≥50 LOC, worst line % first |
| `loc.md` | Per-package LOC + 40 largest `.rs` files |
| `suppressions.md` | `allow`, TODO/FIXME, `ignore`, `let _ =`, `.unwrap()`/`.expect()` in `crates/*/src` |
| `test-counts.md` | Tests per test binary / package from `--list` |
| `pub-usage.md` | Public symbol reference heuristic |
| `unused-deps.md` | `cargo machete` output |
| `deny-audit.md` | Full `deny` + `audit` logs |

## Failures / notes

- One `cargo llvm-cov --workspace` run failed when executing `marengo-homing` unit tests (`No such file or directory` on the test binary under `target/llvm-cov-target/`). A subsequent full workspace run completed and wrote `/tmp/audit-cov.json`; no `--exclude` was used.
- Initial `cargo llvm-cov` invocations hit the 60s shell background cap before JSON was written; a later run finished with `Finished report saved to /tmp/audit-cov.json` (and `cargo llvm-cov report` was available if needed).
- `cargo-llvm-cov --version` is not a supported subcommand; version recorded via `cargo llvm-cov --version`.
