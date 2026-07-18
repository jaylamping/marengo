# bins/marengo-log-cli/

## Responsibility
CLI to **query archived bench sessions** from marengo-store (gateway SQL or hot log files).

## Design
- List sessions, tail logs, grep patterns — used by MCP `pi_logs_*` tools
- Reads from gateway store or `var/log/bench-*.log` on Pi

## Integration
- **Depends on**: marengo-store
- **Used by**: bench debugging, MCP log tools

**Detailed map**: [src/codemap.md](src/codemap.md)
