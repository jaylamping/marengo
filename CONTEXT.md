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
| **Actuator facets** | Per-actuator status is orthogonal: **presence** (Offline/Online), **reference** (Unready/**Ready**/Faulted — Ready ≡ homing Verified), **drive** (Disabled/Active), **health** (Nominal/OutOfLimits/Fault/…). Not one progressive enum. Hardware shows a derived primary badge (Fault > OutOfLimits > Offline > Active > Ready > Online). _Avoid_: Inventory static Enabled/Offline labels; conflating Ready with Active. |
| **Joint Ready** | Reference commissioning complete for one actuator (firmware Set Zero accepted + homing Verified). Does not mean the drive is Active. _Avoid_: Enabled, OperationalMode Ready (robot-level). |
| **Limb Ready** | Aggregation: every joint that counts for that limb is Joint Ready. Enable/test may proceed for that limb without Robot Ready. |
| **Robot Ready** | Aggregation: every master-SoT actuated joint is Joint Ready. Full-robot commissioning gate — not required to Enable a single built limb. |
| **Set Zero** | Durable Hardware action: firmware encoder zero at the current pose → calibration record → Joint Ready. Requires sign attestation; refused while Active. _Avoid_: Home (ambiguous). |
| **Go-to-zero** | Ephemeral motion that commands a joint toward `q = 0`. Not a reference action; stays off Hardware (Testing / similar). _Avoid_: calling this Home or Set Zero. |
