# Intent card — `marengo-jetson`

## 1. Header

| Field | Value |
|---|---|
| Crate | `marengo-jetson` (`bins/marengo-jetson`), bin, baseline `a2b55b3` |
| LOC | src 6 (`main.rs`), tests 0 (`metrics/loc.md:24`); coverage 0 % (`metrics/coverage-by-crate.md:13`) |
| Sources | `Cargo.toml`, `main.rs`, `codemap.md`, `src/codemap.md`, `bins/AGENTS.md` ("Jetson, Planner, Fouché, Chappe, Scaffold"), ADR 0014 (Accepted 2026-06-19) §1, §10, §12, Implementation order; `docs/roadmap.md` M7 row "`marengo-jetson` beyond scaffold — After M6 or clear sim-only planner scope"; `scripts/systemd/marengo-jetson.service:12`; `scripts/deploy-jetson.sh:4-5`; `metrics/unused-deps.md:17-21`; `git log` (code commits 2026-05-19 only) |

## 2. Intent

Classification: **ADR-backed scaffold (intended, not started)**. ADR 0014 §1 defines the Jetson node as **perception + intent, never control**: vision (Fouché), semantic→`NavigatorIntent` decomposition, host metrics; it must not touch CAN, motors, `davout`, `robstride` or real-time loops. ADR 0014 §10 specifies the bin: Chappe producer via `chappe::tracing_layer::init_subscriber`, TCP bridge to the Pi, Fouché pipelines, `HostMetrics` on `host/metrics/jetson`, `Heartbeat` on `heartbeat/jetson`, optional `robot/state` subscription. Code today: `init_tracing` + one log line (`main.rs:1-6`). Roadmap gates it after M6 (`docs/roadmap.md` M7 table). No conflicting intent; codemap says "vision (Fouché) and planning (Talleyrand)", consistent with ADR 0014 §11 (Talleyrand consumes intent on the Pi side — note: ADR places Talleyrand IK **on the Pi**, so the Jetson bin depending on `talleyrand` is questionable).

## 3. Owns / Must not

Must not (ADR 0014 §1): CAN, motors, `davout`, `robstride`, control loops — upheld (no such deps, `Cargo.toml:17-23`). Owns nothing yet.

## 4. Interface

None. Declared deps `chappe`, `fouche`, `talleyrand`, `tokio` all unused (`metrics/unused-deps.md:17-21`). Consumers: `scripts/systemd/marengo-jetson.service:12` (`ExecStart=/opt/marengo/bin/marengo-jetson`), `scripts/deploy-jetson.sh:4` (`cargo build -p marengo-jetson`, then `echo "TODO: deploy …"`, `:5`).

## 5. Invariants owned

None.

## 6. Inputs / outputs

`RUST_LOG` only. Planned (ADR 0014 table): publish `heartbeat/jetson`, `host/metrics/jetson`, perception/intent topics; subscribe `robot/state`.

## 7. Prior review reconciliation

`2026-09-29-repository-review.md:94` — **unchanged**.

## 8. Drift

| Doc | Says | Code |
|---|---|---|
| ADR 0014 §10 | Chappe producer must use `init_subscriber`, not `init_tracing` | uses `marengo_support::init_tracing()` (`main.rs:4`) — acceptable for scaffold, but `bins/AGENTS.md` lists it as a Chappe peer |
| `scripts/systemd/marengo-jetson.service`, `deploy-jetson.sh` | deployable service | binary only logs and exits 0; unit has `Restart=on-failure` (`marengo-jetson.service:13`), so it exits once and stays inactive — a "running" service that does nothing; no script installs the unit (grep `scripts/*.sh`) |
| ADR 0014 §11 | Talleyrand IK runs on Pi | bin declares `talleyrand` dep |

## 9. Prune candidates

| Candidate | Evidence class | Conf. | Touches |
|---|---|---|---|
| Unused deps `chappe`, `fouche`, `talleyrand`, `tokio` | zero references (`cargo machete`, `metrics/unused-deps.md:17-21`) | high | `Cargo.toml` (shrinks Jetson build graph; may orphan `fouche`/`talleyrand` consumers — see their cards) |
| `scripts/deploy-jetson.sh` | scaffold with no consumer (prints TODO) | med | script only |
| `scripts/systemd/marengo-jetson.service` | scaffold for a binary that does nothing | low (ADR 0014 §12 plans it) | systemd dir |
| The crate itself | — | **keep**: ADR 0014 (Accepted) defines its intent | — |

## 10. Phase-B leads

| # | Location | Suspicion |
|---|---|---|
| L1 | `scripts/systemd/marengo-jetson.service:12-14` | Unit is never installed by any script and the bin exits immediately with 0 (`Restart=on-failure` → no restart); health tooling that checks unit state would see "inactive (dead)" rather than a failure. |
| L2 | `Cargo.toml` deps | `talleyrand` on the Jetson contradicts ADR 0014 §11 placement of IK on the Pi; decide before fleshing out. |
