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
- `gravity-fit` requires no database: it fits right-arm link inertials from immutable
  `pi_gravity_calibrate` evidence, requires explicit output and local-URDF paths, checks
  fused sessions' effective calibration windows, and writes dated records plus accepted
  proposed URDF/patch files only under the output directory (never applied).

## Integration
- **Depends on**: marengo-store, marengo-candump, armee-dynamics, marengo-config
- **Used by**: bench debugging, MCP log tools, MCP `pi_gravity_calibrate` (workstation fit)

**Detailed map**: [src/codemap.md](src/codemap.md)
