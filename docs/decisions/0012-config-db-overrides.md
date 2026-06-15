# ADR 0012: Config/settings — YAML source vs DB overrides (stub)

**Status:** Proposed  
**Date:** 2026-06-15

## Context

Robot config lives in git-tracked YAML ([`marengo-config`](../../crates/marengo-config)). Operators will need runtime overrides and UI settings without forking YAML on the Pi.

## Decision (phase 1)

- **YAML remains source of truth** for robot/motor/control/homing/network.
- **`marengo.db` tables** `config_overrides` and `settings` hold runtime keys only (retention, disk budget, Consul prefs).
- **Consul settings UI** deferred; gateway exposes `GET /settings` stub.

## Future

Merge overrides in `marengo-config` loaders or a thin resolver crate when override surface grows.
