# crates/marengo-config/src/

## Responsibility
Serde deserialization, validation helpers, and `resolve_repo_root` for path resolution.

## Design
- `lib.rs`: all config structs, `ConfigError`, load functions, `MotorType`, `DangerZoneRule`
- `safety_validation.rs`: finite numeric/timing policy, unique joint/address identity,
  checked transforms, effective homing settings and full active profile admission
- Cross-file validation: complete joint name/type agreement, hard/soft envelopes,
  URDF existence, motor address uniqueness. Loaders/writers validate borrowed raw
  config; immutable installed generations and schema-key strictness remain open.

## Integration
- Every runtime binary calls loaders before constructing Supervisor/ControlLoop
