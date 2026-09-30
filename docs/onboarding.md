# Onboarding

Steps to a working Marengo dev environment.

## 1. Clone

```bash
git clone <repo-url> marengo && cd marengo
```

SolidWorks CAD is **not in this repo** — restore your local `cad/` tree separately ([cad/README.md](../cad/README.md)). ONNX policies (when added) use Git LFS: `git lfs install && git lfs pull`.

## 2. Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose v2
- Optional: [Cursor](https://cursor.com/) or VS Code with Dev Containers
- Windows: use `J:\code\marengo` and install Git for Windows. macOS: use a normal host checkout. See [Windows and macOS development](windows-macos-development.md).

## 3. Verify the workspace

```bash
docker compose build dev
docker compose run --rm check
```

Or with [just](https://github.com/casey/just): `just check`

## 4. Daily development

CLI (any editor):

```bash
docker compose run --rm dev
# inside container:
cargo build --workspace
cargo test --workspace
```

Cursor / VS Code: Command Palette → Dev Containers: Reopen in Container

## 5. SocketCAN test harness (Linux container)

```bash
just vcan
docker compose --profile vcan run --rm check-vcan
```

This creates virtual `vcan0`/`vcan1` interfaces for tests only. Robot runtime uses production interfaces such as `can0`, `can1`, and `can2` from `config/motors.yaml`.

## 6. Simulation (optional)

```bash
just sim-check
```

## 7. Before you push

```bash
just check
```

[rust-patterns.md](rust-patterns.md), [safety.md](safety.md), [dev-setup.md](dev-setup.md).

## 8. CAD and SolidWorks MCP (Windows)

Software and local CAD live together at `J:\code\marengo` ([ADR 0018](decisions/0018-windows-macos-software-home.md)). For mechanical design with Cursor + SolidWorks:

1. Open a **Windows** Cursor window on `J:\code\marengo`, or open its `marengo.code-workspace` with sibling `solidworks-mcp`.
2. Build the MCP server in `J:\code\solidworks-mcp` with `npm ci` and `npm run build`.
3. Cursor loads workspace MCP from [`.cursor/mcp.json`](../.cursor/mcp.json)
   (`SOLIDWORKS_MCP_ALLOWED_ROOTS=${workspaceFolder}` → the host checkout).
4. Model under [`cad/`](../cad/); run `marengo_design_review` before saving assemblies.
5. URDF: manual Brawner export → `assets/urdf/marengo.urdf`, then MCP `marengo_urdf_export_postcheck` and [`scripts/export-urdf.sh`](../scripts/export-urdf.sh).

CAD standards: [cad/README.md](../cad/README.md). Host workflow: [Windows and macOS development](windows-macos-development.md).
