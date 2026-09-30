# crates/marengo-homing/

## Responsibility
Joint **homing registry** — encoder zero verification, calibration record persistence, homing state per joint.

## Design
- `JointHomingState` tracked by Davout Supervisor; newly constructed registries always start `Unhomed`. Historical rows never grant current readiness, even if every legacy field appears to match.
- `HomingRegistry::new` deterministically joins the supplied root and path. `with_record_path` binds an explicit path as supplied. Environment selection belongs to Supervisor/runtime composition.
- Existing history is retained through `calibration()` and never rewritten by loading. Only `NotFound` means ordinary empty history; directory/encoding/other IO and YAML errors return public typed `RegistryError`.
- Legacy scalar validation still records history/local policy state; it cannot grant Davout's private current-reference authority. Arbitrary state setters are crate-private and the synthetic bench-grant method is removed. Qualified reference/recovery remains follow-up work under ADR0023/CS05/CS06.
- Homing methods and sensor inputs from `homing.yaml`

## Flow
1. Runtime explicitly selects a history resource and constructs the registry.
2. Existing rows remain historical data; every configured joint starts `Unhomed`.
3. Registry-local state is not live permission. Davout independently gates Ready, scoped/normal Enable and output; the installed adapter has no qualified reference capability and refuses before arming.
4. A later registry reconstruction retains history and again starts `Unhomed`. The former separate-process Set Zero/home/enable workflow no longer conveys readiness.

## Verification and limits
- Public history-admission tests retain immutable bytes and reject exact/mismatched startup grants, malformed/directory/non-UTF8 resources, and restart reuse. Missing-file control remains empty/non-authorizing.
- Real persistence is tested through the existing legacy API, followed by reconstruction refusal; this does not qualify physical SetZero or cached-pose evidence.
- Invalid scalar pose/bounds/tolerance/offset, reversed/empty bounds, joint mismatch and unsupported methods return before local state/history mutation. The identical 22-case rejection matrix fails on the exact unchanged baseline and passes after validation.
- Shared test directories are exclusively created with unique per-case paths and kept ownership guards. Explicit bindings avoid global environment mutation; library environment precedence is tested only in a child process. No added dependencies.

## Integration
- **Consumed by**: `davout::Supervisor`, `bins/motor-repl`, `bins/marengo-pi`

**Detailed map**: [src/codemap.md](src/codemap.md)
