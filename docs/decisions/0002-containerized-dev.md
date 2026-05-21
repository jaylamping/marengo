# ADR 0002: Containerized development environment

**Status:** Accepted  
**Date:** 2025-05-19

## Context

Contributors use macOS, Windows (WSL2), and Linux. Host-installed `protoc`, `buf`, and Rust toolchains drift, causing “works locally” failures in CI.

## Decision

- **Single dev image:** [`docker/Dockerfile.dev`](../../docker/Dockerfile.dev) pins Rust 1.85, protoc 28.3, Node 22, cross toolchain, and vcan tools. Advisory DB for `cargo-deny` is pinned in-image (pre-CVSS-4.0) via [`deny.toml`](../../deny.toml).
- **CI parity:** GitHub Actions runs [`scripts/check.sh`](../../scripts/check.sh) inside that image.
- **Compose** services: `dev`, `check`, optional profiles `vcan` and `sim`.
- **Dev Container** (Cursor / VS Code) uses the same Dockerfile; optional for editors that support it.
- **Native tooling** via `mise.toml` / `.tool-versions` is best-effort only.

## Consequences

- Docker required for the supported path ([`docs/onboarding.md`](../onboarding.md)).
- macOS CAN development runs inside Linux containers (`vcan` profile).
- Sim tests use a separate [`docker/Dockerfile.sim`](../../docker/Dockerfile.sim) with MuJoCo Python.

## Alternatives considered

- Host-only mise/asdf: lighter, insufficient cross-OS parity.
- Nix flake: strong reproducibility; higher learning curve for a solo/small team.
