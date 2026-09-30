# Windows and macOS development

Open `J:\code\marengo` on Windows. The software checkout, local `cad/` tree, and exported assets live together. On the MacBook, clone the same repository into a normal host directory such as `~/code/marengo`. [ADR 0018](decisions/0018-windows-macos-software-home.md) records this decision.

## Tools and full checks

Install Git for Windows, Docker Desktop with Linux containers, and optional `just` on Windows. On macOS use Git and Docker Desktop. Git for Windows supplies `sh` for the existing task recipes; PowerShell is the normal Windows terminal.

From the repository root on either host:

```text
docker compose build dev
docker compose run --rm check
```

`just check` runs the same build and check. Cargo and npm dependencies are cached in named volumes. Docker may use a managed Linux VM internally; no Ubuntu checkout or WSL editor session is needed.

## Native work

The pinned versions are in `rust-toolchain.toml`, `mise.toml`, and `consul/package.json`. Native frontend work is supported on both hosts:

```powershell
Set-Location J:\code\marengo\consul
npm ci
npm run gen:proto
npm test
npm run build
```

On macOS use `cd ~/code/marengo/consul` and the same npm commands. Use `npm ci` for installs; regenerate a changed lockfile with `just consul-lock` to match Linux CI.

Portable Rust crates can be tested natively. The full workspace currently depends on Chappe's Unix-only IPC implementation and needs the Linux container on Windows. `protoc` 28.3 is required by `armee-proto`.

## Pi deployment

From Windows PowerShell, use the existing native Docker wrapper:

```powershell
Set-Location J:\code\marengo
$env:MARENGO_PI_HOST = 'joey-robot.tail0b414.ts.net'
powershell -NoProfile -File .\scripts\deploy-pi-docker.ps1
```

From macOS, the deployment scripts currently require Bash 4 or newer and GNU `sha256sum`; the stock macOS Bash 3.2 is insufficient. `just deploy-pi` uses the native cross toolchain and `just deploy-pi-docker` uses Docker. These host-shell requirements are recorded for follow-up in the repository review. Deployment and motion are separate steps; follow [the limb commissioning playbook](commissioning/limb-playbook.md) before enabling the arm.

## CAD and MCP

Keep the local SolidWorks files under `J:\code\marengo\cad`. Open `marengo.code-workspace` to include the sibling `J:\code\solidworks-mcp`. Build Node tooling with `just mcp-build`, install research dependencies with `just research-mcp-setup`, and restart MCP servers after builds. The repo MCP config uses Node launchers and `${workspaceFolder}`.

Git ignores the SolidWorks source folders. Back them up separately; Git pushes and a MacBook clone do not transfer those ignored files. The September migration's recovery data is under `J:\code\marengo-migration-backup-20260929`, including SHA-256 manifests, the recovered local patch, a tagged stash, Git-ref bundles, and alternate CAD versions.

## September Docker repair

Docker Desktop startup was repaired on September 29 by preserving and recreating its stale Inference-manager and Secrets Engine socket directories. Docker virtual disks, images, volumes, and settings were preserved. The stale socket files predated the repository move. Validation results are in [the repository review](reviews/2026-09-29-repository-review.md).
