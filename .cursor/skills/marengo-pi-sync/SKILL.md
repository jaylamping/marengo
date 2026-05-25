---
name: marengo-pi-sync
description: Sync Marengo main to the Pi — pi_sync_main cross-build deploy without user confirm. Use when user says sync pi, deploy, push fixes to pi, or pi_health shows stale .deploy-rev.
---

# Marengo Pi sync-with-main

**Pre-authorized — no confirm.** Rebuild + deploy is always OK.

## Tool

**`pi_sync_main`** — local `git pull main` → `deploy-pi.sh joey@marengo.local` → `install-pi.sh` → `.deploy-rev`

Optional: `strategy: "pi_native"` for on-Pi build (slow).

Requires `MARENGO_LOCAL_ROOT` in MCP env (dev Mac marengo clone).

## When to run (no ask)

- User says *sync pi*, *deploy*, *push fixes to pi*
- `pi_health` shows stale `.deploy-rev` or wrong `motors.yaml`
- Fix committed on `main` needs Pi verification

## When not to sync

- Local repo dirty (tool fails — commit/stash first)
- `marengo-pi` running (warning only; prefer idle bench)

## After sync

1. `pi_health` — `.deploy-rev`, binaries, commissioned config
2. `pi_can_up` if CAN down
3. Continue log-first investigation or request motion confirm

See [marengo-pi-mcp](../marengo-pi-mcp/SKILL.md) for bench motion rules.
