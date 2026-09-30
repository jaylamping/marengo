# crates/marengo-homing/src/

## Responsibility
Homing state machine and calibration persistence implementation.

- `registry.rs`: deterministic/explicit history resource binding, typed read/parse errors and fresh `Unhomed` state. Loading historical rows never grants readiness.
- `calibration.rs`: legacy historical row schema. It is not a current-reference receipt.
- `verify.rs`: existing direct verification/persistence path; fresh protocol evidence/private authority remain separate work. Its tests use unique explicit owned resources.

## Integration
- Loaded from `config/homing.yaml` via marengo-config
