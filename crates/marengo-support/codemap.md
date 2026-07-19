# crates/marengo-support/

## Responsibility
Shared **initialization helpers** — tracing setup, workspace lint policy overrides, repo root resolution utilities.

## Design
- `init_tracing()`: subscriber with env-filter, used by all bins at startup
- `#![forbid(unsafe_code)]` workspace policy enforced here and re-exported patterns

## Integration
- **Consumed by**: all `bins/*` entry points

**Detailed map**: [src/codemap.md](src/codemap.md)
