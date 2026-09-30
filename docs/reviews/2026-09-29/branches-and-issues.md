# Branch PR and issue consolidation review

Audited on September 29, 2026 against main at **`4bc77ba605834fdec04b436daa4bec67bca84fbb`**. The evidence includes all fetched references, PR history and head OIDs, open PR/issue snapshots and comments, branch-tip contents, and the current implementation. The audit itself changed no refs, GitHub PRs/issues, production source, or robot state.

The detailed PR and issue tables describe their **open state at the audit baseline** and the review recommendations. The confirmed action ledger below records later consolidation; its status takes precedence over the baseline tables. The [repository review](../2026-09-29-repository-review.md) records the final consolidated outcome. Historical commissioning comments establish where work stopped, not today's hardware state.

## Confirmed consolidation actions

These actions were completed during the repository review. Retained branches are preserved recovery/design references; the linked GitHub records provide final CI and merge state.

| Item | Confirmed action | Remaining work |
|------|------------------|----------------|
| [PR #109](https://github.com/jaylamping/marengo/pull/109) | Closed as superseded; branch retained. | Useful rendering patterns remain available as design reference; F15 is unresolved. |
| [PR #117](https://github.com/jaylamping/marengo/pull/117) | Closed as superseded; branch retained. | Its glossary had already landed through PR #119. Readiness contract discrepancies remain review follow-up. |
| [PR #107](https://github.com/jaylamping/marengo/pull/107) | Reconciled against current main, independently reviewed and updated onto the verified maintenance baseline, with the final preservation ledger. | Final checks and merge state are on the linked PR record. |
| [Issue #118](https://github.com/jaylamping/marengo/issues/118) | Closed as implemented by PR #119. | Current findings and incomplete physical commissioning remain outstanding. |
| [Issue #115](https://github.com/jaylamping/marengo/issues/115) | Body updated with later #170 Reference/sign/quiet arm-down evidence and remaining elevated/ladder/Wave work; closed as superseded. | This closure does not establish full gravity-compensation or current-reference sign-off. |
| Historical PR #41 CAD payload | Retrieved from archived commit `1b31138`; size **1,707,374 bytes** and SHA256 **3e401d6748429e0015c359e072436f66e2a5dbc54d0180639b6e8d26a269fb65** verified. Preserved as a versioned historical CAD recovery record. | Current local bracket remains unchanged; no mechanical equivalence was inferred. |
| Other references and orphan features | No additional refs deleted and no orphan feature branch merged. | Follow the semantic verdicts below before any later cleanup. |
| Issues #96, #150, #170, #176 and #63 | Retained open. | Acceptance, commissioning and deferred feature work remain as described below. |

## Audit recommendations

- **Close PR #117 as superseded:** all nine added glossary rows are byte-for-byte present on main, absorbed by merged PR #119.
- **Close PR #109 as superseded prototype work:** shipped Hardware already has table-first/optional 3D, import/settings, facets, and scope. Preserve its design variants in the all-ref recovery bundle; production routes/packages already implement the chosen design.
- **PR #107 is useful only after rewriting its claims to match the implemented workflow.** Current URDF Accept promotes a disk file and requires restart, not arbitrary memory-first hot reload. Keep main's already-landed commissioning glossary.
- **Close issue #118 as implemented**, citing PR #119 and its historical verify report. Track current review defects separately; closing an implementation ticket does not declare the entire subsystem safe or physically commissioned.
- **Issue #115 can be closed as superseded narrowly by the later #170 Reference/sign/quiet GravityComp evidence**, with an explicit link to remaining elevated/ladder work. There is no new pass comment on #115 itself. Do not claim full G-comp sign-off.
- **Keep #96, #150, #170, #176, and #63 open.** #96 has an unaccepted end-to-end destination and documented runtime/persistence gaps; commissioning is incomplete; Wave remains locked; WebGPU is deferred.
- **Do not blindly merge the old Auto Learn branch or restore old CAD geometry.** It flips Wave true on outdated evidence and adds substantial new Pi service/proxy behavior. Closed PR #41 contains a unique historical **LFS pointer**. Its payload was initially absent from three inspected local caches and has now been retrieved and verified separately.
- **The initial 47 “merged via PR history” label hides four tip/head mismatches.** Two are already subsumed, but the other two contain genuine post-merge work that must be preserved/triaged before branch cleanup.

## Inventory and preservation limits

The supplied inventory contains 104 references: 40 ancestor-of-main, 47 merged-via-PR-history, four closed-unmerged, ten orphan, three open-PR. It includes the remote-default alias `origin`; that is not a normal feature branch to delete.

`marengo-all-refs.bundle` preserves the Git history/ref snapshots. It does **not** by itself preserve LFS binary bodies. Preserve the bundle, source/local CAD copies, this audit, and any fetched historical LFS payloads before removing references. A ref having a merged PR with the same branch name is insufficient evidence that its current tip was merged.

## Ten orphan branches

All seven research branches are documentation-only. Their files are absent from main, so their actual research text is unique, even when later issue resolutions/implementation absorbed conclusions. Several files still read as current instructions despite being dated snapshots; preserve and label/reconcile them before landing.

| Branch / tip | Semantic verdict | Safe action |
|-------------|------------------|-------------|
| `research/active-mit-modes-acceptance-gates` — `97caaef` | Useful August 11 inventory, superseded in important places: says TorqueOnly aliases GravityComp, says Disabled emits no batch, and links retired `docs/bench-*` suites. Main PR #166 implemented independent torque-command latching; Active+Disabled sends zero keepalives. | Preserve as historical research or refresh only this document with an explicit snapshot/superseded banner and links to the locked playbook. Do not promote its old mode table as current policy. |
| `research/actuator-velocity-baselines-24v` — `d3d341a` | Useful manual-source calculations and a reported protocol-scale discrepancy. Its advice to write 9.4/19.3/14.8 rad/s into live caps before the ladder contradicts the current locked playbook (`effective=min(% baseline, live safety ceiling)`, no safety-cap increase). Linear voltage scaling is expressly a proposed model, not a manufacturer guarantee. | Preserve calculations/source attribution; remove or clearly supersede the cap-raising instructions before merging. Keep physical baselines separate from actual legal command ceilings. Verify per-model/firmware MIT scales independently before changing the driver. |
| `research/harness-coverage-playbook-spine` — `1b17f9e` | Useful gap inventory: smoke scripts are not full commissioning gates. Contains old TorqueOnly-alias and profile/suite references. Current playbook still has TODO automation hooks, so much of the gap analysis remains relevant. | Preserve dated evidence; refresh terminology/links and alias claim if restoring into main. Do not equate smoke pass with chapter completion. |
| `research/limb-master-urdf-merge` — `5e6e8ad` | Historical design proposal; says no implementation exists, proposes memory-first hot reload, old bringup paths, and treats a negated axis as equivalent. Current `urdf_merge` and gateway import exist and differ: supported joint-field merging, disk promotion, restart-required. Negating axis changes coordinate semantics and cannot generally be declared harmless. | Preserve as a dated design proposal with a superseded-by-implementation note. Revise unsafe axis equivalence and current-status/hot-reload claims before using as an operational design doc. |
| `research/right-arm-taught-envelope-caps` — `5f37018` | Useful factual August 11 snapshot with provisional DOF5 and taught DOF1–4 values. Does not establish today's robot state. Includes an overstrong statement that velocity-only danger-zone clipping caps actual gravity descent; current zero-kd gravity mode does not produce braking from clipping an already-zero velocity setpoint. | Preserve unchanged numeric snapshot/provenance, explicitly label it historical, link current config/playbook and review finding CS14. Do not treat provisional firmware/sign/ROM as commissioned. |
| `research/urdf-pipeline-cad-to-pi` — `cc9a9f2` | Contains two useful August 9 research files. Many reported gaps have since changed: master 5-DOF, gateway URDF API/import/archive, master config cutover, and deploy preservation. Code/file links include retired paths. | Preserve both dated files; add current-status/retired-path notes or fold durable lessons into architecture docs. Avoid unedited merging of stale pipeline statements. |
| `research/urdf-v1-sim-completeness-fields` — `0505f2a` | **Exact duplicate final document contents** of the previous branch for both files (different commit histories). Blob IDs: pipeline `a56c2c44b672a2814d2908a62f930a061186bbea`, checklist `fd3f7a4dadf13c27854ebd3bd117e17d10f2d7df` on both tips. | Consolidate one copy of each file; retire the duplicate ref after preserving the bundle. No second merge is needed. |
| `prototype/hardware-page` — `3cfae48` | Throwaway A/B/C UI variants, explicitly labeled not production. Ancestor/earlier subset of open PR #109's prototype evolution; main now ships the chosen Hardware surface. | Archive design reference; retire without merging old prototype routes/package changes. |
| `prototype/limb-commissioning-playbook-stub` — `9b44cdf` | Explicit throwaway, “do not run as procedure yet.” Superseded by locked [docs/commissioning/limb-playbook.md](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/docs/commissioning/limb-playbook.md) from PR #169 plus later amendments; its baseline/caps wording is stale. | Archive or retain in historical reference, not as a second operational playbook. Retire branch after preservation. |
| `feat/auto-learn-pi-bff` — `a3d997b` | Genuine unmerged feature package: authenticated loopback proxy, Pi Node/service install, agent cwd changes, UI tokens/API/tests; also a cosmetic zero-icon commit and an old forearm mass/Wave unlock commit. Main has the earlier local-only Auto Learn tool, not this Pi proxy/service cutover. The branch sets `WAVE_POSE_GCOMP_SIGNED=true` based on retired E6 evidence and old pose/config; later #176 requires new live smoke and main flag is false. | Preserve full branch, reject bulk merge. Any desired proxy must become a fresh reviewed feature against current architecture/auth/deploy. Split cosmetics if still wanted after badge semantics are repaired. Do not cherry-pick the old model changes or Wave unlock. |

Research files are linked from closed issue resolutions/maps, so preserving their content in a clearly historical location is preferable to discarding them merely because there is no PR. If restored under `docs/research/wayfinder/`, repair relative links (many currently point one or two directories too shallow) and prominently link the current playbook; do not revive retired bench suites.

## Four closed-unmerged branch references

| Branch / PR / tip | Verdict and evidence | Safe action |
|------------------|---------------------|-------------|
| `cursor/local-handoffs-disable-mem0-4f96` / [PR #57](https://github.com/jaylamping/marengo/pull/57) / `7b4eec6` | Old agent-memory policy and local handoff scaffolding, plus a Tailscale secret note. Main already has no mem0 server in its MCP configuration and no `docs/mem0-ops.md`/old memory rule; current instruction/skill layout moved further. Wholesale replay would restore stale agent instructions and removed artifacts. | Retire/archive as superseded policy. If the cloud Auth-key troubleshooting note remains useful, port that one sentence to current cloud docs separately; no full branch merge. |
| `feat/consul-actuator-harness-pr2` / [PR #42](https://github.com/jaylamping/marengo/pull/42) / `a5e8882` | Actuator command/session/allowlist foundation was implemented through later merged harness work. Current proto contains `ActuatorCommand`/tuning/session shapes, gateway actuator/session handling, and current command-joint allowlist; old bringup schema is obsolete. | Retire as superseded. Do not restore old proto/allowlist files over current master/commissioning code. |
| `feat/consul-actuator-harness-pr3` / [PR #44](https://github.com/jaylamping/marengo/pull/44) / `12d84da` | Pi overlay/GainOverride work landed through [PR #65](https://github.com/jaylamping/marengo/pull/65) (`d18535a`). Current [bins/marengo-pi/src/overlay.rs](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-pi/src/overlay.rs) is the evolved implementation, including async persistence and live limit handling. Old branch includes PR #42 base changes too. | Retire as superseded; preserve only historical evidence. Its missing patch-ID equivalence is not proof it needs merging. |
| `fix/consul-chappe-deploy-pr5` / [PR #41](https://github.com/jaylamping/marengo/pull/41) / `351ca4f` | Deploy rebuild code already landed via PR #39/main `7d5c599`; old operator docs and mem0 refactor mostly obsolete. **Unique historical right shoulder-roll bracket revision exists**, but Git stores only its LFS pointer. | Do not merge branch wholesale. Recover/preserve historical CAD payload separately if obtainable, preserve pointer identity in this audit/bundle, and leave the newer live CAD file untouched. Refresh useful deploy triage docs against today's flow if needed. |

### Closed PR #41 CAD preservation detail

Path: `cad/parts/shoulder/marengo_shoulder_roll_bracket_right.SLDPRT`.

- Branch commit `1b31138` introduced Git blob `a1851035c8547ca7881a3972bef2003b67c7def0`, **132 bytes**: an LFS pointer.
- Pointer payload: SHA256 **`3e401d6748429e0015c359e072436f66e2a5dbc54d0180639b6e8d26a269fb65`**, size **1,707,374 bytes**.
- Previous tracked pointer (including commit `4c7b940`) targets `7ea94b6107e3257c72069d6f8d8cb7257d1bf1b586badc077c7668d70e6ddf10`, size 1,705,254 bytes.
- Current local J-drive part is **1,849,521 bytes**, SHA256 **`a9a7d098e93de8176d3ea1a67e1ee80f7aedd2914380545e2403b9916ad17f4d`**. It is distinct; no mechanical equivalence or superiority was inferred from size/hash.
- Historical payload `3e401…` was initially absent from inspected `.git/lfs/objects/3e/40/…` caches under J, C, and the retained WSL recovery source. `git-lfs/3.5.1` is installed. A Git bundle alone preserves the pointer, not that payload.

The historical payload has since been retrieved from commit `1b31138` and preserved in a versioned historical CAD recovery record, with its expected hash and size verified. The original branch remains retained. Do not replace the current bracket with the historical version or with the 132-byte pointer.

## Four merged-PR-name references whose tips moved afterward

These were discovered by comparing every merged-history branch SHA with recorded merged PR head OIDs.

| Reference | Recorded merged head vs current tip | Verdict |
|-----------|------------------------------------|---------|
| `cursor/robstride-firmware-modes-e0c4` | PR #1 head `bccf055` vs tip `85e965b` | **Unique post-merge docs:** commits `7d998d8`, `efbcc16`, `85e965b` add/evolve a Rudy retrospective and Marengo principles ADR plus roadmap link. The file is absent from main. Preserve it as historical design context; its ADR0006 number conflicts with current homing ADR0006 and its 4-DOF/“thin bins already enforced” claims are dated. Renumber/archive and annotate before landing, rather than replay old roadmap content. |
| `feat/yaw-suite-wave-teach` | PR #69 head `6029451` vs tip `92e0e51` | **Mixed later work:** 4-DOF profile addition exists in current history/master evolution; several Windows launcher/hook ideas were converted in PR #124/current TS launcher. But local-deploy-current-HEAD fix `3362971` is still missing on main: [tools/marengo-pi-mcp/src/tools/deploy.ts](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/tools/marengo-pi-mcp/src/tools/deploy.ts) local path currently fetches/checks out/pulls main, mutating the user's feature workspace. Preserve/port that behavior deliberately; main-only deployment can use an isolated checkout instead. Do not bulk replay old env/profile/launcher state. |
| `jl/limit-sync-opt-in-c19e` | PR #134 head `6781081` vs tip `304644b` | Post-merge removal of production loopback limit-sync URL is already on main via PR #135 `3571b94`; current `.env.production` contains no baked `VITE_LIMIT_SYNC_URL`. Safe to retire after backup. |
| `jl/set-limits-stale-refresh-551c` | PR #144 head `4bda20d` vs tip `ef2f69c` | Later exact-taught-hard/no-30mrad-pad patch is equivalent to main `8831744`; current [persist-joint-limits.ts](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/lib/persist-joint-limits.ts) persists exact lower/upper and computes soft inset separately. Safe to retire after backup. |

The first two are not fully subsumed merely because a same-name PR merged. Their useful intent can be ported selectively or retained in the bundle with an explicit follow-up; no evidence supports dropping that distinction from cleanup accounting.

## Three PRs open at the audit baseline

### [PR #117 — actuator facets and Ready aggregation glossary](https://github.com/jaylamping/marengo/pull/117)

All nine new rows (`Actuator facets`, `Joint Ready`, `Limb Ready`, `Robot Ready`, `Commissioning scope`, `Hardware page`, `Telemetry page`, `Set Zero`, `Go-to-zero`) match main byte-for-byte. Issue #118's August 9 23:00Z comment explicitly says the glossary was absorbed into PR #119; PR #119 merged August 10 00:27Z. **Close as superseded**, citing PR #119. No rebase/merge is useful. Existing wording discrepancies with actual readiness aggregation can be fixed in the corrected consolidation glossary, not by merging this old draft again.

### [PR #109 — Hardware page 3D variants on vanilla Three.js](https://github.com/jaylamping/marengo/pull/109)

Development-only variants led to the production table-first Hardware design. Main route `/hardware`, [hardware-overview.tsx](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/components/dashboard/hardware/hardware-overview.tsx), optional `Hardware3dView`, settings/import/scope components implement the selected product destination. Old `/prototype/hardware` UI variants and Bender figure remain reference material; adding them now revives dead route/package/chrome assumptions. **Close as superseded**, reference PR #112 (first Hardware SoT) and PR #119 (commissioning cutover), and preserve tip in the bundle. Visual polish remains separate deferred work.

### [PR #107 — Config/Setup URDF hydration glossary](https://github.com/jaylamping/marengo/pull/107)

It contains useful missing master/staging/archive/import vocabulary but incorrect present-tense claims:

- “Master Accept hot-reloads memory”, “incoming value becomes Active after 300–500ms”, and “persist-degraded Accept” describe an older unimplemented design. Current [hardware.rs:319-357](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/bins/marengo-gateway/src/hardware.rs#L319) archives/promotes disk and `:384` says restart required; Pi caches its model at construction.
- “Hardware's primary chrome is interactive 3D” / “Subsystems durable counterpart” are superseded by table-first Hardware and read-only Telemetry.
- “Git only bootstraps and cannot clobber divergent files” is an intended policy, not an unconditional current deploy guarantee.
- “All mapped link inertial fields merge” overstates the implemented joint-field merge projection.

**Recommended action:** rewrite PR #107 from current main with only corrected glossary additions/replacements, retain current session/commissioning rows, update its title/body to the actual final scope, verify doc links/diff, then merge normally. Do not merge the old branch verbatim.

## Seven issues open at the audit baseline

| Issue | Evidence and present status | Recommended action |
|-------|-----------------------------|-------------------|
| [#118 — Hardware commissioning + Consul master chrome cutover](https://github.com/jaylamping/marengo/issues/118) | Main contains merged PR #119 `9a7894e`; PR #120 review fixes merged into its branch first. [openspec/changes/consul-hardware-commissioning/verify-report.md](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/openspec/changes/consul-hardware-commissioning/verify-report.md) records 34/34 applied, Consul 288+workspace tests at the time, PASS WITH WARNINGS; physical smoke deferred to #115. Implementation/facets/scope/routes/glossary are visibly present. | **Close as implemented** with PR #119. Archive the lingering OpenSpec change when consolidating docs, retaining historical warnings. Current defects get linked repair tickets, not erased by this closure. |
| [#115 — Re-zero yaw/elbow and re-run gravity smoke](https://github.com/jaylamping/marengo/issues/115) | The issue only has the old blocked-on118 comment. Later #170 records Reference PASS at 23:28Z on August 11, all five sign checks, and quiet arm-down GComp session 001113Z with Active/residual PASS. Later float and elevated sessions expose additional issues. | **Close as superseded narrowly** by170 if cleaning duplicates: link later Reference/sign/quiet GComp evidence, transfer any unresolved gravity behavior to170/repair backlog. Do not say coupled elevated GComp is commissioned or today's zero is valid. Keeping it until that cross-link is written is also defensible. |
| [#96 — Consul Hardware/URDF SoT map](https://github.com/jaylamping/marengo/issues/96) | Software destination mostly shipped via PRs #112/#119, but original #114 smoke failed; no final explicit destination acceptance was found. The promised Accept=immediate-memory-active contract differs from current restart-required import. Current review finds persistence, import, reference, and auto-rearm defects. | **Keep open**, update its execution status and unresolved gaps; close only with a recorded destination acceptance or explicit rescope separating shipped UI from remaining contracts. Do not infer physical acceptance from unit tests or PR #119 merge. |
| [#150 — Limb commissioning playbook map](https://github.com/jaylamping/marengo/issues/150) | Locked playbook169, TorqueOnly166, params171, Wave175/179 are shipped. The map also includes execution of the right arm to full signed-off usable state.#170's execution stops midway through the 50% ladder and chapters 6–9 are unfinished. | **Keep open.** Record authored procedure vs incomplete physical execution separately. |
| [#170 — Execute right_arm through locked limb playbook](https://github.com/jaylamping/marengo/issues/170) | 58 historical comments. Reference/limits/sign passed; arm-down/mid/float had qualified passes; elevated was NOT GREEN; §4c was PASS with notes while violating the strict dwell criterion; 25% ladder passed,50% failed the roll return, later retry incomplete. No final limb/payload/mode sign-off. | **Keep open**, resume only after current safety review repairs/reassessment and operator support. Link exact stop point and qualified GComp debt. Do not flatten “Ch1–4done” into rigorous complete acceptance. |
| [#176 — Live Wave smoke then flip WAVE_POSE_GCOMP_SIGNED](https://github.com/jaylamping/marengo/issues/176) | Main [consul/src/data/compound-tests.ts:65](https://github.com/jaylamping/marengo/blob/4bc77ba605834fdec04b436daa4bec67bca84fbb/consul/src/data/compound-tests.ts#L65) remains false.#170 §4c comment explicitly keeps false pending documented live raise+elbow-wave smoke. Original issue pose/extrema are outdated after PR #179, while current playbook requires both §4c and new live smoke. | **Keep open and flag false**. Update expected pose/range/rev to current playbook before execution. No merge or backlog tidy-up can substitute for the operator/hardware evidence. |
| [#63 — WebGPURenderer](https://github.com/jaylamping/marengo/issues/63) | Explicitly deferred and conditional on measured CPU/battery/scene wins. No WebGPU/WebGPURenderer/three-webgpu implementation found in current `consul/src` or package configuration. | **Keep open as future work**, optionally label deferred. No reason to merge unrelated prototypes or treat it as a current correctness bug. |

## Last recorded robot session: exact evidence and limits

All times below are **UTC from historical issue comments**. No present robot state was inspected. Key #170 records:

1. **2026-08-11 23:28:38Z:** Reference PASS; operator confirmed Homing Verified/JointReady/fault=0.
2. **2026-08-12 00:19:44Z:** arm-down per-joint GComp PASS with notes; quiet residual session 001113Z passed, float session 001752Z exceeded upper-yaw hard limit.
3. **01:01:08Z:** elevated attempt 005812Z survived but drift failed: pitch−.105rad, yaw+.164, elbow+.413; elevated not green.
4. **01:31:08Z:** wave_pose session 012649Z PASS-with-notes; pitch+.076rad violates the <0.05 rad criterion, residuals soft, operator accepted a checkpoint; Wave flag explicitly false.
5. **03:06/03:07Z:** 25% ladder session 030430Z PASS/ok_candidate using comfort cruise overrides and PR #181; 129 ascent-breakaway trace events noted as jerky.
6. **04:02:56Z:** 50% session 040100Z on `4bc77ba` fixed pitch hang; roll 1.40→0.20 rad failed because target envelope-clamped to approximately 0.382 rad and runner settle timed out.
7. **04:04:37Z:** 040257Z retry stopped midrun, explicitly incomplete/do-not-score; planresume50%steppedroll, then75/100,near-limit,chapters 6–9.
8. **04:07:57Z, latest comment:** **remote stop was NOT confirmed** after Tailscale loss. This supersedes the prior “Stopped safely” claim. No current robot state was inspected in this audit; do not report it as safely stopped or active now.

These records are helpful for reconstructing where the user left off, but historical operator checkpoints do not prove a currently powered robot's reference, safety state, or acceptance.

## Recommended consolidation sequence

1. Retain Git bundle/local recovery/CAD manifests, recover any unique LFS body or explicitly record its absence.
2. Close superseded PRs #117/#109; rewrite useful PR #107 terms from current implementation and merge after review/checks.
3. Close #118 as implemented; narrowly cross-link and close #115 as superseded if desired; leave #96/#150/#170/#176/#63 open with accurate status.
4. Preserve/refresh historical research text; consolidate the two identical pipeline/checklist histories; retire throwaway prototypes.
5. Extract genuine post-merge design/deploy intent from the two mismatched tips; do not merge old branches wholesale. Reclassify those tips accurately in the cleanup ledger.
6. Retire already-subsumed branch refs only after recorded preservation; mark substantial AutoLearn feature as archived/deferred and do not carry its old Wave unlock or model into main.
7. Keep all safety fixes and physical commissioning as separate concrete follow-up work. Documentation cleanup does not satisfy the incomplete hardware gates.

## Legacy Windows branches worktrees and stashes

A final read-only check covered both `marengo.DEAD` recovery sources: the older C-drive checkout and its J-drive copy. Both are on legacy main `51a248af244eb382e144db9ef6bfa59a9b37ce8e`. They contain identical sets of **90 refs: 29 local branches, 40 remote refs, 20 checkpoint refs and the latest stash ref**, plus **23 stash entries** in reflog history. This local inventory is separate from the 104-reference GitHub inventory above.

Every ref was compared with current J-drive refs/main and both earlier migration bundles. **Four local branches contain eight commits absent from those prior preserved histories**. The table below records all 29 local tips. A branch having no new commit does not by itself prove its changes landed on main; the main-ancestor column and semantic verdict distinguish those cases. Full source-specific refs and commit comparisons are retained in the local recovery records.

The two sources now have separate verified bundles containing all refs and reflog history, source-specific staged/working patches and metadata, and readable recovery copies. **Three physical A/B/C worktrees per source contributed 2,456 files / 128,078,906 bytes**, excluding only encountered `consul/node_modules` caches. All 4,912 copies passed SHA256 comparison; matching source-specific manifests confirm the C/J worktree copies are identical. Their modified tracked and untracked UI files are preserved. **440 LFS-cache files / 1,045,184,082 bytes** are preserved once under J with verified source-specific manifests; both caches match. The deleted `.slim/codemap.json` state and its original blob are also preserved.

No legacy source was deleted, moved, repaired, checked out to another branch, or used to issue a robot command. Recovery artifacts live in the migration backup's `legacy-windows-state` folder, with a recovery README and verification manifests.

### All local branch tips

| Branch | Legacy tip | Ancestor of reviewed current main | New commits absent from current refs and prior bundles | Verdict |
|--------|------------|-----------------------------------|------------------------------------------------------|---------|
| `bench-700g-com-calibration` | `aec003558a` | No | 3 | Historical single-joint 700 g COM calibration; archive fixture evidence, do not replace current master. |
| `clear-glider` | `3a87be28a1` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `consul-glinui-dashboard` | `094e65860b` | No | 1 | Historical weighted-arm remediation notes and old context-split skill; selectively archive docs, leave old agent policy retired. |
| `consul-glinui-linear` | `995a6fff1b` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `consul-glinui-pr1-base` | `ec847a5f00` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `consul-glinui-pr8-cleanup` | `a40cc6e4ac` | No | 3 | Radix-only UI cleanup/lint gate already present in main; glass/contrast styling superseded by current panel design. Archive. |
| `cursor/robstride-can-readiness-c935` | `d24e0b0081` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `feat/can-loop-diagnostics` | `c3b2c9859b` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `feat/consul-actuator-harness-pr1` | `a03be99eba` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `feat/consul-actuator-harness-pr2` | `a5e8882131` | No | 0 | Old actuator command/allowlist foundation superseded by later main implementation; archive. |
| `feat/consul-actuator-harness-pr3` | `12d84da003` | No | 0 | Old Pi overlay/gain foundation superseded by evolved main overlay; archive. |
| `feat/consul-actuator-harness-pr4` | `924d7c13c7` | No | 1 | Unique checkpoint changes bracket LFS pointer to the verified PR41 payload. Preserve CAD history; actuator work evolved on main. |
| `feat/rs03-p1a-recv-poll` | `27427f4485` | No | 0 | Receive-poll/watchdog separation exists in current implementation; retain archived fork, repair CS01 in current code. |
| `feat/rs03-p1b-type24` | `3b20f8c703` | No | 0 | Type24 active-reporting intent is implemented and evolved into sensing leases; retain archived fork. |
| `feat/rs03-p1c-params` | `778eb227aa` | No | 0 | Parameter-ID audit variants exist in current driver; actual safety timeout/limit handshake remains current review work. |
| `feat/subsystems-set-limits` | `bc60e268e1` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `feat/yaw-suite-wave-teach` | `92e0e51cb3` | No | 0 | Existing post-merge deploy-current-HEAD intent remains useful; use earlier branch verdict, preserve and port selectively. |
| `fix/consul-chappe-deploy-pr1` | `6a32208abc` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `fix/consul-chappe-deploy-pr2` | `fd2923bba7` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `fix/consul-chappe-deploy-pr3` | `bad7754924` | No | 0 | Always-rebuild deploy intent landed through later merge; archive the older local commit. |
| `fix/consul-chappe-deploy-pr4` | `2ca4e4b317` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `fix/consul-chappe-deploy-pr5` | `351ca4faad` | No | 0 | Existing PR41 verdict applies; current deploy evolved and historical CAD payload has been recovered. |
| `gravity-comp-enhancement` | `00fa690f56` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `main` | `51a248af24` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `majestic-helicona` | `ad66706140` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `pi-bench-urdf-sync-tool` | `2714d776d1` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `redesign/consul-a` | `e01a540534` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `redesign/consul-b` | `e01a540534` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |
| `redesign/consul-c` | `e01a540534` | Yes | 0 | Already included in main ancestry; preserve as historical reference. |

### Unique local branch content

- `bench-700g-com-calibration` adds three June 19 commits (`3b53318`, `a5d6604`, `aec0035`) tuning COM in the old single-joint weighted URDF fixtures and limiting the change to assets. It is evidence for that historical fixture, not calibration of today's five-DOF master. Preserve the measured-history context and assets; do not merge its old geometry into current master.
- `consul-glinui-dashboard` adds checkpoint `094e658`: a weighted 700 g remediation document and old context-split agent skill. The document discusses COM/friction/feedforward and obsolete direct deploy/WSL paths. Archive it as historical reasoning, with a current-status banner if later published. Do not revive its old operational procedure or agent policy.
- `consul-glinui-pr8-cleanup` adds three commits (`169fe3a`, `575a16d`, `a40cc6e`) for sidebar contrast, Base UI removal/Radix lint gating, and glass-card pointer styling. Main already contains the Radix lint gate and no Base UI dependency; current cards use the evolved panel design. The unique commit IDs do not establish a missing production capability. Archive this fork; any visual idea needs a small current-code patch.
- `feat/consul-actuator-harness-pr4` adds checkpoint `924d7c1`, changing the shoulder-roll bracket pointer to SHA256 `3e401d6748429e0015c359e072436f66e2a5dbc54d0180639b6e8d26a269fb65`, size 1,707,374 bytes. This is the same historical PR41 body already recovered and verified. Preserve the version; do not replace the current bracket or merge old actuator/overlay state over current code.

### Uncommitted UI worktrees

All three physical worktrees start from `e01a54053497cad36830010775aea4c53763bd23`. Their original `.git` pointers/registrations still refer to pre-rename paths; the review read them through their preserved administrative gitdirs without repairing the sources.

| Worktree | Preserved differences | Recommended disposition |
|----------|-----------------------|-------------------------|
| `consul-a` | Modified usage-bar plus untracked rail navigation/user-menu and console panel/metric components. | Archive as an A variant; no live hardware capability or current commissioning fix established. |
| `consul-b` | Dependency locks/package, usage-bar, button/card/slider and global CSS changes; untracked flight figure header. | Archive as a B variant; dependency/style changes need independent current-code review before reuse. |
| `consul-c` | Global CSS changes plus untracked machine-state chrome and UI store. | Archive as a C variant; mock/local machine-state presentation must not replace authoritative telemetry. |

`clear-glider` and `majestic-helicona` still have administrative registrations but their `.kilo` working directories are absent from both sources. Their branch commits and administrative metadata, including empty staged diffs, are preserved; no missing uncommitted or untracked working files can be reconstructed from a directory that is absent. Their commits are already in current main ancestry.

### Checkpoint snapshots

The **20 parentless checkpoint commits** represent **two unique file trees**: five snapshots use `17d627273f9b81515dd0666cfa849c97c6721ca3`, and fifteen use `4342f34a6b93c50ad81222ff98d943214ab49dba`. Within each tree group, the snapshot contents are identical. The later tree adds/evolves the old mem0 agent integration, policies and vendored dependency state. Both are whole historical workspace snapshots containing older UI/control/config and generated dependency data, rather than focused feature commits with a normal branch ancestry.

They are preserved in the bundles with exact ref/OID inventories and patches against legacy main. Archive them; do not merge a parentless workspace snapshot into today's master. Selective recovery of a needed historical file remains possible without restoring obsolete profiles, memory policy, old source paths or dependency caches.

### Historical stash records

All **23 stashes**, including tracked/index and available untracked parents, have patches, parent identities and readable file recovery with SHA256 manifests. These are dated May through July 2026. Historical CAD blobs/pointers are preserved alongside the LFS caches; current geometry stays unchanged. The dispositions below are semantic archival decisions, not claims that every old experimental patch was runtime-tested or mechanically validated.

| Stash commit | Content and recommended disposition |
|--------------|-------------------------------------|
| `16cf538` | Subsystems dependency lock backup; preserve for provenance, avoid replaying obsolete dependency resolution. |
| `b087df6` | Old WSL-to-Windows MCP/ignore changes; archive, use the accepted J-drive development policy. |
| `32a1f58` | UI glass/space-backdrop experiment, textures and component tests; archive as visual design reference. |
| `d772948` | Two-DOF setpoint/trace/harness WIP; compare any useful intent against current lead/stall control, never import old gains/config wholesale. |
| `8418b6d` | Gateway config/store/proto/admin WIP; current master/limits APIs evolved, preserve and use current transaction fixes. |
| `c1dcc96` | Old agent-persona policy; archive rather than restore retired instruction layout. |
| `f1e6a8f` | Old dual-shoulder motor map; archive hardware history, do not apply old motor IDs/signs. |
| `01d9983` | Historical shoulder bracket CAD; preserve version independently of current model. |
| `d3006c7` | Old actuator tuning/rate-limit WIP plus bracket CAD; implementation superseded, preserve historical CAD. |
| `5e0e685` | Mixed actuator/overlay/proto WIP plus CAD; archive, avoid replacing evolved control/commissioning contracts. |
| `d889b66` | Historical bracket CAD backup; preserve independently of current model. |
| `6a5bb7e` | Mixed glass Overview WIP and CAD; archive design/CAD evidence, do not replay test deletions. |
| `f35ace9` | Old logs/memory UI WIP; archive, use current log reliability fixes and retired-memory policy. |
| `2d1fb6c` | Mixed logs/memory/sidebar WIP plus CAD; archive, salvage only a deliberately reviewed isolated idea. |
| `c21429f` | Old Overview card/skeleton tests and styles; current UI evolved, preserve design/test provenance. |
| `907a6f2` | Historical right-arm and torso CAD assemblies; preserve versions separately. |
| `1a010fd` | CAD assembly/naming WIP and debug artifact; preserve historical CAD, do not replace current model. |
| `22db1f1` | Left-only overshoot/control/motor-map backup; archive old bench setup, do not apply current-arm gains or mapping from it. |
| `e29a273` | Pi MCP lockfile backup; preserve provenance, avoid old dependency downgrade. |
| `fb6fb59` | Early Chappe/telemetry/host-metrics/deploy WIP; current stack evolved, archive and repair present findings directly. |
| `1e3a96d` | Historical Pi commissioning notes; preserve as a dated record, not the current locked procedure. |
| `b85ba73` | Early CAN/bin dependency WIP; current driver/runtime supersede it, preserve history. |
| `1d44d1a` | Historical torso-frame CAD assembly; preserve version independently. |

### Preservation limits

Git bundles preserve commits and LFS pointers; the verified cache copy preserves all LFS files physically present in these two sources. This does not establish that every historical pointer ever recorded in remote history has a locally available body. The known PR41 gap was explicitly fetched and verified. No CAD geometry, mass, center of mass, firmware setting or present robot reference was validated by these recovery operations.

The missing `.kilo` working directories remain an explicit recovery limit. Cache exclusions were restricted to encountered regenerable node_modules directories in the physical worktrees; all encountered noncache local files, including URDF fixtures and visual artifacts, were copied. The original DEAD directories remain recovery sources. No additional feature branch was merged or deleted as part of this preservation pass.
