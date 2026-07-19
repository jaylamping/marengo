---
name: marengo-pi-sync
description: Sync Marengo main to the Pi — pi_sync_main cross-build deploy without user confirm. Use when user says sync pi, deploy, push fixes to pi, or pi_health shows stale .deploy-rev.
---

# Marengo Pi sync-with-main

**Pre-authorized — no confirm.** Rebuild + deploy is always OK.

## Tool

**`pi_sync_main`** — deploy **current local HEAD** (whatever branch is checked out) via `deploy-pi.sh --install` → `.deploy-rev` → **poll until gateway `/health` OK** (default 180s). On Mac/Windows: uses `deploy-pi-docker.sh` automatically. Does **not** `git checkout main` (that stole feature-branch workspaces).

**`pi_wait_deploy`** — poll only (when deploy already happened): pass `expected_rev` from `git rev-parse HEAD`.

Optional: `strategy: "pi_native"` for on-Pi build (slow; still syncs Pi tree to `origin/main`). `wait_for_ready: false` to skip polling.

Requires marengo repo open as Cursor workspace (server derives `localRoot` from install path; override with `MARENGO_LOCAL_ROOT` only if needed).

## When to run (no ask — agent runs it)

- User says *sync pi*, *deploy*, *push fixes to pi*, or UI fix needs Pi verification
- `pi_health` shows stale `.deploy-rev` or wrong `motors.yaml`
- Fix committed on `main` needs Pi verification
- **Never** tell the user to run `deploy-pi.sh` / `install-pi.sh` — use MCP

## After sync

1. `pi_sync_main` output should end with `[ready]` from wait poll; if TIMEOUT, run `pi_health` and investigate gateway
2. `pi_health` — `.deploy-rev`, gateway `/health`, `www/index.html`, commissioned config
2. `pi_can_up` if CAN down
3. Continue log-first investigation or request motion confirm

See [marengo-pi-mcp](../marengo-pi-mcp/SKILL.md) for bench motion rules.
