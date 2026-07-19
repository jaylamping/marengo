# bins/wave-demo/

## Responsibility
**Sine-wave position excitation** demo for bench frequency response testing.

## Design
- Uses Berthier `position_wave` module for periodic setpoints
- Short-lived binary for characterization without full marengo-pi REPL

## Integration
- **Depends on**: berthier, davout, robstride, marengo-config

**Detailed map**: [src/codemap.md](src/codemap.md)
