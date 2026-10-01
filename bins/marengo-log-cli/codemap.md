# bins/marengo-log-cli/

## Responsibility
CLI for bench-session registration, archival, retention, imports and SQLite maintenance.
Candump inspection calls `marengo-candump` directly. Explicit `recover-known-v2`
delegates to `marengo-store` before normal Store open and prints its completed receipt.

## Design
- Session and maintenance commands open the configured Store.
- Recovery requires explicit source, fresh backup and fresh output paths; it never
  selects the configured database implicitly or replaces the source.
- Candump summary/page commands require no database.

## Integration
- **Depends on**: marengo-store
- **Used by**: bench debugging, MCP log tools

**Detailed map**: [src/codemap.md](src/codemap.md)
