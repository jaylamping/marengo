# bins/probe/

## Responsibility
Low-level **CAN and hardware probe** for diagnostics outside the Davout safety path.

## Design
- Direct CAN frame inspection for bring-up debugging
- Not for production motion — use motor-repl for supervised commands

**Detailed map**: [src/codemap.md](src/codemap.md)
