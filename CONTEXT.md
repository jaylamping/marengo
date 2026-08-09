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
| **Make Active** | _(retired for Config/Setup)_ Not a Config/Setup control. Live config is the **master URDF** working set; limb-subset bringup/testing is Testing UI / CLI. |
| **Master URDF** | The combined hardware-description URDF Config/Setup treats as the durable assembly SoT — composition of limb/DoF contributions (target `marengo.urdf`). Limb/DoF uploads merge into it by actuator identity; Config/Setup hot-reloads this working set. Limb-subset bringup for Testing/CLI is separate from this SoT. Unpowered limbs may remain in master for Consul visibility. |
| **URDF archive** | On-Pi (and/or library) archive of contributor URDF files that were merged into the master — for tracking/backups, not a second live SoT. |
| **Updates available** | Field-level badge/icon on Config/Setup settings when an upload (or seed diff) has **incoming changes** not yet resolved. Clicking opens the resolve wizard. Not a second settings universe. _Avoid_: per-file inbox workflow as the primary UX. |
| **Resolve wizard** | Guided screen after drop/upload (or via field badge) to accept incoming changes — **per value** or **Accept all** — or decline. Skippable; unresolved leftovers stay as field badges. |
| **Incoming changes** | Proposed values from an upload/diff not yet committed. Until wizard Accept, memory and durable active files keep current values. Accepting a value makes it Active (hot reload when safe) and runs a short sync (~300–500ms). |
| **Assembly identity** | Which physical limb/assembly an URDF/config set refers to — keyed by the **actuators (and fixed sensors) it contains**, not by filename. Filename is a storage label only. Match live working set by joint/actuator name overlap; ambiguous cases need an explicit wizard pick. |
| **Import gate** | Required guided step when an upload’s actuator names / CAN IDs don’t match the existing system (or target a different limb). Skipping abandons the upload — operator must re-upload; not a long-lived sideline assembly. |
| **Mapped config fields** | The standardized Config/Setup settings surface projected from actuators in use (Robstride for the foreseeable future) and fixed sensors. Uploads must parse into these fields; freeform unmapped URDF blobs are not a parallel SoT. |
| **URDF staging** | On-Pi buffer for uploads that are not yet resolved into the live working set — especially assemblies that are not the same limb as what’s loaded. Operator surface remains resolve wizard + field badges, not a second settings app. |
| **Persist-degraded Accept** | Accept succeeded in memory but durable write-behind failed. UI must alert that the change will be lost on Pi restart until persist recovers. Do not silently roll memory back. Extends ADR 0012 persist-degraded. |
