# Session handoff — Marengo

- observed_at: 2026-07-14T02:15:00Z
- git_sha: 308fa2f (main baseline); branch `cursor/local-handoffs-disable-mem0-4f96` for memory/Tailscale work
- source_ref: Cursor Cloud Agent bc-177f87b3-b48a-4ec6-9602-e9099f1e4f96
- resume_pending: true

## Goal

Re-onboard via Cloud Agents (phone-friendly). Disable mem0 → local `.omo/` handoffs. Get Tailscale + `pi-remote.sh` working to the bench.

## Current state

- Active bench profile: `config/bringup/arm_2dof_right/` (roll CAN id 1, pitch CAN id 2)
- Position-hold baseline signed off: kp 18 / kd 3 / ki 5 — see `docs/bench-2dof-right-smoke.md`
- mem0 MCP: **disabled** (removed from `.cursor/mcp.json`; local handoffs preferred)
- Cloud secrets present: `MARENGO_PI_HOST`, `MARENGO_PI_USER`, `MARENGO_PI_SSH_PRIVATE_KEY_B64`, `TAILSCALE_AUTH_KEY`
- **Blocker:** `TAILSCALE_AUTH_KEY` rejected — `invalid key: API key does not exist` (format `tskey-…`, length 61 — revoked/expired, not missing)
- Tailscale package + userspace `tailscaled` install path verified on this VM after re-running `setup-cloud-pi.sh`
- Pi health: not reachable until Tailscale connects
- Older handoff archive: `.omo/session-handoff-2026-06-21.md` (single-joint gravity suite; superseded profile)

## Work completed

- Cloud agent environment recognized; install had failed on missing Tailscale — packages + Tailscale installed on demand
- Switched agent memory to local: `marengo-memory.mdc`, OpenSpec default, skills/automation marked disabled
- Confirmed SSH key secret length looks sane (not validated until Tailscale is up)

## Pending next

1. Rotate `TAILSCALE_AUTH_KEY` in Cursor Cloud Agents secrets (new reusable/ephemeral key from Tailscale admin)
2. Re-run `./scripts/setup-cloud-pi.sh --verify` → expect `SSH_OK` + gateway health
3. `./scripts/pi-remote.sh health` / `logs-last-fault` for bench snapshot
4. Continue re-onboard (smoke path or software task)

## Key files

- `.omo/session-handoff.md` — this file (canonical)
- `docs/cloud-pi-tailscale.md` — secrets + verify
- `docs/bench-2dof-right-smoke.md` — motion smoke
- `config/bringup/arm_2dof_right/`
- `./scripts/pi-remote.sh` / `./scripts/setup-cloud-pi.sh`
