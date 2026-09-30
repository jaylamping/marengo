# crates/marengo-homing/

## Responsibility
Joint **homing registry** — encoder zero verification, calibration record persistence, homing state per joint.

## Design
- `JointHomingState` tracked by Davout Supervisor; newly constructed registries always start `Unhomed`. Historical rows never grant current readiness, even if every legacy field appears to match.
- `HomingRegistry::new` deterministically joins the supplied root and path. `with_record_path` binds an explicit path as supplied. Environment selection belongs to Supervisor/runtime composition.
- Existing history is retained through `calibration()` and never rewritten by loading. Only `NotFound` means ordinary empty history; directory/encoding/other IO and YAML errors return public typed `RegistryError`.
- Legacy same-process verification still writes records and changes state; correlated current reference, private grant authority and qualified recovery remain follow-up work under ADR0022/CS05/CS06/CS15.
- Homing methods and sensor inputs from `homing.yaml`

## Flow
1. Runtime explicitly selects a history resource and constructs the registry.
2. Existing rows remain historical data; every configured joint starts `Unhomed`.
3. Checked readiness refuses until a reference is granted within that process. This slice adds no qualified grant mechanism.
4. A later registry reconstruction retains history and again starts `Unhomed`. The former separate-process Set Zero/home/enable workflow no longer conveys readiness.

## Verification and limits
- Public history-admission tests retain immutable bytes and reject exact/mismatched startup grants, malformed/directory/non-UTF8 resources, and restart reuse. Missing-file control remains empty/non-authorizing.
- Real persistence is tested through the existing legacy API, followed by reconstruction refusal; this does not qualify physical SetZero or cached-pose evidence.
- Shared test directories are exclusively created with unique per-case paths and kept ownership guards. Explicit bindings avoid global environment mutation; library environment precedence is tested only in a child process. No added dependencies.

## Integration
- **Consumed by**: `davout::Supervisor`, `bins/motor-repl`, `bins/marengo-pi`

**Detailed map**: [src/codemap.md](src/codemap.md)
