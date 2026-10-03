# Intent card — `probe`

## 1. Header

| Field | Value |
|---|---|
| Crate | `probe` (`bins/probe`), bin, baseline `a2b55b3` |
| LOC | src 6 (`main.rs`), tests 0 (`metrics/loc.md:31`); coverage 0 % (`metrics/coverage-by-crate.md:18`) |
| Sources | `bins/probe/Cargo.toml`, `codemap.md`, `src/codemap.md`, `bins/AGENTS.md` ("Dev, Bus / diagnostics, Scaffold"), `docs/roadmap.md`, root `Cargo.toml:27`, `git log` (3 code commits, all 2026-05-19 repo-setup: `61fe36d`, `d7787d5`, `5106357`; then docs-only `5b4f222`) |

## 2. Intent

Classification: **abandoned scaffold**. Cargo description "Diagnostics and bus probing for Marengo" (`Cargo.toml:4`); codemap: "Low-level CAN and hardware probe … outside the Davout safety path … Direct CAN frame inspection" (`bins/probe/codemap.md`). The code only logs `"probe: diagnostics scaffold"` (`main.rs:1-6`). No roadmap milestone or ADR mentions it (grep `docs/roadmap.md`, `docs/decisions/`). Its stated purpose is already served by MCP `pi_candump_once`/`pi_candump_summary` and `marengo-log-cli` candump parsing, and **conflicts** with `bins/AGENTS.md` anti-pattern "Direct CAN access from bins → go through Davout" and roadmap non-goal "Direct robstride calls from Berthier or bins" (`docs/roadmap.md`, "What we are not building").

## 3. Owns / Must not

Owns nothing. Must not access CAN directly (`bins/AGENTS.md`) — its documented intent would violate that.

## 4. Interface

No CLI args, no output beyond one log line; deps `marengo-support`, `tracing` (`Cargo.toml:17-19`). Consumers: none (no reference in scripts, tools, deploy, CI build lists; grep). Only workspace membership (`Cargo.toml:27`).

## 5. Invariants owned

None.

## 6. Inputs / outputs

`RUST_LOG` only.

## 7. Prior review reconciliation

`control.md:17` "`probe` and `wave-demo` binaries are logging scaffolds"; `2026-09-29-repository-review.md:94` names should not imply completed products. Status: **unchanged** (no code commit since 2026-05-19).

## 8. Drift

`bins/probe/codemap.md` and `src/codemap.md` ("CAN probe utilities") describe functionality that does not exist.

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Touches |
|---|---|---|---|
| Whole crate `bins/probe` | scaffold with no consumer; stated intent superseded by candump MCP tools/`marengo-log-cli` and contrary to `bins/AGENTS.md` | high | root `Cargo.toml:27` members, `Cargo.lock`, `bins/AGENTS.md`/`bins/codemap.md`/root `codemap.md:40` tables |

## 10. Phase-B leads

None (no logic).
