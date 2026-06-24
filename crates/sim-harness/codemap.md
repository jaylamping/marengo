# crates/sim-harness/

## Responsibility
**Simulation test harness** for MuJoCo or synthetic bus integration tests without hardware.

## Design
- Provides test fixtures and harness utilities for cross-crate integration tests
- Works with `sim/fixtures/` URDF models and `MemoryBus` for CAN-free validation

## Integration
- **Used by**: crate integration tests, CI sim targets

**Detailed map**: [src/codemap.md](src/codemap.md)
