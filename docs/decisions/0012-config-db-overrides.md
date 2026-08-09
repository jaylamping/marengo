# ADR 0012: Runtime config — memory SoT + YAML write-behind

**Status:** Accepted (supersedes 2026-06-15 stub)  
**Date:** 2026-07-22

## Context

Operators need to change joint hard/soft limits and gains from Consul without treating git-tracked YAML as the only runtime truth, and without dual-writing from the gateway into the active profile while claiming live success.

## Decision

1. **Active runtime SoT is in-memory on the Pi** (Davout / Berthier config aggregate). Durable disk SoT is root `config/{robot,motors,control,homing}.yaml` and `assets/urdf/marengo.urdf` (Pi: `/opt/marengo/config/`, `/opt/marengo/assets/urdf/marengo.urdf`). `MARENGO_CONFIG_DIR` defaults to `/opt/marengo/config`.
2. **Live numerical limit changes** (hard/soft position, torque cap, `velocity_max_rad_s`) go gateway → Chappe `LimitPatchCommand` → Pi validate/apply → ACK with `persist_status` (`pending` | `durable` | `failed`). HTTP success requires apply ACK, not publish alone.
3. **Master YAML/URDF disk edits** use atomic validate-then-commit (`marengo-config` profile txn) with CAS `expected_revision` on the master config dir. Legacy bringup profile paths are retired.
4. **Structural / wiring / membership** changes always require restart when they affect the loaded master description. Persist-degraded is **not** NeedsRestart (restart would reload stale YAML).
5. **`marengo.db` `config_overrides` / `settings`** remain audit / operator prefs — not the motor limit SoT.
6. **One persist coordinator** on the Pi queues motors+control (and control-only overlay) writes; restart drains or refuses while pending.

## Consequences

- Set Limits no longer opens the NeedsRestart dialog on success.
- Consul invalidates the live snapshot after ACK.
- Syncing repo YAML onto a dirty Pi can clobber unrecovered write-behind — operators should treat persist-failed as a flush/retry event.

## Consul UI contract (operator Range SoT)

| Layer | Source of truth |
|-------|-----------------|
| **Enable / hard-limit faults** | Davout in-memory `JointLimitPolicy` hard = `URDF ∩ motors.yaml bench` |
| **Consul Range (Inventory + Testing gauges)** | `GET /snapshot/actuator/limits` → `JointActuatorLimit.pos_*` (and soft `pos_soft_*`) |
| **Disk `GET /config/snapshot`** | Boot seed / edit forms / CAS revision — **not** live Range when the actuator snapshot is present |
| **YAML / URDF on disk** | Write-behind after Durable ACK; deploy can clobber unrecovered writes |

Do not diagnose Enable trips from disk soft limits alone. Prefer the actuator limit snapshot; if poses sit inside hard, point at Pi journal (CAN ENOBUFS, watchdog).
