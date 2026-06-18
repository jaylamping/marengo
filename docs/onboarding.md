# Onboarding

Steps to a working Marengo dev environment.

## 1. Clone and LFS

```bash
git clone <repo-url> marengo && cd marengo
git lfs install
git lfs pull
```

## 2. Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose v2
- Optional: [Cursor](https://cursor.com/) or VS Code with Dev Containers
- Windows: prefer [WSL2 setup](wsl-setup.md). Clone under `~/code`, not `C:\`.

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

For mechanical design with Cursor + SolidWorks:

1. Open `marengo.code-workspace` (marengo + sibling `solidworks-mcp` repo).
2. Build the MCP server: `cd ../solidworks-mcp && npm install && npm run build`.
3. Cursor loads workspace MCP from [`.cursor/mcp.json`](../.cursor/mcp.json) (`SOLIDWORKS_MCP_ALLOWED_ROOTS=C:/code/marengo`).
4. Model under [`hardware/cad/`](../hardware/cad/); run `marengo_design_review` before saving assemblies.
5. URDF: manual Brawner export → `assets/urdf/marengo.urdf`, then MCP `marengo_urdf_export_postcheck` and [`scripts/export-urdf.sh`](../scripts/export-urdf.sh).

CAD standards: [hardware/docs/cad-standards.md](../hardware/docs/cad-standards.md).
