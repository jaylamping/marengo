# Intent card — `wave-demo`

## 1. Header

| Field | Value |
|---|---|
| Crate | `wave-demo` (`bins/wave-demo`), bin, baseline `a2b55b3` |
| LOC | src 4 (`main.rs`), tests 0 (`metrics/loc.md:36`); coverage 0 % (`metrics/coverage-by-crate.md:20`) |
| Sources | `Cargo.toml`, `codemap.md`, `src/codemap.md`, `bins/AGENTS.md` ("Dev, Demo trajectories, Scaffold"), `bins/codemap.md` ("Sine-wave position excitation demo"), `metrics/unused-deps.md:22-23`, commits `bede6d6` (2026-06-21 "add in-loop wave command for marengo-pi"), `39e8a8a` (2026-07-18 "native cosine Wave compound test"), `git log` (code commits only 2026-05-19) |

## 2. Intent

Classification: **superseded scaffold**. Declared intent: "Demo trajectory / wave motion" (`Cargo.toml:4`); codemap: short-lived sine-wave position excitation via Berthier `position_wave` "without full marengo-pi REPL" (`bins/wave-demo/codemap.md`). Code: logs `"wave-demo: motion demo scaffold"` only (`main.rs:1-4`). The capability now lives in the owner process: Berthier `start_position_wave` driven by marengo-pi stdin `wave …` (`marengo-pi/src/main.rs:191-226,971-986`) and Consul Testing `wave:` batches (`marengo-pi/src/main.rs:597-627`). A separate motion binary would also need its own process-local reference grant (ADR 0036 §Process lifetime), so a standalone wave demo cannot enable anyway. No roadmap/ADR reference.

## 3. Owns / Must not

Owns nothing. Must not become a second CAN owner beside marengo-pi (ADR 0036; MCP sole-owner rule).

## 4. Interface

No CLI. Declared dep `berthier` is unused (`cargo machete`, `metrics/unused-deps.md:22-23`). Consumers: none (grep scripts/tools/deploy).

## 5. Invariants owned

None.

## 6. Inputs / outputs

`RUST_LOG` only.

## 7. Prior review reconciliation

`control.md:17` "logging scaffolds" — **unchanged**.

## 8. Drift

`bins/wave-demo/codemap.md` claims "Uses Berthier `position_wave`", "Depends on: berthier, davout, robstride, marengo-config"; `src/codemap.md` "ControlLoop setup, and timed excitation loop" — none exist. Also Berthier's wave is a **triangle** (`marengo-pi/src/main.rs:598` "in-loop triangle"), not sine.

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Touches |
|---|---|---|---|
| Whole crate `bins/wave-demo` | superseded by commits `bede6d6`/`39e8a8a` (in-owner wave); scaffold with no consumer | high | root `Cargo.toml:30`, `Cargo.lock`, bins docs tables |

## 10. Phase-B leads

None (no logic).
