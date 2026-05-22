# ADR 0006: Lessons from Rudy and the design principles of Marengo

**Status:** Accepted (informational)  
**Date:** 2026-05-22

## Context

Rudy was the author's first full humanoid upper-body project (RobStride RS03 actuators on CAN, ROS 2 Jazzy + ros2_control + MoveIt, Isaac Lab sim-to-real ambitions, and a sophisticated standalone operator console called `cortex` + `link`).

At the point of sunset, the repository had grown past 80 kLOC and was continuing to expand rapidly. The decision was made to start fresh with Marengo rather than attempt a large-scale refactor.

The goal of this ADR is to capture the specific early architectural and process mistakes that became structurally expensive, so that Marengo (and future contributors/agents) can consciously avoid repeating them while pursuing the same 23-DOF biped vision.

On a personal note, the author is currently deep in Napoleonic history. The opportunity to name the entire runtime after Napoleon's marshals and generals — Berthier (control), Davout (safety), Talleyrand (planning), Fouché (perception), Chappe (the optical telegraph as the message bus), and Marengo itself (the horse and the 1800 battle) — was simply too good to pass up. The "Armée" workspace and Napoleonic corps naming theme gives the second system both a memorable identity and a constant, slightly irreverent reminder of what disciplined command structures can (and cannot) achieve.

## The costly early decisions in Rudy

1. **Heavy framework adoption before the core motor + safety loop was proven**
   - ROS 2 Jazzy, colcon, ros2_control `SystemInterface`, MoveIt 2, and a hybrid Rust driver node were introduced while the real CAN protocol, gravity compensation, enable/disable semantics, and clean shutdown behavior were still immature.
   - Result: build system complexity, multiple modeling languages (xacro + URDF + custom messages), and a large surface area that had to be maintained even when the fundamental "can I safely power the joints?" question was not yet answered on hardware.

2. **Rich operator console built in parallel with low-level bring-up**
   - `cortex` (axum + WebTransport over Tailscale, ts-rs type generation, audit logging, single-operator locks, per-actuator deep editors for firmware parameters, jog/enable/set-zero, test harness streaming) was developed while the underlying driver and safety model were still changing.
   - The console took exclusive ownership of the CAN bus, creating another writer that had to be kept in sync with any future ROS 2 driver node.
   - Result: a beautiful but brittle piece of infrastructure whose assumptions about the control plane were invalidated as the real hardware story evolved.

3. **Multiple sources of truth and complex regeneration pipelines**
   - Robot model lived in xacro, actuator specs in YAML, firmware parameter catalogs in another YAML, TypeScript types generated from Rust via ts-rs with post-processing scripts, protobuf-adjacent custom messages, etc.
   - Every structural change required coordinated edits across several languages and regeneration steps.
   - CI had to orchestrate ROS + Cargo + Node + Python pytest + launch_testing.

4. **Ambitious sim-to-real and full-system features before real-hardware gravity compensation was trustworthy**
   - Isaac Lab environments, policy training pipelines, and high-level behaviors were scoped while the 4-DOF (or even 1-DOF) real arm had not yet demonstrated stable gravity compensation, sign-correct torque feedforward, and safe E-stop behavior under load.
   - The gap between simulation fidelity and real actuator dynamics (especially RobStride firmware modes, friction, thermal limits, and homing) became a permanent source of thrash.

5. **No strict, thin, auditable path from intent to torque**
   - Multiple components could ultimately influence motor commands.
   - There was no single safety gateway analogous to Davout that every motion — whether from a planner, jog widget, or firmware editor — had to pass through with the same limit, rate, danger-zone, and mode checks.

## Symptoms observed at 80 k+ LOC

- Changing the actuator protocol or adding a new safety rule required coordinated changes in the ROS driver, the standalone `cortex` daemon, the UI, the parameter catalog, the simulation configs, and several regeneration steps.
- Onboarding a new contributor (or an agent) required understanding ROS 2, colcon, ament/Cargo hybrids, WebTransport, ts-rs, Tailscale certs, MoveIt, Isaac Lab, and the custom safety/audit model simultaneously.
- Real hardware sessions were high-friction because the "boring" questions (Does SetZero actually match the URDF zero? Does gravity compensation hold an elevated pose without runaway? What happens on CAN timeout?) had not been closed early with a minimal, focused stack.
- The project had strong opinions in many places, but the opinions that mattered most for safety and long-term maintainability (single motor command path, hardware truth upstream of everything, containerized reproducible checks) were under-expressed or absent.

## Marengo's explicit counter-principles

These are not just "we will do it better." They are deliberate, documented rejections of the above patterns, enforced by the repository rules and crate boundaries.

1. **Hardware truth is the single source (CAD → URDF + yaml configs)**
   - `hardware/cad/`, `assets/urdf/`, `config/robot.yaml`, `config/motors.yaml`, `config/control.yaml` are authoritative.
   - No parallel modeling in xacro or ROS-specific formats for the runtime.
   - See `docs/roadmap.md` (M3), `hardware/docs/kinematics.md`, and the export scripts.

2. **Thin bins, logic in crates, strict layering**
   - `marengo-pi`, `motor-repl`, `marengo-jetson` are wiring only.
   - All control logic lives in the Armée workspace (`berthier`, `davout`, `robstride`, `armee-dynamics`, etc.).
   - The motor command path is **fixed and non-negotiable**: Berthier → Davout → robstride. No other crate or binary may open a CAN socket or send motion frames. (See `docs/rust-patterns.md`, `docs/safety.md`, `crates/davout/src/lib.rs`, `crates/robstride/src/lib.rs`.)

3. **Protobuf on the wire (Chappe), generated types, one regeneration step**
   - `proto/` is the contract. `cd consul && npm run gen:proto` is the only regeneration.
   - No ts-rs, no custom message packages, no parallel type systems.
   - See ADR 0001 and the `consul/src/gen/` guard in `AGENTS.md`.

4. **Containerized, reproducible checks (`just check`) from day one**
   - The entire workspace (Rust + protobuf + frontend + fixture validation) must pass in the dev container.
   - Native host tooling is best-effort only.
   - This eliminates the "works on my ROS desktop" problem.

5. **Safety gateway is non-negotiable and early**
   - `Davout` owns `OperationalMode` (Disabled / Ready / Active), `ControlMode` mapping, per-motor-type limits, rate limiting on `tau_ff`, danger zones, comm watchdog, and `disable_all` on exit or E-stop.
   - The RobStride firmware mode work (ADR 0007 context) was done precisely to make the low-level protocol correct *before* any higher-level ambitions.
   - The bench gravity-compensation procedure in `docs/safety.md` must pass on real hardware before "cool" features are exercised at scale.

6. **Operator UI is a consumer, not a bus owner or second writer**
   - `consul/` reads `RobotState` / `SafetyState` from Chappe and sends high-level requests (enable, gravity-on, etc.).
   - It never talks directly to motors. All motion still goes through Davout.
   - This is the direct rejection of the `cortex` model.

7. **Sim and high-level behaviors are downstream of real-hardware validation**
   - The current execution slice is the 4-DOF arm on real CAN with gravity compensation (M4).
   - Isaac Lab / MuJoCo / Talleyrand / Fouché work is explicitly later (see roadmap M7–M8) and will be constrained by what the real stack actually does on hardware.

8. **"Boring" foundations are completed in public before feature work**
   - Correct RobStride MIT encoding + lifecycle + Davout gating was the prerequisite for everything else.
   - The same discipline will apply to homing/SetZero repeatability, real-mass URDF validation, thermal limits, full-body config switch, etc.

## Consequences

- Marengo will feel "slow" in the first 6–12 months compared to the pace of feature work in Rudy. That is intentional.
- New contributors and agents will be able to understand the system by reading a handful of crate root docs (`//!` blocks), `docs/safety.md`, `docs/rust-patterns.md`, and the ADRs — not by reverse-engineering a hybrid ROS + custom daemon + generated TypeScript stack.
- When the first 12+ DOF are powered (legs + waist + arms), the safety model, limit enforcement, and shutdown paths will already have been stress-tested on the 4-DOF slice with real masses and real timing.
- The project will be in a much stronger position to decide, later, whether a richer operator console, sim-to-real policy work, or other ambitions are worth the complexity — because the cost of that complexity will be measured against a proven, minimal, safe core rather than assumed.

## References

- [Rudy repository](https://github.com/jaylamping/rudy) (sunsetted)
- `docs/roadmap.md` (current execution slice and milestone gates)
- `docs/safety.md` (bench procedure that must be passed on real hardware)
- `docs/rust-patterns.md` and `AGENTS.md` (enforcement mechanisms)
- ADR 0001 (protobuf wire format)
- ADR 0004 (control modes and MIT semantics)
- The RobStride firmware modes work (2026-05, context for ADR 0007) that corrected the wire format and lifecycle before any higher-level motion features were exercised at scale

---

**This ADR is intentionally blunt.** Its purpose is to protect the 23-DOF vision from the accumulation of "just one more reasonable layer" that turned an exciting first project into an 80 kLOC maintenance burden before the robot could reliably stand up under its own power.

When in doubt, ask: "Does this change preserve the single hardware truth, the single motor command path through Davout, and the requirement that real-hardware gravity compensation + safe disable be proven before we add more surface area?"

If the answer is not an immediate, enthusiastic yes, the change should be deferred or re-scoped.