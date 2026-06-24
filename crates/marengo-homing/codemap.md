# crates/marengo-homing/

## Responsibility
Joint **homing registry** — encoder zero verification, calibration record persistence, homing state per joint.

## Design
- `JointHomingState` tracked by Davout Supervisor
- Calibration records written after successful `set-zero` (motor-repl / pi commands)
- Homing methods and sensor inputs from `homing.yaml`

## Flow
1. Operator positions arm at mechanical zero
2. `motor-repl set-zero` or `marengo-pi set-zero` → robstride SetZero frame via Davout
3. Verify |pos| < tolerance → persist calibration record
4. `homing-status` reads registry state

## Integration
- **Consumed by**: `davout::Supervisor`, `bins/motor-repl`, `bins/marengo-pi`

**Detailed map**: [src/codemap.md](src/codemap.md)
