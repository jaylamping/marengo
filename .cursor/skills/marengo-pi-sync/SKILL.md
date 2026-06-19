---
name: marengo-pi-sync
description: Sync Marengo main to the Pi — pi_sync_main cross-build deploy without user confirm. Use when user says sync pi, deploy, push fixes to pi, or pi_health shows stale .deploy-rev.
---

# Marengo Pi sync-with-main

**Pre-authorized — no confirm.** Rebuild + deploy is always OK.

## Tool

**`pi_sync_main`** — local `git pull main` → `deploy-pi.sh --install joey@marengo.local` → `.deploy-rev` → **poll until gateway `/health` OK** (default 180s). On Mac/Windows: uses `deploy-pi-docker.sh` automatically.

**`pi_wait_deploy`** — poll only (when deploy already happened): pass `expected_rev` from `git rev-parse HEAD`.

Optional: `strategy: "pi_native"` for on-Pi build (slow). `wait_for_ready: false` to skip polling.

Requires marengo repo open as Cursor workspace (server derives `localRoot` from install path; override with `MARENGO_LOCAL_ROOT` only if needed).

## Deploy pipeline (Consul + rev)

Canonical operator redeploy after merge or CHAPPE fix:

1. **`pi_sync_main`** — cross-build, install, poll gateway
2. **`pi_health`** — confirm `/opt/marengo/.deploy-rev` first field == local `git rev-parse HEAD`
3. Open `https://marengo.local:8444` — no **CHAPPE ERR** with `127.0.0.1` in badge tooltip

What deploy does (see ADR 0008):

| Step | Script | Purpose |
|------|--------|---------|
| Env scrub | `deploy-pi.sh` | `env -u VITE_CHAPPE_* npm run build` + `consul/.env.production` empty anchors |
| Dist gate | `check-consul-dist.sh` | Fail if dist embeds `127.0.0.1:8080` or `VITE_CHAPPE_` |
| Always rebuild | `deploy-pi.sh` | No stale-dist skip on deploy |
| Deploy rev | `install-pi.sh` | Single writer → **`/opt/marengo/.deploy-rev`** (local HEAD + UTC) |

**Never** tell the user to run `deploy-pi.sh` / `install-pi.sh` manually — use MCP.

## When to run (no ask — agent runs it)

- User says *sync pi*, *deploy*, *push fixes to pi*, or UI fix needs Pi verification
- `pi_health` shows stale `.deploy-rev` or wrong `motors.yaml`
- Fix committed on `main` needs Pi verification
- Pi Consul shows **CHAPPE ERR** with baked `127.0.0.1` URLs (poisoned bundle) — redeploy immediately after deploy-pipeline merge

## After sync

1. `pi_sync_main` output should end with `[ready]` from wait poll; if TIMEOUT, run `pi_health` and investigate gateway
2. `pi_health` — `.deploy-rev`, gateway `/health`, `www/index.html`, commissioned config
3. `pi_can_up` if CAN down
4. Continue log-first investigation or request motion confirm

See [marengo-pi-mcp](../marengo-pi-mcp/SKILL.md) for bench motion rules.
