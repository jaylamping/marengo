# Marengo context

Shared glossary for workflow and product language. Expand only when a term is
resolved in conversation or an ADR.

## Glossary

| Term | Meaning |
|------|---------|
| **Software home** | The only Marengo git tree used for edit/commit of software and tracked CAD sources: WSL2 `~/code/marengo` on ext4. See [ADR 0016](docs/decisions/0016-wsl-software-home.md). |
| **CAD session** | A Windows Cursor window opened on `\\wsl$\Ubuntu\home\<user>\code\marengo` so SolidWorks MCP (COM) can see the same tree. Not a second clone. |
| **Software session** | Cursor connected to WSL with folder `~/code/marengo` — Rust, Consul, Docker/`just check`, marengo-pi MCP. |
| **Portable MCP config** | Repo [`.cursor/mcp.json`](.cursor/mcp.json) uses `${workspaceFolder}` and launchers only. Host-specific Pi defaults belong in `tools/marengo-pi-mcp/launch.mjs` (and `run-mcp.sh` / `run-mcp.ps1`), not in that JSON env block. |
| **Position hold** | Berthier’s joint-space motion primitive for `ControlMode::Position` (operator hold-at / hold-on): trapezoid advance policy plus MIT composition. Rust module/type: `PositionHold`. Not a separate control mode. _Avoid_: HoldExecutor, Controller (for this concept). |
| **MIT feedforward** | Berthier’s Active MIT packing for `GravityComp` / `Impedance` / `TorqueOnly`: mode gains (including hard-zero under GravityComp) plus τ_ff composition. Rust module/type: `MitFeedforward`. Not dynamics (`armee-dynamics` owns τ_g). _Avoid_: GravityCompCompose, Controller (for this concept). |
| **GainRuntime** | Berthier runtime gains: Testing overrides + mode-transition ramp + per-tick resolve. Types `GainRuntime` / `ResolvedGains`. Not a control mode. `ControlLoop` keeps the Pi facade. Not MIT compose (`MitFeedforward` / `PositionHold`). _Avoid_: stashing override/ramp on `MitFfJointIn`. |
| **Joint feedback** | Davout’s joint-space sample for one actuated joint after motor→joint conversion (position, velocity, torque, temperature, fault). Control and Chappe publish read this; they do not address the bus. Rust type: `JointFeedback`. _Avoid_: MotorState (robstride wire/cache), JointState (proto wire), motor_states map. |
| **Pi URDF library** | The durable on-Pi URDF directory Config/Setup defaults to (`/opt/marengo/assets/urdf/`). Holds one or more assembly `.urdf` files (e.g. different limbs). Operator gospel for those files lives here (plus in-memory runtime), not in git. _Avoid_: treating git `assets/urdf/` as the live library; mesh bundles (deferred). |
| **URDF seed** | Git-tracked `assets/urdf/` (and full deploy) used only to bootstrap the Pi URDF library. Not the Config/Setup working SoT for v1. |
| **Make Active** | Explicit operator action that selects which library assembly (and companion profile YAML) the runtime treats as active. Upload/hydration alone must not activate. |
| **Updates available** | Config/Setup chrome after a staged upload (or detected seed/library diff): per limb/assembly and per companion YAML, a pending-change signal the operator can accept or dismiss. Not the same as Make Active; accepting updates revises durable files/fields without necessarily switching the active assembly. |
| **URDF staging** | On-Pi inbox at `/opt/marengo/assets/urdf/staging/` for uploaded-but-not-yet-accepted `.urdf` files (bringup YAML uses a parallel `staging/` under the target profile). Config/Setup may read staged bytes for diff/preview; Accept applies into memory immediately and write-behind updates the durable library/profile files. Dismiss deletes the staged entry. _Avoid_: browser-only pending state. |
| **Persist-degraded Accept** | Accept succeeded in memory but durable write-behind failed. UI must alert that the change will be lost on Pi restart until persist recovers. Do not silently roll memory back. Extends ADR 0012 persist-degraded. |
