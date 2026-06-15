# Agent instructions (Marengo)

Short index — full rules live in linked docs.

## Before any change

1. Read [docs/rust-patterns.md](docs/rust-patterns.md) (good/bad Rust).
2. Read [docs/safety.md](docs/safety.md) if touching control, CAN, or enable paths.
3. Run **`just check`** or `docker compose run --rm check` before finishing.

## Never

- Hand-edit `consul/src/gen/` (run `cd consul && npm run gen:proto`).
- Add JSON as the Chappe wire format ([ADR 0001](docs/decisions/0001-protobuf-wire-types.md)).
- Use `unwrap()` / `expect()` in `crates/*` library code.
- Skip Davout for motor commands.
- Edit the plan file in `.cursor/plans/` during implementation tasks unless asked.

## Always

- Proto-first API changes under `proto/`.
- Thin `bins/`, logic in `crates/`.
- Workspace forbids `unsafe` via `[workspace.lints]` unless an ADR says otherwise.
- Call `marengo_support::init_tracing()` in bin `main`.
- Update `docs/rust-patterns.md` when introducing a new recurring pattern.

## Environment

- Preferred: Dev Container or `docker compose` ([onboarding.md](docs/onboarding.md)).
- Native host tooling is best-effort ([dev-setup.md](docs/dev-setup.md)).

## Architecture

- [architecture.md](docs/architecture.md)
- [roadmap.md](docs/roadmap.md) — full humanoid target; 4-DOF arm is current execution slice, not project scope
- ADRs: [docs/decisions/](docs/decisions/)

## Simulation & CAN

- Default tests: no hardware.
- SocketCAN test harness: `just vcan` then `cargo test -p robstride --features socketcan -- --ignored`
- Sim: `just sim-check` ([ADR 0003](docs/decisions/0003-simulation-testing.md))

## Cursor Cloud specific instructions

Cloud agents run natively (no Docker) with pre-installed tooling: Rust 1.88, Node 24, protoc 28.3, cargo-deny 0.16.3, cargo-audit, and the pinned advisory-db.

### First-time VM setup

Fresh cloud VMs do not include Docker or the dev-container toolchain. Run once:

```bash
./scripts/setup-cloud.sh
```

This installs protoc, cargo-deny, cargo-audit, the pinned advisory-db, and runs `./scripts/bootstrap.sh`.

### Running checks

Use `./scripts/check.sh` directly (the `just check-native` target). Docker and `just check` are unavailable in the cloud VM. The cross-build smoke test (aarch64) is skipped; this is non-fatal.

### Key commands

| Task | Command |
|------|---------|
| Full CI-parity check | `./scripts/check.sh` |
| Build workspace | `cargo build --workspace` |
| Lint (fmt) | `cargo fmt --all -- --check` |
| Lint (clippy) | `cargo clippy --workspace --all-targets -- -D warnings` |
| Test | `cargo test --workspace` |
| Consul proto codegen | `cd consul && npm run gen:proto` |
| Consul type-check | `cd consul && npm run build` |

### Gotchas

- `cargo deny check --disable-fetch` requires the pinned advisory-db at `/usr/local/cargo/advisory-db-pinned/github.com-a946fc29ac602819`. `./scripts/setup-cloud.sh` installs and pins it; do not let `cargo audit` auto-update that tree.
- Consul `dev` script is a scaffold (`echo "Vite app scaffold TBD"`); there is no running frontend dev server.
- `motor-repl` uses SocketCAN only; use `just vcan` / the ignored SocketCAN tests for no-hardware bus checks.
- Default `cargo test` requires no hardware. vCAN and simulation tests are optional and need Docker.

### Pi bench access (Tailscale)

Cloud VMs reach the robot via **Tailscale userspace networking** — not `marengo.local` mDNS. `.cursor/environment.json` runs `./scripts/setup-cloud-pi.sh` on boot.

**One-time:** add secrets per [docs/cloud-pi-tailscale.md](docs/cloud-pi-tailscale.md) (`TAILSCALE_AUTH_KEY`, `MARENGO_PI_SSH_PRIVATE_KEY_B64`, `MARENGO_PI_HOST`).

**Pi / logs (no marengo-pi MCP in cloud):** use `./scripts/pi-remote.sh`:

| Task | Command |
|------|---------|
| Verify connectivity | `./scripts/pi-remote.sh verify` |
| Health | `./scripts/pi-remote.sh health` |
| Log triage | `./scripts/pi-remote.sh logs-last-fault` → `logs-tail` → `logs-grep` |
| Archive sessions | `./scripts/pi-remote.sh logs-sessions` |
| CAN wire truth | `./scripts/pi-remote.sh candump-summary` |
| Deploy to Pi | `./scripts/pi-remote.sh deploy --install` |

Do not ask the user to paste Pi logs or run SSH when `pi-remote.sh` can fetch them.
