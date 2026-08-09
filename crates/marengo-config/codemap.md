# marengo-config

Typed loaders for master `config/*.yaml`.

## Flow

1. Bin sets `MARENGO_CONFIG_DIR` or passes `--config-dir`
2. `resolve_config_dir` → `/opt/marengo/config` on Pi, else `<repo>/config` in dev
3. Validate motors ⊆ `robot.joints`, resolve URDF path
4. Profile txn / URDF expand target master paths only (no bringup CAS)

## Modules

| File | Role |
|------|------|
| `lib.rs` | YAML structs, loaders, validation |
| `config_revision.rs` | `profile_content_revision` CAS hash |
| `profile_txn.rs` | Limit upsert, master YAML atomic writes |
| `urdf_expand.rs` | Expand-only URDF hard envelope (ADR 0017) |
| `bench_joints.rs` | Command joint allowlist from `robot.joints` |
| `completeness.rs` | Warn-only hardware completeness v1 |
| `urdf_merge.rs` | Joint-keyed URDF merge preview + apply |
