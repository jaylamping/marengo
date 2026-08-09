# Research: limb→master URDF merge by actuator identity

**Ticket:** [Research: limb→master URDF merge by actuator identity](https://github.com/jaylamping/marengo/issues/108)  
**Parent map:** [Wayfinder: Consul Config/Setup — URDF SoT completeness gate](https://github.com/jaylamping/marengo/issues/96)  
**Branch:** `research/limb-master-urdf-merge`  
**Builds on:** [#98](https://github.com/jaylamping/marengo/issues/98) (master URDF + archive contract), [#101](https://github.com/jaylamping/marengo/issues/101) (Pi URDF paths), [#99](https://github.com/jaylamping/marengo/issues/99) (v1 mapped fields)

---

## Summary

Config/Setup should treat **`/opt/marengo/assets/urdf/marengo.urdf`** as the durable **master URDF** SoT. Limb/DoF uploads are **fragments** keyed by **actuator identity** (revolute/prismatic joint names, cross-checked against `motors.yaml` CAN mapping at import gate). A kinematics-preserving merge **replaces mapped fields per actuated joint** (and that joint's child-link inertial subtree) inside the master tree while **preserving global topology** — parent link names on the attachment path, unpowered limbs, and joints outside the upload's actuator set stay untouched.

**Bringup profiles** stop pointing at competing slice URDFs (`arm_4dof_right.urdf`, etc.); they keep YAML + `robot.joints` eligibility and set `urdf: assets/urdf/marengo.urdf`. Limb-subset bench operation remains **Testing UI / CLI** via the joint list, not a second URDF file.

Today there is **no merge implementation** — only per-file `resolve_urdf_path`, string-match `joint_in_profile_urdf`, and expand-only limit patches on whichever file `robot.yaml` names ([ADR 0017](../../decisions/0017-bench-set-limits-urdf-expand.md)).

---

## Glossary alignment (CONTEXT.md)

| Term | Merge implication |
|------|-------------------|
| **Master URDF** | Single combined file; all Accept paths write here |
| **Assembly identity** | Match uploads by **joint-name overlap** with master (+ motors CAN map at import gate); filename is storage label only |
| **Import gate** | Zero overlap or ambiguous overlap (e.g. left vs right bare names) → wizard pick or Cancel/re-upload; no sideline live assembly |
| **URDF archive** | Immutable snapshot of each accepted contributor file |
| **URDF staging** | Pre-resolve upload buffer (`staging/`) |
| **Mapped config fields** | Diff/merge projects into per-actuator field schema (kinematics + limits + inertial), not raw XML blobs |
| **Accept = Active** | Per-value merge → hot-reload in-memory `urdf_robot`; async write-behind master + archive entry |

---

## Proposed Pi directory layout

Under `{MARENGO_ROOT}/assets/urdf/` (default `/opt/marengo/assets/urdf/`):

```
assets/urdf/
├── marengo.urdf                 # master SoT — only live assembly file Config/Setup edits
├── staging/                     # upload buffer (pre-resolve); cleared on Accept or Dismiss
│   └── <upload-id>/
│       ├── contributor.urdf     # original bytes as uploaded
│       └── meta.json            # sha256, upload time, detected actuator set, assembly guess
├── archive/                     # accepted contributors — read-only backups, not a second SoT
│   ├── manifest.json            # index: id → actuator_set, archived_at, source_name, sha256
│   └── <upload-id>/
│       └── contributor.urdf     # frozen copy at Accept time
└── seed/                        # optional git gap-fill copies; never clobber divergent master
    └── …                        # deploy seed only (#97); not operator working set
```

**Conventions:**

- **Master filename is fixed:** `marengo.urdf` (matches git seed target and `config/robot_humanoid.yaml`).
- **Archive entries** keyed by opaque `upload-id` (UUID), not limb filename. Store original filename in `meta.json` / manifest for operator display.
- **Staging** from [#97](https://github.com/jaylamping/marengo/issues/97) remains a buffer; operator UX is resolve wizard + field badges ([#98](https://github.com/jaylamping/marengo/issues/98)), not a staging inbox.
- **Git seed** (`assets/urdf/` in repo) bootstraps empty Pi library only; Pi master diverges after first Accept (same hydration contract as [#101](https://github.com/jaylamping/marengo/issues/101)).
- **Meshes** (future): `assets/meshes/{visual,collision}/` beside URDF; archive should record mesh bundle hash when uploads include mesh references.

`pi_read_file` already allowlists `${piRoot}/assets/urdf/` ([`tools/marengo-pi-mcp/src/paths.ts`](../../../tools/marengo-pi-mcp/src/paths.ts)); extend allowlist for `staging/` and `archive/` subpaths.

---

## Actuator identity and import gate

### Primary key

**Actuated joint name** — revolute, continuous, and prismatic joints in URDF document order ([`armee_kinematics::actuated_joint_names`](../../../crates/armee-kinematics/src/lib.rs)).

Secondary validation at import gate (wizard-hard-stop, not warn-only):

- Each uploaded actuated joint name must appear in **master** or be declared **new** with a resolved **attachment parent link** on master.
- Cross-check against active `motors.yaml`: joint name ↔ `(can_interface, device_id)` must not contradict an existing motor row (CAN ID remap = separate wizard flow).

### Assembly matching algorithm

Given upload actuators `J_u`, master actuators `J_m`:

1. `O = J_u ∩ J_m` (name overlap).
2. **Match** if `|O| ≥ 1` and overlap is a **contiguous kinematic chain** on master (each joint in `O` shares the same side prefix, e.g. all `right_*`, or generic bench names `shoulder_pitch`…`elbow`).
3. **Ambiguous** if `O` matches both a left and right chain, or generic bench names overlap humanoid names inconsistently → **wizard pick** target assembly (import gate).
4. **Reject** if `O = ∅` and upload does not include a valid attachment declaration (no automatic graft of unrelated tree).

**Filename is never used** for matching ([CONTEXT.md](../../../CONTEXT.md) **Assembly identity**).

### Example: right-arm bench fragment

Upload `arm_4dof_right.urdf` actuators:

`right_shoulder_roll`, `right_shoulder_pitch`, `right_upper_arm_yaw`, `right_elbow_pitch`

Master humanoid (future CAD export) shares those joint names on the right arm chain → assembly identity = **right arm**, merge scope = those four joints + their child links.

---

## Kinematics-preserving merge algorithm

### Design principle

**Kinematics** = tree topology + joint `parent`/`child` + `origin` + `axis` + link transforms along the actuated chain. **Dynamics envelope** (mass, COM, limits, effort) can change without altering FK structure but affects gravity and Davout — still mapped fields, still per-value Accept.

**Preserve:** master links/joints outside merge scope; attachment joint's **parent link** on master; joint **names**; overall connectivity to `base_link`.

**Replace (on Accept):** mapped fields for joints in merge scope and their **direct child link** inertial/visual blocks (and fixed joints between actuated joints within the fragment).

### Phase 1 — Parse and scope

```
master  ← load_urdf(marengo.urdf)
upload  ← load_urdf(staging/.../contributor.urdf)
J_scope ← actuated_joint_names(upload) after import gate
```

For each `j ∈ J_scope`:

- **Kinematic unit** = joint `j` + `child` link + descendant **fixed** joints/links until the next actuated joint or end of upload chain.
- Verify `j` exists in master (or attachment rules pass for new `j`).

### Phase 2 — Field-level diff (mapped projection)

Project each unit into wizard rows (aligned with [#99](https://github.com/jaylamping/marengo/issues/99) v1 checks):

| Group | URDF source | Kinematics-critical? |
|-------|-------------|----------------------|
| `joint.origin` xyz/rpy | `<joint><origin>` | **Yes** |
| `joint.axis` | `<axis xyz>` (normalized) | **Yes** |
| `joint.type` | revolute vs continuous | **Yes** (type change = conflict) |
| `joint.parent` / `joint.child` | parent/child link names | **Yes** (link rename = conflict) |
| `joint.limit` lower/upper/effort | `<limit>` | No (envelope) |
| `joint.safety_controller` | soft bounds | No |
| `link.inertial` mass, origin, inertia | child link `<inertial>` | No (τ_g uses mass+COM; tensor deferred) |
| `link.visual` / `collision` | child link geometry | No (v1 warn) |

Davout **does not** read URDF velocity for command caps ([ADR 0010](../../decisions/0010-command-velocity-caps.md)); omit URDF `limit velocity` from merge conflicts.

### Phase 3 — Conflict detection

Compare master vs upload per field with tolerances (`1e-6` rad/m for angles/lengths, `1e-9` for mass).

| Conflict | Severity | Default wizard behavior |
|----------|----------|-------------------------|
| **Axis** differs (not negated equivalent on same DOF) | critical | Must pick master or incoming; **no auto-merge** |
| **Origin** xyz/rpy differs | critical | Must pick; affects FK + gravity lever arms |
| **Parent/child link** name differs for same joint | critical | Block auto-merge; operator remap or Cancel |
| **Joint type** differs | critical | Block auto-merge |
| **Hard limits** differ | high | Default **incoming** for CAD refresh; offer **widen-only union** if master wider from Set Limits ([ADR 0017](../../decisions/0017-bench-set-limits-urdf-expand.md)): `lower = min(m,u)`, `upper = max(m,u)` |
| **Mass/COM** differs | high | Default incoming; warn gravity impact |
| **Effort** differs | medium | Default incoming |
| **Soft limits** differ | medium | Default incoming; reconcile with `control.yaml` soft inset separately |
| **Inertia tensor** differs | low (deferred) | Warn only — `UrdfGravityModel` ignores tensor today |
| **Visual/collision** differs | low | Default incoming |

**Negated axis:** treat `axis` and `-axis` as equivalent only when joint origin frame is unchanged; still surface as informational diff.

### Phase 4 — Apply (Accept per value or Accept all)

1. Build in-memory `master'` from `master` by patching accepted fields only (unaccepted → keep master).
2. **Topology guard:** for every `j ∈ J_scope`, walk master' from `j` to `base_link`; parent chain link names must match master's chain except for links inside the replaced child subtrees.
3. **Parse validation:** `load_urdf` round-trip or `urdf_rs` in-memory consistency.
4. **Runtime dry checks** (warn on failure, same as completeness gate):
   - `validate_motors_against_robot` for active profile
   - Davout `build_limits` overlap (URDF hard ∩ motors bench)
   - `armee_dynamics` model builds for profile joints
5. **Memory apply** when motors not Enabled ([#98](https://github.com/jaylamping/marengo/issues/98) gate): hot-reload `Supervisor.urdf_robot` + `rebuild_limits()`.
6. **Disk write-behind** (ADR 0012): atomically write `marengo.urdf`, append `archive/<upload-id>/`, update `archive/manifest.json`. On failure → **persist-degraded Accept** alert.

### Phase 5 — Post-merge limit policy

- **CAD import** of tighter limits: accepted incoming hard limits replace master values (may shrink envelope vs taught bench — operator must re-Set-Limits if needed).
- **Bench Set Limits expand** on master: continue expand-only on **`marengo.urdf`** via existing [`expand_urdf_file_to_cover_motors`](../../../crates/marengo-config/src/urdf_expand.rs); merge must not shrink operator-expanded hard without explicit Accept.
- Recommend tracking per-joint `limit_provenance: {cad|bench_expand}` in manifest sidecar (implementation detail) so wizard can explain widen-only union.

### What this algorithm is not

- **Not** a generic URDF combiner / xacro replacement — scope is Marengo master + limb fragments sharing Marengo joint names per [`hardware/docs/kinematics.md`](../../../hardware/docs/kinematics.md).
- **Not** merging unrelated trees by filename.
- **Not** replacing `motors.yaml` / `control.yaml` — those stay profile YAML; only URDF geometry/envelope merges here (YAML diffs are a parallel wizard track).

---

## Link and joint merge detail

### Links

- **Match key:** link `name`.
- **In scope:** child link of each actuated joint in `J_scope`, plus fixed-intermediate links on the fragment chain.
- **Out of scope:** `base_link`, `torso_link`, pelvis, opposite limb — never overwritten by a single-arm upload.
- **On Accept:** replace `<inertial>`, `<visual>`, `<collision>` blocks for in-scope links; do not rename links unless import gate explicitly maps renames (v2).

### Joints

- **Match key:** joint `name` (actuator identity).
- **On Accept:** replace `<origin>`, `<axis>`, `<limit>`, `<safety_controller>`, `type` when accepted.
- **Fixed joints** inside fragment: merge as part of kinematic unit (preserve internal zero-DOF geometry).
- **Mimic joints:** none in active bench; defer until humanoid export.

### Attachment for new actuators

When upload adds a joint not in master (commissioning new DOF):

1. Operator selects **parent link** on master (e.g. `torso_link` for `right_shoulder_pitch`).
2. Import gate verifies no existing joint already uses that parent→child edge.
3. Graft upload subtree at attachment; still per-value Accept.

---

## Bringup `robot.yaml` — point at master, derive subset in YAML

### Target shape

Every bringup profile should converge to:

```yaml
robot:
  name: marengo_arm_4dof_right   # profile identity unchanged
  urdf: assets/urdf/marengo.urdf # master SoT — same path for all profiles
  joints:
    - right_shoulder_roll        # command-eligible subset only
    - right_shoulder_pitch
    - right_upper_arm_yaw
    - right_elbow_pitch
```

`motors.yaml` lists only wired motors for that profile. Davout already enables/filters by motor map ([`davout::Supervisor::from_repo`](../../../crates/davout/src/lib.rs)); unpowered master joints remain in URDF for Consul FK preview but are not commanded.

### Resolution path (unchanged mechanism, new default target)

[`resolve_urdf_path`](../../../crates/marengo-config/src/lib.rs) stays:

```rust
repo_root.join(&robot.robot.urdf)  // → .../assets/urdf/marengo.urdf
```

No second resolver needed. **Subset selection is not a second URDF** — it is `robot.joints` + `motors.motors`.

### Migration from today's slice URDFs

| Today | Future |
|-------|--------|
| `arm_4dof_right/robot.yaml` → `arm_4dof_right.urdf` | → `marengo.urdf` + same joint list |
| `arm_4dof_left/robot.yaml` → `arm_4dof.urdf` (generic names) | → `marengo.urdf` + `left_*` or generic joints per kinematics doc |
| `arm_3dof_right` regression slice | → `marengo.urdf` + 3-joint list |
| Root `config/robot.yaml` → `arm_4dof.urdf` | → `marengo.urdf` or stay on bringup profile only |

**One-time porting task:** fold slice URDF chains into master (merge algorithm above), normalize joint names to humanoid table in [`hardware/docs/kinematics.md`](../../../hardware/docs/kinematics.md). Until master is populated, profiles may temporarily keep slice paths — seed/bootstrap only.

### `robot_humanoid.yaml`

Already points at `marengo.urdf` ([`config/robot_humanoid.yaml`](../../../config/robot_humanoid.yaml)). Full-body commissioning becomes the default profile with all 23 joints listed.

---

## Gaps vs today's stack

| Area | Today | Gap for master merge |
|------|-------|----------------------|
| **URDF files** | Parallel slice files: `arm_4dof.urdf`, `arm_4dof_right.urdf`, `arm_3dof_right.urdf`, … with **different naming and axis conventions** (e.g. bench `arm_4dof` uses Z axes + rpy offsets; `arm_4dof_right` uses native Y/X/Z) | Must normalize into single `marengo.urdf` before master SoT is truthful |
| **`resolve_urdf_path`** | Path join + exists check only | Sufficient for master path; no merge awareness (correct separation) |
| **`joint_in_profile_urdf`** | `urdf.contains("name=\"{joint}\"")` string hack ([`profile_txn.rs`](../../../crates/marengo-config/src/profile_txn.rs)) | Replace with parsed `actuated_joint_names` against resolved master path |
| **Merge / diff code** | None | New crate module or `marengo-config` URDF merge (parse → diff → patch → serialize) |
| **URDF serialization** | Expand-only XML rewrite for `<limit>` ([`urdf_expand.rs`](../../../crates/marengo-config/src/urdf_expand.rs)) | General patch writer or round-trip via `urdf_rs` + formatter for full merge |
| **Set Limits expand** | Targets profile's `robot.yaml` URDF path | Retarget to **master**; merge widen-only union policy |
| **`profile_content_revision`** | Hashes YAML only, not URDF | Add master URDF hash to gateway CAS or separate revision for Config/Setup |
| **Pi sync** | `pi_sync_bench_urdf` allowlists slice files ([#101](https://github.com/jaylamping/marengo/issues/101)) | Sync `marengo.urdf` + optional `archive/`; deprecate per-slice sync |
| **Consul FK preview** | Build-time import of `arm_3dof_right.urdf` | Fetch active master from gateway (new API per #101) |
| **Completeness gate** | Defined in [#99](https://github.com/jaylamping/marengo/issues/99) research | Run against **master** + profile joint subset |
| **Git seed** | Multiple bench URDFs committed | Keep as **URDF seed** only; reduce over time as master absorbs slices |
| **`marengo.urdf`** | 2-DOF placeholder ([`assets/urdf/marengo.urdf`](../../../assets/urdf/marengo.urdf)) | Must be replaced by merged humanoid skeleton before Config/Setup gate is meaningful |

---

## Recommended implementation sequence

1. **Populate master skeleton** — Brawner export or mechanical merge of bench slices into `marengo.urdf` with canonical joint names from kinematics doc (one-time CAD/engineering task).
2. **`marengo-config::urdf_merge`** — parse both sides, actuator-scoped diff, conflict taxonomy, in-memory patch (unit tests using `arm_4dof_right` → minimal master fixture).
3. **Gateway endpoints** — staging upload, diff JSON for wizard, Accept/per-field, archive write, master hash in snapshot.
4. **Profile migration** — switch `config/bringup/*/robot.yaml` to `marengo.urdf`; retire slice URDFs from active profiles (keep in git seed during transition).
5. **Retarget ADR 0017 expand** — `write_motors_control_and_urdf` always resolves master path from a profile-local or global `robot.yaml` pointer.
6. **Consul** — resolve wizard UI consuming mapped field diffs; warn-only completeness on master.

---

## References

| Source | Role |
|--------|------|
| [CONTEXT.md](../../../CONTEXT.md) glossary | Master URDF, archive, assembly identity, import gate |
| [#98 Resolution](https://github.com/jaylamping/marengo/issues/98#issuecomment-5229860946) | Accept = Active; merge into master; archive contributors |
| [#101 Research](https://github.com/jaylamping/marengo/tree/research/urdf-pipeline-cad-to-pi/docs/research/wayfinder/urdf-pipeline-cad-to-pi.md) | Pi paths, `resolve_urdf_path`, hydration gaps |
| [#99 Research](https://github.com/jaylamping/marengo/tree/research/urdf-v1-sim-completeness-fields/docs/research/wayfinder/urdf-v1-sim-completeness-fields.md) | Mapped field priority for diff projection |
| [hardware/docs/kinematics.md](../../../hardware/docs/kinematics.md) | Canonical joint names, axes, limits |
| [ADR 0017](../../decisions/0017-bench-set-limits-urdf-expand.md) | Expand-only hard limits on active URDF |
| [`crates/armee-kinematics`](../../../crates/armee-kinematics/src/lib.rs) | `load_urdf`, `actuated_joint_names`, limits |
| [`crates/marengo-config/src/urdf_expand.rs`](../../../crates/marengo-config/src/urdf_expand.rs) | Today’s only URDF mutation path |
| [`crates/davout/src/lib.rs`](../../../crates/davout/src/lib.rs) | URDF load at supervisor init, `build_limits` |

---

## Open questions (deferred)

- **Offline actuator pruning** — remove master joints absent from CAN bus for prolonged period (map #96 fog).
- **Left/right generic bench names** (`shoulder_pitch` vs `right_shoulder_pitch`) during migration — import gate alias table vs one-time rename in master.
- **Mesh bundle merge** — when CAD exports include `package://` meshes, archive and merge must handle companion files (deferred past v1 primitives-only Consul).
