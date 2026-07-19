# tools/ — Vendored MCP servers + build tooling

Development tooling vendored into the repo. Not part of the Rust workspace or consul npm workspace.

## STRUCTURE

```
tools/
├── marengo-pi-mcp/           # Node/TS MCP server: Pi bench tools (health, logs, CAN, deploy)
├── marengo-research-mcp/     # Python/uv MCP server: research tooling
└── protoc-28.3-win64/        # Vendored protoc binary (Windows x64)
```

## WHERE TO LOOK

| Task | Location |
|------|----------|
| Rebuild Pi MCP | `just mcp-build` or `cd tools/marengo-pi-mcp && npm install && npm run build` |
| Research MCP setup | `just research-mcp-setup` (uv sync) |
| MCP config | `.cursor/mcp.json` (which servers Cursor loads) |
| Protoc (Windows) | `tools/protoc-28.3-win64/` (used by proto codegen) |

## CONVENTIONS

- MCP servers are rebuilt with `just mcp-build` — restart MCP in Cursor after.
- `marengo-pi-mcp` exposes tools like `pi_health`, `pi_can_status`, `pi_hold_on`, `pi_sync_main`, `pi_logs_*` — see `.cursor/rules/pi-mcp-first.mdc`.
- When MCP is unavailable (cloud), fall back to `scripts/pi-remote.sh`.
- `marengo-research-mcp` uses `uv` (Python), not npm.

## ANTI-PATTERNS

- Falling back to user-run SSH when marengo-pi MCP tools exist and can do the task.
- Letting MCP servers drift from the Pi runtime API — rebuild after proto/bin changes.
- Editing vendored protoc binary.
