# Research: URDF pipeline from CAD to Pi directory

**Ticket:** [Research: current URDF pipeline from CAD to Pi directory](https://github.com/jaylamping/marengo/issues/101)  
**Parent map:** [Wayfinder: Consul Config/Setup — URDF SoT completeness gate](https://github.com/jaylamping/marengo/issues/96)  
**Branch:** `research/urdf-pipeline-cad-to-pi`

---

## Summary

Marengo has **two URDF tracks** today:

1. **Full humanoid (future SoT):** manual SolidWorks/Brawner export → `assets/urdf/marengo.urdf` (+ meshes under `assets/meshes/`). MCP audits readiness/postcheck; no automated writer. Milestone M3 in [`docs/roadmap.md`](../../roadmap.md).
2. **Bench slices (active runtime):** hand-maintained URDF files under `assets/urdf/` referenced by `config/bringup/<profile>/robot.yaml`. These are **provisional discovery envelopes** (see comments in e.g. `arm_4dof_right.urdf`), not CAD exports.

**Pi runtime** resolves URDF as `MARENGO_ROOT` + `robot.yaml` → `urdf` path (typically `/opt/marengo/assets/urdf/<file>.urdf`). Full-tree deploy (`deploy-pi.sh` / `pi_sync_main`) copies all of `assets/` to the Pi; **config-only sync does not touch URDF** (`pi_sync_bench_config`). Targeted URDF push exists (`pi_sync_bench_urdf`) but covers a **partial, stale filename list**.

**Consul today** does not read the Pi URDF directory. Dashboard FK preview bakes `arm_3dof_right.urdf` at build time ([`consul/src/assets/urdf/shoulder-pitch-right-only.ts`](../../../consul/src/assets/urdf/shoulder-pitch-right-only.ts)). Gateway `/config/snapshot` exposes YAML-derived limits only — **no URDF path, content, or directory listing**. A future Config/Setup page would need new gateway APIs (or file-serving) to default-read `/opt/marengo/assets/urdf/`.

**Hydration is unreliable** because: (a) CAD→git is manual; (b) deploy can clobber Pi-side Set Limits URDF expands ([ADR 0017](../../decisions/0017-bench-set-limits-urdf-expand.md)); (c) `profile_content_revision` ignores URDF bytes; (d) `pi_sync_bench_urdf` defaults reference a nonexistent file; (e) `install-pi.sh` uses non-deleting `rsync` for assets; (f) no CI/Actions step pushes URDF to Pi.

---

## Pipeline stages (manual vs automated)

| Stage | What happens | Automated? | Key paths / tools |
|-------|----------------|------------|-------------------|
| **CAD modeling** | SolidWorks under local `cad/` (gitignored binaries) | No | [`cad/README.md`](../../../cad/README.md), `cad/assemblies/marengo.SLDASM` |
| **Design audit** | MCP checks conventions, hardware coverage, URDF readiness | Partial (audit-only) | `marengo_design_review`, `marengo_urdf_readiness` — [`.cursor/rules/solidworks-mcp.mdc`](../../../.cursor/rules/solidworks-mcp.mdc), [`docs/onboarding.md`](../../onboarding.md) §8 |
| **URDF export** | Brawner/sw2urdf manual export | **Manual** | Target: `assets/urdf/marengo.urdf`; MCP `marengo_urdf_export_postcheck` after export |
| **Export helper script** | Prints workflow, verifies URDF exists, ensures mesh dirs | Stub/check only | [`scripts/export-urdf.sh`](../../../scripts/export-urdf.sh) |
| **Mesh export** | Copy visual/collision STLs to `assets/meshes/` | **Manual** | [`assets/meshes/README.md`](../../../assets/meshes/README.md) — empty until M3 |
| **MJCF / sim** | Hand-maintained MJCF + DOF match test | **Manual** | [`scripts/urdf-to-mjcf.sh`](../../../scripts/urdf-to-mjcf.sh) |
| **Validation** | `cargo test` on kinematics, sim-harness, marengo-config | **Automated** (CI) | [`scripts/validate-urdf.sh`](../../../scripts/validate-urdf.sh) → [`scripts/check.sh`](../../../scripts/check.sh) |
| **Bench URDF authoring** | Edit slice URDFs (limits, mass/COM) in git | **Manual** | e.g. `assets/urdf/arm_4dof_right.urdf` |
| **Profile binding** | `robot.yaml` `urdf:` field per bringup profile | Config in git | [`config/bringup/*/robot.yaml`](../../../config/bringup/) |
| **Deploy to Pi** | Cross-build + rsync staging + optional `install-pi.sh` | **Automated** (on demand) | [`scripts/deploy-pi.sh`](../../../scripts/deploy-pi.sh) |
| **Config-only sync** | Rsync bringup YAML to Pi | **Automated** (MCP) | `pi_sync_bench_config` — **URDF excluded** |
| **URDF-only sync** | Rsync selected URDF files to Pi | **Automated** (MCP, partial list) | `pi_sync_bench_urdf` in [`tools/marengo-pi-mcp/src/tools/sync-config.ts`](../../../tools/marengo-pi-mcp/src/tools/sync-config.ts) |
| **Runtime load** | `resolve_urdf_path(MARENGO_ROOT, robot)` at boot | Automatic at process start | [`crates/marengo-config/src/lib.rs`](../../../crates/marengo-config/src/lib.rs) (`resolve_urdf_path`, `resolve_repo_root`) |
| **Set Limits expand** | In-memory URDF widen + async write-behind to disk | **Automated** (bench) | [ADR 0017](../../decisions/0017-bench-set-limits-urdf-expand.md), [`bins/marengo-pi/src/limit_persist.rs`](../../../bins/marengo-pi/src/limit_persist.rs) |
| **Local git sync after Durable** | `marengo-limit-sync` + loopback server | Optional / best-effort | [`bins/marengo-limit-sync`](../../../bins/marengo-limit-sync), [`tools/limit-sync-local/server.mjs`](../../../tools/limit-sync-local/server.mjs) |
| **Consul visualization** | Static import of repo URDF at Vite build | Build-time only | [`consul/src/components/dashboard/overview/dashboard-overview.tsx`](../../../consul/src/components/dashboard/overview/dashboard-overview.tsx) |
| **Consul config snapshot** | Gateway reads YAML from active `MARENGO_CONFIG_DIR` | Live read | `GET /config/snapshot` — [`bins/marengo-gateway/src/config.rs`](../../../bins/marengo-gateway/src/config.rs) |

**SolidWorks MCP explicitly does not write URDF** (audit-first; no URDF writer per [`.cursor/rules/solidworks-mcp.mdc`](../../../.cursor/rules/solidworks-mcp.mdc)).

---

## Repo paths

### URDF files (committed)

All under [`assets/urdf/`](../../../assets/urdf/):

| File | Typical profile |
|------|-----------------|
| `marengo.urdf` | Full humanoid target (M3); root `config/robot.yaml` does **not** point here |
| `arm_4dof.urdf` | [`config/bringup/arm_4dof_left/robot.yaml`](../../../config/bringup/arm_4dof_left/robot.yaml), root [`config/robot.yaml`](../../../config/robot.yaml) |
| `arm_4dof_right.urdf` | [`config/bringup/arm_4dof_right/robot.yaml`](../../../config/bringup/arm_4dof_right/robot.yaml) — **active bench default** |
| `arm_3dof_right.urdf` | [`config/bringup/arm_3dof_right/robot.yaml`](../../../config/bringup/arm_3dof_right/robot.yaml); also baked into Consul overview |
| `shoulder_pitch_dual.urdf` | `shoulder_pitch_dual` profile |
| `shoulder_pitch_left_bare.urdf` | `shoulder_pitch_left_only` profile |
| `shoulder_pitch_right_bare.urdf` | (no dedicated bringup profile in tree) |
| `shoulder_pitch_weighted.urdf` | `shoulder_pitch_weighted` profile |

Bench URDFs use primitive geometry (cylinders) — **no mesh filenames** in active slice files. Full export path expects [`assets/meshes/visual/`](../../../assets/meshes/) and `collision/` ([`assets/meshes/README.md`](../../../assets/meshes/README.md)).

### Config → URDF binding

Each bringup profile's [`robot.yaml`](../../../config/AGENTS.md) sets a **repo-relative** path:

```yaml
urdf: assets/urdf/arm_4dof_right.urdf
```

Resolved at runtime: `PathBuf::from(MARENGO_ROOT).join(robot.robot.urdf)` ([`resolve_urdf_path`](../../../crates/marengo-config/src/lib.rs)).

### Sim fixtures

[`sim/fixtures/minimal.urdf`](../../../sim/fixtures/minimal.urdf) — test-only, not deployed.

---

## Pi paths

| Location | Role |
|----------|------|
| `~/marengo/` | Deploy **staging** tree (`MARENGO_PI_STAGING_ROOT`, default `~/marengo`) — receives `deploy-pi.sh` rsync |
| `/opt/marengo/` | **Install root** (`MARENGO_ROOT` in [`scripts/env.example`](../../../scripts/env.example)) |
| `/opt/marengo/assets/urdf/` | Runtime URDF directory (mirror of repo `assets/urdf/`) |
| `/opt/marengo/config/bringup/<profile>/` | Active profile YAML |
| `/etc/marengo/env` | `MARENGO_ROOT`, `MARENGO_CONFIG_DIR` — loaded by [`scripts/systemd/marengo-pi.service`](../../../scripts/systemd/marengo-pi.service) |

**Default active profile on Pi** ([`scripts/env.example`](../../../scripts/env.example)):

```
MARENGO_ROOT=/opt/marengo
MARENGO_CONFIG_DIR=/opt/marengo/config/bringup/arm_4dof_right
```

→ Active URDF file: **`/opt/marengo/assets/urdf/arm_4dof_right.urdf`**

`marengo-pi` fails fast at boot if that file is missing ([`bins/marengo-pi/src/main.rs`](../../../bins/marengo-pi/src/main.rs) calls `resolve_urdf_path`).

**Two-tree pattern:** deploy lands in `~/marengo`; `install-pi.sh` (sudo) copies into `/opt/marengo`. MCP sync tools can write staging only (`install_to_opt: false`) or attempt direct writes to `/opt/marengo/assets/urdf/`.

---

## Deploy / sync mechanisms

### Full deploy (`deploy-pi.sh` / `pi_sync_main`)

1. Cross-build Pi binaries ([`scripts/deploy-pi.sh`](../../../scripts/deploy-pi.sh)).
2. Stage `config/`, **`assets/`** (includes all URDF), `scripts/`, Consul `www/`.
3. `rsync --delete` staging → `user@host:~/marengo/`.
4. Optional `--install` → remote `sudo install-pi.sh`:
   - `rsync -a --delete` for `config/`
   - **`rsync -a` (no `--delete`) for `assets/`** — stale URDF files can remain on Pi
   - Binaries → `/opt/marengo/bin/`

Cloud equivalent: [`scripts/pi-remote.sh deploy --install`](../../../scripts/pi-remote.sh).

### Config-only sync (`pi_sync_bench_config`)

Rsyncs `config/bringup/<profile>/` only. Documented explicitly: **does not sync `assets/urdf/`** ([`AGENTS.md`](../../../AGENTS.md), MCP tool description in [`tools/marengo-pi-mcp/src/tools/admin.ts`](../../../tools/marengo-pi-mcp/src/tools/admin.ts)).

### URDF-only sync (`pi_sync_bench_urdf`)

Rsyncs **explicit filenames** from local `assets/urdf/` to Pi staging, then optionally `install` into `/opt/marengo/assets/urdf/`.

Allowlisted assets in code ([`sync-config.ts`](../../../tools/marengo-pi-mcp/src/tools/sync-config.ts)):

```typescript
const benchUrdfAssets = [
  "shoulder_pitch_right_only.urdf",  // ⚠ not in repo — see Gaps
  "shoulder_pitch_weighted.urdf",
  "shoulder_pitch_left_bare.urdf",
  "arm_4dof_right.urdf",
  "arm_3dof_right.urdf",
] as const;
```

Default MCP call syncs `shoulder_pitch_right_only.urdf` + `shoulder_pitch_weighted.urdf` — **first file does not exist** in `assets/urdf/`.

Missing from allowlist: `arm_4dof.urdf`, `marengo.urdf`, `shoulder_pitch_dual.urdf`, `shoulder_pitch_right_bare.urdf`.

### GitHub Actions

[`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) runs `scripts/check.sh` which calls `validate-urdf.sh` (repo-side tests only). **No workflow deploys or hydrates Pi URDF directory.**

### Set Limits write-back

Consul Apply → gateway → `marengo-pi` expands URDF on disk under the active profile path ([ADR 0017](../../decisions/0017-bench-set-limits-urdf-expand.md)). This creates **Pi-local URDF drift** vs git until `marengo-limit-sync` or manual pull/sync.

---

## Gaps / unreliability

1. **CAD → git is fully manual.** No script or MCP exports URDF; [`export-urdf.sh`](../../../scripts/export-urdf.sh) documents steps only.

2. **`pi_sync_bench_urdf` default is broken.** Default asset `shoulder_pitch_right_only.urdf` is not in the repo (actual bare right file is `shoulder_pitch_right_bare.urdf`; 3-DOF profile uses `arm_3dof_right.urdf`).

3. **Partial URDF allowlist.** Active profile `arm_4dof_right.urdf` is listed, but `arm_4dof.urdf`, `marengo.urdf`, and dual-pitch URDF are omitted from MCP sync enum.

4. **Config sync ≠ URDF sync.** Editing URDF locally and running `pi_sync_bench_config` leaves Pi URDF stale ([`AGENTS.md`](../../../AGENTS.md)).

5. **Deploy can clobber taught limits.** `pi_sync_main` / deploy from a git checkout that lacks Pi-side Set Limits expands overwrites URDF ([ADR 0017](../../decisions/0017-bench-set-limits-urdf-expand.md) consequence).

6. **CAS revision ignores URDF.** `profile_content_revision` hashes only `robot.yaml`, `motors.yaml`, `control.yaml`, `homing.yaml` — not the URDF file ([`bringup_presets.rs`](../../../crates/marengo-config/src/bringup_presets.rs)). Gateway CAS can accept YAML transactions while URDF on disk diverges.

7. **Non-deleting assets install.** [`install-pi.sh`](../../../scripts/install-pi.sh) line 76: `rsync -a` without `--delete` for `assets/` — removed/renamed URDFs may linger on Pi.

8. **Staging vs `/opt` split.** Failed or skipped `install_to_opt`, or writing only `~/marengo`, leaves runtime reading `/opt/marengo` out of date unless `pi_install_staging` runs.

9. **Consul ≠ Pi URDF.** Overview FK uses build-time `arm_3dof_right.urdf` while Pi may run `arm_4dof_right` — subtitle/URDF mismatch noted in Consul critique docs.

10. **No gateway URDF API.** Routes in [`bins/marengo-gateway/src/http.rs`](../../../bins/marengo-gateway/src/http.rs): `/config/snapshot`, `/config/profiles`, profile snapshots — **none expose URDF path, listing, or body**. `ConfigSnapshotJson` has no `urdf_path` field ([`config.rs`](../../../bins/marengo-gateway/src/config.rs)).

11. **Meshes not hydrated.** `assets/meshes/` empty; bench URDFs don't reference mesh files; Consul [`README.md`](../../../consul/README.md) still notes visualization upcoming.

12. **Two URDF semantics.** `marengo.urdf` (CAD SoT target) vs bench slice files (discovery envelopes, expandable via Set Limits) — operators must not treat bench files as CAD-complete.

---

## Implications for Consul Config/Setup default directory

For [Wayfinder #96](https://github.com/jaylamping/marengo/issues/96):

| Question | Answer today |
|----------|--------------|
| **Default directory to show** | `{MARENGO_ROOT}/assets/urdf/` → `/opt/marengo/assets/urdf/` on a standard Pi install |
| **Active file** | Parse active profile's `robot.yaml` from `MARENGO_CONFIG_DIR` (gateway already resolves this for YAML); join `urdf` field with `MARENGO_ROOT` — same as [`resolve_urdf_path`](../../../crates/marengo-config/src/lib.rs) |
| **How Consul would read it** | **No existing API.** Needs new gateway endpoints (list directory, GET/PUT URDF by relative path, active-file indicator) or static file serving under auth |
| **Git vs Pi gap** | File in git reaches Pi only via full deploy, `pi_sync_bench_urdf`, or `pi_sync_main`; config-only sync does not; Set Limits can mutate Pi URDF without git |
| **Completeness gate inputs** | Gateway can check `joint_in_profile_urdf` ([`profile_txn.rs`](../../../crates/marengo-config/src/profile_txn.rs)) for joint membership; sim-impact fields (mass, COM, inertia) require parsing URDF server-side — not in snapshot today |
| **Hydration recommendation** | Config/Setup should treat **`/opt/marengo/assets/urdf/`** as the operator-facing directory, mark the active profile's file, warn on git/Pi revision skew (URDF not in CAS hash), and not assume GitHub Actions hydration (none exists) |

**Bottom line:** The Pi *can* read URDF from a well-defined directory today, but Consul *cannot* default-read it without new gateway surface area. Hydration from CAD/git to that directory remains manual and easy to desync.

---

## References

- [`docs/onboarding.md`](../../onboarding.md) §8 — CAD/MCP URDF workflow  
- [`cad/README.md`](../../../cad/README.md) — export workflow, URDF reference geometry  
- [`docs/decisions/0017-bench-set-limits-urdf-expand.md`](../../decisions/0017-bench-set-limits-urdf-expand.md)  
- [`docs/decisions/0012-config-db-overrides.md`](../../decisions/0012-config-db-overrides.md)  
- [`tools/marengo-pi-mcp/README.md`](../../../tools/marengo-pi-mcp/README.md) — sync tool behavior  
- [`config/AGENTS.md`](../../../config/AGENTS.md) — profile selection via `MARENGO_CONFIG_DIR`
