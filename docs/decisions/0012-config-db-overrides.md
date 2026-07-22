# ADR 0012: Runtime config — memory SoT + YAML write-behind

**Status:** Accepted (supersedes 2026-06-15 stub)  
**Date:** 2026-07-22

## Context

Operators need to change joint hard/soft limits and gains from Consul without treating git-tracked YAML as the only runtime truth, and without dual-writing from the gateway into the active profile while claiming live success.

## Decision

1. **Active profile runtime SoT is in-memory on the Pi** (Davout / Berthier config aggregate). YAML under `MARENGO_CONFIG_DIR` is the boot seed and async write-behind.
2. **Live numerical limit changes** (hard/soft position, torque cap, `velocity_max_rad_s`) go gateway → Chappe `LimitPatchCommand` → Pi validate/apply → ACK with `persist_status` (`pending` | `durable` | `failed`). HTTP success requires apply ACK, not publish alone.
3. **Inactive bringup profiles** are updated only via atomic validate-then-commit disk transactions (`marengo-config` profile txn) with CAS `expected_revision`. `applied_live: false`.
4. **Structural / wiring / membership** changes always require restart when the target is the active profile. Persist-degraded is **not** NeedsRestart (restart would reload stale YAML).
5. **`marengo.db` `config_overrides` / `settings`** remain audit / operator prefs — not the motor limit SoT.
6. **One persist coordinator** on the Pi queues motors+control (and control-only overlay) writes; restart drains or refuses while pending.

## Consequences

- Set Limits no longer opens the NeedsRestart dialog on success.
- Consul invalidates the live snapshot after ACK; inactive toasts name the target profile.
- Syncing repo YAML onto a dirty Pi can clobber unrecovered write-behind — operators should treat persist-failed as a flush/retry event.
