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
