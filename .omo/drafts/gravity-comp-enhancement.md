---
slug: gravity-comp-enhancement
status: drafting
intent: clear
pending-action: write .omo/plans/gravity-comp-enhancement.md
approach: <fill: the approach you intend to plan>
---

# Draft: gravity-comp-enhancement

## Components (topology ledger)
<!-- Lock the SHAPE before depth. One row per top-level component that can succeed or fail independently. -->
<!-- id | outcome (one line) | status: active|deferred | evidence path -->

| id | outcome | status | evidence |
|----|---------|--------|----------|
| C1-judges | Panel of 3 Oracle judges scores existing gravity-comp impl against industry rubric (Algorithm, Validation/Safety, Integration/Mode-isolation) | active | bg_3c77079b, bg_c934af19, bg_bc43eb32 (running) |
| C2-enhance | Greatly enhance gravity-comp control code & architecture based on judge verdicts — goes beyond existing narrow remediation (A/B/C workstreams) | active | codegraph: armee-dynamics, berthier/loop.rs, davout/lib.rs; ADRs 0004/0005 |
| C3-test-suite | Design extremely detailed physical bench test suite for 700g weighted right-shoulder-pitch actuator | active | docs/bench-weighted-700g-results.md, explore inventory (MCP tools, motor-repl, marengo-pi, config) |
| C4-mode-isolation | Prove gravity-comp enhancements do NOT interfere with MIT / Torque / Impedance / Position modes — CRITICAL user requirement | active | berthier/loop.rs L605-700 mode dispatch, L517-536 mode switching; davout MIT filtering |

## Open assumptions (announced defaults)
<!-- Record any default you adopt instead of asking, so the user can veto it at the gate. -->
<!-- assumption | adopted default | rationale | reversible? -->

| assumption | default | rationale | reversible? |
|-----------|---------|-----------|-------------|
| URDF COM correction | Apply existing remediation Workstream A (COM z -0.4572→-0.36) as part of enhancement | Already designed + expert-reviewed (sdd/bench-weighted-700g-remediation/design) | yes |
| Numerical gradient retention | Keep virtual-work numerical gradient for now; add Pinocchio/RNEA as optional backend behind trait, not replacement | ADR 0005 chose numerical for no-FFI/no-unsafe; 1-DOF bench doesn't need RNEA yet | yes |
| Test suite scope | Physical bench tests only (no sim-only tests in this plan); sim cross-check is separate ADR 0005 D1 | User explicitly wants physical test suite for the 700g rig | yes |
| Mode isolation strategy | Add characterization tests (not just runtime guards) that prove each mode is independent — TDD for mode isolation | User said CRITICAL; tests are the durable proof | yes |
| Enhancement scope | Broader than existing 3-warning remediation: add friction identification, payload robustness, mode-switch transients, torque saturation pre-check, wrong-sign watchdog | Librarian rubric identifies these as industry-standard requirements the current impl lacks | yes |

## Findings (cited - path:lines)

### Gravity-comp code surface (codegraph)
- `crates/armee-dynamics/src/urdf_gravity.rs` L1-136: UrdfGravityModel, numerical central-difference ∂P/∂q, DQ_EPS=1e-6, GRAVITY=(0,0,-9.81)
- `crates/armee-dynamics/src/lib.rs` L42-53: DynamicsModel trait, gravity_model_from_urdf
- `crates/berthier/src/loop.rs` L607: tau_g computed every tick regardless of mode; L624-627: GravityComp/TorqueOnly use (0,0,tau_g,q,0); L629-638: Impedance uses (kp,kd,tau_g+tau_f,q,0); L640+: Position uses composed FF including tau_g
- `crates/berthier/src/loop.rs` L517-536: set_control_mode — instant switch, no torque matching/ramping
- `crates/davout/src/lib.rs` L78-83: ControlMode enum; L891-918: disable_all; L738-755: send_joint_command (legacy); L757+: send_mit_joint (filtered)
- `crates/berthier/src/friction.rs` L163: friction_torque (Coulomb+viscous+Stribeck)
- `crates/berthier/src/position_feedforward.rs` L21: compose_position_hold_feedforward

### Bench results (docs/bench-weighted-700g-results.md)
- Phase 1: tau_g table at 6 poses — all PASS, matches analytical m·g·L·sin(q)
- Phase 0b: gravity-on sign check — PASS, no runaway
- Phase 4: distance ladder 0→2.0 rad — ALL PASS, no trips, τ_meas~0.73×τ_g (27% over-comp)
- Phase 2: Layer 2 smoothness FAIL — friction-FF discontinuity, jerk_rms 1295-1560, tau_ff slew 144 Nm/s
- Phase 6: negative-retarget velocity trip at -2.56 rad/s (direction-specific); recovery RECOVER_OK

### Test coverage gaps (codegraph blast radius)
- ControlLoop: ⚠️ no covering tests
- ControlMode: ⚠️ no covering tests
- friction_torque: ⚠️ no covering tests
- compose_position_hold_feedforward: ⚠️ no covering tests
- Only 4 unit tests in armee-dynamics, 1 integration test in berthier

### Existing remediation (mem0: sdd/bench-weighted-700g-remediation/design)
- Workstream A: URDF COM correction (asset-only, lowest risk)
- Workstream B: friction FF graded fade (berthier/friction.rs)
- Workstream C: negative-retarget descent gate (conditional on C1 re-test)
- Expert verdict: "Revise" — proceed after deploy path resolved, strict A→C1→B→C2-C5 sequencing

### Industry rubric (librarian report)
- 20 criteria across 4 sections: Algorithm(1-5), Validation(6-10), Safety(11-15), Integration(16-20)
- Key gaps identified: numerical gradient (not RNEA), no friction identification, no payload robustness, no energy comparison, no mode-switch transient mitigation, no torque saturation pre-check, no wrong-sign watchdog

## Decisions (with rationale)

| decision | rationale |
|---------|-----------|
| Fire 3 Oracle judges in parallel for adversarial review | User explicitly asked for "panel of judges"; Oracle is the high-IQ read-only agent for this |
| Enhancement goes beyond existing remediation | User said "greatly enhance" — existing plan only fixes 3 warnings, not architecture |
| Mode isolation via TDD characterization tests | User said CRITICAL; tests are durable proof, not just runtime guards |
| Keep numerical gradient, add RNEA as optional backend | ADR 0005 constraint (no FFI/unsafe); trait already abstracts the model |

## Scope IN
- Gravity-comp code in armee-dynamics, berthier, davout
- Control architecture enhancement (mode switching, safety, validation)
- Physical bench test suite design (700g weighted right-shoulder-pitch)
- Mode isolation proof (MIT, Torque, Impedance, Position unaffected)
- Friction identification methodology
- URDF COM correction (existing Workstream A)
- Torque saturation pre-check, wrong-sign watchdog, mode-switch transients

## Scope OUT (Must NOT have)
- Full humanoid dynamics (stay 1-DOF bench for now)
- Pinocchio/FFI replacement of numerical gradient (keep as optional backend only)
- Sim-only test suite (user wants physical bench tests)
- Changes to robstride CAN protocol
- Changes to Chappe wire types
- Left shoulder pitch (right-only bench)
- Consul UI changes (unless needed for test suite operator workflow)

## Open questions
- None yet — waiting for judge verdicts to inform enhancement scope

## Approval gate
status: approved (2026-06-19)
user_reply: "i approve"
pending_action: write .omo/plans/gravity-comp-enhancement.md (todos + TL;DR)
