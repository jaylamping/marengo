# marengo-config

Typed loaders for master `config/*.yaml`.

## Flow

1. Bin sets `MARENGO_CONFIG_DIR` or passes `--config-dir`
2. `resolve_config_dir` → `MARENGO_CONFIG_DIR`, else `/opt/marengo/config` whenever that
   directory exists (also on a dev host), else `<repo>/config`
3. Validate each file's numeric/identity policy; `validate_safety_config` checks
   complete active joint/motor/control/homing agreement and cross-file envelopes.
   Resolve URDF path; raw config is still mutable after validation.
4. Profile txn / URDF expand target master paths only (no bringup CAS)

## Modules

| File | Role |
|------|------|
| `lib.rs` | YAML structs, loaders, validation |
| `safety_validation.rs` | Shared numeric, identity, timing and full-profile admission |
| `config_revision.rs` | `profile_content_revision` CAS hash |
| `profile_txn.rs` | Master YAML atomic writes (validate-then-commit) |
| `urdf_expand.rs` | Expand-only URDF hard envelope (ADR 0017) |
| `bench_joints.rs` | Command joint allowlist from `robot.joints` |
| `completeness.rs` | Warn-only hardware completeness v1 |
| `urdf_merge.rs` | Joint-keyed URDF merge preview + apply |
| `limit_patch.rs` | `LimitPatch` validation and apply to motors/control entries |
| `commissioning_scope.rs` | Persisted commissioning scope (`var/commissioning-scope.yaml`) |
| `atomic_file.rs` | `ProfileWriteLock`, unique-temp atomic writes |

`profile_content_revision` hashes the canonical robot/motors/control/homing YAML
plus the bytes of the URDF `robot.yaml` names, with SHA-256. It resolves that URDF under
the caller's repo root, the same path `resolve_urdf_path` gives the writers. Profile writes use same-directory atomic replacement and a shared
profile lock; replacement is per-file, not a crash-atomic multi-file transaction.
