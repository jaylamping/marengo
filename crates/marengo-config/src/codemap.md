# crates/marengo-config/src/

## Responsibility
Serde deserialization, validation helpers, and `resolve_repo_root` for path resolution.

## Design
- `lib.rs`: all config structs, `ConfigError`, load functions, `MotorType`, `DangerZoneRule`
- Cross-file validation: joint name sets, URDF existence, motor device_id uniqueness

## Integration
- Every runtime binary calls loaders before constructing Supervisor/ControlLoop
