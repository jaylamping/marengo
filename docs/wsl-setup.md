# WSL2 development setup (Windows)

Run Marengo’s Linux toolchain **inside WSL2** for faster Docker/Rust I/O. Keep **SolidWorks + solidworks-mcp on Windows** when you need CAD.

## Why WSL2

| On `C:\code\...` (Windows filesystem) | On `~/code/...` (WSL ext4) |
|---------------------------------------|----------------------------|
| Slow bind mounts into containers | Fast native Linux I/O |
| More permission quirks (`EACCES`) | Fewer volume permission issues |
| vcan awkward | vcan works in Linux container |

**Rule:** clone under **`~/code`**, never **`/mnt/c/code`**.

---

## 1. Install WSL2 + Ubuntu

**PowerShell (Admin):**

```powershell
wsl --install
```

Reboot if prompted. Default distro is usually **Ubuntu**. Set username/password on first launch.

Verify:

```powershell
wsl -l -v
```

`VERSION` should be **2** for your distro.

Update inside WSL:

```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y git git-lfs build-essential
```

---

## 2. Docker Desktop + WSL integration

1. Install [Docker Desktop for Windows](https://docs.docker.com/desktop/setup/install/windows-install/) if not already.
2. **Settings → General:** enable **Use the WSL 2 based engine**.
3. **Settings → Resources → WSL integration:** enable your Ubuntu distro.
4. Apply & restart Docker Desktop.

Verify **inside WSL** (not PowerShell):

```bash
docker version
docker compose version
```

Both should succeed without `npipe` errors.

---

## 3. Clone Marengo in the Linux filesystem

```bash
mkdir -p ~/code
cd ~/code
git clone <repo-url> marengo
cd marengo
git lfs install
git lfs pull
```

Optional sibling for SolidWorks MCP (Windows path still used for CAD; see §7):

```bash
cd ~/code
git clone <solidworks-mcp-url> solidworks-mcp
```

---

## 4. Verify the workspace

From `~/code/marengo`:

```bash
docker compose build dev
docker compose run --rm check
```

Or install [just](https://github.com/casey/just) in WSL and run `just check`.

First build takes a while; later runs use cached Docker layers and named volumes (`target/`, `consul/node_modules`).

---

## 5. Cursor on WSL

### Option A — Open folder in WSL (recommended for daily Rust/docker work)

1. Install **WSL** extension in Cursor (usually bundled).
2. Command Palette → **WSL: Connect to WSL** (pick Ubuntu).
3. **File → Open Folder** → `/home/<you>/code/marengo`.
4. Terminal in Cursor runs in WSL automatically.

### Option B — Dev Container (same as today)

1. Open `~/code/marengo` in WSL.
2. Command Palette → **Dev Containers: Reopen in Container**.

Uses [`.devcontainer/devcontainer.json`](../.devcontainer/devcontainer.json) — same image as `docker compose`.

### Option C — Multi-root workspace (marengo + solidworks-mcp)

Open `marengo.code-workspace` from the WSL path:

```bash
cursor ~/code/marengo/marengo.code-workspace
```

---

## 6. Daily commands (WSL terminal)

```bash
cd ~/code/marengo

just check          # CI parity
just shell          # interactive dev container
just vcan           # virtual CAN (Linux)
just sim-check      # MuJoCo smoke tests
```

Inside dev container:

```bash
cargo build --workspace
cargo test --workspace
```

---

## 7. Hybrid: SolidWorks stays on Windows

SolidWorks and the MCP worker run on **Windows**. Software build/run uses **WSL**.

| Task | Where |
|------|--------|
| `just check`, Rust, Docker | WSL (`~/code/marengo`) |
| SolidWorks CAD | Windows |
| solidworks-mcp | Windows (`C:\code\solidworks-mcp`) |
| MCP CAD root | Windows path in [`.cursor/mcp.json`](../.cursor/mcp.json) |

**Two-repo layout:**

- WSL: `~/code/marengo` — primary git clone for software
- Windows: `C:\code\marengo` — optional second clone **only for CAD** if MCP must read `C:/code/marengo`

**Avoid two divergent clones.** Prefer one git remote; sync via push/pull. For CAD-only Windows copy, pull before modeling; commit from WSL.

**Cursor on Windows** (when doing CAD): open `C:\code\marengo` + `marengo.code-workspace` so MCP paths in [`.cursor/mcp.json`](../.cursor/mcp.json) resolve.

**Cursor on WSL** (when doing software): open `~/code/marengo` — faster checks; use Windows Cursor session when you need SolidWorks MCP.

---

## 8. Migrating from `C:\code\marengo`

If you already have a Windows clone:

```bash
# WSL — fresh clone (simplest)
cd ~/code && git clone <repo-url> marengo && cd marengo && git lfs pull
```

Copy uncommitted work from Windows manually, or commit/stash on Windows first.

Do **not** work daily from `/mnt/c/code/marengo` — that path is still slow.

Optional: remove old Windows clone after WSL is verified to free disk.

---

## 9. Optional native tools in WSL (without Docker)

Best-effort; container remains source of truth ([dev-setup.md](dev-setup.md)).

```bash
# One-shot: SSH config, git identity, cross-GCC, mise, Pi MCP build
./scripts/setup-wsl-dev.sh

# Or piecemeal:
./scripts/setup-wsl-pi-cross.sh
just deploy-pi-wsl
# Binary-only (skip consul): ./scripts/deploy-pi.sh --install --skip-consul joey@marengo.local

# mise (matches mise.toml + rust-toolchain.toml 1.88)
curl https://mise.run | sh
cd ~/code/marengo && mise install
```

After `setup-wsl-dev.sh`, **restart the marengo-pi MCP server** in Cursor so it picks up `.cursor/mcp.json` (WSL uses `SSH_IDENTITY_FILE` + `marengo.local`).

---

## 10. Troubleshooting

| Symptom | Fix |
|---------|-----|
| `failed to connect to docker API at npipe://...` | Start Docker Desktop; enable WSL integration; run commands **inside WSL**. Avoid a `docker` shim that only calls `docker.exe` once the unix socket exists. |
| `Permission denied (publickey)` to Pi | Ensure `~/.ssh/id_ed25519_marengo` + `~/.ssh/config` (`./scripts/setup-wsl-dev.sh`). Bare `ssh joey@marengo.local` must work. |
| `Author identity unknown` on commit | Set git `user.name` / `user.email` (or re-run `setup-wsl-dev.sh`). |
| Builds still slow | Confirm repo is `~/code/...`, not `/mnt/c/...`. Use `just deploy-pi-docker` (not bare `docker run`); 2nd run should reuse `cargo-target` / `cargo-registry` volumes. |
| Deploy looks hung, no output | Use `./scripts/deploy-pi-docker.sh` (line-buffered + `CARGO_TERM_PROGRESS_WHEN=always`). Verbose: `MARENGO_DEPLOY_VERBOSE=1 just deploy-pi-docker`. |
| Want native deploy without Docker | WSL2: `./scripts/setup-wsl-pi-cross.sh` (needs `libc6-dev-arm64-cross`, not just the GCC package) then `just deploy-pi-wsl`. |
| Cross-build fails: `bits/libc-header-start.h: No such file` | Install aarch64 libc headers: `sudo apt-get install -y libc6-dev-arm64-cross` (or re-run `./scripts/setup-wsl-pi-cross.sh`). |
| `git lfs` smudge errors | `git lfs install && git lfs pull` inside WSL clone. |
| Line-ending noise | In WSL clone: `git config core.autocrlf input`. |
| Permission errors on `target/` | `docker compose build dev` (entrypoint chowns volumes). See [troubleshooting.md](troubleshooting.md). |
| SolidWorks MCP can’t see files | MCP `SOLIDWORKS_MCP_ALLOWED_ROOTS` must match Windows CAD path; use Windows Cursor session for CAD. |
| marengo-pi MCP missing / disabled in WSL | Usually disabled, not missing. Enable in MCP settings, or quit Cursor and run `just mcp-ensure-enabled --write`. `sessionStart` also runs a best-effort ensure hook (Node). Do **not** put profile/SSH env in `.cursor/mcp.json` (hash thrash auto-disables). Defaults are in `tools/marengo-pi-mcp/run-mcp.sh` (shared Mac/Windows/WSL; `run-mcp.ps1` is optional Windows fallback). |
| `spawn node ENOENT` on marengo-pi | Cursor's PATH lacks mise. mcp.json must use `sh` + `run-mcp.sh` (not bare `node`). Restart MCP after pull. On Windows, ensure Git for Windows `sh` is on PATH (not System32 WSL `bash.exe`). |

---

## Quick reference

```bash
# One-time
wsl --install
# Docker Desktop: WSL integration ON
mkdir -p ~/code && cd ~/code && git clone <repo-url> marengo && cd marengo && git lfs pull

# Every session
cd ~/code/marengo && just check
```

See also [onboarding.md](onboarding.md), [dev-setup.md](dev-setup.md).
