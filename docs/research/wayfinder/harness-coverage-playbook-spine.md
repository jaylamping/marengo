# Inventory: harness coverage vs playbook spine gaps

**Wayfinder research** for [marengo#151](https://github.com/jaylamping/marengo/issues/151) (map [marengo#150](https://github.com/jaylamping/marengo/issues/150)).

**Snapshot date:** 2026-08-11 (branch `research/harness-coverage-playbook-spine`, `origin/main`).

**Locked spine (map #150):** Reference → Limits confirm → Sign → G-comp (per-joint→coupled) → Position speed ladder → Impedance → TorqueOnly → Payload critical gates → Limb sign-off.

**Primary sources:** `tools/marengo-pi-mcp/src/bench-profiles.ts`, `tools/marengo-pi-mcp/src/harness/{index,scripts}.ts`, `tools/marengo-pi-mcp/src/tools/motion.ts`, `tools/marengo-pi-mcp/README.md`, `scripts/analyze-position-trace.py`, `docs/bench-*.md` (gravity, position-tuning, roll/yaw/elbow, 2dof smoke, weighted sign/results), [ADR 0007](../decisions/0007-bench-position-trajectory-control.md), [ADR 0009](../decisions/0009-dynamic-position-limit-envelope.md), [ADR 0017](../decisions/0017-bench-set-limits-urdf-expand.md).

**Sibling research (other branches, not required here):** [active MIT modes / gates](https://github.com/jaylamping/marengo/issues/154) inventories *acceptance criteria*; this ticket inventories *automation coverage* of those criteria against the spine.

---

## Summary

MCP `pi_bench_harness` today is a **smoke runner**: every scripted profile uses `pass_kind: "smoke"`; `commissioning_criteria_met` is always `null`; the `"commissioning"` pass kind exists in TypeScript but no profile selects it (`tools/marengo-pi-mcp/src/harness/scripts.ts`, `index.ts`). Automation covers **preflight + sign/hold *motion scripts*** for roll/yaw/elbow/2-DOF and a thin weighted gravity-on path. Metric gates (±50 mrad, Layer 2 jerk/lead, τ_g vs τ_meas) live in **suite prose + offline analyzer**, not in harness JSON. **Limits confirm, coupled G-comp, Impedance, TorqueOnly, speed-ladder/near-limit stress, payload critical subset, and limb-wide sign-off have no harness or analyzer automation.**

---

## Harness inventory (what exists)

### Profiles (`bench-profiles.ts`)

| Profile | Joint subset | Weighted | Script suite | Notes |
|---------|--------------|----------|--------------|-------|
| `bare_motor` | (none / master) | no | yes — `gravity-on` ×3 | Dual-pitch style; gravity preview in preflight |
| `weighted_single_arm` | (none) | yes | **null** — angles path | `gravity-preview` angles + one `gravity-on` pipe |
| `arm_attached` | (none) | yes | **null** | No pipe suite |
| `roll_attached` | roll+pitch+yaw | yes | smoke scripts | `skipGravityPreview` |
| `arm_2dof_smoke` | roll+pitch+yaw | yes | smoke scripts | Pitch/roll/cross-talk holds |
| `yaw_attached` | +elbow | yes | smoke; `operatorSignoffRequired` | Docs: not ±50 mrad |
| `elbow_attached` | +elbow | yes | smoke; `operatorSignoffRequired` | After E6; discovery amplitudes |

### Common preflight (every harness run)

From `harness/index.ts`: `health` → `can_up` → `motor_repl_status` → optional `set_zero_*` → **strict** `homing_preflight` (`homing-preflight.sh`, Verified required) → optional `gravity_preview_0_0` → script suite or weighted path → `final_disable` / `final_status`. Sessions tee `bench-*.log`, `position-trace-*.csv`, candump; symlink `*-latest.*`.

Pass heuristics (smoke): exit 0/2, no `control tick failed`, no nonzero `fault=`, no `watchdog` / `outside [` lines. **Does not read position-trace metrics.**

### Analyzer (`scripts/analyze-position-trace.py`)

| Capability | Wired to harness? |
|------------|-------------------|
| Per-segment overshoot / settle / lead_sat / jerk / τ_f flips / τ_ff slew | No — offline CLI |
| `--gate layer2` (0.1 rad approach + return; lead/jerk/slew/flips; optional home start) | No |
| `--onset` diagnostics | No |
| ±50 mrad hold gate | **Not implemented** (hints only at ~30 mrad settle) |
| Speed / near-limit / Impedance / TorqueOnly gates | **Not implemented** |

---

## Coverage by spine chapter

| Spine chapter | Existing automation | Gap | Notes |
|---------------|---------------------|-----|-------|
| **Reference** | Harness preflight (health, CAN, status, set-zero opt, strict homing Verified, optional gravity-preview). MCP: `pi_health`, `pi_homing_status`, `pi_set_zero`, `pi_can_up`, `homing-preflight.sh`. Suite “Y0/R0/E0/D0” prose. | No limb-level Reference commissioning record; harness does not assert deploy-rev == HEAD; mechanical zero remains operator. Bringup `config_dir` slugs in suite docs vs master `config/` + `MARENGO_JOINT_SUBSET`. | Closest to automated of any chapter’s *prerequisites*; still not a named playbook gate. |
| **Limits confirm** | None as a harness/analyzer chapter. Set Limits Apply is Consul → Davout hot-reload ([ADR 0012](../decisions/0012-config-db-overrides.md), [ADR 0017](../decisions/0017-bench-set-limits-urdf-expand.md)); MCP only documents durable persist before URDF sync. Roll **R5** is informal “no `outside`” during hold—not taught-envelope readback. | **No** automated readback of taught hard/soft vs live Davout; **no** near-limit probe through Set Limits envelope; map decision (verify + re-teach on fail) unimplemented. | Sibling #155 snapshots YAML envelopes; does not add tools. |
| **Sign** | Scripted probes: `roll_sign_probe`, `yaw_sign_probe`, `elbow_sign_probe` (`harness/scripts.ts`). Weighted: `weighted_gravity_on` + `docs/bench-weighted-gravity-sign.md` harness recipe. `bare_motor` gravity-on scripts. MCP `pi_marengo_pi_script` / `pi_hold_on` for manual. | All `pass_kind=smoke`; **no** automatic direction/sign metric; visual/operator confirmation required (roll R1; yaw/elbow notes). T6 wrong-sign watchdog is manual protocol, not harness. | Smoke scripts *exercise* sign motion; they do not *certify* sign. |
| **G-comp (per-joint → coupled)** | **Per-joint (partial):** `pi_gravity_preview`; harness gravity-preview + weighted/`bare_motor` `gravity-on` smoke; elbow E6 / weighted sign are **manual** MCP procedures; T1–T9 suite is prose + `pi_hold_on` / analyzer (T4 Layer 2), not a harness suite. **Coupled:** none. | **No** harness asserting τ_meas vs τ_g thresholds (T1); **no** coupled multi-joint gravity-on / τ alignment; `arm_2dof_smoke` explicitly excludes “coupled gravity preview, dual gravity-on” ([bench-2dof-right-smoke.md](../../docs/bench-2dof-right-smoke.md)). Right-arm profiles skip gravity-preview. | Per-joint numeric gates exist as docs; coupled is a pure gap. |
| **Position speed ladder** | **Amplitude / hold ladders (smoke):** `roll_hold_sweep`, `yaw_hold_ladder`, `elbow_hold_ladder`, `arm_2dof_smoke` D1–D3 scripts. **Offline metric:** Layer 2 gate in `analyze-position-trace.py` ([bench-position-tuning.md](../../docs/bench-position-tuning.md)); distance steps 0.3→0.8→1.57 documented for weighted pitch. Suite ±50 mrad (R3/Y3/E4) are **operator + CSV**, not harness. | Harness never calls the analyzer; **no** multi-speed rung ladder; **no** near-limit stress through taught ROM; amplitude ladders ≠ speed ladder. `pass_kind` never `"commissioning"` despite field existing. | First rung (Layer 2) is the only numeric automated *analyzer* gate—and it is offline. |
| **Impedance** | None. Mode reachable via `mode impedance` / tuning docs; weighted results defer impedance push ([bench-weighted-700g-results.md](../../docs/bench-weighted-700g-results.md)). | **No** harness profile, script, or analyzer gate; no numeric acceptance table (see also #154). | Full chapter is hybrid/manual until gates are specified. |
| **TorqueOnly** | None as a chapter. T2 uses gravity-off / TorqueOnly as A/B contrast only ([bench-gravity-comp-test-suite.md](../../docs/bench-gravity-comp-test-suite.md)). | **No** harness or analyzer; TorqueOnly still aliases GravityComp on MIT path (code/ADR — see #154). | Diagnostic contrast ≠ commissioning chapter. |
| **Payload critical gates** | `weighted_single_arm` harness path (preview angles + gravity-on smoke). T3 payload robustness and 700 g results are **manual** protocols. Optional 90° round-trip doc. | Map wants unweighted then one standard payload with a **critical subset** of gates — neither the subset nor fixture identity is automated; no “critical payload” harness profile. | Weighted smoke ≠ payload chapter. |
| **Limb sign-off** | Per-DOF prose tables (yaw/elbow “Sign-off”); T10 full T1–T9 re-run + health sweep for weighted pitch; harness `operator_signoff_required` flags on yaw/elbow. | **No** limb-wide harness aggregating all chapters; green smoke must not unlock Wave/teach (documented for yaw); no commissioning JSON that records limb ready. | Closest pattern: T10 + suite sign-off tables — all manual assembly. |

---

## Cross-cutting automation facts

1. **`commissioning` pass kind is dead code path today.** `HarnessPassKind = "smoke" | "commissioning"` and `commissioning_criteria_met` are returned, but every `SCRIPT_SUITES` entry sets `passKind: "smoke"`; weighted null-suite uses `defaultPassKind` → `"smoke"` (`harness/scripts.ts`, `harness/index.ts`). Yaw suite docs state this explicitly.

2. **Position-trace is produced but not gated in-process.** Harness always sets `MARENGO_POSITION_TRACE` and archives CSV; metric evaluation is a separate `python scripts/analyze-position-trace.py` invocation.

3. **Profiles are DOF-slice specific, not limb-parameterized.** `BENCH_PROFILES` hard-code joint subsets for right-arm bring-up slices; map #150 wants limb injection from day one — no `right_arm` commissioning profile exists.

4. **Suite docs still cite bringup `config_dir` slugs** (`arm_3dof_right`, `arm_4dof_right`) while harness code comments say master `/opt/marengo/config` + `MARENGO_JOINT_SUBSET` (`bench-profiles.ts`, `harness/index.ts`). Hybrid playbook must resolve this when wiring gates.

---

## What a hybrid playbook must still cover (automation gaps)

Ordered by spine; “new check” means harness/analyzer/MCP gate that does not exist yet:

| Priority | Chapter | Likely hybrid fill |
|----------|---------|-------------------|
| P0 | Limits confirm | Manual or new: durable Set Limits readback + near-limit probe |
| P0 | Position speed ladder | Wire Layer 2 + ±50 mrad into `commissioning` pass; **new** speed rungs + taught-envelope near-limit stress |
| P0 | G-comp coupled | **New** multi-joint τ / backdrive gates (per-joint can absorb T1 prose) |
| P1 | Sign | Keep smoke scripts; add operator checkbox or metric; optional T6 |
| P1 | Impedance / TorqueOnly | **New** numeric gates (unspecified — grilling) + scripts |
| P1 | Payload critical | Select subset of T1/T3/Layer2/ladder; fixture ID TBD |
| P1 | Limb sign-off | Aggregate record (T10-style) over limb chapters |
| P2 | Reference | Formalize existing preflight as named chapter gate + deploy-rev |

---

## References (quick index)

| Artifact | Path |
|----------|------|
| Profile SoT | `tools/marengo-pi-mcp/src/bench-profiles.ts` |
| Script suites | `tools/marengo-pi-mcp/src/harness/scripts.ts` |
| Harness runner | `tools/marengo-pi-mcp/src/harness/index.ts` |
| MCP motion tools | `tools/marengo-pi-mcp/src/tools/motion.ts` (`pi_bench_harness`, `pi_hold_on`, …) |
| Analyzer / Layer 2 | `scripts/analyze-position-trace.py` |
| Gravity suite T1–T10 | `docs/bench-gravity-comp-test-suite.md` |
| Position / Layer 2 | `docs/bench-position-tuning.md` |
| Roll / yaw / elbow / 2-DOF | `docs/bench-roll-test-suite.md`, `bench-yaw-test-suite.md`, `bench-elbow-test-suite.md`, `bench-2dof-right-smoke.md` |
| Weighted sign / results | `docs/bench-weighted-gravity-sign.md`, `bench-weighted-700g-results.md` |
| Set Limits ADRs | `docs/decisions/0009-*.md`, `0012-*.md`, `0017-*.md` |
