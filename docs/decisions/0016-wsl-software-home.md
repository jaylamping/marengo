# ADR 0016: WSL ext4 is the software home

**Status:** Accepted
**Date:** 2026-07-19

## Context

Marengo software targets Linux (Pi, Docker/`just check`, vcan). On a Windows host
that also runs SolidWorks, it is easy to keep two git clones (`C:\code\marengo`
and `~/code/marengo`) and two Cursor hosts. That produced:

- Branch/commit drift between clones
- Cursor MCP auto-disable when host-specific env was edited into
  [`.cursor/mcp.json`](../../.cursor/mcp.json) (Cursor hashes that env)
- Confusion about whether Windows or WSL was the source of truth

SolidWorks and solidworks-mcp remain Windows-only (COM). The question was whether
that forced a second NTFS git clone.

## Decision

1. **One git tree** for Marengo software + CAD sources: WSL2 Ubuntu at
   `~/code/marengo` (ext4). Never day-to-day from `/mnt/c/...` or a second
   writeable Windows clone.
2. **Two Cursor roles, one tree:**
   - Software / Pi: Cursor **WSL: Connect** → `~/code/marengo`
   - CAD: Cursor on **Windows** →
     `\\wsl$\Ubuntu\home\<user>\code\marengo` (same inode tree via UNC)
3. **SolidWorks MCP** uses `${workspaceFolder}` as
   `SOLIDWORKS_MCP_ALLOWED_ROOTS` so the Windows CAD session’s UNC workspace is
   the allowed root. solidworks-mcp itself stays on Windows
   (`C:\code\solidworks-mcp`).
4. **Pi MCP defaults** live in
   [`tools/marengo-pi-mcp/launch.mjs`](../../tools/marengo-pi-mcp/launch.mjs)
   (and `run-mcp.sh` / `run-mcp.ps1` for shell fallbacks), plus optional user
   overrides. Do **not** put `MARENGO_CONFIG_DIR`, `MARENGO_BENCH_PROFILE`, or
   machine SSH identity paths in repo `.cursor/mcp.json`.
5. Retire any former `C:\code\marengo` clone: rename to `marengo.DEAD`, then
   delete after a short seatbelt window.

## Consequences

- Software I/O and Docker bind mounts stay on fast ext4.
- Operators must open the correct Cursor host for the job (WSL vs Windows CAD).
- SolidWorks-on-UNC must be validated on this machine; if it proves unstable,
   revisit with a new ADR (e.g. CAD-only NTFS mirror) — do not silently grow a
   second software clone.
- Existing docs that assumed `C:/code/marengo` as CAD root are updated to UNC /
   `${workspaceFolder}`.

## See also

- [docs/wsl-setup.md](../wsl-setup.md)
- [CONTEXT.md](../../CONTEXT.md) (Software home, CAD session)
