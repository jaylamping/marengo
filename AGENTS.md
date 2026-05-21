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
- vcan: `just vcan` then `cargo test -p robstride --features vcan -- --ignored`
- Sim: `just sim-check` ([ADR 0003](docs/decisions/0003-simulation-testing.md))
